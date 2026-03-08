// UI rendering — dispatches to screen-specific renderers.
//
// Render takes `&mut AppState` because FormNode::render(&mut self, ...) needs
// to store the last rendered Rect for mouse hit-testing (layout cache).

pub mod dashboard;
pub mod form_node;
pub mod logs;
pub mod new_project;
pub mod nodes;
pub mod welcome;
pub mod widgets;

use ratatui::Frame;
use crate::app::{AppState, OverlayLayer, Screen};

pub fn render(f: &mut Frame, state: &mut AppState) {
    match state.screen {
        Screen::Welcome    => welcome::render(f, state),
        Screen::Dashboard  => dashboard::render(f, state),
        Screen::NewProject => new_project::render(f, state),
    }

    // Overlay layers drawn on top (Ebene system)
    // Each variant is rendered by its own function — OOP: variant carries own rendering.
    for layer in &state.overlay_stack {
        match layer {
            OverlayLayer::Logs(_)        => logs::render(f, state),
            OverlayLayer::Confirm { .. } => render_confirm(f, state),
            OverlayLayer::Deploy(_)      => render_deploy(f, state),
        }
    }
}

fn render_confirm(f: &mut Frame, state: &AppState) {
    use ratatui::{
        layout::{Alignment, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Clear, Paragraph},
    };

    let Some((msg_key, _)) = state.confirm_overlay() else { return };
    let area = f.area();
    let popup = Rect {
        x: area.width / 4,
        y: area.height / 2 - 2,
        width: area.width / 2,
        height: 3,
    };

    f.render_widget(Clear, popup);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            state.t(msg_key),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )))
        .block(Block::default().borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)))
        .alignment(Alignment::Center),
        popup,
    );
}

fn render_deploy(f: &mut Frame, state: &AppState) {
    use ratatui::{
        layout::{Alignment, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Clear, Paragraph},
    };

    let ds = state.overlay_stack.iter().rev().find_map(|o| {
        if let OverlayLayer::Deploy(ref d) = o { Some(d) } else { None }
    });
    let Some(ds) = ds else { return };

    let area  = f.area();
    let width = (area.width * 2 / 3).max(50).min(area.width.saturating_sub(4));
    let log_lines = ds.log.len() as u16;
    let height = (log_lines + 4).max(6).min(area.height.saturating_sub(4));
    let popup = Rect {
        x:      area.width.saturating_sub(width) / 2,
        y:      area.height.saturating_sub(height) / 2,
        width,
        height,
    };

    let border_color = if ds.done {
        if ds.success { Color::Green } else { Color::Red }
    } else {
        Color::Cyan
    };

    let title = format!(" {} — {} ", state.t("deploy.title"), ds.target);

    f.render_widget(Clear, popup);

    let inner = Block::default()
        .title(Span::styled(&title, Style::default().fg(border_color).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner_area = inner.inner(popup);
    f.render_widget(inner, popup);

    // Log lines
    let log_area = Rect { x: inner_area.x, y: inner_area.y, width: inner_area.width, height: inner_area.height.saturating_sub(1) };
    let lines: Vec<Line> = ds.log.iter().map(|l| {
        let color = if l.starts_with('✓') { Color::Green }
                    else if l.starts_with('✗') { Color::Red }
                    else { Color::White };
        Line::from(Span::styled(l.as_str(), Style::default().fg(color)))
    }).collect();
    f.render_widget(Paragraph::new(lines), log_area);

    // Hint bar at bottom
    let hint_text = if ds.done { state.t("deploy.hint") } else { state.t("deploy.running") };
    let hint_area = Rect { x: inner_area.x, y: inner_area.bottom().saturating_sub(1), width: inner_area.width, height: 1 };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(hint_text, Style::default().fg(Color::DarkGray))))
            .alignment(Alignment::Center),
        hint_area,
    );
}
