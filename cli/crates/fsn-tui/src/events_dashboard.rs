// Dashboard keyboard event handling.
//
// Design Pattern: Chain of Responsibility — key events are passed through a
// shared pre-handler (handle_dashboard_shared) before reaching the focus-specific
// handler (sidebar or services). Shared keys (quit, lang-toggle, new-resource)
// are handled once, not duplicated in each branch.
//
// Entry point: handle_dashboard() — called from events.rs Screen::Dashboard arm.
// activate_sidebar_item() is pub so mouse.rs can reuse the same activation logic.

use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{
    AppState, ConfirmAction, DashFocus, LogsState, OverlayLayer, ResourceKind, Screen,
    SidebarAction, SidebarItem, NEW_RESOURCE_ITEMS,
};
use crate::actions::{
    delete_selected_project, delete_selected_host, delete_service_by_name,
    fetch_logs, podman_status, stop_service_container, sync_sidebar_selection,
};
use crate::deploy_thread::trigger_deploy;

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn handle_dashboard(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    match state.dash_focus {
        DashFocus::Sidebar  => handle_dashboard_sidebar(key, state, root),
        DashFocus::Services => handle_dashboard_services(key, state, root),
    }
}

// ── Shared dashboard shortcuts ────────────────────────────────────────────────

/// Handle keys that are identical in both sidebar and services focus.
/// Returns `true` if the key was consumed so the caller can return early.
///
/// Shared keys:  q/Esc → quit confirm  |  L → lang toggle  |  n → new-resource popup
fn handle_dashboard_shared(key: KeyEvent, state: &mut AppState) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.push_overlay(OverlayLayer::Confirm {
                message: "confirm.quit".into(), data: None, yes_action: ConfirmAction::Quit,
            });
            true
        }
        KeyCode::Char('L') => { state.lang = state.lang.toggle(); true }
        KeyCode::Char('n') => {
            state.push_overlay(OverlayLayer::NewResource { selected: 0 });
            true
        }
        _ => false,
    }
}

// ── Sidebar focus ─────────────────────────────────────────────────────────────

fn handle_dashboard_sidebar(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    if handle_dashboard_shared(key, state) { return Ok(()); }

    match key.code {
        KeyCode::Tab => state.dash_focus = DashFocus::Services,

        KeyCode::Up => {
            let cur = state.sidebar_cursor;
            if let Some(prev) = (0..cur).rev().find(|&i| state.sidebar_items[i].is_selectable()) {
                state.sidebar_cursor = prev;
                sync_sidebar_selection(state, root);
            }
        }
        KeyCode::Down => {
            let cur = state.sidebar_cursor;
            let len = state.sidebar_items.len();
            if let Some(next) = (cur + 1..len).find(|&i| state.sidebar_items[i].is_selectable()) {
                state.sidebar_cursor = next;
                sync_sidebar_selection(state, root);
            }
        }

        KeyCode::Char('S') => {
            state.settings_cursor = 0;
            state.screen = Screen::Settings;
        }

        // 'e' = explicit edit (same as Enter on a resource item, but not on Action items).
        KeyCode::Char('e') => {
            if let Some(item) = state.current_sidebar_item().cloned() {
                open_edit_form_for_item(&item, state);
            }
        }
        // Enter = "activate": opens create form for Action items, edit form for resources.
        KeyCode::Enter => {
            if let Some(item) = state.current_sidebar_item().cloned() {
                activate_sidebar_item(item, state, root);
            }
        }

        KeyCode::Char('s') => sidebar_start_resource(state, root),
        KeyCode::Char('x') | KeyCode::Delete => sidebar_confirm_delete(state),

        _ => {}
    }
    Ok(())
}

// ── Services focus ────────────────────────────────────────────────────────────

