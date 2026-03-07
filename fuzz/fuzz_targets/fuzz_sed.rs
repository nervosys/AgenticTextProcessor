#![no_main]

use atp_core::engine::sed::{SedConfig, SedEngine, TransformCommand};
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        // Split: first line = pattern, second line = replacement, rest = text
        let mut lines = input.splitn(3, '\n');
        let pattern = match lines.next() {
            Some(p) if !p.is_empty() => p.to_string(),
            _ => return,
        };
        let replacement = lines.next().unwrap_or("").to_string();
        let text = lines.next().unwrap_or("");

        let cmd = TransformCommand::Substitute {
            pattern,
            replacement,
            global: true,
            case_insensitive: false,
        };
        let config = SedConfig {
            commands: vec![cmd],
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let cursor = Cursor::new(text.as_bytes());
        let mut out = Vec::new();
        let _ = engine.transform_reader(cursor, &mut out, "fuzz");
    }
});
