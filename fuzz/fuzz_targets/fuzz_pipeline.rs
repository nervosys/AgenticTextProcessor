#![no_main]

use libfuzzer_sys::fuzz_target;

/// Fuzz the pipeline YAML/DSL parser — should never panic on arbitrary input.
fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        // Try YAML pipeline parsing
        let _ = atp_core::engine::pipeline::Pipeline::from_yaml(input);

        // Try DSL pipeline parsing
        let _ = atp_core::engine::pipeline::Pipeline::from_dsl(input);
    }
});
