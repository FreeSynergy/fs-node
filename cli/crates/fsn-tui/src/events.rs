// Keyboard and mouse event handling.
//
// The form event handler no longer checks field types directly.
// Instead it calls `form.handle_key(key)` which dispatches to the focused
// FormNode. Each node type handles its own input and returns a FormAction.
// This makes adding new field types zero-boilerplate in events.rs.

use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::app::{
    AppState, ConfirmAction, DashFocus, LogsState, OverlayLayer, ResourceKind, RunState, Screen,
};
use crate::ui::form_node::FormAction;

pub fn handle(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    state.ctrl_hint = key.modifiers.contains(KeyModifiers::CONTROL);

    // Ctrl-C always quits
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        state.should_quit = true;
        return Ok(());
    }

    // Topmost overlay layer captures all input (Ebene system)
    if state.has_overlay() {
        return handle_overlay(key, state, root);
    }

    match state.screen {
        Screen::Welcome    => handle_welcome(key, state),
        Screen::Dashboard  => handle_dashboard(key, state, root),
        Screen::NewProject => handle_resource_form(key, state, root),
    }
}

// ── Overlay layer handler ─────────────────────────────────────────────────────

fn handle_overlay(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    // Peek at the topmost overlay type before potentially popping it
    let overlay_kind = state.top_overlay().map(|o| match o {
        OverlayLayer::Logs(_)    => "logs",
        OverlayLayer::Confirm{..} => "confirm",
    });

    match overlay_kind {
        Some("logs") => {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => { state.pop_overlay(); }
                KeyCode::Up => {
                    if let Some(logs) = state.logs_overlay_mut() {
                        if logs.scroll > 0 { logs.scroll -= 1; }
                    }
                }
                KeyCode::Down => {
                    if let Some(logs) = state.logs_overlay_mut() {
                        let max = logs.lines.len().saturating_sub(1);
                        if logs.scroll < max { logs.scroll += 1; }
                    }
                }
                _ => {}
            }
        }
        Some("confirm") => {
            let (_, yes_action) = state.confirm_overlay().unwrap();
            match key.code {
                KeyCode::Char('j') | KeyCode::Char('J')
                | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    state.pop_overlay();
                    match yes_action {
                        ConfirmAction::DeleteProject => delete_selected_project(state, root)?,
                    }
                }
                _ => { state.pop_overlay(); } // any other key = cancel
            }
        }
        _ => { state.pop_overlay(); }
    }
    Ok(())
}

// ── Welcome screen ────────────────────────────────────────────────────────────

