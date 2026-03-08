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
    AppState, ConfirmAction, DashFocus, DeployMsg, DeployState, LogsState,
    OverlayLayer, ResourceKind, RunState, Screen, SidebarAction, SidebarItem,
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
        OverlayLayer::Logs(_)     => "logs",
        OverlayLayer::Confirm{..} => "confirm",
        OverlayLayer::Deploy(_)   => "deploy",
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
        Some("deploy") => {
            // Only closeable once done
            let done = state.top_overlay().map(|o| {
                if let OverlayLayer::Deploy(ref d) = o { d.done } else { false }
            }).unwrap_or(false);
            if done && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                state.pop_overlay();
                state.deploy_rx = None;
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
        Some(ResourceKind::Bot)     => submit_bot(state, root)?,
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
                let cur = state.sidebar_cursor;
                let prev = (0..cur).rev().find(|&i| state.sidebar_items[i].is_selectable());
                if let Some(prev) = prev {
                    state.sidebar_cursor = prev;
                    sync_sidebar_selection(state, root);
                }
            }
            KeyCode::Down => {
                let cur = state.sidebar_cursor;
                let len = state.sidebar_items.len();
                let next = (cur + 1..len).find(|&i| state.sidebar_items[i].is_selectable());
                if let Some(next) = next {
                    state.sidebar_cursor = next;
                    sync_sidebar_selection(state, root);
                }
            }

            // Context-aware 'n': new project when on project context, new host when on host context.
            KeyCode::Char('n') => {
                let item = state.current_sidebar_item().cloned();
                match item {
                    Some(SidebarItem::Host { .. })
                    | Some(SidebarItem::Action { kind: SidebarAction::NewHost, .. }) => {
                        let project_slugs = state.projects.iter().map(|p| p.slug.clone()).collect();
                        let current = state.projects.get(state.selected_project)
                            .map(|p| p.slug.as_str()).unwrap_or("");
                        state.current_form = Some(crate::host_form::new_host_form(project_slugs, current));
                        state.screen = Screen::NewProject;
                    }
                    _ => {
                        state.current_form = Some(crate::project_form::new_project_form());
                        state.screen = Screen::NewProject;
                    }
                }
            }

            // Context-aware 'e': edit the item under the cursor (project or host).
            KeyCode::Char('e') => {
                let item = state.current_sidebar_item().cloned();
                match item {
                    Some(SidebarItem::Project { slug, .. }) => {
                        if let Some(proj) = state.projects.iter().find(|p| p.slug == slug).cloned() {
                            state.current_form = Some(crate::project_form::edit_project_form(&proj));
                            state.screen = Screen::NewProject;
                        }
                    }
                    Some(SidebarItem::Host { slug, .. }) => {
                        if let Some(host) = state.hosts.iter().find(|h| h.slug == slug).cloned() {
                            let project_slugs = state.projects.iter().map(|p| p.slug.clone()).collect();
                            state.current_form = Some(crate::host_form::edit_host_form(&host, project_slugs));
                            state.screen = Screen::NewProject;
                        }
                    }
                    _ => {}
                }
            }

            // Enter activates the current sidebar item.
            KeyCode::Enter => {
                let item = state.current_sidebar_item().cloned();
                match item {
                    Some(SidebarItem::Action { kind: SidebarAction::NewProject, .. }) => {
                        state.current_form = Some(crate::project_form::new_project_form());
                        state.screen = Screen::NewProject;
                    }
                    Some(SidebarItem::Action { kind: SidebarAction::NewHost, .. }) => {
                        let project_slugs = state.projects.iter().map(|p| p.slug.clone()).collect();
                        let current = state.projects.get(state.selected_project)
                            .map(|p| p.slug.as_str()).unwrap_or("");
                        state.current_form = Some(crate::host_form::new_host_form(project_slugs, current));
                        state.screen = Screen::NewProject;
                    }
                    Some(SidebarItem::Project { slug, .. }) => {
                        if let Some(idx) = state.projects.iter().position(|p| p.slug == slug) {
                            state.selected_project = idx;
                            reload_hosts(state, root);
                            state.rebuild_services();
                        }
                        state.dash_focus = DashFocus::Services;
                    }
                    Some(SidebarItem::Host { .. }) | Some(SidebarItem::Action { .. }) => {
                        state.dash_focus = DashFocus::Services;
                    }
                    _ => {}
                }
            }

            // Context-aware 'x': delete project or (future) host.
            KeyCode::Char('x') | KeyCode::Delete => {
                let item = state.current_sidebar_item();
                match item {
                    Some(SidebarItem::Project { .. }) if !state.projects.is_empty() => {
                        state.push_overlay(OverlayLayer::Confirm {
                            message:    "dash.hint.confirm".into(),
                            yes_action: ConfirmAction::DeleteProject,
                        });
                    }
                    _ => {}
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

            KeyCode::Char('b') => {
                state.current_form = Some(crate::bot_form::new_bot_form());
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
                if let Some(proj) = state.projects.get(state.selected_project).cloned() {
                    trigger_compose_export(state, root, proj.slug.clone(), proj.config.clone());
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

// ── Compose export (background thread) ───────────────────────────────────────

/// Spawn a background thread that generates compose.yml + .env.example
/// for the given project and reports progress via the deploy overlay.
fn trigger_compose_export(
    state:       &mut AppState,
    root:        &Path,
    slug:        String,
    project_cfg: fsn_core::config::ProjectConfig,
) {
    let (tx, rx) = std::sync::mpsc::channel::<DeployMsg>();
    state.deploy_rx = Some(rx);
    state.push_overlay(OverlayLayer::Deploy(DeployState {
        target:  project_cfg.project.name.clone(),
        log:     Vec::new(),
        done:    false,
        success: false,
    }));

    let project_dir = root.join("projects").join(&slug);

    std::thread::spawn(move || {
        let out_dir = project_dir.join("compose");
        let _ = tx.send(DeployMsg::Log(format!("Ziel: {}/", out_dir.display())));

        if let Err(e) = std::fs::create_dir_all(&out_dir) {
            let _ = tx.send(DeployMsg::Done { success: false, error: Some(e.to_string()) });
            return;
        }

        // Generate compose.yml
        let _ = tx.send(DeployMsg::Log("Schreibe compose.yml...".into()));
        let compose_content = fsn_engine::generate::compose::generate_compose(&project_cfg);
        let compose_path = out_dir.join("compose.yml");
        if let Err(e) = std::fs::write(&compose_path, &compose_content) {
            let _ = tx.send(DeployMsg::Done { success: false, error: Some(format!("compose.yml: {e}")) });
            return;
        }
        let _ = tx.send(DeployMsg::Log("✓ compose.yml".into()));

        // Generate .env.example
        let _ = tx.send(DeployMsg::Log("Schreibe .env.example...".into()));
        let env_content = fsn_engine::generate::compose::generate_env_example(&project_cfg);
        let env_path = out_dir.join(".env.example");
        if let Err(e) = std::fs::write(&env_path, &env_content) {
            let _ = tx.send(DeployMsg::Done { success: false, error: Some(format!(".env.example: {e}")) });
            return;
        }
        let _ = tx.send(DeployMsg::Log("✓ .env.example".into()));

        let _ = tx.send(DeployMsg::Done { success: true, error: None });
    });
}

fn delete_selected_project(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project) else { return Ok(()); };
    let project_dir = root.join("projects").join(&proj.slug);
    let _ = std::fs::remove_dir_all(&project_dir);
    state.projects.remove(state.selected_project);
    if state.selected_project > 0 && state.selected_project >= state.projects.len() {
        state.selected_project -= 1;
    }
    state.hosts.clear();
    state.rebuild_sidebar();
    state.rebuild_services();
    if state.projects.is_empty() { state.screen = Screen::Welcome; }
    Ok(())
}

fn reload_hosts(state: &mut AppState, root: &Path) {
    if let Some(proj) = state.projects.get(state.selected_project) {
        state.hosts = crate::load_hosts(&root.join("projects").join(&proj.slug));
        state.rebuild_sidebar();
    }
}

/// Called after `sidebar_cursor` moves — syncs `selected_project` / `selected_host`
/// and reloads dependent data when a Project item is selected.
fn sync_sidebar_selection(state: &mut AppState, root: &Path) {
    match state.current_sidebar_item().cloned() {
        Some(SidebarItem::Project { slug, .. }) => {
            if let Some(idx) = state.projects.iter().position(|p| p.slug == slug) {
                state.selected_project = idx;
                reload_hosts(state, root);
                state.rebuild_services();
            }
        }
        Some(SidebarItem::Host { slug, .. }) => {
            if let Some(idx) = state.hosts.iter().position(|h| h.slug == slug) {
                state.selected_host = idx;
            }
        }
        _ => {}
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
            state.rebuild_sidebar();
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
    let Some(proj) = state.projects.get(state.selected_project).cloned() else {
        if let Some(ref mut f) = state.current_form {
            f.error = Some("Kein Projekt ausgewählt".into());
        }
        return Ok(());
    };

    let project_dir  = root.join("projects").join(&proj.slug);
    let services_dir = project_dir.join("services");
    std::fs::create_dir_all(&services_dir)?;

    let result = state.current_form.as_ref()
        .map(|form| crate::service_form::submit_service_form(form, &services_dir));

    match result {
        Some(Ok(())) => {
            // Also register in project.toml [load.services.{slug}]
            if let Some(ref form) = state.current_form {
                let svc_name  = form.field_value("name");
                let svc_class = form.field_value("class");
                let slug      = crate::app::slugify(&svc_name);
                let mut proj_content = std::fs::read_to_string(&proj.toml_path)?;
                if !proj_content.contains(&format!("[load.services.{}]", slug)) {
                    let version = form.field_value("version");
                    let ver = if version.is_empty() { "latest".to_string() } else { version };
                    proj_content.push_str(&format!(
                        "\n[load.services.{slug}]\nservice_class = \"{svc_class}\"\nversion       = \"{ver}\"\n"
                    ));
                    std::fs::write(&proj.toml_path, proj_content)?;
                }
            }
            state.projects = crate::load_projects(root);
            state.rebuild_services();
            state.screen      = Screen::Dashboard;
            state.dash_focus  = DashFocus::Services;
            state.current_form = None;
        }
        Some(Err(e)) => {
            if let Some(ref mut form) = state.current_form {
                form.error = Some(format!("{e}"));
            }
        }
        None => {}
    }
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
            state.rebuild_sidebar();
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

fn submit_bot(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project).cloned() else {
        if let Some(ref mut f) = state.current_form {
            f.error = Some("Kein Projekt ausgewählt".into());
        }
        return Ok(());
    };
    let project_dir = root.join("projects").join(&proj.slug);

    let result = state.current_form.as_ref()
        .map(|form| crate::bot_form::submit_bot_form(form, &project_dir, &proj.slug));

    match result {
        Some(Ok(())) => {
            state.screen      = Screen::Dashboard;
            state.dash_focus  = DashFocus::Services;
            state.current_form = None;
        }
        Some(Err(e)) => {
            if let Some(ref mut form) = state.current_form {
                form.error = Some(format!("{e}"));
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
        // inner area starts 1 row below the block (top padding in render_sidebar)
        const INNER_OFFSET: u16 = 1;
        if body_row < INNER_OFFSET { return; }
        let item_idx = (body_row - INNER_OFFSET) as usize;
        if let Some(item) = state.sidebar_items.get(item_idx) {
            if item.is_selectable() {
                state.sidebar_cursor = item_idx;
                // Note: full sync (reload_hosts, rebuild_services) only via keyboard.
                // Mouse click just moves focus; press a key to activate.
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
