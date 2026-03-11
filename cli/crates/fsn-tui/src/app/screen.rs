// Screen and dashboard focus enums.
//
// Pattern: State Machine discriminant — Screen is the top-level state that
// drives which renderer and event handler are active. DashFocus is a
// sub-state within Screen::Dashboard.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    Dashboard,
    /// Form screen — shows the active form from `form_queue`.
    /// Queue tab bar is visible when `form_queue.has_multiple()`.
    NewProject,
    /// Application settings — store management, preferences.
    Settings,
}

// ── Dashboard focus ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashFocus {
    Sidebar,
    Services,
}
