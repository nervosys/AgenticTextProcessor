#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        // Fuzz the AQL parser — should never panic
        let _ = atp_core::engine::aql::parse(input);

        // If it parses, try executing on empty input
        if let Ok(pipeline) = atp_core::engine::aql::parse(input) {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("fuzz.txt");
            std::fs::write(&file, "line one\nline two\nline three\n").unwrap();
            let mut engine = atp_core::AqlEngine::new();
            let _ = engine.execute(&pipeline, &[file.as_path()]);
        }
    }
});