fn handle_dashboard_services(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    if handle_dashboard_shared(key, state) { return Ok(()); }

    match key.code {
        KeyCode::Tab => state.dash_focus = DashFocus::Sidebar,

        KeyCode::Up   => { if state.selected > 0 { state.selected -= 1; } }
        KeyCode::Down => {
            if state.selected + 1 < state.services.len() { state.selected += 1; }
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
                let host = state.hosts.first().map(|h| h.config.clone());
                trigger_deploy(state, root, proj, host);
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
                state.push_overlay(OverlayLayer::Confirm {
                    message:    "confirm.stop.service".into(),
                    data:       Some(svc.name.clone()),
                    yes_action: ConfirmAction::StopService,
                });
            }
        }
        KeyCode::Char('s') => {
            if let Some(svc) = state.services.get(state.selected).cloned() {
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "start", &format!("{}.service", svc.name)])
                    .output();
                if let Some(row) = state.services.iter_mut().find(|s| s.name == svc.name) {
                    row.status = podman_status(&svc.name);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Sidebar action helpers ────────────────────────────────────────────────────

/// Open the edit form for an existing resource (Project, Host, or Service).
/// Does nothing for Section and Action items — they have no edit form.
fn open_edit_form_for_item(item: &SidebarItem, state: &mut AppState) {
    match item {
        SidebarItem::Project { slug, .. } => {
            if let Some(proj) = state.projects.iter().find(|p| p.slug == *slug).cloned() {
                state.current_form = Some(crate::project_form::edit_project_form(&proj));
                state.screen = Screen::NewProject;
            }
        }
        SidebarItem::Host { slug, .. } => {
            if let Some(host) = state.hosts.iter().find(|h| h.slug == *slug).cloned() {
                let slugs = project_slugs(state);
                state.current_form = Some(crate::host_form::edit_host_form(&host, slugs));
                state.screen = Screen::NewProject;
            }
        }
        SidebarItem::Service { name, .. } => {
            if let Some(proj) = state.projects.get(state.selected_project).cloned() {
                if let Some(entry) = proj.config.load.services.get(name).cloned() {
                    let slug = crate::resource_form::slugify(name);
                    state.current_form = Some(crate::service_form::edit_service_form(name, &entry, slug));
                    state.screen = Screen::NewProject;
                }
            }
        }
        _ => {}
    }
}

/// Activate a sidebar item — the single source of truth for "what happens when
/// an item is selected by keyboard or mouse".
///
/// For Action items: opens the corresponding create form or wizard.
/// For resource items (Project, Host, Service): opens the edit form.
/// Called by both keyboard Enter and mouse click handlers.
pub fn activate_sidebar_item(item: SidebarItem, state: &mut AppState, root: &Path) {
    match item {
        SidebarItem::Action { kind: SidebarAction::NewProject, .. } => {
            let queue = crate::task_queue::TaskQueue::new(
                crate::task_queue::TaskKind::NewProject, state,
            );
            state.task_queue = Some(queue);
            state.screen = Screen::TaskWizard;
        }
        SidebarItem::Action { kind: SidebarAction::NewHost, .. } => {
            let slugs   = project_slugs(state);
            let current = current_project_slug(state).to_string();
            state.current_form = Some(crate::host_form::new_host_form(slugs, &current));
            state.screen = Screen::NewProject;
        }
        SidebarItem::Action { kind: SidebarAction::NewService, .. } => {
            state.current_form = Some(crate::service_form::new_service_form());
            state.screen = Screen::NewProject;
        }
        // Resource items: open their edit form (same behavior as 'e' key).
        other => open_edit_form_for_item(&other, state),
    }
    let _ = root;
}

fn sidebar_start_resource(state: &mut AppState, root: &Path) {
    let item = state.current_sidebar_item().cloned();
    match item {
        Some(SidebarItem::Project { slug, .. }) => {
            if let Some(proj) = state.projects.iter().find(|p| p.slug == slug).cloned() {
                let host = state.hosts.first().map(|h| h.config.clone());
                trigger_deploy(state, root, proj, host);
            }
        }
        Some(SidebarItem::Host { slug, .. }) => {
            if let Some(proj) = state.projects.get(state.selected_project).cloned() {
                let host_cfg = state.hosts.iter()
                    .find(|h| h.slug == slug)
                    .map(|h| h.config.clone());
                trigger_deploy(state, root, proj, host_cfg);
            }
        }
        Some(SidebarItem::Service { name, .. }) => {
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "start", &format!("{}.service", name)])
                .output();
            if let Some(row) = state.services.iter_mut().find(|s| s.name == name) {
                row.status = podman_status(&name);
            }
        }
        _ => {}
    }
}

