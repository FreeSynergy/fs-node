// fuzz_config.rs — Fuzzes ProjectConfig + HostConfig TOML parsing.
//
// Run with:
//   cargo fuzz run fuzz_config
//
// Any panic or unexpected error in the parser = potential bug.

#![no_main]

use libfuzzer_sys::fuzz_target;
use fsn_core::config::{ProjectConfig, HostConfig};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Fuzz project config parsing
        let _ = toml::from_str::<ProjectConfig>(s);
        // Fuzz host config parsing
        let _ = toml::from_str::<HostConfig>(s);
    }
});
