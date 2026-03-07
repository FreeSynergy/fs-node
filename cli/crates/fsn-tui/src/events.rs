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
        Screen::NewProject => handle_new_project(key, state, root),
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

fn handle_new_project(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
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
            // Determine action without holding a mutable borrow
            let action = state.new_project.as_ref().map(|form| {
                let is_last   = form.active_tab == FormTab::count() - 1;
                let missing_t = form.tab_missing_count(form.active_tab);
                if missing_t > 0 {
                    FormAction::Error(format!(
                        "{} {}",
                        missing_t,
                        if missing_t == 1 { "Pflichtfeld fehlt" } else { "Pflichtfelder fehlen" },
                    ))
                } else if is_last {
                    let missing = form.missing_required();
                    if missing.is_empty() { FormAction::Submit }
                    else { FormAction::Error(format!("{} Pflichtfeld(er) auf anderen Tabs fehlen", missing.len())) }
                } else {
                    FormAction::NextTab
                }
            });

            match action {
                Some(FormAction::Error(msg)) => {
                    if let Some(ref mut form) = state.new_project { form.error = Some(msg); }
                }
                Some(FormAction::NextTab) => {
                    if let Some(ref mut form) = state.new_project { form.error = None; form.next_tab(); }
                }
                Some(FormAction::Submit) => {
                    submit_project(state, root)?;
                }
                None => {}
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
    let (tw, _th) = crossterm::terminal::size().unwrap_or((80, 24));

    match event.kind {
        MouseEventKind::ScrollDown => {
            if let Some(ref mut logs) = state.logs_overlay {
                let max = logs.lines.len().saturating_sub(1);
                if logs.scroll < max { logs.scroll += 1; }
            } else if let Some(ref mut form) = state.new_project {
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
        MouseEventKind::Down(_) => {
            // Language button — top-right 6 columns
            if event.column >= tw.saturating_sub(6) && event.row <= 2 {
                state.lang = state.lang.toggle();
                return Ok(());
            }

            // Form: click on dropdown option
            if state.screen == Screen::NewProject {
                if let Some(opt_idx) = find_clicked_dropdown(event.column, event.row, state.new_project.as_ref(), tw) {
                    if let Some(ref mut form) = state.new_project {
                        form.set_select_by_index(opt_idx);
                    }
                    return Ok(());
                }
                // Form: click on a field → focus it
                if let Some(slot) = find_clicked_field(event.column, event.row, state.new_project.as_ref(), tw) {
                    if let Some(ref mut form) = state.new_project {
                        form.active_field = slot;
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Layout helpers for mouse hit-testing ─────────────────────────────────────
//
// These reproduce the layout from ui/new_project.rs so we don't need to store
// Rects in state (which would require &mut AppState in render functions).

/// Returns the slot index of the form field the user clicked on (active tab only).
fn find_clicked_field(col: u16, row: u16, form: Option<&NewProjectForm>, tw: u16) -> Option<usize> {
    let form = form?;
    let pad_x   = tw * 5 / 100;
    let inner_x = pad_x;
    let inner_w = tw - 2 * pad_x;
    let fields_y = 6u16;  // header(3) + tabs(3)

    if col < inner_x || col >= inner_x + inner_w { return None; }

    let indices = form.tab_field_indices();
    for (slot, _) in indices.iter().enumerate() {
        let field_top = fields_y + slot as u16 * 5;
        let field_bot = field_top + 5;
        if row >= field_top && row < field_bot {
            return Some(slot);
        }
    }
    None
}

/// Returns the option index if the user clicked inside an open dropdown.
fn find_clicked_dropdown(col: u16, row: u16, form: Option<&NewProjectForm>, tw: u16) -> Option<usize> {
    let form = form?;
    let idx   = form.focused_field_idx()?;
    let field = &form.fields[idx];
    if !matches!(field.field_type, crate::app::FormFieldType::Select) { return None; }

    let pad_x    = tw * 5 / 100;
    let inner_x  = pad_x;
    let inner_w  = tw - 2 * pad_x;
    let fields_y = 6u16;

    if col < inner_x || col >= inner_x + inner_w { return None; }

    // Input box: label(1) + input(3) → dropdown starts at field_y + 4
    let field_y    = fields_y + form.active_field as u16 * 5;
    let dropdown_y = field_y + 4;  // below input box

    // Items start at dropdown_y + 1 (inside border)
    if row > dropdown_y && row <= dropdown_y + field.options.len() as u16 {
        let opt_idx = (row - dropdown_y - 1) as usize;
        if opt_idx < field.options.len() {
            return Some(opt_idx);
        }
    }
    None
}

// ── Form submit ───────────────────────────────────────────────────────────────

enum FormAction {
    Error(String),
    NextTab,
    Submit,
}

fn submit_project(state: &mut AppState, root: &Path) -> Result<()> {
    // Collect data while immutably borrowing form
    let result = {
        let form = state.new_project.as_ref().unwrap();
        write_project_to_disk(form, root)
    };

    match result {
        Ok(()) => {
            state.screen = Screen::Dashboard;
            state.new_project = None;
        }
        Err(e) => {
            if let Some(ref mut form) = state.new_project {
                form.error = Some(format!("{}", e));
            }
        }
    }
    Ok(())
}

fn write_project_to_disk(form: &crate::app::NewProjectForm, root: &Path) -> anyhow::Result<()> {
    let name = form.field_value("name");
    let slug = crate::app::slugify(&name);

    if slug.is_empty() {
        return Err(anyhow::anyhow!("Projektname ist ungültig (leer nach Bereinigung)"));
    }

    let project_dir = root.join("projects").join(&slug);
    std::fs::create_dir_all(&project_dir)?;

    let toml_path = project_dir.join(format!("{}.project.toml", slug));
    if toml_path.exists() {
        // Project already exists — don't overwrite, just go to dashboard
        return Ok(());
    }

    // Simple TOML escaping (replace \ and " in string values)
    let ts = |s: String| -> String {
        format!("\"{}\"", s.replace('\\', r"\\").replace('"', "\\\""))
    };

    let content = format!(
        "[project]\nname        = {}\ndomain      = {}\ndescription = {}\nemail       = {}\nlanguage    = {}\nversion     = {}\npath        = {}\n",
        ts(form.field_value("name")),
        ts(form.field_value("domain")),
        ts(form.field_value("description")),
        ts(form.field_value("contact_email")),
        ts(form.field_value("language")),
        ts(form.field_value("version")),
        ts(form.field_value("path")),
    );

    std::fs::write(toml_path, content)?;
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
