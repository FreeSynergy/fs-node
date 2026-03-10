// Selection popup — Strategy component for all form-based selection UI.
//
// Design Pattern: Strategy — isolates "how to present choices" from "what field is shown".
//   SingleMode → centered popup with radio-style (◉/○) items (one choice)
//   MultiMode  → centered popup with checkbox-style ([x]/[ ]) items (many choices)
//
// To swap to a different rendering backend (e.g. rat-widget's native Radio/Checkbox),
// only the two private render_radio() / render_checkboxes() functions need to change.
// The SelectionPopup struct, its public API, and all callers stay the same.
//
// Keyboard:
//   Both modes: ↑↓=move cursor, Esc/←=cancel, Enter/→=confirm
//   Multi only: Space=toggle selected item
//
// Callers: SelectInputNode (single), MultiSelectInputNode (multi).

use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear},
};
use rat_widget::paragraph::{Paragraph, ParagraphState};

use crate::app::Lang;
use crate::ui::render_ctx::RenderCtx;

// ── Public result type ────────────────────────────────────────────────────────

/// What the popup returns after handling a key.
#[derive(Debug, PartialEq)]
pub enum SelectionResult {
    /// Key handled internally — no value change.
    Consumed,
    /// Confirmed: single value (for SingleMode).
    Accepted(String),
    /// Confirmed: multiple values (for MultiMode).
    AcceptedMulti(Vec<String>),
    /// Closed without confirming.
    Rejected,
    /// Key not for the popup — fall through.
    Unhandled,
}

// ── SelectionMode ─────────────────────────────────────────────────────────────

/// Whether this popup is single-choice (radio) or multi-choice (checkboxes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectionMode {
    Single,
    Multi,
}

// ── SelectionPopup ────────────────────────────────────────────────────────────

/// Popup state for single- or multi-choice selection.
///
/// Constructed via [`SelectionPopup::single()`] or [`SelectionPopup::multi()`].
/// Drop into any FormNode that needs a popup chooser:
/// ```ignore
/// fn render_overlay(&mut self, f, available, lang) {
///     self.popup.render(f, &self.options, self.display_fn, lang);
/// }
/// fn handle_key(&mut self, key) -> FormAction {
///     if self.popup.is_open {
///         return match self.popup.handle_key(key, &self.options) { ... };
///     }
///     ...
/// }
/// ```
#[derive(Debug)]
pub struct SelectionPopup {
    /// Whether the popup is currently visible.
    pub is_open: bool,
    mode: SelectionMode,
    /// Cursor position in the options list.
    pub pending_idx: usize,
    /// Checked items for MultiMode (indices into options).
    pub multi_checked: HashSet<usize>,
    /// Rect of the popup after the last render — used for mouse hit-testing.
    /// Set by render(), read by handle_mouse().
    rendered_rect: Option<Rect>,
}

impl SelectionPopup {
    pub fn single() -> Self {
        Self { is_open: false, mode: SelectionMode::Single, pending_idx: 0, multi_checked: HashSet::new(), rendered_rect: None }
    }

    pub fn multi() -> Self {
        Self { is_open: false, mode: SelectionMode::Multi, pending_idx: 0, multi_checked: HashSet::new(), rendered_rect: None }
    }

    /// Open the popup. `current_idx` positions the cursor at the current value.
    /// `checked` pre-fills the multi-select checkboxes.
    pub fn open(&mut self, current_idx: usize, checked: HashSet<usize>) {
        self.pending_idx  = current_idx;
        self.multi_checked = checked;
        self.is_open = true;
    }

    /// Handle a key while the popup is open.
    pub fn handle_key(&mut self, key: KeyEvent, options: &[String]) -> SelectionResult {
        if !self.is_open { return SelectionResult::Unhandled; }

        let n = options.len();
        match key.code {
            KeyCode::Up => {
                if self.pending_idx > 0 { self.pending_idx -= 1; }
                SelectionResult::Consumed
            }
            KeyCode::Down => {
                if self.pending_idx + 1 < n { self.pending_idx += 1; }
                SelectionResult::Consumed
            }
            // Space toggles checkbox in multi mode; no-op in single mode.
            KeyCode::Char(' ') if self.mode == SelectionMode::Multi => {
                let i = self.pending_idx;
                if self.multi_checked.contains(&i) {
                    self.multi_checked.remove(&i);
                } else {
                    self.multi_checked.insert(i);
                }
                SelectionResult::Consumed
            }
            // Confirm
            KeyCode::Enter | KeyCode::Right => {
                self.is_open = false;
                match self.mode {
                    SelectionMode::Single => {
                        let v = options.get(self.pending_idx).cloned().unwrap_or_default();
                        SelectionResult::Accepted(v)
                    }
                    SelectionMode::Multi => {
                        let mut selected: Vec<String> = self.multi_checked.iter()
                            .filter_map(|&i| options.get(i).cloned())
                            .collect();
                        selected.sort();
                        SelectionResult::AcceptedMulti(selected)
                    }
                }
            }
            // Cancel
            KeyCode::Esc | KeyCode::Left => {
                self.is_open = false;
                SelectionResult::Rejected
            }
            _ => SelectionResult::Consumed, // swallow all other keys while open
        }
    }

