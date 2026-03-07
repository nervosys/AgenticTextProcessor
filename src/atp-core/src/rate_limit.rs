//! Rate-limited and throttled pipeline execution.
//!
//! Wraps any `StreamingPipeline` with back-pressure, rate limiting, and
//! circuit-breaker semantics so that downstream consumers are never overwhelmed.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Configuration for rate-limited pipeline execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum records per second (0 = unlimited).
    #[serde(default)]
    pub max_records_per_second: u64,
    /// Maximum burst size before throttling kicks in.
    #[serde(default = "default_burst")]
    pub burst_size: u64,
    /// Circuit-breaker: max consecutive errors before tripping.
    #[serde(default = "default_max_errors")]
    pub max_consecutive_errors: u32,
    /// Circuit-breaker: cool-down duration before half-open.
    #[serde(default = "default_cooldown")]
    pub cooldown: Duration,
    /// Whether the limiter is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_burst() -> u64 {
    256
}
fn default_max_errors() -> u32 {
    10
}
fn default_cooldown() -> Duration {
    Duration::from_secs(5)
}
fn default_enabled() -> bool {
    true
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_records_per_second: 0,
            burst_size: default_burst(),
            max_consecutive_errors: default_max_errors(),
            cooldown: default_cooldown(),
            enabled: default_enabled(),
        }
    }
}

/// Circuit-breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// A token-bucket rate limiter.
#[derive(Debug)]
pub struct TokenBucket {
    capacity: u64,
    tokens: f64,
    rate: f64, // tokens per nanosecond
    last_refill: Instant,
}

impl TokenBucket {
    /// Create a new token bucket.
    pub fn new(rate_per_second: u64, burst: u64) -> Self {
        Self {
            capacity: burst,
            tokens: burst as f64,
            rate: rate_per_second as f64 / 1_000_000_000.0,
            last_refill: Instant::now(),
        }
    }

    /// Try to acquire one token. Returns `true` if granted.
    pub fn try_acquire(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// How long until a token is available.
    pub fn wait_duration(&self) -> Duration {
        if self.tokens >= 1.0 || self.rate <= 0.0 {
            Duration::ZERO
        } else {
            let deficit = 1.0 - self.tokens;
            Duration::from_nanos((deficit / self.rate) as u64)
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_nanos() as f64;
        self.tokens = (self.tokens + elapsed * self.rate).min(self.capacity as f64);
        self.last_refill = now;
    }

    /// Current tokens available.
    pub fn available_tokens(&self) -> u64 {
        self.tokens as u64
    }
}

/// A circuit-breaker for error handling.
#[derive(Debug)]
pub struct CircuitBreaker {
    state: CircuitState,
    consecutive_errors: u32,
    max_errors: u32,
    cooldown: Duration,
    last_error_time: Option<Instant>,
    total_trips: u64,
}

impl CircuitBreaker {
    /// Create a new circuit-breaker.
    pub fn new(max_errors: u32, cooldown: Duration) -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_errors: 0,
            max_errors,
            cooldown,
            last_error_time: None,
            total_trips: 0,
        }
    }

    /// Record a success.
    pub fn record_success(&mut self) {
        self.consecutive_errors = 0;
        if self.state == CircuitState::HalfOpen {
            self.state = CircuitState::Closed;
        }
    }

    /// Record a failure.
    pub fn record_failure(&mut self) {
        self.consecutive_errors += 1;
        self.last_error_time = Some(Instant::now());

        if self.consecutive_errors >= self.max_errors && self.state == CircuitState::Closed {
            self.state = CircuitState::Open;
            self.total_trips += 1;
        }
    }

    /// Check if requests are allowed.
    pub fn is_allowed(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last) = self.last_error_time {
                    if last.elapsed() >= self.cooldown {
                        self.state = CircuitState::HalfOpen;
                        true
                    } else {
                        false
                    }
                } else {
                    true
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Get current state.
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Total times the circuit tripped.
    pub fn total_trips(&self) -> u64 {
        self.total_trips
    }
}

/// Runtime statistics for rate-limited execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStats {
    pub records_processed: u64,
    pub records_throttled: u64,
    pub records_rejected: u64,
    pub circuit_trips: u64,
    pub circuit_state: CircuitState,
    pub elapsed: Duration,
    pub effective_rate: f64,
}

