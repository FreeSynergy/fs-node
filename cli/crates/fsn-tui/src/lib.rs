// fsn-tui — Terminal UI for FreeSynergy.Node.
//
// Entry point: `run(root)` — called by `fsn tui`.
// Detects whether a project exists → Welcome screen or Dashboard.

pub mod app;
pub mod events;
pub mod i18n;
pub mod sysinfo;
pub mod ui;

use std::io;
use std::path::Path;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{AppState, ProjectHandle, RunState, ServiceRow};
use sysinfo::SysInfo;

/// Start the TUI. Blocks until the user quits.
pub fn run(root: &Path) -> Result<()> {
    let sysinfo  = SysInfo::collect();
    let services = load_services(root);
    let projects = load_projects(root);
    let mut state = AppState::new(sysinfo, services, projects);

    // If a project.toml already exists, go straight to Dashboard
    // even when no containers are running yet.
    if state.screen == app::Screen::Welcome && project_toml_exists(root) {
        state.screen = app::Screen::Dashboard;
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = app::run_loop(&mut terminal, &mut state, root);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Load all projects from `root/projects/` using `ProjectConfig::load()`.
pub fn load_projects(root: &Path) -> Vec<ProjectHandle> {
    let projects_dir = root.join("projects");
    if !projects_dir.exists() { return vec![]; }

    let mut projects = Vec::new();
    let Ok(entries) = std::fs::read_dir(&projects_dir) else { return projects; };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }
        let Ok(inner) = std::fs::read_dir(&path) else { continue; };
        for f in inner.flatten() {
            let fp = f.path();
            let is_project_toml = fp.extension().and_then(|e| e.to_str()) == Some("toml")
                && fp.file_stem().and_then(|s| s.to_str())
                    .map(|s| s.ends_with(".project"))
                    .unwrap_or(false);
            if !is_project_toml { continue; }

            let stem = fp.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let slug = stem.strip_suffix(".project").unwrap_or(stem).to_string();

            if let Ok(config) = fsn_core::config::project::ProjectConfig::load(&fp) {
                projects.push(ProjectHandle { slug, toml_path: fp, config });
            }
        }
    }
    projects
}

/// Returns true if any `*.project.toml` exists under `root/projects/`.
fn project_toml_exists(root: &Path) -> bool {
    let projects_dir = root.join("projects");
    if !projects_dir.exists() { return false; }
    let Ok(entries) = std::fs::read_dir(&projects_dir) else { return false; };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }
        let Ok(inner) = std::fs::read_dir(&path) else { continue; };
        for f in inner.flatten() {
            let fp = f.path();
            if fp.extension().and_then(|e| e.to_str()) == Some("toml")
                && fp.file_stem().and_then(|s| s.to_str())
                    .map(|s| s.ends_with(".project"))
                    .unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
}

/// Try to load active services from Podman.
/// Returns empty Vec if no project found — triggers Welcome screen.
fn load_services(root: &Path) -> Vec<ServiceRow> {
    if !project_toml_exists(root) { return vec![]; }
    podman_service_rows()
}

/// Query Podman for all containers and map to `ServiceRow`.
fn podman_service_rows() -> Vec<ServiceRow> {
    let out = std::process::Command::new("podman")
        .args(["ps", "-a", "--format", "{{.Names}}|{{.Status}}"])
        .output();

    let Ok(output) = out else { return vec![] };
    let text = String::from_utf8_lossy(&output.stdout);

    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '|');
            let name   = parts.next()?.trim().to_string();
            let status = parts.next().unwrap_or("").trim();
            if name.is_empty() { return None; }

            let run_state = if status.starts_with("Up") {
                RunState::Running
            } else if status.starts_with("Exited") {
                RunState::Stopped
            } else {
                RunState::Missing
            };

            Some(ServiceRow {
                domain:       format!("{}.example.com", name),
                service_type: "custom".into(),
                name,
                status: run_state,
            })
        })
        .collect()
}
