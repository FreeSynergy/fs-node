// Shared dashboard shortcuts and sidebar filter key handler.
//
// Pattern: Chain of Responsibility — shared shortcuts are checked first (once),
// before the focus-specific handler (sidebar or services) sees the key.
//
// Shared keys: q/Esc → quit confirm | n → new-resource popup.
// Sidebar filter: intercepts all keys when `state.sidebar_filter.is_some()`.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{AppState, ConfirmAction, OverlayLayer};

// ── Shared dashboard shortcuts ────────────────────────────────────────────────

/// Handle keys that are identical in both sidebar and services focus.
/// Returns `true` if the key was consumed so the caller can return early.
///
/// Shared keys:  q/Esc → quit confirm  |  n → new-resource popup
/// Note: 'L' lang-toggle is handled globally in events.rs before screen dispatch.
pub(super) fn handle_dashboard_shared(key: KeyEvent, state: &mut AppState) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.push_overlay(OverlayLayer::Confirm {
                message: "confirm.quit".into(), data: None, yes_action: ConfirmAction::Quit,
            });
            true
        }
        KeyCode::Char('n') => {
            // Show the full new-resource picker (all 4 types: Project / Host / Service / Bot).
            // Uses NewResource overlay — rendered by ui/mod.rs::render_new_resource(),
            // handled by handle_new_resource_overlay().
            state.push_overlay(crate::app::OverlayLayer::NewResource { selected: 0 });
            true
        }
        _ => false,
    }
}

// ── Sidebar filter ────────────────────────────────────────────────────────────

/// Handle key events while the sidebar filter is open.
/// Esc/Enter close the filter; Up/Down navigate visible items; typing refines.
pub(super) fn handle_sidebar_filter_key(key: KeyEvent, state: &mut AppState) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            state.sidebar_filter = None;
        }
        KeyCode::Up => {
            let indices: Vec<usize> = state.visible_sidebar_items().into_iter().map(|(i, _)| i).collect();
            if let Some(pos) = indices.iter().position(|&i| i == state.sidebar_cursor) {
                if pos > 0 { state.sidebar_cursor = indices[pos - 1]; }
            }
        }
        KeyCode::Down => {
            let indices: Vec<usize> = state.visible_sidebar_items().into_iter().map(|(i, _)| i).collect();
            if let Some(pos) = indices.iter().position(|&i| i == state.sidebar_cursor) {
                if pos + 1 < indices.len() { state.sidebar_cursor = indices[pos + 1]; }
            } else if let Some(&first) = indices.first() {
                state.sidebar_cursor = first;
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut f) = state.sidebar_filter { f.pop(); }
            adjust_cursor_to_filter(state);
        }
        KeyCode::Char(c) => {
            if let Some(ref mut f) = state.sidebar_filter { f.push(c); }
            adjust_cursor_to_filter(state);
        }
        _ => {}
    }
    Ok(())
}

/// After the filter query changes, ensure sidebar_cursor points to a visible item.
pub(super) fn adjust_cursor_to_filter(state: &mut AppState) {
    let indices: Vec<usize> = state.visible_sidebar_items().into_iter().map(|(i, _)| i).collect();
    if indices.is_empty() { return; }
    if !indices.contains(&state.sidebar_cursor) {
        state.sidebar_cursor = indices[0];
    }
}