fn handle_welcome(key: KeyEvent, state: &mut AppState) -> Result<()> {
    match key.code {
        KeyCode::Char('q') => state.should_quit = true,
        KeyCode::Char('l') | KeyCode::Char('L') => state.lang = state.lang.toggle(),
        KeyCode::Left | KeyCode::Right => state.welcome_focus = 1 - state.welcome_focus,
        KeyCode::Enter => {
            if state.welcome_focus == 0 {
                state.current_form = Some(crate::project_form::new_project_form());
                state.screen = Screen::NewProject;
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Generic resource form handler ─────────────────────────────────────────────

fn handle_resource_form(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    // Dispatch to the focused FormNode — it handles its own input and navigation
    let action = if let Some(ref mut form) = state.current_form {
        form.handle_key(key)
    } else {
        FormAction::Unhandled
    };

    match action {
        FormAction::Cancel => {
            state.current_form = None;
            state.screen = if state.projects.is_empty() {
                Screen::Welcome
            } else {
                Screen::Dashboard
            };
        }

        FormAction::LangToggle => state.lang = state.lang.toggle(),

        FormAction::Submit => handle_form_submit(state, root)?,

        FormAction::Consumed => {} // node handled it, nothing to do

        FormAction::Unhandled => {
            // Keys not handled by the focused node: lang toggle, quit
            match key.code {
                KeyCode::Char('l') | KeyCode::Char('L') => state.lang = state.lang.toggle(),
                _ => {}
            }
        }

        // These are resolved inside ResourceForm::handle_key before returning
        FormAction::FocusNext | FormAction::FocusPrev
        | FormAction::TabNext  | FormAction::TabPrev
        | FormAction::ValueChanged => {}

        FormAction::Quit => state.should_quit = true,
    }
    Ok(())
}

fn handle_form_submit(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(ref form) = state.current_form else { return Ok(()); };
    let missing_t = form.tab_missing_count(form.active_tab);

    if missing_t > 0 {
        let msg = format!(
            "{} {}",
            missing_t,
            if missing_t == 1 { "Pflichtfeld fehlt" } else { "Pflichtfelder fehlen" }
        );
        if let Some(ref mut f) = state.current_form { f.error = Some(msg); }
        return Ok(());
    }

    if !form.is_last_tab() {
        if let Some(ref mut f) = state.current_form { f.error = None; f.next_tab(); }
        return Ok(());
    }

    let missing = form.missing_required();
    if !missing.is_empty() {
        let msg = format!("{} Pflichtfeld(er) auf anderen Tabs fehlen", missing.len());
        if let Some(ref mut f) = state.current_form { f.error = Some(msg); }
        return Ok(());
    }

    // All good — dispatch to resource-specific submit
    let kind = state.current_form.as_ref().map(|f| f.kind);
    match kind {
        Some(ResourceKind::Project) => submit_project(state, root)?,
        Some(ResourceKind::Service) => submit_service(state, root)?,
        Some(ResourceKind::Host)    => submit_host(state, root)?,
        None => {}
    }
    Ok(())
}

// ── Dashboard ─────────────────────────────────────────────────────────────────

fn handle_dashboard(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    match state.dash_focus {
        // ── Sidebar ────────────────────────────────────────────────────────
        DashFocus::Sidebar => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => state.should_quit = true,
            KeyCode::Char('L') => state.lang = state.lang.toggle(),
            KeyCode::Tab => state.dash_focus = DashFocus::Services,

            KeyCode::Up => {
                if state.selected_project > 0 {
                    state.selected_project -= 1;
                    state.rebuild_services();
                    reload_hosts(state, root);
                }
            }
            KeyCode::Down => {
                if state.selected_project + 1 < state.projects.len() {
                    state.selected_project += 1;
                    state.rebuild_services();
                    reload_hosts(state, root);
                }
            }

            KeyCode::Char('n') => {
                state.current_form = Some(crate::project_form::new_project_form());
                state.screen = Screen::NewProject;
            }
            KeyCode::Char('e') => {
                if let Some(proj) = state.projects.get(state.selected_project) {
                    state.current_form = Some(crate::project_form::edit_project_form(proj));
                    state.screen = Screen::NewProject;
                }
            }
            KeyCode::Char('h') => {
                if let Some(proj) = state.projects.get(state.selected_project) {
                    let current_slug = proj.slug.clone();
                    let project_slugs: Vec<String> = state.projects.iter().map(|p| p.slug.clone()).collect();
                    state.current_form = Some(crate::host_form::new_host_form(project_slugs, &current_slug));
                    state.screen = Screen::NewProject;
                }
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                if !state.projects.is_empty() {
                    state.push_overlay(OverlayLayer::Confirm {
                        message:    "dash.hint.confirm".into(),
                        yes_action: ConfirmAction::DeleteProject,
                    });
                }
            }

            _ => {}
        },

        // ── Services ───────────────────────────────────────────────────────
        DashFocus::Services => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => state.should_quit = true,
            KeyCode::Char('L') => state.lang = state.lang.toggle(),
            KeyCode::Tab => state.dash_focus = DashFocus::Sidebar,

            KeyCode::Up   => { if state.selected > 0 { state.selected -= 1; } }
            KeyCode::Down => {
                if state.selected + 1 < state.services.len() { state.selected += 1; }
            }

            KeyCode::Char('n') => {
                state.current_form = Some(crate::service_form::new_service_form());
                state.screen = Screen::NewProject;
            }

            KeyCode::Char('l') => {
                if let Some(svc) = state.services.get(state.selected) {
                    let lines = fetch_logs(&svc.name);
                    state.push_overlay(OverlayLayer::Logs(LogsState {
                        service_name: svc.name.clone(), lines, scroll: 0,
                    }));
                }
            }

            KeyCode::Char('d') => {
                if let Some(svc) = state.services.get_mut(state.selected) {
                    svc.status = RunState::Missing;
                }
            }

            KeyCode::Char('r') => {
                if let Some(svc) = state.services.get(state.selected) {
                    let _ = std::process::Command::new("podman")
                        .args(["restart", &svc.name]).output();
                    if let Some(row) = state.services.get_mut(state.selected) {
                        row.status = podman_status(&row.name);
                    }
                }
            }

            KeyCode::Char('x') => {
                if let Some(svc) = state.services.get(state.selected) {
                    let _ = std::process::Command::new("podman")
                        .args(["stop", &svc.name]).output();
                    let _ = std::process::Command::new("podman")
                        .args(["rm",   &svc.name]).output();
                    state.services.remove(state.selected);
                    if state.selected > 0 && state.selected >= state.services.len() {
                        state.selected -= 1;
                    }
                }
            }

            _ => {}
        },
    }
    Ok(())
}

fn delete_selected_project(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project) else { return Ok(()); };
    let project_dir = root.join("projects").join(&proj.slug);
    let _ = std::fs::remove_dir_all(&project_dir);
    state.projects.remove(state.selected_project);
    if state.selected_project > 0 && state.selected_project >= state.projects.len() {
        state.selected_project -= 1;
    }
    if state.projects.is_empty() { state.screen = Screen::Welcome; }
    Ok(())
}

fn reload_hosts(state: &mut AppState, root: &Path) {
    if let Some(proj) = state.projects.get(state.selected_project) {
        state.hosts = crate::load_hosts(&root.join("projects").join(&proj.slug));
    }
}

// ── Form submit dispatch ──────────────────────────────────────────────────────

fn submit_project(state: &mut AppState, root: &Path) -> Result<()> {
    let result = state.current_form.as_ref()
        .map(|form| crate::project_form::submit_project_form(form, root));

    match result {
        Some(Ok(())) => {
            state.projects = crate::load_projects(root);
            if let Some(ref form) = state.current_form {
                let slug = form.edit_id.clone()
                    .unwrap_or_else(|| crate::app::slugify(&form.field_value("name")));
                state.selected_project = state.projects.iter()
                    .position(|p| p.slug == slug).unwrap_or(0);
            }
            state.rebuild_services();
            state.screen     = Screen::Dashboard;
            state.dash_focus = DashFocus::Sidebar;
            state.current_form = None;
        }
        Some(Err(e)) => {
            if let Some(ref mut form) = state.current_form {
                form.error = Some(format!("{}", e));
            }
        }
        None => {}
    }
    Ok(())
}

fn submit_service(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(ref form) = state.current_form else { return Ok(()); };
    let Some(proj) = state.projects.get(state.selected_project) else {
        if let Some(ref mut f) = state.current_form {
            f.error = Some("Kein Projekt ausgewählt".into());
        }
        return Ok(());
    };

    let svc_name      = form.field_value("name");
    let svc_class     = form.field_value("class");
    let svc_subdomain = form.field_value("subdomain");
    let svc_alias     = form.field_value("alias");
    let svc_version   = form.field_value("version");
    let svc_port      = form.field_value("port");

    if svc_name.is_empty() {
        if let Some(ref mut f) = state.current_form {
            f.error = Some("Service-Name ist erforderlich".into());
        }
        return Ok(());
    }

    let project_dir  = root.join("projects").join(&proj.slug);
    let services_dir = project_dir.join("services");
    std::fs::create_dir_all(&services_dir)?;

    let slug = crate::app::slugify(&svc_name);
    let path = services_dir.join(format!("{}.service.toml", slug));

    let mut content = format!(
        "[service]\nname          = \"{svc_name}\"\nservice_class = \"{svc_class}\"\nproject       = \"{}\"\n",
        proj.slug
    );
    if !svc_version.is_empty()  { content.push_str(&format!("version       = \"{svc_version}\"\n")); }
    if !svc_subdomain.is_empty(){ content.push_str(&format!("subdomain     = \"{svc_subdomain}\"\n")); }
    if !svc_alias.is_empty()    { content.push_str(&format!("alias         = \"{svc_alias}\"\n")); }
    if !svc_port.is_empty() {
        if let Ok(p) = svc_port.parse::<u16>() {
            content.push_str(&format!("port          = {p}\n"));
        }
    }
    std::fs::write(&path, content)?;

    // Backward-compat reference in project.toml
    let mut proj_content = std::fs::read_to_string(&proj.toml_path)?;
    if !proj_content.contains(&format!("[load.services.{}]", slug)) {
        proj_content.push_str(&format!(
            "\n[load.services.{}]\nservice_class = \"{svc_class}\"\n", slug
        ));
        std::fs::write(&proj.toml_path, proj_content)?;
    }

    state.projects = crate::load_projects(root);
    state.rebuild_services();
    state.screen     = Screen::Dashboard;
    state.dash_focus = DashFocus::Services;
    state.current_form = None;
    Ok(())
}

fn submit_host(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project) else {
        if let Some(ref mut f) = state.current_form {
            f.error = Some("Kein Projekt ausgewählt".into());
        }
        return Ok(());
    };
    let project_dir = root.join("projects").join(&proj.slug);

    let result = state.current_form.as_ref()
        .map(|form| crate::host_form::submit_host_form(form, &project_dir));

    match result {
        Some(Ok(())) => {
            state.hosts = crate::load_hosts(&project_dir);
            state.screen     = Screen::Dashboard;
            state.dash_focus = DashFocus::Sidebar;
            state.current_form = None;
        }
        Some(Err(e)) => {
            if let Some(ref mut form) = state.current_form {
                form.error = Some(format!("{}", e));
            }
        }
        None => {}
    }
    Ok(())
}

