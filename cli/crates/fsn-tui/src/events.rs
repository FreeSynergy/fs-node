// Keyboard event handling.

use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::app::{AppState, DashFocus, FormFieldType, LogsState, ResourceForm, ResourceKind, RunState, Screen};

pub fn handle(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    state.ctrl_hint = key.modifiers.contains(KeyModifiers::CONTROL);

    // Ctrl-C always quits
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        state.should_quit = true;
        return Ok(());
    }

    // Logs overlay is modal — handle first
    if state.logs_overlay.is_some() {
        return handle_logs(key, state);
    }

    match state.screen {
        Screen::Welcome    => handle_welcome(key, state),
        Screen::Dashboard  => handle_dashboard(key, state, root),
        Screen::NewProject => handle_resource_form(key, state, root),
    }
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
    // Language toggle only when not typing
    if matches!(key.code, KeyCode::Char('l') | KeyCode::Char('L')) && !is_typing(state) {
        state.lang = state.lang.toggle();
        return Ok(());
    }

    match key.code {
        KeyCode::Esc => {
            state.screen = if state.projects.is_empty() { Screen::Welcome } else { Screen::Dashboard };
        }

        KeyCode::Tab => {
            if let Some(ref mut form) = state.current_form { form.focus_next(); }
        }
        KeyCode::BackTab => {
            if let Some(ref mut form) = state.current_form { form.focus_prev(); }
        }

        // Ctrl+Left/Right: switch tabs
        KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ref mut form) = state.current_form { form.prev_tab(); form.error = None; }
        }
        KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ref mut form) = state.current_form { form.next_tab(); form.error = None; }
        }

        // Left/Right without Ctrl: cursor in text fields
        KeyCode::Left  => { if let Some(ref mut form) = state.current_form { form.cursor_left(); } }
        KeyCode::Right => { if let Some(ref mut form) = state.current_form { form.cursor_right(); } }

        // Up/Down: cycle Select options
        KeyCode::Up => {
            if let Some(ref mut form) = state.current_form {
                if is_select_field(form) { form.select_prev(); }
            }
        }
        KeyCode::Down => {
            if let Some(ref mut form) = state.current_form {
                if is_select_field(form) { form.select_next(); }
            }
        }

        // Enter: for Select fields — confirm selection only (no advance/submit)
        // Enter: for text fields — next tab or submit
        KeyCode::Enter => {
            if state.current_form.as_ref().map(|f| is_select_field(f)).unwrap_or(false) {
                return Ok(());
            }
            let action = state.current_form.as_ref().map(|form| {
                let missing_t = form.tab_missing_count(form.active_tab);
                if missing_t > 0 {
                    FormAction::Error(format!(
                        "{} {}",
                        missing_t,
                        if missing_t == 1 { "Pflichtfeld fehlt" } else { "Pflichtfelder fehlen" },
                    ))
                } else if form.is_last_tab() {
                    let missing = form.missing_required();
                    if missing.is_empty() { FormAction::Submit }
                    else { FormAction::Error(format!("{} Pflichtfeld(er) auf anderen Tabs fehlen", missing.len())) }
                } else {
                    FormAction::NextTab
                }
            });

            match action {
                Some(FormAction::Error(msg)) => {
                    if let Some(ref mut form) = state.current_form { form.error = Some(msg); }
                }
                Some(FormAction::NextTab) => {
                    if let Some(ref mut form) = state.current_form { form.error = None; form.next_tab(); }
                }
                Some(FormAction::Submit) => submit_form(state, root)?,
                None => {}
            }
        }

        KeyCode::Backspace => { if let Some(ref mut form) = state.current_form { form.backspace(); } }
        KeyCode::Delete    => { if let Some(ref mut form) = state.current_form { form.delete_char(); } }
        KeyCode::Home      => { if let Some(ref mut form) = state.current_form { form.cursor_home(); } }
        KeyCode::End       => { if let Some(ref mut form) = state.current_form { form.cursor_end(); } }

        KeyCode::Char(c) => {
            if let Some(ref mut form) = state.current_form {
                if !is_select_field(form) { form.insert_char(c); }
            }
        }

        _ => {}
    }
    Ok(())
}

