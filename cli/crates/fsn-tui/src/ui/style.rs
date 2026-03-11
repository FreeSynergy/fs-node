// Visual style trait — centralized OOP dispatch for colors and indicators.
//
// Pattern: Trait-based dispatch (OOP) — eliminates scattered match blocks.
// All types that carry visual style implement Styleable, so call sites
// say `state.fg_color()` instead of `run_state_color(state)`.
//
// Implementing types:
//   RunState    — replaces run_state_color() and run_state_char() in widgets.rs
//   HealthLevel — centralizes health indicator styles (was health_color() in widgets.rs)
//   ContextAction — centralizes action danger styling (red for danger actions)
//
// To change the visual style of any type: edit only its Styleable impl here.
// No other files need to change.

use ratatui::style::{Color, Modifier, Style};

use fsn_core::health::HealthLevel;
use fsn_core::state::actual::RunState;

use crate::app::ContextAction;

// ── Styleable trait ───────────────────────────────────────────────────────────

/// A type that has a canonical visual representation in the TUI.
///
/// Implementing this trait on a type moves all color/indicator logic onto the
/// type itself — eliminating free functions like `run_state_color(state)` and
/// `health_color(level)` from widgets.rs.
pub trait Styleable {
    /// The full ratatui Style (fg + modifiers) for this value.
    fn style(&self) -> Style;
    /// The foreground color only — convenience for spans.
    fn fg_color(&self) -> Color;
    /// A single-character (or short string) indicator glyph.
    fn indicator_char(&self) -> &'static str;
}

// ── RunState ─────────────────────────────────────────────────────────────────

impl Styleable for RunState {
    fn fg_color(&self) -> Color {
        match self {
            RunState::Running => Color::Green,
            RunState::Stopped => Color::DarkGray,
            RunState::Failed  => Color::Red,
            RunState::Missing => Color::DarkGray,
        }
    }

    fn style(&self) -> Style {
        Style::default().fg(self.fg_color())
    }

    fn indicator_char(&self) -> &'static str {
        match self {
            RunState::Running => "●",
            RunState::Stopped => "○",
            RunState::Failed  => "✕",
            RunState::Missing => "·",
        }
    }
}

// ── HealthLevel ───────────────────────────────────────────────────────────────

impl Styleable for HealthLevel {
    fn fg_color(&self) -> Color {
        match self {
            HealthLevel::Ok      => Color::Green,
            HealthLevel::Warning => Color::Yellow,
            HealthLevel::Error   => Color::Red,
        }
    }

    fn style(&self) -> Style {
        let base = Style::default().fg(self.fg_color());
        if *self == HealthLevel::Error {
            base.add_modifier(Modifier::BOLD)
        } else {
            base
        }
    }

    fn indicator_char(&self) -> &'static str {
        self.indicator()
    }
}

// ── ContextAction ─────────────────────────────────────────────────────────────

impl Styleable for ContextAction {
    fn fg_color(&self) -> Color {
        if self.is_danger() { Color::Red } else { Color::White }
    }

    fn style(&self) -> Style {
        Style::default().fg(self.fg_color())
    }

    fn indicator_char(&self) -> &'static str {
        ""
    }
}