// ── Mouse events ──────────────────────────────────────────────────────────────

pub fn handle_mouse(event: MouseEvent, state: &mut AppState) -> Result<()> {
    let (tw, _) = crossterm::terminal::size().unwrap_or((80, 24));

    // Overlay scroll support
    match event.kind {
        MouseEventKind::ScrollDown => {
            if let Some(logs) = state.logs_overlay_mut() {
                let max = logs.lines.len().saturating_sub(1);
                if logs.scroll < max { logs.scroll += 1; }
                return Ok(());
            }
        }
        MouseEventKind::ScrollUp => {
            if let Some(logs) = state.logs_overlay_mut() {
                if logs.scroll > 0 { logs.scroll -= 1; }
                return Ok(());
            }
        }
        _ => {}
    }

    match event.kind {
        MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
            if state.screen == Screen::NewProject {
                if let Some(ref mut form) = state.current_form {
                    // Find focused SelectInputNode and cycle its options
                    if let Some(idx) = form.focused_node_global_idx() {
                        use crossterm::event::KeyCode;
                        let fake_key = crossterm::event::KeyEvent::new(
                            if matches!(event.kind, MouseEventKind::ScrollDown) {
                                KeyCode::Down
                            } else {
                                KeyCode::Up
                            },
                            KeyModifiers::empty(),
                        );
                        form.nodes[idx].handle_key(fake_key);
                    }
                }
            }
        }

        MouseEventKind::Down(_) => {
            // Language button — top-right corner
            if event.column >= tw.saturating_sub(6) && event.row <= 2 {
                state.lang = state.lang.toggle();
                return Ok(());
            }

            if state.screen == Screen::NewProject {
                handle_form_click(event.column, event.row, state);
            } else if state.screen == Screen::Dashboard && !state.has_overlay() {
                handle_dashboard_click(event.column, event.row, state);
            }
        }

        _ => {}
    }
    Ok(())
}

