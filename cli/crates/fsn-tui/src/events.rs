// Keyboard event handling.

use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::app::{AppState, FormFieldType, FormTab, LogsState, NewProjectForm, Screen, ServiceStatus};

pub fn handle(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    // Update ctrl-hint display (switches hint bar to show Ctrl shortcuts)
    state.ctrl_hint = key.modifiers.contains(KeyModifiers::CONTROL);

    // Global: Ctrl-C always quits
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        state.should_quit = true;
        return Ok(());
    }

    // Logs overlay is modal — handle separately
    if state.logs_overlay.is_some() {
        return handle_logs(key, state);
    }

    match state.screen {
        Screen::Welcome    => handle_welcome(key, state, root),
        Screen::Dashboard  => handle_dashboard(key, state, root),
        Screen::NewProject => handle_new_project(key, state),
    }
}

// ── Welcome screen ────────────────────────────────────────────────────────────

fn handle_welcome(key: KeyEvent, state: &mut AppState, _root: &Path) -> Result<()> {
    match key.code {
        KeyCode::Char('q') => {
            state.should_quit = true;
        }
        // L = language toggle
        KeyCode::Char('l') | KeyCode::Char('L') => {
            state.lang = state.lang.toggle();
        }
        // Arrow keys move between buttons
        KeyCode::Left | KeyCode::Right => {
            state.welcome_focus = 1 - state.welcome_focus;
        }
        KeyCode::Enter => {
            if state.welcome_focus == 0 {
                // Open New Project form
                state.new_project = Some(NewProjectForm::new());
                state.screen = Screen::NewProject;
            }
            // Button 1 (Open Project) is grayed out — no action
        }
        _ => {}
    }
    Ok(())
}

// ── New Project form ──────────────────────────────────────────────────────────

fn handle_new_project(key: KeyEvent, state: &mut AppState) -> Result<()> {
    // Language toggle available everywhere
    if matches!(key.code, KeyCode::Char('l') | KeyCode::Char('L'))
        && !is_typing(state)
    {
        state.lang = state.lang.toggle();
        return Ok(());
    }

    match key.code {
        KeyCode::Esc => {
            // Close modal entirely; form data is preserved in state.new_project
            state.screen = Screen::Welcome;
        }

        // Tab switches to next field
        KeyCode::Tab => {
            if let Some(ref mut form) = state.new_project {
                form.focus_next();
            }
        }

        // Shift+Tab goes to previous field
        KeyCode::BackTab => {
            if let Some(ref mut form) = state.new_project {
                form.focus_prev();
            }
        }

        // Ctrl+Left: previous tab
        KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ref mut form) = state.new_project {
                form.prev_tab();
                form.error = None;
            }
        }

        // Ctrl+Right: next tab
        KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ref mut form) = state.new_project {
                form.next_tab();
                form.error = None;
            }
        }

        // Left/Right without Ctrl: cursor movement in text fields
        KeyCode::Left => {
            if let Some(ref mut form) = state.new_project {
                form.cursor_left();
            }
        }

        KeyCode::Right => {
            if let Some(ref mut form) = state.new_project {
                form.cursor_right();
            }
        }

        // Up/Down: cycle Select fields or move cursor in text
        KeyCode::Up => {
            if let Some(ref mut form) = state.new_project {
                if is_select_field(form) {
                    // cycle backward (select_prev could be added, for now use select_next repeatedly)
                    form.select_prev();
                }
            }
        }
        KeyCode::Down => {
            if let Some(ref mut form) = state.new_project {
                if is_select_field(form) {
                    form.select_next();
                }
            }
        }

        // Enter: go to next tab, or submit on last tab
        KeyCode::Enter => {
            if let Some(ref mut form) = state.new_project {
                let is_last_tab = form.active_tab == FormTab::count() - 1;
                let missing_on_tab = form.tab_missing_count(form.active_tab);
                if missing_on_tab > 0 {
                    form.error = Some(format!(
                        "{} {}",
                        missing_on_tab,
                        if missing_on_tab == 1 { "Pflichtfeld fehlt" } else { "Pflichtfelder fehlen" },
                    ));
                } else if is_last_tab {
                    let missing_total = form.missing_required();
                    if missing_total.is_empty() {
                        // TODO: trigger project creation
                        form.error = None;
                        state.screen = Screen::Welcome;
                    } else {
                        form.error = Some(format!("{} Pflichtfeld(er) auf anderen Tabs fehlen", missing_total.len()));
                    }
                } else {
                    form.error = None;
                    form.next_tab();
                }
            }
        }

        // Backspace: delete char before cursor
        KeyCode::Backspace => {
            if let Some(ref mut form) = state.new_project {
                form.backspace();
            }
        }

        // Delete: delete char at cursor
        KeyCode::Delete => {
            if let Some(ref mut form) = state.new_project {
                form.delete_char();
            }
        }

        // Home/End: jump cursor
        KeyCode::Home => {
            if let Some(ref mut form) = state.new_project {
                form.cursor_home();
            }
        }
        KeyCode::End => {
            if let Some(ref mut form) = state.new_project {
                form.cursor_end();
            }
        }

        // Printable characters → insert into focused field (unless Select)
        KeyCode::Char(c) => {
            if let Some(ref mut form) = state.new_project {
                if !is_select_field(form) {
                    form.insert_char(c);
                }
            }
        }

        _ => {}
    }
    Ok(())
}

