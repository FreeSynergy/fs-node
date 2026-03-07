// Reusable widget helpers.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{run_state_i18n, AppState, RunState};

/// Language toggle button: "[DE]" or "[EN]" in the top-right corner.
pub fn lang_button<'a>(state: &AppState) -> Span<'a> {
    Span::styled(
        format!("[{}]", state.lang.label()),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
}

/// Status badge with color, backed by fsn-core's `RunState`.
pub fn status_span(status: RunState, state: &AppState) -> Span<'static> {
    let (label, color) = match status {
        RunState::Running => (state.t(run_state_i18n(RunState::Running)), Color::Green),
        RunState::Stopped => (state.t(run_state_i18n(RunState::Stopped)), Color::Yellow),
        RunState::Failed  => (state.t(run_state_i18n(RunState::Failed)),  Color::Red),
        RunState::Missing => (state.t(run_state_i18n(RunState::Missing)), Color::Gray),
    };
    Span::styled(label, Style::default().fg(color))
}

/// Centered popup box — clears background and draws a bordered block.
pub fn popup_area(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    use ratatui::layout::{Constraint, Direction, Layout};
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1]);
    horiz[1]
}

/// Draw a clear + bordered block at `area` (used for overlays).
pub fn clear_block(f: &mut Frame, area: Rect, title: &str) {
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("─ {} ", title))
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(block, area);
}

/// Button widget — filled if focused, bordered if not.
pub fn button_line(label: &str, focused: bool, disabled: bool) -> Line<'static> {
    let (fg, bg, modifier) = if disabled {
        (Color::DarkGray, Color::Reset, Modifier::empty())
    } else if focused {
        (Color::Black, Color::Cyan, Modifier::BOLD)
    } else {
        (Color::Cyan, Color::Reset, Modifier::empty())
    };
    Line::from(Span::styled(
        format!("  {}  ", label),
        Style::default().fg(fg).bg(bg).add_modifier(modifier),
    ))
}
