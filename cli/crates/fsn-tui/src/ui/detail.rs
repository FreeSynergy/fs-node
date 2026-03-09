// Detail panel renderers for Dashboard center area.
//
// Design Pattern: Composite — each resource type (Project, Host, Service)
// renders its own detail view. dashboard.rs calls these via SidebarItem::render_center()
// without knowing which variant is selected.
//
// Shared helpers (run_state_color, run_state_char) live in widgets.rs — no
// duplicated match blocks here.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{AppState, RunState};
use crate::ui::widgets;

// ── Project detail panel ──────────────────────────────────────────────────────

pub fn render_project_detail(f: &mut Frame, state: &AppState, area: Rect, slug: &str) {
    let Some(proj) = state.projects.iter().find(|p| p.slug == slug) else {
        f.render_widget(Paragraph::new("—"), area);
        return;
    };

    let name       = proj.config.project.name.as_str();
    let domain     = proj.config.project.domain.as_str();
    let email      = proj.email();
    let install    = proj.install_dir();
    let svc_count  = proj.config.load.services.len();
    let host_count = state.hosts.len();
    let langs      = proj.config.project.languages.join(", ");

    let svc_ok  = state.services.iter().filter(|s| s.status == RunState::Running).count();
    let svc_err = state.services.iter().filter(|s| s.status == RunState::Failed).count();

    let block = Block::default()
        .borders(Borders::NONE)
        .title(Span::styled(
            format!(" {} ", name),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Domain:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(domain.to_string(), Style::default().fg(Color::Blue)),
        ]),
    ];
    if !email.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("E-Mail:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(email.to_string(), Style::default().fg(Color::White)),
        ]));
    }
    if !langs.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Sprachen:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(langs, Style::default().fg(Color::White)),
        ]));
    }
    if !install.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Install:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(install.to_string(), Style::default().fg(Color::White)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Services:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(svc_count.to_string(), Style::default().fg(Color::White)),
        Span::styled("  (", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("● {}", svc_ok), Style::default().fg(Color::Green)),
        if svc_err > 0 {
            Span::styled(format!("  ✕ {}", svc_err), Style::default().fg(Color::Red))
        } else {
            Span::styled("", Style::default())
        },
        Span::styled(")", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Hosts:     ", Style::default().fg(Color::DarkGray)),
        Span::styled(host_count.to_string(), Style::default().fg(Color::White)),
    ]));

    if let Some(desc) = proj.config.project.description.as_deref() {
        if !desc.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(desc.to_string(), Style::default().fg(Color::DarkGray))));
        }
    }

    f.render_widget(Paragraph::new(lines), inner);
}

// ── Host detail panel ─────────────────────────────────────────────────────────

pub fn render_host_detail(f: &mut Frame, state: &AppState, area: Rect, slug: &str) {
    let Some(host) = state.hosts.iter().find(|h| h.slug == slug) else {
        f.render_widget(Paragraph::new("—"), area);
        return;
    };

    let display = host.config.host.alias.as_deref().unwrap_or(&host.config.host.name);
    let block = Block::default()
        .borders(Borders::NONE)
        .title(Span::styled(
            format!(" {} ", display),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let addr     = host.config.host.addr();
    let ssh_user = &host.config.host.ssh_user;
    let ssh_port = host.config.host.ssh_port;
    let external = if host.config.host.external { "extern" } else { "lokal" };
    let alias    = host.config.host.alias.as_deref().unwrap_or("—");

    let lines = vec![
        Line::from(vec![
            Span::styled("Adresse:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(addr.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("SSH:       ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}@{}:{}", ssh_user, addr, ssh_port), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Alias:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(alias.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Typ:       ", Style::default().fg(Color::DarkGray)),
            Span::styled(external.to_string(), Style::default().fg(Color::White)),
        ]),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

// ── Service detail panel ──────────────────────────────────────────────────────

pub fn render_service_detail(f: &mut Frame, state: &AppState, area: Rect, svc_name: &str) {
    let Some(proj) = state.projects.get(state.selected_project) else {
        f.render_widget(Paragraph::new("—"), area);
        return;
    };
    let Some(entry) = proj.config.load.services.get(svc_name) else {
        f.render_widget(Paragraph::new("—"), area);
        return;
    };

    let status       = state.last_podman_statuses.get(svc_name).copied().unwrap_or(RunState::Missing);
    // widgets::run_state_color / run_state_char — single source of truth, no local match needed.
    let status_color = widgets::run_state_color(status);
    let status_label = format!("{} {}", widgets::run_state_char(status), match status {
        RunState::Running => "Running",
        RunState::Stopped => "Stopped",
        RunState::Failed  => "Failed",
        RunState::Missing => "Missing",
    });

    let block = Block::default()
        .borders(Borders::NONE)
        .title(Span::styled(
            format!(" {} ", svc_name),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let subdomain = entry.subdomain.as_deref().unwrap_or("—");
    let port      = entry.port.map(|p| p.to_string()).unwrap_or_else(|| "—".to_string());
    let env_count = entry.env.len();
    let domain    = format!("{}.{}", svc_name, proj.domain());

    let lines = vec![
        Line::from(vec![
            Span::styled("Klasse:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(entry.service_class.clone(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Projekt:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(proj.slug.clone(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Domain:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(domain, Style::default().fg(Color::Blue)),
        ]),
        Line::from(vec![
            Span::styled("Subdomain: ", Style::default().fg(Color::DarkGray)),
            Span::styled(subdomain.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Port:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(port, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Status:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(status_label, Style::default().fg(status_color)),
        ]),
        Line::from(vec![
            Span::styled("Env-Vars:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(env_count.to_string(), Style::default().fg(Color::White)),
        ]),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}
