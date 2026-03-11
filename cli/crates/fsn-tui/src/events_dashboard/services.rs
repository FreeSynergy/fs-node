// Services focus keyboard event handler.
//
// Pattern: Chain of Responsibility — shared shortcuts checked first (via
// handle_dashboard_shared), then services-specific keys.
//
// Services-specific keys: ↑↓ navigation, Space = multi-select toggle,
// u = clear selection, l = logs, d = deploy, r = restart, x = stop/batch-stop,
// s = start/batch-start, y = yank domain.

use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{AppState, ConfirmAction, DashFocus, LogsState, NotifKind, OverlayLayer};
use crate::actions::{copy_to_clipboard, fetch_logs, restart_service, start_service, stop_service_container};
use crate::deploy_thread::trigger_deploy;

use super::shortcuts::handle_dashboard_shared;

/// Handle keyboard input when dashboard focus is on the services table.
pub(super) fn handle_dashboard_services(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    if handle_dashboard_shared(key, state) { return Ok(()); }

    match key.code {
        KeyCode::Tab => state.dash_focus = DashFocus::Sidebar,

        KeyCode::Up   => { if state.selected > 0 { state.selected -= 1; } }
        KeyCode::Down => {
            if state.selected + 1 < state.services.len() { state.selected += 1; }
        }

        // Space = toggle current service in multi-select set.
        KeyCode::Char(' ') => {
            let idx = state.selected;
            if state.selected_services.contains(&idx) {
                state.selected_services.remove(&idx);
            } else {
                state.selected_services.insert(idx);
            }
        }

        // 'u' = clear all selections.
        KeyCode::Char('u') => {
            state.selected_services.clear();
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
            if let Some(name) = state.services.get(state.selected).map(|s| s.name.clone()) {
                restart_service(state, &name);
            }
        }
        KeyCode::Char('x') => {
            if !state.selected_services.is_empty() {
                // Batch stop: stop all selected services immediately (no confirm for batch).
                let names: Vec<String> = state.selected_services.iter()
                    .filter_map(|&i| state.services.get(i).map(|s| s.name.clone()))
                    .collect();
                let count = names.len();
                for name in names {
                    stop_service_container(state, name);
                }
                state.selected_services.clear();
                state.push_notif(NotifKind::Info, format!("{} services stopped", count));
            } else if let Some(svc) = state.services.get(state.selected) {
                state.push_overlay(OverlayLayer::Confirm {
                    message:    "confirm.stop.service".into(),
                    data:       Some(svc.name.clone()),
                    yes_action: ConfirmAction::StopService,
                });
            }
        }
        KeyCode::Char('s') => {
            if !state.selected_services.is_empty() {
                // Batch start: start all selected services.
                let names: Vec<String> = state.selected_services.iter()
                    .filter_map(|&i| state.services.get(i).map(|s| s.name.clone()))
                    .collect();
                let count = names.len();
                for name in names {
                    start_service(state, &name);
                }
                state.selected_services.clear();
                state.push_notif(NotifKind::Info, format!("{} services started", count));
            } else if let Some(name) = state.services.get(state.selected).map(|s| s.name.clone()) {
                start_service(state, &name);
            }
        }

        // 'y' = yank domain of selected service to clipboard.
        KeyCode::Char('y') => {
            if let Some(domain) = state.services.get(state.selected).map(|s| s.domain.clone()) {
                copy_to_clipboard(state, &domain);
            }
        }

        _ => {}
    }
    Ok(())
}