    /// Render the popup centered on the full terminal area.
    ///
    /// Call from `FormNode::render_overlay`. Uses `f.area()` for the full screen rect.
    pub fn render(
        &mut self,
        f:          &mut RenderCtx<'_>,
        options:    &[String],
        display_fn: Option<fn(&str) -> &'static str>,
        title_key:  &'static str,
        lang:       Lang,
    ) {
        if !self.is_open { return; }

        let screen = f.area();
        let popup  = popup_rect(options.len(), self.mode, screen);
        self.rendered_rect = Some(popup); // store for mouse hit-testing

        let hint_line = hint_line(self.mode, lang);
        let title_text = crate::i18n::t(lang, title_key);

        let block = Block::default()
            .title(Span::styled(
                format!(" {} ", title_text),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup);
        f.render_widget(Clear, popup);
        f.render_widget(block, popup);

        // Split inner into: items area + empty row + hint row
        let items_h    = (inner.height).saturating_sub(2);
        let items_area = Rect { height: items_h, ..inner };
        let hint_area  = Rect { y: inner.bottom().saturating_sub(1), height: 1, ..inner };

        match self.mode {
            SelectionMode::Single => render_radio(f, items_area, options, display_fn, self.pending_idx),
            SelectionMode::Multi  => render_checkboxes(f, items_area, options, display_fn, self.pending_idx, &self.multi_checked),
        }

        f.render_stateful_widget(
            Paragraph::new(Line::from(Span::styled(hint_line, Style::default().fg(Color::DarkGray)))),
            hint_area,
            &mut ParagraphState::new(),
        );
    }

    /// Handle a mouse event while the popup is open.
    ///
    /// Uses `rendered_rect` (set during the last `render()` call) for hit-testing.
    ///
    /// Behaviour:
    ///   Single mode — click on item: accept & close.  Click outside: reject (cancel).
    ///   Multi mode  — click on item: toggle it.       Click outside: accept checked state & close.
    ///   Both modes  — scroll up/down: move cursor.
    pub fn handle_mouse(&mut self, event: MouseEvent, options: &[String]) -> Option<SelectionResult> {
        if !self.is_open { return None; }
        let popup = self.rendered_rect?;
        let col = event.column;
        let row = event.row;

        match event.kind {
            // Scroll moves cursor inside the popup
            MouseEventKind::ScrollUp => {
                if self.pending_idx > 0 { self.pending_idx -= 1; }
                return Some(SelectionResult::Consumed);
            }
            MouseEventKind::ScrollDown => {
                if self.pending_idx + 1 < options.len() { self.pending_idx += 1; }
                return Some(SelectionResult::Consumed);
            }
            MouseEventKind::Down(MouseButton::Left) => {}
            _ => return None,
        }

        // Left click — outside popup?
        let outside = col < popup.x || col >= popup.right() || row < popup.y || row >= popup.bottom();
        if outside {
            self.is_open = false;
            return Some(match self.mode {
                // Single: cancel (value unchanged)
                SelectionMode::Single => SelectionResult::Rejected,
                // Multi: accept whatever is checked
                SelectionMode::Multi => {
                    let mut selected: Vec<String> = self.multi_checked.iter()
                        .filter_map(|&i| options.get(i).cloned())
                        .collect();
                    selected.sort();
                    SelectionResult::AcceptedMulti(selected)
                }
            });
        }

        // Click inside popup — compute which item row was hit.
        // inner = popup with 1-cell border removed on each side.
        // items start at inner.y, hint occupies the last row, empty row before that.
        let inner_y = popup.y + 1;
        let items_h = (popup.height as i32 - 4).max(0) as u16; // items + 2 border + 1 gap + 1 hint
        if row >= inner_y && row < inner_y + items_h {
            let item_idx = (row - inner_y) as usize;
            if item_idx < options.len() {
                self.pending_idx = item_idx;
                match self.mode {
                    SelectionMode::Single => {
                        self.is_open = false;
                        return Some(SelectionResult::Accepted(options[item_idx].clone()));
                    }
                    SelectionMode::Multi => {
                        if self.multi_checked.contains(&item_idx) {
                            self.multi_checked.remove(&item_idx);
                        } else {
                            self.multi_checked.insert(item_idx);
                        }
                        return Some(SelectionResult::Consumed);
                    }
                }
            }
        }

        Some(SelectionResult::Consumed) // click on border / hint area — swallow
    }
}

// ── Popup geometry ────────────────────────────────────────────────────────────

/// Center the popup on `screen`. Width is fixed at 40% of screen (min 36, max 60).
/// Height = items + border(2) + hint gap(1) + hint(1).
fn popup_rect(n_items: usize, _mode: SelectionMode, screen: Rect) -> Rect {
    let width  = (screen.width * 40 / 100).clamp(36, 60).min(screen.width.saturating_sub(4));
    let height = ((n_items as u16) + 4).min(screen.height.saturating_sub(4));
    Rect {
        x:      screen.width.saturating_sub(width) / 2,
        y:      screen.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

// ── Radio rendering (single select) ──────────────────────────────────────────
//
// Renders items as:
//   ◉ Selected option
//   ○ Other option
//
// To swap to rat-widget's Radio StatefulWidget: replace this function body only.

fn render_radio(
    f:          &mut RenderCtx<'_>,
    area:       Rect,
    options:    &[String],
    display_fn: Option<fn(&str) -> &'static str>,
    cursor:     usize,
) {
    let lines: Vec<Line> = options.iter().enumerate().map(|(i, opt)| {
        let label  = display_fn.map(|f| f(opt.as_str())).unwrap_or(opt.as_str());
        let marker = if i == cursor { "◉ " } else { "○ " };
        let style  = if i == cursor {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        Line::from(Span::styled(format!("  {}{}", marker, label), style))
    }).collect();

    f.render_stateful_widget(Paragraph::new(lines), area, &mut ParagraphState::new());
}

// ── Checkbox rendering (multi select) ────────────────────────────────────────
//
// Renders items as:
//   [x] Checked option
//   [ ] Unchecked option
//
// To swap to rat-widget's Checkbox StatefulWidget: replace this function body only.

fn render_checkboxes(
    f:          &mut RenderCtx<'_>,
    area:       Rect,
    options:    &[String],
    display_fn: Option<fn(&str) -> &'static str>,
    cursor:     usize,
    checked:    &HashSet<usize>,
) {
    let lines: Vec<Line> = options.iter().enumerate().map(|(i, opt)| {
        let label    = display_fn.map(|f| f(opt.as_str())).unwrap_or(opt.as_str());
        let is_checked  = checked.contains(&i);
        let checkbox = if is_checked { "[x]" } else { "[ ]" };
        let is_cursor   = i == cursor;
        let (prefix, style) = if is_cursor {
            ("▶ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        } else if is_checked {
            ("  ", Style::default().fg(Color::Green))
        } else {
            ("  ", Style::default().fg(Color::White))
        };
        Line::from(Span::styled(format!("  {}{} {}", prefix, checkbox, label), style))
    }).collect();

    f.render_stateful_widget(Paragraph::new(lines), area, &mut ParagraphState::new());
}

// ── Hint text ─────────────────────────────────────────────────────────────────

fn hint_line(mode: SelectionMode, lang: Lang) -> &'static str {
    match (mode, lang) {
        (SelectionMode::Single, crate::app::Lang::De) => "↑↓=Wählen  Enter=OK  Esc=Abbrechen",
        (SelectionMode::Single, crate::app::Lang::En) => "↑↓=Navigate  Enter=OK  Esc=Cancel",
        (SelectionMode::Multi,  crate::app::Lang::De) => "↑↓=Wählen  Leer=Auswählen  Enter=OK  Esc=Abbrechen",
        (SelectionMode::Multi,  crate::app::Lang::En) => "↑↓=Navigate  Space=Toggle  Enter=OK  Esc=Cancel",
    }
}