fn sidebar_confirm_delete(state: &mut AppState) {
    let item = state.current_sidebar_item().cloned();
    match item {
        Some(SidebarItem::Project { .. }) if !state.projects.is_empty() => {
            state.push_overlay(OverlayLayer::Confirm {
                message: "confirm.delete.project".into(), data: None,
                yes_action: ConfirmAction::DeleteProject,
            });
        }
        Some(SidebarItem::Host { slug, .. }) => {
            state.push_overlay(OverlayLayer::Confirm {
                message: "confirm.delete.host".into(), data: Some(slug),
                yes_action: ConfirmAction::DeleteHost,
            });
        }
        Some(SidebarItem::Service { name, .. }) => {
            state.push_overlay(OverlayLayer::Confirm {
                message: "confirm.delete.service".into(), data: Some(name),
                yes_action: ConfirmAction::DeleteService,
            });
        }
        _ => {}
    }
}

// ── New-resource overlay helpers ──────────────────────────────────────────────

pub(crate) fn handle_new_resource_overlay(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    let count = NEW_RESOURCE_ITEMS.len();
    match key.code {
        KeyCode::Esc => { state.pop_overlay(); }

        // Circular navigation: Up wraps from 0 → last, Down wraps from last → 0.
        KeyCode::Up => {
            if let Some(OverlayLayer::NewResource { selected }) = state.top_overlay_mut() {
                *selected = selected.checked_sub(1).unwrap_or(count - 1);
            }
        }
        KeyCode::Down => {
            if let Some(OverlayLayer::NewResource { selected }) = state.top_overlay_mut() {
                *selected = (*selected + 1) % count;
            }
        }
        KeyCode::Enter => {
            let idx = match state.top_overlay() {
                Some(OverlayLayer::NewResource { selected }) => *selected,
                _ => return Ok(()),
            };
            state.pop_overlay();
            open_new_resource_form(idx, state, root);
        }
        _ => {}
    }
    Ok(())
}

fn open_new_resource_form(item_idx: usize, state: &mut AppState, root: &Path) {
    let Some(&(_, kind)) = NEW_RESOURCE_ITEMS.get(item_idx) else { return };
    match kind {
        ResourceKind::Project => {
            let queue = crate::task_queue::TaskQueue::new(
                crate::task_queue::TaskKind::NewProject, state,
            );
            state.task_queue = Some(queue);
            state.screen = Screen::TaskWizard;
        }
        ResourceKind::Host => {
            let slugs   = project_slugs(state);
            let current = current_project_slug(state).to_string();
            state.current_form = Some(crate::host_form::new_host_form(slugs, &current));
            state.screen = Screen::NewProject;
        }
        ResourceKind::Service => {
            state.current_form = Some(crate::service_form::new_service_form());
            state.screen = Screen::NewProject;
        }
        ResourceKind::Bot => {
            state.current_form = Some(crate::bot_form::new_bot_form());
            state.screen = Screen::NewProject;
        }
    }
    let _ = root;
}

// ── Confirm action helpers ────────────────────────────────────────────────────

pub(crate) fn execute_confirm_action(
    state: &mut AppState,
    root: &Path,
    data: Option<String>,
    yes_action: ConfirmAction,
) -> Result<()> {
    match yes_action {
        ConfirmAction::DeleteProject => delete_selected_project(state, root)?,
        ConfirmAction::DeleteHost    => delete_selected_host(state, root)?,
        ConfirmAction::LeaveForm => {
            state.current_form = None;
            state.screen = if state.projects.is_empty() {
                Screen::Welcome
            } else {
                Screen::Dashboard
            };
        }
        ConfirmAction::LeaveWizard => {
            state.task_queue = None;
            state.screen = Screen::Dashboard;
        }
        ConfirmAction::Quit => { state.should_quit = true; }
        ConfirmAction::DeleteService => {
            delete_service_by_name(state, root, data.unwrap_or_default())?;
        }
        ConfirmAction::StopService => {
            stop_service_container(state, data.unwrap_or_default());
        }
    }
    Ok(())
}

// ── Small helpers ─────────────────────────────────────────────────────────────

/// Collect all project slugs — used when building host form dropdowns.
fn project_slugs(state: &AppState) -> Vec<String> {
    state.projects.iter().map(|p| p.slug.clone()).collect()
}

/// Slug of the currently selected project, or empty string.
fn current_project_slug(state: &AppState) -> &str {
    state.projects.get(state.selected_project)
        .map(|p| p.slug.as_str())
        .unwrap_or("")
}
