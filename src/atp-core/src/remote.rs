//! Remote execution — run ATP queries across remote hosts and as a daemon.
//!
//! # Architecture
//!
//! - [`RemoteTarget`]: A remote host specification (hostname, user, port)
//! - [`RemoteExecutor`]: Execute ATP commands on remote hosts via SSH
//! - [`DaemonConfig`]: Configuration for running ATP as a persistent daemon
//! - [`RemoteResult`]: Aggregated results from multiple hosts
//!
//! # Usage
//!
//! ```text
//! atp remote search "TODO" --hosts prod1,prod2,prod3 --path /app/src
//! atp remote query 'find "error" | count' --hosts @hostfile.txt
//! atp daemon start --port 9876
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

/// A remote host target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteTarget {
    /// Hostname or IP address.
    pub host: String,
    /// SSH user (defaults to current user).
    pub user: Option<String>,
    /// SSH port (defaults to 22).
    pub port: Option<u16>,
    /// Remote path to search/process.
    pub path: Option<String>,
    /// SSH identity file.
    pub identity_file: Option<String>,
}

impl RemoteTarget {
    /// Create a new remote target from a host string.
    ///
    /// Supports formats:
    /// - `hostname`
    /// - `user@hostname`
    /// - `user@hostname:port`
    pub fn parse(spec: &str) -> Self {
        let (user_host, port) = if let Some((rest, port_str)) = spec.rsplit_once(':') {
            if let Ok(port) = port_str.parse::<u16>() {
                (rest, Some(port))
            } else {
                (spec, None)
            }
        } else {
            (spec, None)
        };

        let (user, host) = if let Some((user, host)) = user_host.split_once('@') {
            (Some(user.to_string()), host.to_string())
        } else {
            (None, user_host.to_string())
        };

        Self {
            host,
            user,
            port,
            path: None,
            identity_file: None,
        }
    }

    /// Parse a comma-separated list of host specs.
    pub fn parse_list(specs: &str) -> Vec<Self> {
        specs.split(',').map(|s| Self::parse(s.trim())).collect()
    }

    /// Parse hosts from a file (one per line, # comments).
    pub fn parse_file(path: &str) -> Result<Vec<Self>> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read host file: {path}"))?;
        Ok(content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(Self::parse)
            .collect())
    }

    /// Build the SSH destination string (user@host).
    pub fn ssh_dest(&self) -> String {
        match &self.user {
            Some(user) => format!("{user}@{}", self.host),
            None => self.host.clone(),
        }
    }

    /// Build SSH command arguments.
    pub fn ssh_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        if let Some(port) = self.port {
            args.push("-p".to_string());
            args.push(port.to_string());
        }

        if let Some(identity) = &self.identity_file {
            args.push("-i".to_string());
            args.push(identity.clone());
        }

        // Disable strict host key checking for non-interactive use
        args.push("-o".to_string());
        args.push("StrictHostKeyChecking=accept-new".to_string());
        args.push("-o".to_string());
        args.push("ConnectTimeout=10".to_string());

        args
    }
}

/// Result from a single remote host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteResult {
    /// The host this result came from.
    pub host: String,
    /// Whether the command succeeded.
    pub success: bool,
    /// stdout from the remote command.
    pub stdout: String,
    /// stderr from the remote command.
    pub stderr: String,
    /// Exit code.
    pub exit_code: Option<i32>,
    /// Execution time in milliseconds.
    pub duration_ms: u64,
}

/// Execute ATP commands on remote hosts via SSH.
pub struct RemoteExecutor {
    /// Path to the `atp` binary on remote hosts.
    pub remote_atp_path: String,
    /// Additional SSH options.
    pub ssh_options: Vec<String>,
}

impl Default for RemoteExecutor {
    fn default() -> Self {
        Self {
            remote_atp_path: "atp".to_string(),
            ssh_options: Vec::new(),
        }
    }
}

