// Component-based form field architecture — the HTML element analogy for fsn-tui.
//
// Design principle: analogous to the HTML input element hierarchy.
//   HTMLElement → HTMLInputElement (text, password, email, …)
//   FormNode    → TextInputNode / SelectInputNode / …
//
// Each FormNode is a fully self-contained UI component:
//   • Owns its own state (value, cursor position, options, dirty flag)
//   • Renders itself: label + input box + hint (render)
//   • Renders overlays that must appear on top of siblings (render_overlay)
//   • Handles keyboard input, returns a typed FormAction
//   • Stores its last rendered Rect for mouse hit-testing (hit_test)
//
// This eliminates all per-field-type checks from events.rs (no more
// `is_select_field()`, `is_typing()`, etc.) — correct behavior is built in.
//
// Future extensions (same FormNode interface, different output backend):
//   fn render_html(&self, lang: Lang) -> String
//   fn to_json(&self, lang: Lang) -> serde_json::Value

use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

use crate::app::Lang;

// ── FormAction ────────────────────────────────────────────────────────────────

/// What a form node returns after handling a keyboard event.
/// The outer handler (events.rs) reacts to these without knowing field details.
#[derive(Debug, Clone, PartialEq)]
pub enum FormAction {
    /// Event consumed internally; no outer action needed.
    Consumed,
    /// Value was modified (triggers the form's `on_change` hook).
    ValueChanged,
    /// Move focus to the next node in the current tab.
    FocusNext,
    /// Move focus to the previous node in the current tab.
    FocusPrev,
    /// Advance to the next form tab (Ctrl+Right).
    TabNext,
    /// Go back to the previous form tab (Ctrl+Left).
    TabPrev,
    /// Enter was pressed: attempt to advance or submit the form.
    Submit,
    /// Close the form / pop the current screen (Esc).
    Cancel,
    /// Toggle the UI language (L/l key outside text input).
    LangToggle,
    /// Quit the application (Ctrl+C — handled before node dispatch).
    Quit,
    /// Event not handled by this node; fall through to the outer handler.
    Unhandled,
}

// ── FormNode trait ────────────────────────────────────────────────────────────

/// A UI component analogous to an HTML input element.
///
/// Each FormNode is fully self-contained:
/// - **State**: owns its value, cursor position, dirty flag, and last rendered `Rect`
/// - **Render**: draws label + input box + hint; overlays (dropdowns) via `render_overlay`
/// - **Events**: handles keyboard input and returns a [`FormAction`] — no external dispatch
/// - **Hit-test**: stores its `Rect` during `render` for mouse click detection next cycle
///
/// Adding a new field type = implement `FormNode`. No changes needed in `events.rs`.
///
/// Implementing types: [`super::nodes::TextInputNode`], [`super::nodes::SelectInputNode`].
pub trait FormNode: std::fmt::Debug {
    // ── Identity ───────────────────────────────────────────────────────────

    /// Unique field identifier, used by `on_change` hooks to find siblings.
    fn key(&self) -> &'static str;
    /// i18n key for the label shown above the input.
    fn label_key(&self) -> &'static str;
    /// Optional i18n key for the hint line below the input.
    fn hint_key(&self) -> Option<&'static str>;
    /// Which tab this field belongs to (0-based).
    fn tab(&self) -> usize;
    /// Whether the field must be non-empty to submit.
    fn required(&self) -> bool;

    // ── Value ──────────────────────────────────────────────────────────────

    /// Raw value as typed by the user.
    fn value(&self) -> &str;
    /// Value for submit: returns the built-in default when the user left the field empty.
    fn effective_value(&self) -> &str;
    /// Set value programmatically (smart-defaults from `on_change`).
    fn set_value(&mut self, v: &str);
    /// Whether the user has manually edited this field.
    fn is_dirty(&self) -> bool;
    fn set_dirty(&mut self, v: bool);

    // ── Rendering ──────────────────────────────────────────────────────────

    /// Render the field (label + input box + hint) into `area`.
    /// Must call `self.set_rect(area)` so hit-testing works.
    fn render(&mut self, f: &mut Frame, area: Rect, focused: bool, lang: Lang);

    /// Render a floating overlay (e.g., dropdown list) below the input box.
    /// Called *after* all fields are rendered so the overlay appears on top.
    /// Default: no-op (text inputs have no overlay).
    fn render_overlay(&mut self, _f: &mut Frame, _available: Rect, _lang: Lang) {}

    /// Record the last rendered rect for hit-testing.
    fn set_rect(&mut self, rect: Rect);
    /// The last recorded rect, or `None` before first render.
    fn last_rect(&self) -> Option<Rect>;

    /// Returns `true` when the terminal coordinate falls within this field's area.
    fn hit_test(&self, col: u16, row: u16) -> bool {
        self.last_rect()
            .map(|r| col >= r.x && col < r.right() && row >= r.y && row < r.bottom())
            .unwrap_or(false)
    }

    // ── Input ──────────────────────────────────────────────────────────────

    /// Handle a keyboard event. Returns the action for the outer handler.
    fn handle_key(&mut self, key: KeyEvent) -> FormAction;

    // ── Validation ─────────────────────────────────────────────────────────

    fn is_filled(&self) -> bool {
        !self.effective_value().trim().is_empty()
    }
    fn is_valid(&self) -> bool {
        !self.required() || self.is_filled()
    }
}