fn handle_form_click(col: u16, row: u16, state: &mut AppState) {
    let Some(ref mut form) = state.current_form else { return };

    // First check if a dropdown option was clicked
    if let Some(idx) = form.focused_node_global_idx() {
        // Dropdown click: synthesize Up/Down keys based on cursor position
        // (full downcast-based approach is a future improvement)
        let _ = idx;  // will be used when downcast is implemented
    }

    // Focus the field that was clicked
    form.click_focus(col, row);
}

// ── Dashboard click handler ───────────────────────────────────────────────────

fn handle_dashboard_click(col: u16, row: u16, state: &mut AppState) {
    const SIDEBAR_W: u16 = 22;
    const HEADER_H:  u16 = 3;

    if row < HEADER_H { return; }
    let body_row = row - HEADER_H;

    if col < SIDEBAR_W {
        state.dash_focus = DashFocus::Sidebar;
        if state.projects.is_empty() { return; }

        let mut cur_row: u16 = 0;
        for (i, _) in state.projects.iter().enumerate() {
            if body_row == cur_row {
                state.selected_project = i;
                return;
            }
            cur_row += 1;
            if i == state.selected_project {
                let extra = state.hosts.len() as u16 + 1;
                if body_row > cur_row && body_row < cur_row + extra { return; }
                cur_row += extra;
            }
        }
    } else {
        state.dash_focus = DashFocus::Services;
        const TABLE_HEADER: u16 = 1;
        if body_row <= TABLE_HEADER { return; }
        let svc_row = (body_row - TABLE_HEADER - 1) as usize;
        if svc_row < state.services.len() {
            state.selected = svc_row;
        }
    }
}

// ── Podman helpers ────────────────────────────────────────────────────────────

pub fn podman_status(name: &str) -> RunState {
    let out = std::process::Command::new("podman")
        .args(["inspect", "--format", "{{.State.Status}}", name])
        .output();
    match out {
        Ok(o) => match String::from_utf8_lossy(&o.stdout).trim() {
            "running"            => RunState::Running,
            "exited" | "stopped" => RunState::Stopped,
            "error"              => RunState::Failed,
            _                    => RunState::Missing,
        },
        Err(_) => RunState::Missing,
    }
}

fn fetch_logs(name: &str) -> Vec<String> {
    let out = std::process::Command::new("podman")
        .args(["logs", "--tail", "100", name])
        .output();
    match out {
        Ok(o) => {
            let text = if o.stdout.is_empty() { o.stderr } else { o.stdout };
            String::from_utf8_lossy(&text).lines().map(|l| l.to_string()).collect()
        }
        Err(_) => vec!["[Logs nicht verfügbar]".into()],
    }
}