// ── Dashboard ─────────────────────────────────────────────────────────────────

fn handle_dashboard(key: KeyEvent, state: &mut AppState, _root: &Path) -> Result<()> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.should_quit = true;
        }
        KeyCode::Char('l') | KeyCode::Char('L') => {
            // L without selection = language toggle; with selection ambiguous
            // Check if services list is focused first
            if state.services.is_empty() {
                state.lang = state.lang.toggle();
            } else {
                // Open logs overlay for selected service
                if let Some(svc) = state.services.get(state.selected) {
                    let lines = fetch_logs(&svc.name);
                    state.logs_overlay = Some(LogsState {
                        service_name: svc.name.clone(),
                        lines,
                        scroll: 0,
                    });
                }
            }
        }
        KeyCode::Up => {
            if state.selected > 0 {
                state.selected -= 1;
            }
        }
        KeyCode::Down => {
            if state.selected + 1 < state.services.len() {
                state.selected += 1;
            }
        }
        KeyCode::Char('d') => {
            // Deploy: mark as unknown while running (async in future)
            if let Some(svc) = state.services.get_mut(state.selected) {
                svc.status = ServiceStatus::Unknown;
            }
            // TODO: spawn deploy task
        }
        KeyCode::Char('r') => {
            // Restart selected service via podman
            if let Some(svc) = state.services.get(state.selected) {
                let _ = std::process::Command::new("podman")
                    .args(["restart", &svc.name])
                    .output();
                if let Some(row) = state.services.get_mut(state.selected) {
                    row.status = podman_status(&row.name);
                }
            }
        }
        KeyCode::Char('x') => {
            // Remove selected service (stop + remove)
            if let Some(svc) = state.services.get(state.selected) {
                let _ = std::process::Command::new("podman")
                    .args(["stop", &svc.name])
                    .output();
                let _ = std::process::Command::new("podman")
                    .args(["rm", &svc.name])
                    .output();
                state.services.remove(state.selected);
                if state.selected > 0 && state.selected >= state.services.len() {
                    state.selected -= 1;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Logs overlay ──────────────────────────────────────────────────────────────

fn handle_logs(key: KeyEvent, state: &mut AppState) -> Result<()> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.logs_overlay = None;
        }
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

// ── Helpers ───────────────────────────────────────────────────────────────────

/// True if the focused field accepts free-form text input (not a Select).
fn is_text_field(form: &NewProjectForm) -> bool {
    if let Some(idx) = form.focused_field_idx() {
        !matches!(form.fields[idx].field_type, FormFieldType::Select)
    } else {
        false
    }
}

fn is_select_field(form: &NewProjectForm) -> bool {
    if let Some(idx) = form.focused_field_idx() {
        matches!(form.fields[idx].field_type, FormFieldType::Select)
    } else {
        false
    }
}

/// True if the user is currently typing in a text-style field.
/// Used to disambiguate single-key shortcuts vs. typed characters.
fn is_typing(state: &AppState) -> bool {
    if let Some(ref form) = state.new_project {
        is_text_field(form)
    } else {
        false
    }
}

// ── Mouse events ──────────────────────────────────────────────────────────────

pub fn handle_mouse(event: MouseEvent, state: &mut AppState) -> Result<()> {
    match event.kind {
        // Scroll wheel in logs overlay
        MouseEventKind::ScrollDown => {
            if let Some(ref mut logs) = state.logs_overlay {
                let max = logs.lines.len().saturating_sub(1);
                if logs.scroll < max { logs.scroll += 1; }
            } else if let Some(ref mut form) = state.new_project {
                // Scroll in form: cycle Select fields or move to next field
                if is_select_field(form) { form.select_next(); }
            }
        }
        MouseEventKind::ScrollUp => {
            if let Some(ref mut logs) = state.logs_overlay {
                if logs.scroll > 0 { logs.scroll -= 1; }
            } else if let Some(ref mut form) = state.new_project {
                if is_select_field(form) { form.select_prev(); }
            }
        }
        // Left-click: toggle language button (top-right corner)
        MouseEventKind::Down(_) => {
            // A click on [DE]/[EN] button — approximate position check
            // We look for a click in the top-right 6 columns
            let terminal_width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
            if event.column >= terminal_width.saturating_sub(6) && event.row <= 2 {
                state.lang = state.lang.toggle();
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Podman helpers ────────────────────────────────────────────────────────────

pub fn podman_status(name: &str) -> ServiceStatus {
    let out = std::process::Command::new("podman")
        .args(["inspect", "--format", "{{.State.Status}}", name])
        .output();
    match out {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            match s.trim() {
                "running"           => ServiceStatus::Running,
                "exited" | "stopped" => ServiceStatus::Stopped,
                "error"             => ServiceStatus::Error,
                _                   => ServiceStatus::Unknown,
            }
        }
        Err(_) => ServiceStatus::Unknown,
    }
}

fn fetch_logs(name: &str) -> Vec<String> {
    let out = std::process::Command::new("podman")
        .args(["logs", "--tail", "100", name])
        .output();
    match out {
        Ok(o) => {
            let text = if o.stdout.is_empty() { o.stderr } else { o.stdout };
            String::from_utf8_lossy(&text)
                .lines()
                .map(|l| l.to_string())
                .collect()
        }
        Err(_) => vec!["[Logs nicht verfügbar]".into()],
    }
}
