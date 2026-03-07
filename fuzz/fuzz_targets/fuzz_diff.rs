#![no_main]

use libfuzzer_sys::fuzz_target;

/// Fuzz the JSON diff engine — should never panic on arbitrary JSON.
fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        // Split input in half to get two JSON values
        let mid = input.len() / 2;
        let (left_str, right_str) = input.split_at(mid);

        if let (Ok(left), Ok(right)) = (
            serde_json::from_str::<serde_json::Value>(left_str),
            serde_json::from_str::<serde_json::Value>(right_str),
        ) {
            let result = atp_core::diff::diff_json(&left, &right);
            // Try applying the patch back
            let _ = atp_core::diff::apply_patch(left.clone(), &result.ops);
        }

        // Also fuzz text diff
        let lines = atp_core::diff::diff_text(left_str, right_str);
        let _ = atp_core::diff::format_unified(&lines);
    }
});