// ── Dashboard ─────────────────────────────────────────────────────────────────

fn handle_dashboard(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    // Delete confirmation mode
    if state.dash_confirm {
        match key.code {
            KeyCode::Char('j') | KeyCode::Char('J') | KeyCode::Char('y') | KeyCode::Char('Y') => {
                delete_selected_project(state, root)?;
            }
            _ => {}
        }
        state.dash_confirm = false;
        return Ok(());
    }

    match state.dash_focus {
        // ── Sidebar ────────────────────────────────────────────────────────────
        DashFocus::Sidebar => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => state.should_quit = true,
            KeyCode::Char('L') => state.lang = state.lang.toggle(),
            KeyCode::Tab => state.dash_focus = DashFocus::Services,

            KeyCode::Up => {
                if state.selected_project > 0 {
                    state.selected_project -= 1;
                    state.rebuild_services();
                    if let Some(proj) = state.projects.get(state.selected_project) {
                        state.hosts = crate::load_hosts(&root.join("projects").join(&proj.slug));
                    }
                }
            }
            KeyCode::Down => {
                if state.selected_project + 1 < state.projects.len() {
                    state.selected_project += 1;
                    state.rebuild_services();
                    if let Some(proj) = state.projects.get(state.selected_project) {
                        state.hosts = crate::load_hosts(&root.join("projects").join(&proj.slug));
                    }
                }
            }

            // n = new project
            KeyCode::Char('n') => {
                state.current_form = Some(crate::project_form::new_project_form());
                state.screen = Screen::NewProject;
            }

            // e = edit selected project (pre-filled)
            KeyCode::Char('e') => {
                if let Some(proj) = state.projects.get(state.selected_project) {
                    state.current_form = Some(crate::project_form::edit_project_form(proj));
                    state.screen = Screen::NewProject;
                }
            }

            // h = new host for selected project
            KeyCode::Char('h') => {
                if let Some(proj) = state.projects.get(state.selected_project) {
                    let slug = proj.slug.clone();
                    state.current_form = Some(crate::host_form::new_host_form(&slug));
                    state.screen = Screen::NewProject;
                }
            }

            // x = confirm delete
            KeyCode::Char('x') | KeyCode::Delete => {
                if !state.projects.is_empty() { state.dash_confirm = true; }
            }

            _ => {}
        },

        // ── Services ───────────────────────────────────────────────────────────
        DashFocus::Services => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => state.should_quit = true,
            KeyCode::Char('L') => state.lang = state.lang.toggle(),
            KeyCode::Tab => state.dash_focus = DashFocus::Sidebar,

            KeyCode::Up => {
                if state.selected > 0 { state.selected -= 1; }
            }
            KeyCode::Down => {
                if state.selected + 1 < state.services.len() { state.selected += 1; }
            }

            // n = new service
            KeyCode::Char('n') => {
                state.current_form = Some(crate::service_form::new_service_form());
                state.screen = Screen::NewProject;
            }

            // l = logs overlay
            KeyCode::Char('l') => {
                if let Some(svc) = state.services.get(state.selected) {
                    let lines = fetch_logs(&svc.name);
                    state.logs_overlay = Some(LogsState { service_name: svc.name.clone(), lines, scroll: 0 });
                }
            }

            // d = deploy (stub)
            KeyCode::Char('d') => {
                if let Some(svc) = state.services.get_mut(state.selected) {
                    svc.status = RunState::Missing;
                }
            }

            // r = restart
            KeyCode::Char('r') => {
                if let Some(svc) = state.services.get(state.selected) {
                    let _ = std::process::Command::new("podman").args(["restart", &svc.name]).output();
                    if let Some(row) = state.services.get_mut(state.selected) {
                        row.status = podman_status(&row.name);
                    }
                }
            }

            // x = stop + remove container
            KeyCode::Char('x') => {
                if let Some(svc) = state.services.get(state.selected) {
                    let _ = std::process::Command::new("podman").args(["stop", &svc.name]).output();
                    let _ = std::process::Command::new("podman").args(["rm",   &svc.name]).output();
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

// ── Logs overlay ──────────────────────────────────────────────────────────────

fn handle_logs(key: KeyEvent, state: &mut AppState) -> Result<()> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => { state.logs_overlay = None; }
        KeyCode::Up => {
            if let Some(ref mut logs) = state.logs_overlay {
                if logs.scroll > 0 { logs.scroll -= 1; }
            }
        }
        KeyCode::Down => {
            if let Some(ref mut logs) = state.logs_overlay {
                let max = logs.lines.len().saturating_sub(1);
                if logs.scroll < max { logs.scroll += 1; }
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Form submit dispatch ──────────────────────────────────────────────────────

enum FormAction { Error(String), NextTab, Submit }

fn submit_form(state: &mut AppState, root: &Path) -> Result<()> {
    let kind = state.current_form.as_ref().map(|f| f.kind);

    match kind {
        Some(ResourceKind::Project) => submit_project(state, root),
        Some(ResourceKind::Service) => submit_service(state, root),
        Some(ResourceKind::Host)    => submit_host(state, root),
        None => Ok(()),
    }
}

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
                    .position(|p| p.slug == slug)
                    .unwrap_or(0);
            }
            state.rebuild_services();
            state.screen = Screen::Dashboard;
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

/// Write service as `projects/{slug}/services/{name}.service.toml`.
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
    if svc_class.is_empty() {
        if let Some(ref mut f) = state.current_form {
            f.error = Some("Service-Typ ist erforderlich".into());
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
    if !svc_version.is_empty() {
        content.push_str(&format!("version       = \"{svc_version}\"\n"));
    }
    if !svc_subdomain.is_empty() {
        content.push_str(&format!("subdomain     = \"{svc_subdomain}\"\n"));
    }
    if !svc_alias.is_empty() {
        content.push_str(&format!("alias         = \"{svc_alias}\"\n"));
    }
    if !svc_port.is_empty() {
        if let Ok(p) = svc_port.parse::<u16>() {
            content.push_str(&format!("port          = {p}\n"));
        }
    }
    std::fs::write(&path, content)?;

    // Also keep a reference in project.toml [load.services.{name}] for backward compat
    let mut proj_content = std::fs::read_to_string(&proj.toml_path)?;
    if !proj_content.contains(&format!("[load.services.{}]", slug)) {
        proj_content.push_str(&format!(
            "\n[load.services.{}]\nservice_class = \"{svc_class}\"\n",
            slug
        ));
        std::fs::write(&proj.toml_path, proj_content)?;
    }

    state.projects = crate::load_projects(root);
    state.rebuild_services();
    state.screen = Screen::Dashboard;
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
            state.screen = Screen::Dashboard;
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

// ── Helpers ───────────────────────────────────────────────────────────────────

fn is_select_field(form: &ResourceForm) -> bool {
    form.focused_field_idx()
        .map(|idx| matches!(form.fields[idx].field_type, FormFieldType::Select))
        .unwrap_or(false)
}

fn is_typing(state: &AppState) -> bool {
    state.current_form.as_ref()
        .and_then(|f| f.focused_field_idx())
        .map(|idx| {
            let form = state.current_form.as_ref().unwrap();
            !matches!(form.fields[idx].field_type, FormFieldType::Select)
        })
        .unwrap_or(false)
}

// ── Mouse events ──────────────────────────────────────────────────────────────

pub fn handle_mouse(event: MouseEvent, state: &mut AppState) -> Result<()> {
    let (tw, _th) = crossterm::terminal::size().unwrap_or((80, 24));

    match event.kind {
        MouseEventKind::ScrollDown => {
            if let Some(ref mut logs) = state.logs_overlay {
                let max = logs.lines.len().saturating_sub(1);
                if logs.scroll < max { logs.scroll += 1; }
            } else if let Some(ref mut form) = state.current_form {
                if is_select_field(form) { form.select_next(); }
            }
        }
        MouseEventKind::ScrollUp => {
            if let Some(ref mut logs) = state.logs_overlay {
                if logs.scroll > 0 { logs.scroll -= 1; }
            } else if let Some(ref mut form) = state.current_form {
                if is_select_field(form) { form.select_prev(); }
            }
        }
        MouseEventKind::Down(_) => {
            // Language button — top-right
            if event.column >= tw.saturating_sub(6) && event.row <= 2 {
                state.lang = state.lang.toggle();
                return Ok(());
            }

            if state.screen == Screen::NewProject {
                if let Some(opt_idx) = find_clicked_dropdown(event.column, event.row, state.current_form.as_ref(), tw) {
                    if let Some(ref mut form) = state.current_form {
                        form.set_select_by_index(opt_idx);
                    }
                    return Ok(());
                }
                if let Some(slot) = find_clicked_field(event.column, event.row, state.current_form.as_ref(), tw) {
                    if let Some(ref mut form) = state.current_form {
                        form.active_field = slot;
                    }
                }
            } else if state.screen == Screen::Dashboard && state.logs_overlay.is_none() {
                handle_dashboard_click(event.column, event.row, state);
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Mouse layout helpers ──────────────────────────────────────────────────────

fn find_clicked_field(col: u16, row: u16, form: Option<&ResourceForm>, tw: u16) -> Option<usize> {
    let form    = form?;
    let pad_x   = tw * 5 / 100;
    let inner_x = pad_x;
    let inner_w = tw - 2 * pad_x;
    let fields_y = 6u16;  // header(3) + tabs(3)

    if col < inner_x || col >= inner_x + inner_w { return None; }

    let indices = form.tab_field_indices();
    for (slot, _) in indices.iter().enumerate() {
        let top = fields_y + slot as u16 * 5;
        if row >= top && row < top + 5 { return Some(slot); }
    }
    None
}

fn find_clicked_dropdown(col: u16, row: u16, form: Option<&ResourceForm>, tw: u16) -> Option<usize> {
    let form  = form?;
    let idx   = form.focused_field_idx()?;
    let field = &form.fields[idx];
    if !matches!(field.field_type, FormFieldType::Select) { return None; }

    let pad_x    = tw * 5 / 100;
    let inner_x  = pad_x;
    let inner_w  = tw - 2 * pad_x;
    let fields_y = 6u16;

    if col < inner_x || col >= inner_x + inner_w { return None; }

    let field_y    = fields_y + form.active_field as u16 * 5;
    let dropdown_y = field_y + 4;

    if row > dropdown_y && row <= dropdown_y + field.options.len() as u16 {
        let opt_idx = (row - dropdown_y - 1) as usize;
        if opt_idx < field.options.len() { return Some(opt_idx); }
    }
    None
}

// ── Dashboard click handler ───────────────────────────────────────────────────

/// Map a mouse click in the dashboard to the correct focus + selection.
/// Dashboard layout: header=3 rows, body starts at row 3, sidebar=22 cols wide.
fn handle_dashboard_click(col: u16, row: u16, state: &mut AppState) {
    const SIDEBAR_W: u16 = 22;
    const HEADER_H:  u16 = 3;

    if row < HEADER_H { return; }
    let body_row = row - HEADER_H;  // row within the body area

    if col < SIDEBAR_W {
        // Click in sidebar → focus sidebar, select project
        state.dash_focus = DashFocus::Sidebar;
        // Sidebar items start at body_row 0:
        //   project0, [host0, host1, ..., +New Host] if selected, project1, ...
        // Simple approximation: map row to project index (ignoring host sub-rows)
        let proj_count = state.projects.len();
        if proj_count == 0 { return; }
        // Each selected project has 1 + hosts.len() + 1 extra rows; others have 1 row.
        // Walk through to find which item was clicked.
        let mut cur_row: u16 = 0;
        for (i, _) in state.projects.iter().enumerate() {
            if body_row == cur_row {
                state.selected_project = i;
                return;
            }
            cur_row += 1;
            // Extra rows for the selected project (hosts + "New Host")
            if i == state.selected_project {
                let extra = state.hosts.len() as u16 + 1; // hosts + "+ New Host"
                if body_row > cur_row && body_row < cur_row + extra {
                    return; // clicked a host entry, keep selection
                }
                cur_row += extra;
            }
        }
    } else {
        // Click in services panel → focus services, select row
        state.dash_focus = DashFocus::Services;
        // Services table: 1 header row, then one row per service.
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