/// A rate-limited pipeline executor.
#[derive(Debug)]
pub struct RateLimitedExecutor {
    config: RateLimitConfig,
    bucket: TokenBucket,
    breaker: CircuitBreaker,
    records_processed: u64,
    records_throttled: u64,
    records_rejected: u64,
    start_time: Instant,
    cancelled: Arc<AtomicBool>,
}

impl RateLimitedExecutor {
    /// Create a new executor with the given configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        let bucket = TokenBucket::new(config.max_records_per_second, config.burst_size);
        let breaker = CircuitBreaker::new(config.max_consecutive_errors, config.cooldown);
        Self {
            config,
            bucket,
            breaker,
            records_processed: 0,
            records_throttled: 0,
            records_rejected: 0,
            start_time: Instant::now(),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get a cancellation handle.
    pub fn cancel_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancelled)
    }

    /// Try processing a single record.
    /// Returns Ok(true) if the record was processed, Ok(false) if throttled, Err if circuit-open.
    pub fn try_process_record<F>(&mut self, f: F) -> Result<bool, RateLimitError>
    where
        F: FnOnce() -> Result<(), String>,
    {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(RateLimitError::Cancelled);
        }

        if !self.config.enabled || self.config.max_records_per_second == 0 {
            // Unlimited — just run
            match f() {
                Ok(()) => {
                    self.records_processed += 1;
                    self.breaker.record_success();
                    Ok(true)
                }
                Err(e) => {
                    self.breaker.record_failure();
                    Err(RateLimitError::ProcessingError(e))
                }
            }
        } else if !self.breaker.is_allowed() {
            self.records_rejected += 1;
            Err(RateLimitError::CircuitOpen)
        } else if self.bucket.try_acquire() {
            match f() {
                Ok(()) => {
                    self.records_processed += 1;
                    self.breaker.record_success();
                    Ok(true)
                }
                Err(e) => {
                    self.breaker.record_failure();
                    Err(RateLimitError::ProcessingError(e))
                }
            }
        } else {
            self.records_throttled += 1;
            Ok(false)
        }
    }

    /// Get the wait duration if throttled.
    pub fn wait_duration(&self) -> Duration {
        self.bucket.wait_duration()
    }

    /// Snapshot current statistics.
    pub fn stats(&self) -> RateLimitStats {
        let elapsed = self.start_time.elapsed();
        let secs = elapsed.as_secs_f64();
        RateLimitStats {
            records_processed: self.records_processed,
            records_throttled: self.records_throttled,
            records_rejected: self.records_rejected,
            circuit_trips: self.breaker.total_trips(),
            circuit_state: self.breaker.state(),
            elapsed,
            effective_rate: if secs > 0.0 {
                self.records_processed as f64 / secs
            } else {
                0.0
            },
        }
    }

    /// Reset statistics.
    pub fn reset(&mut self) {
        self.records_processed = 0;
        self.records_throttled = 0;
        self.records_rejected = 0;
        self.start_time = Instant::now();
        self.breaker =
            CircuitBreaker::new(self.config.max_consecutive_errors, self.config.cooldown);
        self.bucket = TokenBucket::new(self.config.max_records_per_second, self.config.burst_size);
    }
}

/// Errors from rate-limited execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitError {
    /// Circuit-breaker is open.
    CircuitOpen,
    /// Processing was cancelled.
    Cancelled,
    /// A processing error occurred.
    ProcessingError(String),
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CircuitOpen => write!(f, "circuit breaker is open"),
            Self::Cancelled => write!(f, "processing was cancelled"),
            Self::ProcessingError(e) => write!(f, "processing error: {e}"),
        }
    }
}

