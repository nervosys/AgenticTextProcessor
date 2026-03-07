#![no_main]

use libfuzzer_sys::fuzz_target;

/// Fuzz the optimizer on arbitrary stage names — should never panic.
fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        let stage_names: Vec<&str> = input.lines().collect();
        if stage_names.is_empty() || stage_names.len() > 100 {
            return;
        }

        let stages: Vec<atp_core::optimizer::StageDesc> = stage_names
            .iter()
            .enumerate()
            .map(|(i, name)| atp_core::optimizer::stage_desc(name, i))
            .collect();

        let plan = atp_core::optimizer::optimize(&stages);
        let _ = atp_core::optimizer::format_plan(&plan);
        let _ = serde_json::to_string(&plan);
    }
});
