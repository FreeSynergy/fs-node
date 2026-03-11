// Dashboard screen — layout coordinator + component composition.
//
// Design Pattern: Composite — this module composes named Components into
// the dashboard layout. Each component is responsible for its own rendering;
// this module only decides where each component goes.
//
// ┌──────────────────────────────────────────────────────────────────────┐
// │ [BigText FSN]  FreeSynergy.Node                         v0.1  [DE]  │ ← HeaderBar (5 rows)
// │ [          ]  Modular Deployment System  —  by KalEl               │
// │ [          ]  myproject @ example.com                               │
// │ [          ]  ──────────────────────────────────────────────────── │
// │  [Projekte]│[Hosts]│[Services]│[Store]│[⚙ Einstellungen]           │
// ├──────────────┬──────────────────────────────────────────────────────┤
// │              │ ╭──────────╮╭──────────╮╭──────────╮╭──────────╮   │ ← DetailPanel
// │  SidebarList │ │  RAM     ││  System  ││ Running  ││  Alerts  │   │
// │              │ ╰──────────╯╰──────────╯╰──────────╯╰──────────╯   │
// │              │ Services / Detail / Env vars                         │
// ├──────────────┴──────────────────────────────────────────────────────┤
// │  MIT © FreeSynergy.Net             ↑↓=Nav  F1=Hilfe  q=Ende        │ ← FooterBar (1 row)
// └──────────────────────────────────────────────────────────────────────┘
//
// F1 help panel slides in from the right as body.right (via LayoutConfig::right_width).

use ratatui::layout::Rect;

use crate::ui::render_ctx::RenderCtx;
use crate::ui::layout::{AppLayout, LayoutConfig};
use crate::ui::components::{Component, DetailPanel, FooterBar, HeaderBar, SidebarList};
use crate::ui::{help_sidebar};
use crate::app::AppState;

/// Sidebar column width — used by LayoutConfig and mouse.rs for hit-test approximation.
pub(crate) const SIDEBAR_COL_WIDTH: u16 = 28;

pub fn render(f: &mut RenderCtx<'_>, state: &mut AppState, area: Rect) {
    let help_w = (state.help_visible && area.width > help_sidebar::SIDEBAR_WIDTH + 40 + SIDEBAR_COL_WIDTH)
        .then_some(help_sidebar::SIDEBAR_WIDTH);

    let layout = AppLayout::compute(area, &LayoutConfig {
        topbar_height: 5,               // 4 logo rows + 1 tab bar
        left_width:    Some(SIDEBAR_COL_WIDTH),
        right_width:   help_w,
        ..LayoutConfig::default()
    });

    HeaderBar.render(f, layout.topbar, state);
    render_body(f, state, &layout);
    FooterBar.render(f, layout.footer_primary, state);
}

fn render_body(f: &mut RenderCtx<'_>, state: &mut AppState, layout: &AppLayout) {
    if let Some(area) = layout.body.left {
        SidebarList.render(f, area, state);
    }
    DetailPanel.render(f, layout.body.main, state);

    // F1 help panel (right side of body only — header and footer stay untouched)
    if let Some(help_area) = layout.body.right {
        let kind    = state.active_form().map(|f| f.kind);
        let foc_key = state.active_form()
            .and_then(|f| f.focused_node())
            .map(|n| n.key());
        let sections = help_sidebar::build_help(state.screen, kind, foc_key, state.lang);
        help_sidebar::render_help_sidebar(f, help_area, &sections, state.lang);
    }
}
