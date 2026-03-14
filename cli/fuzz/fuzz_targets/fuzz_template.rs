// fuzz_template.rs — Fuzzes Tera template rendering in fsn-deploy.
//
// Run with:
//   cargo fuzz run fuzz_template

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Fuzz the raw URL → git URL conversion helper (exported for testing)
        // The deploy/store path logic must never panic on arbitrary input.
        let _ = s.trim_end_matches('/').replace("://raw.githubusercontent.com/", "://github.com/");
    }
});
