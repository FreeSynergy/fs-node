mod cli;
mod commands;
mod db;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

// Only EN is bundled — all other languages are downloaded via `fsn store i18n set`.
const LOCALE_EN: &str = include_str!("../locales/en/cli.toml");

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("(no message)");
        tracing::error!(panic.location = %location, panic.message = %message, "fsn panicked — this is a bug, please report it");
    }));

    // Active language: user-set marker file → system env → "en"
    let lang = commands::store::I18nCmd::active_lang();

    // Build locale list: EN always present as fallback; add cached pack if available
    let cached = load_cached_lang(&lang);
    let cached_str = cached.as_deref().unwrap_or("");

    if cached.is_some() {
        let _ = fs_i18n::init_with_toml_strs(&lang, &[("en", LOCALE_EN), (&lang, cached_str)]);
    } else {
        let _ = fs_i18n::init_with_toml_strs("en", &[("en", LOCALE_EN)]);
    }

    if let Err(e) = db::init().await {
        tracing::warn!("DB unavailable: {e}");
    } else {
        db::spawn_flush_loop();
    }

    let result = cli::run().await;
    db::flush().await;
    result
}

/// Load the cached ui.toml for `lang` from `~/.local/share/fsn/i18n/{lang}.toml`.
/// Returns None if not found.
fn load_cached_lang(lang: &str) -> Option<String> {
    if lang == "en" {
        return None;
    }
    let path = commands::store::I18nCmd::cache_dir().join(format!("{lang}.toml"));
    std::fs::read_to_string(path).ok()
}
