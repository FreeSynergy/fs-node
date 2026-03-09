// Settings screen — manage module stores and application preferences.
//
// Layout:
//   ┌─────────────────────────────────────────────┐
//   │  Settings – Module Stores                   │
//   ├─────────────────────────────────────────────┤
//   │  ▶ FSN Official  (enabled)                  │
//   │    https://github.com/Lord-KalEl/…          │
//   │                                             │
//   │    Mein Store   (disabled)                  │
//   │    https://git.example.com/modules          │
//   ├─────────────────────────────────────────────┤
//   │  A=Add  D=Delete  Space=Enable  Esc=Back    │
//   └─────────────────────────────────────────────┘

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::AppState;

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    // Outer block
    let block = Block::default()
        .title(Span::styled(
            format!(" {} ", state.t("settings.title")),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split: store list | hint bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(inner);

    render_store_list(f, state, chunks[0]);
    render_hint(f, state, chunks[1]);
}

fn render_store_list(f: &mut Frame, state: &AppState, area: Rect) {
    let stores = &state.settings.stores;

    if stores.is_empty() {
        let p = Paragraph::new(Line::from(Span::styled(
            state.t("settings.empty"),
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(p, area);
        return;
    }

    // Two lines per store: name+status, url
    let mut lines: Vec<Line> = Vec::new();
    for (i, store) in stores.iter().enumerate() {
        let is_sel = i == state.settings_cursor;

        let status_key = if store.enabled {
            "settings.store.enabled"
        } else {
            "settings.store.disabled"
        };
        let status    = state.t(status_key);
        let status_col = if store.enabled { Color::Green } else { Color::DarkGray };
        let marker    = if is_sel { "▶ " } else { "  " };

        // Row 1: marker + name + status
        let name_style = if is_sel {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(vec![
            Span::raw(marker),
            Span::styled(store.name.as_str(), name_style),
            Span::raw("  "),
            Span::styled(status, Style::default().fg(status_col)),
        ]));

        // Row 2: url (indented, gray)
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(store.url.as_str(), Style::default().fg(Color::DarkGray)),
        ]));

        // Spacing between entries
        lines.push(Line::from(""));
    }

    f.render_widget(Paragraph::new(lines), area);
}

fn render_hint(f: &mut Frame, state: &AppState, area: Rect) {
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            state.t("settings.hint"),
            Style::default().fg(Color::DarkGray),
        ))),
        area,
    );
}
