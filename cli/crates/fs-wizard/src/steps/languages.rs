// Language selection step — set the primary UI locale.

use super::WizardStep;

/// Input data for the language selection step.
#[derive(Debug, Clone)]
pub struct LanguagesInput {
    /// IETF language tag for the primary locale (e.g. "en", "de").
    pub locale: String,
}

impl Default for LanguagesInput {
    fn default() -> Self {
        Self {
            locale: "en".to_string(),
        }
    }
}

/// Wizard step that selects the primary display language.
pub struct LanguagesStep;

impl LanguagesStep {
    /// Create a new `LanguagesStep`.
    pub fn new() -> Self {
        Self
    }

    /// Well-known supported locales.
    pub fn supported_locales() -> &'static [&'static str] {
        &["en", "de", "fr", "es", "pt", "nl", "it", "ru", "zh", "ja"]
    }
}

impl Default for LanguagesStep {
    fn default() -> Self {
        Self::new()
    }
}

impl WizardStep for LanguagesStep {
    type Input = LanguagesInput;
    type Output = LanguagesInput;

    fn title(&self) -> &str {
        "Language"
    }

    fn validate(&self, input: &Self::Input) -> Vec<String> {
        let mut errors = Vec::new();
        if input.locale.trim().is_empty() {
            errors.push("A locale is required.".to_string());
        } else if input.locale.len() < 2 {
            errors.push("Locale must be at least 2 characters (e.g. \"en\", \"de\").".to_string());
        }
        errors
    }
}