impl std::error::Error for RateLimitError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RateLimitConfig::default();
        assert_eq!(config.max_records_per_second, 0);
        assert_eq!(config.burst_size, 256);
        assert_eq!(config.max_consecutive_errors, 10);
        assert!(config.enabled);
    }

    #[test]
    fn test_token_bucket_unlimited() {
        let bucket = TokenBucket::new(0, 256);
        // With rate=0, tokens never refill, but we start with burst
        assert!(bucket.available_tokens() > 0);
    }

    #[test]
    fn test_token_bucket_acquire() {
        let mut bucket = TokenBucket::new(1000, 10);
        // Should have initial burst tokens
        let mut acquired = 0;
        for _ in 0..10 {
            if bucket.try_acquire() {
                acquired += 1;
            }
        }
        assert!(acquired > 0);
    }

    #[test]
    fn test_circuit_breaker_closed() {
        let mut cb = CircuitBreaker::new(3, Duration::from_secs(1));
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_allowed());
    }

    #[test]
    fn test_circuit_breaker_opens_on_errors() {
        let mut cb = CircuitBreaker::new(3, Duration::from_secs(60));
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.is_allowed());
        assert_eq!(cb.total_trips(), 1);
    }

    #[test]
    fn test_circuit_breaker_success_resets() {
        let mut cb = CircuitBreaker::new(3, Duration::from_secs(1));
        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
        // Counter is reset, so 3 more failures needed
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_unlimited_executor() {
        let config = RateLimitConfig {
            max_records_per_second: 0,
            ..Default::default()
        };
        let mut exec = RateLimitedExecutor::new(config);
        let result = exec.try_process_record(|| Ok(()));
        assert_eq!(result, Ok(true));
        assert_eq!(exec.stats().records_processed, 1);
    }

    #[test]
    fn test_executor_with_error() {
        let config = RateLimitConfig::default();
        let mut exec = RateLimitedExecutor::new(config);
        let result = exec.try_process_record(|| Err("test error".into()));
        assert!(result.is_err());
        match result {
            Err(RateLimitError::ProcessingError(e)) => assert_eq!(e, "test error"),
            _ => panic!("unexpected result"),
        }
    }

    #[test]
    fn test_executor_cancel() {
        let config = RateLimitConfig::default();
        let mut exec = RateLimitedExecutor::new(config);
        let handle = exec.cancel_handle();
        handle.store(true, Ordering::Relaxed);
        let result = exec.try_process_record(|| Ok(()));
        assert_eq!(result, Err(RateLimitError::Cancelled));
    }

    #[test]
    fn test_disabled_limiter() {
        let config = RateLimitConfig {
            enabled: false,
            max_records_per_second: 1,
            ..Default::default()
        };
        let mut exec = RateLimitedExecutor::new(config);
        // Even with max_records_per_second=1, disabled means unlimited
        for _ in 0..100 {
            let r = exec.try_process_record(|| Ok(()));
            assert_eq!(r, Ok(true));
        }
        assert_eq!(exec.stats().records_processed, 100);
    }

    #[test]
    fn test_executor_stats() {
        let config = RateLimitConfig::default();
        let mut exec = RateLimitedExecutor::new(config);
        for _ in 0..5 {
            let _ = exec.try_process_record(|| Ok(()));
        }
        let stats = exec.stats();
        assert_eq!(stats.records_processed, 5);
        assert_eq!(stats.circuit_state, CircuitState::Closed);
    }

    #[test]
    fn test_executor_reset() {
        let config = RateLimitConfig::default();
        let mut exec = RateLimitedExecutor::new(config);
        let _ = exec.try_process_record(|| Ok(()));
        exec.reset();
        let stats = exec.stats();
        assert_eq!(stats.records_processed, 0);
    }

    #[test]
    fn test_rate_limit_error_display() {
        assert_eq!(
            format!("{}", RateLimitError::CircuitOpen),
            "circuit breaker is open"
        );
        assert_eq!(
            format!("{}", RateLimitError::Cancelled),
            "processing was cancelled"
        );
    }

    #[test]
    fn test_config_serialization() {
        let config = RateLimitConfig {
            max_records_per_second: 100,
            burst_size: 50,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("100"));
        assert!(json.contains("50"));
    }
}
