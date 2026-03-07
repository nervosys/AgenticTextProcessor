#![no_main]

use atp_core::engine::grep::{GrepConfig, GrepEngine};
use atp_core::output::PatternType;
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        // Split input: first line = pattern, rest = text
        let (pattern, text) = match input.split_once('\n') {
            Some((p, t)) => (p, t),
            None => (input, ""),
        };

        if pattern.is_empty() {
            return;
        }

        // Fuzz regex search
        let config = GrepConfig {
            pattern: pattern.to_string(),
            pattern_type: PatternType::Regex,
            ..Default::default()
        };
        if let Ok(engine) = GrepEngine::new(config) {
            let cursor = Cursor::new(text.as_bytes());
            let _ = engine.search_reader(cursor, "fuzz");
        }

        // Fuzz literal search
        let config = GrepConfig {
            pattern: pattern.to_string(),
            pattern_type: PatternType::Literal,
            ..Default::default()
        };
        if let Ok(engine) = GrepEngine::new(config) {
            let cursor = Cursor::new(text.as_bytes());
            let _ = engine.search_reader(cursor, "fuzz");
        }
    }
});
