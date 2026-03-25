// Timezone selection step — set the system timezone.

use super::WizardStep;

/// Input data for the timezone selection step.
#[derive(Debug, Clone)]
pub struct TimezoneInput {
    /// IANA timezone identifier (e.g. "Europe/Berlin", "America/New_York").
    pub tz: String,
}

impl Default for TimezoneInput {
    fn default() -> Self {
        Self {
            tz: "UTC".to_string(),
        }
    }
}

/// Wizard step that selects the system timezone.
pub struct TimezoneStep;

impl TimezoneStep {
    /// Create a new `TimezoneStep`.
    pub fn new() -> Self {
        Self
    }

    /// A selection of common timezone identifiers.
    pub fn common_timezones() -> &'static [&'static str] {
        &[
            "UTC",
            "Europe/London",
            "Europe/Berlin",
            "Europe/Paris",
            "Europe/Madrid",
            "Europe/Rome",
            "Europe/Amsterdam",
            "America/New_York",
            "America/Chicago",
            "America/Denver",
            "America/Los_Angeles",
            "America/Sao_Paulo",
            "Asia/Tokyo",
            "Asia/Shanghai",
            "Asia/Singapore",
            "Asia/Kolkata",
            "Australia/Sydney",
            "Pacific/Auckland",
        ]
    }
}

impl Default for TimezoneStep {
    fn default() -> Self {
        Self::new()
    }
}

impl WizardStep for TimezoneStep {
    type Input = TimezoneInput;
    type Output = TimezoneInput;

    fn title(&self) -> &str {
        "Timezone"
    }

    fn validate(&self, input: &Self::Input) -> Vec<String> {
        let mut errors = Vec::new();
        if input.tz.trim().is_empty() {
            errors.push("Timezone is required.".to_string());
        } else if !input.tz.contains('/') && input.tz != "UTC" {
            errors.push(
                "Timezone must be a valid IANA identifier (e.g. \"Europe/Berlin\") or \"UTC\"."
                    .to_string(),
            );
        }
        errors
    }
}