impl RemoteExecutor {
    /// Create a new executor with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute an ATP command on a single remote host.
    pub fn execute_on(&self, target: &RemoteTarget, atp_args: &[&str]) -> RemoteResult {
        let start = std::time::Instant::now();

        // Build the remote command
        let remote_cmd = format!(
            "{} {}",
            self.remote_atp_path,
            atp_args
                .iter()
                .map(|a| shell_escape(a))
                .collect::<Vec<_>>()
                .join(" ")
        );

        // Build the full SSH command
        let mut cmd = Command::new("ssh");
        for arg in target.ssh_args() {
            cmd.arg(&arg);
        }
        for opt in &self.ssh_options {
            cmd.arg(opt);
        }
        cmd.arg(target.ssh_dest());
        cmd.arg(&remote_cmd);

        let result = cmd.output();
        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => RemoteResult {
                host: target.host.clone(),
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
                duration_ms,
            },
            Err(e) => RemoteResult {
                host: target.host.clone(),
                success: false,
                stdout: String::new(),
                stderr: format!("SSH connection failed: {e}"),
                exit_code: None,
                duration_ms,
            },
        }
    }

    /// Execute an ATP command on multiple hosts (sequentially).
    pub fn execute_on_all(&self, targets: &[RemoteTarget], atp_args: &[&str]) -> Vec<RemoteResult> {
        targets
            .iter()
            .map(|target| self.execute_on(target, atp_args))
            .collect()
    }

    /// Execute an ATP command on multiple hosts in parallel using rayon.
    pub fn execute_parallel(
        &self,
        targets: &[RemoteTarget],
        atp_args: &[&str],
    ) -> Vec<RemoteResult> {
        use rayon::prelude::*;
        targets
            .par_iter()
            .map(|target| self.execute_on(target, atp_args))
            .collect()
    }
}

/// Daemon configuration for running ATP as a persistent service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// Port to listen on.
    pub port: u16,
    /// Bind address.
    pub bind: String,
    /// Working directory.
    pub work_dir: String,
    /// Maximum concurrent requests.
    pub max_concurrent: usize,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            port: 9876,
            bind: "127.0.0.1".to_string(),
            work_dir: ".".to_string(),
            max_concurrent: 16,
            timeout_secs: 60,
        }
    }
}

/// Simple shell escaping for remote command arguments.
fn shell_escape(s: &str) -> String {
    if s.contains(|c: char| c.is_whitespace() || c == '\'' || c == '"' || c == '\\' || c == '$') {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_host() {
        let target = RemoteTarget::parse("prod1");
        assert_eq!(target.host, "prod1");
        assert_eq!(target.user, None);
        assert_eq!(target.port, None);
    }

    #[test]
    fn test_parse_user_host() {
        let target = RemoteTarget::parse("deploy@prod1");
        assert_eq!(target.host, "prod1");
        assert_eq!(target.user.as_deref(), Some("deploy"));
        assert_eq!(target.port, None);
    }

    #[test]
    fn test_parse_user_host_port() {
        let target = RemoteTarget::parse("deploy@prod1:2222");
        assert_eq!(target.host, "prod1");
        assert_eq!(target.user.as_deref(), Some("deploy"));
        assert_eq!(target.port, Some(2222));
    }

    #[test]
    fn test_parse_list() {
        let targets = RemoteTarget::parse_list("host1, user@host2, admin@host3:22");
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0].host, "host1");
        assert_eq!(targets[1].user.as_deref(), Some("user"));
        assert_eq!(targets[2].port, Some(22));
    }

    #[test]
    fn test_ssh_dest() {
        let t1 = RemoteTarget::parse("prod1");
        assert_eq!(t1.ssh_dest(), "prod1");

        let t2 = RemoteTarget::parse("deploy@prod2");
        assert_eq!(t2.ssh_dest(), "deploy@prod2");
    }

    #[test]
    fn test_ssh_args_basic() {
        let target = RemoteTarget::parse("host1");
        let args = target.ssh_args();
        assert!(args.contains(&"-o".to_string()));
        assert!(args.contains(&"ConnectTimeout=10".to_string()));
    }

    #[test]
    fn test_ssh_args_with_port() {
        let target = RemoteTarget::parse("host1:2222");
        let args = target.ssh_args();
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"2222".to_string()));
    }

    #[test]
    fn test_shell_escape_simple() {
        assert_eq!(shell_escape("hello"), "hello");
        assert_eq!(shell_escape("hello world"), "'hello world'");
    }

    #[test]
    fn test_shell_escape_quotes() {
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[test]
    fn test_daemon_config_default() {
        let config = DaemonConfig::default();
        assert_eq!(config.port, 9876);
        assert_eq!(config.bind, "127.0.0.1");
        assert_eq!(config.max_concurrent, 16);
    }

    #[test]
    fn test_remote_executor_default() {
        let exec = RemoteExecutor::new();
        assert_eq!(exec.remote_atp_path, "atp");
    }

    #[test]
    fn test_remote_result_serde() {
        let result = RemoteResult {
            host: "prod1".into(),
            success: true,
            stdout: "output".into(),
            stderr: String::new(),
            exit_code: Some(0),
            duration_ms: 150,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: RemoteResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.host, "prod1");
        assert!(parsed.success);
    }
}
