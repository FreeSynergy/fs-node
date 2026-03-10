// UI layout abstraction — pure geometry, no rendering.
//
// AppLayout::compute(area, cfg) → named Rects for every zone.
// Screens declare what they need via LayoutConfig; the engine handles
// all constraint arithmetic. No layout math lives outside this file.
//
// Zone hierarchy:
//
//   ┌──────────────────────────────────────────────┐
//   │  topbar              (fixed height, always)  │
//   ├──────────────────────────────────────────────┤
//   │  menubar             (optional)              │
//   ├──────────────────────────────────────────────┤
//   │  subheader           (optional)              │
//   ├────────────┬──────────────────┬──────────────┤
//   │  left      │      main        │    right     │
//   │ (optional) │   (always)       │  (optional)  │
//   ├────────────┴──────────────────┴──────────────┤
//   │  footer_primary      (fixed height, always)  │
//   ├──────────────────────────────────────────────┤
//   │  footer_secondary    (optional)              │
//   └──────────────────────────────────────────────┘
//
// Usage:
//   let layout = AppLayout::compute(area, &LayoutConfig {
//       left_width: Some(28),
//       right_width: Some(30),
//       ..LayoutConfig::default()
//   });
//   render_header(f, state, layout.topbar);
//   render_sidebar(f, state, layout.body.left.unwrap_or(layout.body.main));

use ratatui::layout::{Constraint, Layout, Rect};

// ── Configuration ─────────────────────────────────────────────────────────────

/// Declares which zones exist and how large they are.
/// Zero height / None width = zone is absent from the computed layout.
pub struct LayoutConfig {
    /// Height of the top bar in rows.
    pub topbar_height: u16,

    /// Height of the menu bar in rows (0 = no menu bar).
    pub menubar_height: u16,

    /// Height of the sub-header row in rows (0 = none).
    pub subheader_height: u16,

    /// Width of the left sidebar column in characters (None = no sidebar).
    pub left_width: Option<u16>,

    /// Width of the right panel in characters (None = no right panel).
    pub right_width: Option<u16>,

    /// Height of the primary footer row.
    pub footer_height: u16,

    /// Height of the secondary footer row in rows (0 = none).
    pub footer_secondary_height: u16,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            topbar_height:           5,
            menubar_height:          0,
            subheader_height:        0,
            left_width:              None,
            right_width:             None,
            footer_height:           1,
            footer_secondary_height: 0,
        }
    }
}

// ── Computed layout ───────────────────────────────────────────────────────────

/// Computed layout with a named Rect for every zone.
/// Optional zones are `None` when the config height / width is zero.
pub struct AppLayout {
    pub topbar:           Rect,
    pub menubar:          Option<Rect>,
    pub subheader:        Option<Rect>,
    pub body:             BodyLayout,
    pub footer_primary:   Rect,
    pub footer_secondary: Option<Rect>,
}

/// Horizontal split inside the body zone.
pub struct BodyLayout {
    /// Left sidebar column — `None` when `LayoutConfig::left_width` is `None`.
    pub left:  Option<Rect>,
    /// Main content area — always present.
    pub main:  Rect,
    /// Right panel — `None` when `LayoutConfig::right_width` is `None`.
    pub right: Option<Rect>,
}

impl AppLayout {
    /// Compute the full layout from a terminal area and a config.
    /// Pure function: same inputs produce identical Rects.
    pub fn compute(area: Rect, cfg: &LayoutConfig) -> Self {
        // ── Vertical zones ────────────────────────────────────────────────────
        let mut v = vec![Constraint::Length(cfg.topbar_height)];
        if cfg.menubar_height          > 0 { v.push(Constraint::Length(cfg.menubar_height)); }
        if cfg.subheader_height        > 0 { v.push(Constraint::Length(cfg.subheader_height)); }
        v.push(Constraint::Min(1)); // body
        v.push(Constraint::Length(cfg.footer_height));
        if cfg.footer_secondary_height > 0 { v.push(Constraint::Length(cfg.footer_secondary_height)); }

        let v_zones = Layout::vertical(v).split(area);

        let mut i = 0usize;
        let topbar           = v_zones[i]; i += 1;
        let menubar          = if cfg.menubar_height > 0          { let r = Some(v_zones[i]); i += 1; r } else { None };
        let subheader        = if cfg.subheader_height > 0        { let r = Some(v_zones[i]); i += 1; r } else { None };
        let body_rect        = v_zones[i]; i += 1;
        let footer_primary   = v_zones[i]; i += 1;
        let footer_secondary = if cfg.footer_secondary_height > 0 { Some(v_zones[i]) }         else { None };

        // ── Horizontal body zones ─────────────────────────────────────────────
        let body = BodyLayout::compute(body_rect, cfg);

        Self { topbar, menubar, subheader, body, footer_primary, footer_secondary }
    }
}

impl BodyLayout {
    fn compute(area: Rect, cfg: &LayoutConfig) -> Self {
        let mut h: Vec<Constraint> = vec![];
        if let Some(w) = cfg.left_width  { h.push(Constraint::Length(w)); }
        h.push(Constraint::Min(1));
        if let Some(w) = cfg.right_width { h.push(Constraint::Length(w)); }

        let h_zones = Layout::horizontal(h).split(area);

        let mut i = 0usize;
        let left  = if cfg.left_width.is_some()  { let r = Some(h_zones[i]); i += 1; r } else { None };
        let main  = h_zones[i]; i += 1;
        let right = if cfg.right_width.is_some()  { Some(h_zones[i]) }               else { None };

        Self { left, main, right }
    }
}
