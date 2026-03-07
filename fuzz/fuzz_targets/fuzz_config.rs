#![no_main]

use libfuzzer_sys::fuzz_target;

/// Fuzz the TOML project-config parser — should never panic.
fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        // Try parsing as project config
        let _ = toml::from_str::<atp_core::config::ProjectConfig>(input);
    }
});
