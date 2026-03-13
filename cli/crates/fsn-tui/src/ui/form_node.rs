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
//
// This eliminates all per-field-type checks from events.rs (no more
// `is_select_field()`, `is_typing()`, etc.) — correct behavior is built in.
//
// Mouse handling is delegated to rat-widget (HandleEvent trait).
//
// Future extensions (same FormNode interface, different output backend):
//   fn render_html(&self, lang: Lang) -> String
//   fn to_json(&self, lang: Lang) -> serde_json::Value

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

pub use fsn_core::FormAction;

use crate::ui::render_ctx::RenderCtx;

// ── Common navigation helper ──────────────────────────────────────────────────

/// Handle form-level navigation shortcuts — call at the top of every `FormNode::handle_key`.
///
/// Returns `Some(action)` for:
///   Ctrl+S           → Submit (works on all terminals; Ctrl+Enter does NOT)
///   Ctrl+←           → TabPrev
///   Ctrl+→           → TabNext
///
/// Tab / BackTab / Esc are intentionally excluded because widgets handle them
/// differently (e.g. TextArea: Tab=FocusNext, EnvTable: Tab=column-nav).
pub fn handle_form_nav(key: KeyEvent) -> Option<FormAction> {
    match key.code {
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(FormAction::Submit),
        KeyCode::Left      if key.modifiers.contains(KeyModifiers::CONTROL) => Some(FormAction::TabPrev),
        KeyCode::Right     if key.modifiers.contains(KeyModifiers::CONTROL) => Some(FormAction::TabNext),
        _ => None,
    }
}

/// Handle navigation shortcuts for selection-type nodes (Select, MultiSelect, ServiceSlot).
///
/// Design Pattern: Utility Library — Single Source of Truth for shared shortcuts.
///
/// These nodes do NOT accept free-text input, so Tab / BackTab / Esc / L / l can be
/// handled uniformly here instead of being duplicated in each node's `handle_key`.
///
/// Extends `handle_form_nav` with:
///   Tab              → FocusNext
///   BackTab          → FocusPrev
///   Esc              → Cancel
///   l / L            → LangToggle  (safe because no text entry here)
///
/// Text-accepting nodes (TextInput, TextArea, EnvTable) must NOT call this — they
/// handle Tab and character keys differently, and L should type the letter L.
pub fn handle_selection_nav(key: KeyEvent) -> Option<FormAction> {
    if let Some(nav) = handle_form_nav(key) { return Some(nav); }
    match key.code {
        KeyCode::Tab                                => Some(FormAction::FocusNext),
        KeyCode::BackTab                            => Some(FormAction::FocusPrev),
        KeyCode::Esc                                => Some(FormAction::Cancel),
        KeyCode::Char('l') | KeyCode::Char('L')    => Some(FormAction::LangToggle),
        _                                           => None,
    }
}

// ── FormNode trait ────────────────────────────────────────────────────────────

/// A UI component analogous to an HTML input element.
///
/// Each FormNode is fully self-contained:
/// - **State**: owns its value, cursor position, dirty flag
/// - **Render**: draws label + input box + hint; overlays (dropdowns) via `render_overlay`
/// - **Events**: handles keyboard input and returns a [`FormAction`] — no external dispatch
///
/// Mouse handling will be provided by rat-widget's `HandleEvent` trait.
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

    // ── Layout ─────────────────────────────────────────────────────────────

    /// Column span in a 12-column grid (1–12). Default: 12 (full row).
    /// Adjacent fields are placed side-by-side while their spans sum to ≤ 12.
    fn col_span(&self) -> u8 { 12 }

    /// Minimum rendered width (terminal columns) before this field wraps to
    /// its own row. 0 = always inline. Default: 0.
    fn min_width(&self) -> u16 { 0 }

    /// Whether keyboard focus can land on this field.
    /// `SectionNode` returns `false`; all interactive nodes return `true` (default).
    fn is_focusable(&self) -> bool { true }

    // ── Rendering ──────────────────────────────────────────────────────────

    /// How many rows this field needs in the form layout.
    ///
    /// Default: 4 (box-with-title 3 rows + hint 1 row).
    /// TextAreaNode overrides this based on its configured `visible_lines`.
    fn preferred_height(&self) -> u16 { 4 }

    /// Render the field (label-in-title + input box + hint) into `area`.
    /// Use `f.translate(key)` for i18n — lang is carried by `RenderCtx`.
    fn render(&mut self, f: &mut RenderCtx<'_>, area: Rect, focused: bool);

    /// Render a floating overlay (e.g., dropdown list) below the input box.
    /// Called *after* all fields are rendered so the overlay appears on top.
    /// Default: no-op (text inputs have no overlay).
    fn render_overlay(&mut self, _f: &mut RenderCtx<'_>, _available: Rect) {}

    // ── Input ──────────────────────────────────────────────────────────────

    /// Handle a keyboard event. Returns the action for the outer handler.
    fn handle_key(&mut self, key: KeyEvent) -> FormAction;

    /// Handle a mouse event inside this node's rendered area.
    ///
    /// `area` is the full Rect that `render()` was called with (stored in
    /// `ResourceForm::field_rects` after each frame).  The default is a no-op.
    fn handle_mouse(&mut self, _event: crossterm::event::MouseEvent, _area: Rect) -> FormAction {
        FormAction::Unhandled
    }

    /// Whether this node currently has an open overlay popup.
    ///
    /// When `true`, mouse events are routed to `handle_popup_mouse()` first,
    /// bypassing normal field hit-testing — the popup is its own interaction layer.
    fn has_open_popup(&self) -> bool { false }

    /// Handle a mouse event directed at an open popup (called when `has_open_popup()`
    /// returns `true`).  Returns the action to propagate, or `None` if not handled.
    fn handle_popup_mouse(&mut self, _event: crossterm::event::MouseEvent) -> Option<FormAction> {
        None
    }

    // ── Validation ─────────────────────────────────────────────────────────

    fn is_filled(&self) -> bool {
        !self.effective_value().trim().is_empty()
    }
    fn is_valid(&self) -> bool {
        !self.required() || self.is_filled()
    }
}
