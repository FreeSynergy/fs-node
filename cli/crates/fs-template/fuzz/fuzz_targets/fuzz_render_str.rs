//! Fuzz target: render an arbitrary template string with an empty context.
//!
//! Finds panics, OOM, or infinite loops in Tera's template parser/renderer
//! when handling malformed or adversarial template strings.
//!
//! Run: cargo fuzz run fuzz_render_str
#![no_main]

use fs_template::{TemplateContext, TemplateEngine};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(template) = std::str::from_utf8(data) {
        let engine = TemplateEngine::new();
        let ctx = TemplateContext::new();
        // Errors are expected — we only care about panics / OOM
        let _ = engine.render_str(template, &ctx);
    }
});
