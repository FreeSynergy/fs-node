// Dashboard screen — project sidebar + services table.
//
// ┌──────────────────────────────────────────────────────────────────┐
// │  FSN · myproject @ example.com                          [DE]    │
// ├────────────────────┬─────────────────────────────────────────────┤
// │ Projekte           │  Services                                   │
// │ ▶ myproject        │  ┌───────────────────────────────────────┐  │
// │   testprojekt      │  │  Name      Typ    Domain    Status    │  │
// │                    │  │▶ kanidm    iam    auth.ex   ● Aktiv   │  │
// │  + Neues Projekt   │  │  forgejo   git    git.ex    ○ Stopp   │  │
// │                    │  └───────────────────────────────────────┘  │
// ├────────────────────┴─────────────────────────────────────────────┤
// │  ↑↓=Projekt  n=Neu  e=Bearbeiten  x=Löschen  Tab=Services  q=Quit │
// └──────────────────────────────────────────────────────────────────┘

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::app::{AppState, DashFocus};
use crate::ui::widgets;

pub fn render(f: &mut Frame, state: &AppState) {
    let area = f.area();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(f, state, outer[0]);
    render_body(f, state, outer[1]);
    render_hint(f, state, outer[2]);
}

// ── Header ────────────────────────────────────────────────────────────────────

fn render_header(f: &mut Frame, state: &AppState, area: Rect) {
    // Show active project's name + domain if available
    let (name, domain) = state.projects.get(state.selected_project)
        .map(|p| (p.name(), p.domain()))
        .unwrap_or(("FreeSynergy.Node", ""));

    let title = if domain.is_empty() {
        Line::from(vec![
            Span::styled(" FSN ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("· ", Style::default().fg(Color::DarkGray)),
            Span::styled(name.to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" FSN ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("· ", Style::default().fg(Color::DarkGray)),
            Span::styled(name.to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(" @ ", Style::default().fg(Color::DarkGray)),
            Span::styled(domain.to_string(), Style::default().fg(Color::DarkGray)),
        ])
    };

    let header = Paragraph::new(title)
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)))
        .alignment(Alignment::Left);
    f.render_widget(header, area);

    let lang_area = Rect { x: area.right().saturating_sub(6), y: area.y + 1, width: 4, height: 1 };
    f.render_widget(Paragraph::new(Line::from(widgets::lang_button(state))), lang_area);
}

// ── Body ──────────────────────────────────────────────────────────────────────

fn render_body(f: &mut Frame, state: &AppState, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(22),  // sidebar
            Constraint::Min(1),      // main panel
        ])
        .split(area);

    render_sidebar(f, state, cols[0]);
    render_services(f, state, cols[1]);
}

// ── Sidebar ───────────────────────────────────────────────────────────────────

fn render_sidebar(f: &mut Frame, state: &AppState, area: Rect) {
    let sidebar_focused = state.dash_focus == DashFocus::Sidebar;

    let border_style = if sidebar_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(border_style)
        .title(Span::styled(
            format!(" {} ", state.t("sidebar.projects")),
            Style::default().fg(if sidebar_focused { Color::Cyan } else { Color::DarkGray }),
        ));
    f.render_widget(block, area);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let mut lines: Vec<Line> = Vec::new();

    if state.projects.is_empty() {
        lines.push(Line::from(Span::styled(
            state.t("dash.no_projects"),
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, proj) in state.projects.iter().enumerate() {
            let selected = i == state.selected_project;
            let (prefix, style) = if selected && sidebar_focused {
                ("▶ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            } else if selected {
                ("▶ ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            } else {
                ("  ", Style::default().fg(Color::White))
            };

            // Truncate to fit sidebar width
            let max_w = inner.width.saturating_sub(2) as usize;
            let proj_name = proj.name();
            let display = if proj_name.len() > max_w {
                format!("{}{}…", prefix, &proj_name[..max_w.saturating_sub(1)])
            } else {
                format!("{}{}", prefix, proj_name)
            };

            lines.push(Line::from(Span::styled(display, style)));
        }
    }

    // Spacer + "New Project" button at bottom
    let btn_y = inner.y + lines.len() as u16 + 1;
    if btn_y < inner.bottom() {
        let para = Paragraph::new(lines);
        f.render_widget(para, inner);

        let btn_area = Rect { x: inner.x, y: btn_y, width: inner.width, height: 1 };
        let btn_style = if sidebar_focused {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(state.t("dash.new_project"), btn_style))),
            btn_area,
        );
    } else {
        f.render_widget(Paragraph::new(lines), inner);
    }
}

// ── Services table ────────────────────────────────────────────────────────────

fn render_services(f: &mut Frame, state: &AppState, area: Rect) {
    let services_focused = state.dash_focus == DashFocus::Services;

    let block = Block::default()
        .borders(Borders::NONE)
        .title(Span::styled(
            format!(" {} ", state.t("dash.services")),
            Style::default()
                .fg(if services_focused { Color::Cyan } else { Color::White })
                .add_modifier(Modifier::BOLD),
        ));

    if state.services.is_empty() {
        let msg = Paragraph::new(Line::from(Span::styled(
            "(keine Services)",
            Style::default().fg(Color::DarkGray),
        )))
        .block(block);
        f.render_widget(msg, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from(state.t("dash.col.name"))  .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED)),
        Cell::from(state.t("dash.col.type"))  .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED)),
        Cell::from(state.t("dash.col.domain")).style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED)),
        Cell::from(state.t("dash.col.status")).style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED)),
    ])
    .height(1);

    let rows: Vec<Row> = state.services.iter().enumerate().map(|(i, svc)| {
        let selected = i == state.selected && services_focused;
        let name_style = if selected {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        Row::new(vec![
            Cell::from(if selected { format!("▶ {}", svc.name) } else { format!("  {}", svc.name) })
                .style(name_style),
            Cell::from(svc.service_type.as_str()).style(Style::default().fg(Color::DarkGray)),
            Cell::from(svc.domain.as_str()).style(Style::default().fg(Color::Blue)),
            Cell::from(Line::from(widgets::status_span(svc.status, state))),
        ])
        .height(1)
    }).collect();

    let table = Table::new(rows, [
        Constraint::Length(20),
        Constraint::Length(10),
        Constraint::Min(25),
        Constraint::Length(14),
    ])
    .header(header)
    .block(block)
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    let mut table_state = TableState::default().with_selected(
        if services_focused { Some(state.selected) } else { None }
    );
    f.render_stateful_widget(table, area, &mut table_state);
}

// ── Hint bar ──────────────────────────────────────────────────────────────────

fn render_hint(f: &mut Frame, state: &AppState, area: Rect) {
    let key = if state.dash_confirm {
        "dash.hint.confirm"
    } else if state.dash_focus == DashFocus::Services {
        "dash.hint.services"
    } else {
        "dash.hint"
    };

    let style = if state.dash_confirm {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(state.t(key), style)))
            .alignment(Alignment::Center),
        area,
    );
}
