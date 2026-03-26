//! Fuzz target: render an arbitrary template with arbitrary key-value variables.
//!
//! Uses a structured input to split fuzz data into template + variable pairs,
//! exercising variable substitution, filters (to_slug, to_env_key, indent, …)
//! and error paths.
//!
//! Run: cargo fuzz run fuzz_render_str_with_vars
#![no_main]

use fs_template::{TemplateContext, TemplateEngine};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Split data at the first NUL byte: template | vars
    let (template_bytes, rest) = match data.iter().position(|&b| b == 0) {
        Some(pos) => (&data[..pos], &data[pos + 1..]),
        None => (data, b"".as_ref()),
    };

    let Ok(template) = std::str::from_utf8(template_bytes) else { return };
    let Ok(vars_str) = std::str::from_utf8(rest) else { return };

    let engine = TemplateEngine::new();
    let mut ctx = TemplateContext::new();

    // Parse vars_str as "KEY=VALUE\nKEY2=VALUE2\n..."
    for line in vars_str.lines().take(16) {
        if let Some((k, v)) = line.split_once('=') {
            ctx.set_str(k.trim(), v.trim());
        }
    }

    let _ = engine.render_str(template, &ctx);
});
