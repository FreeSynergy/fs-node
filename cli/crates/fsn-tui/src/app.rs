// Application state and main event loop.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use fsn_core::config::project::ProjectConfig;
pub use fsn_core::state::actual::RunState;

use crate::sysinfo::SysInfo;
use crate::ui;

// ── Screens ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    Dashboard,
    NewProject,
}

// ── Dashboard focus ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashFocus {
    Sidebar,
    Services,
}

// ── Language ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
}

impl Lang {
    pub fn toggle(self) -> Self {
        match self { Lang::De => Lang::En, Lang::En => Lang::De }
    }
    pub fn label(self) -> &'static str {
        match self { Lang::De => "DE", Lang::En => "EN" }
    }
}

// ── Project handle (loaded from disk) ─────────────────────────────────────────

/// A project loaded from `projects/{slug}/{slug}.project.toml`.
/// `config` is the parsed `ProjectConfig` from fsn-core.
/// `slug` and `toml_path` are TUI-level metadata not stored inside the TOML.
#[derive(Debug, Clone)]
pub struct ProjectHandle {
    pub slug:      String,
    pub toml_path: std::path::PathBuf,
    pub config:    ProjectConfig,
}

impl ProjectHandle {
    /// Convenience: project display name.
    pub fn name(&self) -> &str { &self.config.project.name }
    /// Convenience: primary domain.
    pub fn domain(&self) -> &str { &self.config.project.domain }
    /// Convenience: contact e-mail (first non-empty of email / acme_email).
    pub fn email(&self) -> &str {
        self.config.project.contact.as_ref()
            .and_then(|c| c.email.as_deref().or(c.acme_email.as_deref()))
            .unwrap_or("")
    }
    /// Convenience: install directory.
    pub fn install_dir(&self) -> &str {
        self.config.project.install_dir.as_deref().unwrap_or("")
    }
}

// ── Service table ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ServiceRow {
    pub name:         String,
    pub service_type: String,
    pub domain:       String,
    pub status:       RunState,
}

/// Map `RunState` to an i18n key for the status column.
pub fn run_state_i18n(state: RunState) -> &'static str {
    match state {
        RunState::Running => "status.running",
        RunState::Stopped => "status.stopped",
        RunState::Failed  => "status.error",
        RunState::Missing => "status.unknown",
    }
}

// ── New Project form ──────────────────────────────────────────────────────────

/// Which tab is active in the New Project form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormTab {
    Project = 0,
    Options = 1,
}

impl FormTab {
    pub fn from_index(i: usize) -> Self {
        match i { 1 => FormTab::Options, _ => FormTab::Project }
    }
    pub fn count() -> usize { 2 }
    pub fn i18n_key(self) -> &'static str {
        match self {
            FormTab::Project => "form.tab.project",
            FormTab::Options => "form.tab.options",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormFieldType {
    Text,
    Email,
    Ip,
    Secret,
    Path,
    Select,  // uses `options` list
}

#[derive(Debug, Clone)]
pub struct FormField {
    /// Config key (written to project.toml / host.toml)
    pub key:        &'static str,
    /// i18n key for the label
    pub label_key:  &'static str,
    /// i18n key for the description / hint below the field
    pub hint_key:   Option<&'static str>,
    pub tab:        FormTab,
    pub required:   bool,
    pub field_type: FormFieldType,
    /// Current text value (or selected option index as string for Select)
    pub value:      String,
    /// Cursor position within `value`
    pub cursor:     usize,
    /// True once the user has manually edited this field (disables auto-fill).
    pub dirty:      bool,
    /// For Select fields — available options (i18n keys or static strings)
    pub options:    Vec<&'static str>,
}

impl FormField {
    fn new(key: &'static str, label_key: &'static str, tab: FormTab, required: bool, field_type: FormFieldType) -> Self {
        Self { key, label_key, hint_key: None, tab, required, field_type,
               value: String::new(), cursor: 0, dirty: false, options: vec![] }
    }
    fn hint(mut self, k: &'static str) -> Self { self.hint_key = Some(k); self }
    fn default_val(mut self, v: &str) -> Self { self.value = v.to_string(); self.cursor = v.len(); self }
    fn opts(mut self, o: Vec<&'static str>) -> Self { self.options = o; self }
    fn dirty(mut self) -> Self { self.dirty = true; self }
}

#[derive(Debug)]
pub struct NewProjectForm {
    pub active_tab:   usize,        // 0..FormTab::count()-1
    pub active_field: usize,        // index into `fields` filtered by active_tab
    pub fields:       Vec<FormField>,
    pub error:        Option<String>,
    /// When Some(slug), editing an existing project (overwrite on submit).
    pub edit_slug:    Option<String>,
}

impl NewProjectForm {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".into());
        let fields = vec![
            // ── Tab: Project ──────────────────────────────────────────────────
            FormField::new("name",          "form.project.name",        FormTab::Project, true,  FormFieldType::Text)
                .hint("form.project.name.hint"),
            FormField::new("domain",        "form.project.domain",      FormTab::Project, true,  FormFieldType::Text)
                .hint("form.project.domain.hint"),
            FormField::new("description",   "form.project.description", FormTab::Project, false, FormFieldType::Text)
                .hint("form.project.description.hint"),
            FormField::new("contact_email", "form.project.email",       FormTab::Project, true,  FormFieldType::Email)
                .hint("form.project.email.hint"),

            // ── Tab: Options ──────────────────────────────────────────────────
            FormField::new("language", "form.options.language",  FormTab::Options, false, FormFieldType::Select)
                .opts(vec!["de", "en", "fr", "es", "it", "pt"])
                .default_val("de"),
            FormField::new("path",     "form.project.path",      FormTab::Options, true,  FormFieldType::Path)
                .default_val(&format!("{}/fsn", home))
                .hint("form.project.path.hint"),
            FormField::new("version",  "form.options.version",   FormTab::Options, false, FormFieldType::Text)
                .default_val("0.1.0"),
        ];

        Self { active_tab: 0, active_field: 0, fields, error: None, edit_slug: None }
    }

    /// Create a pre-filled edit form from an existing project.
    pub fn from_project(handle: &ProjectHandle) -> Self {
        let p = &handle.config.project;
        let desc = p.description.as_deref().unwrap_or("");
        let fields = vec![
            FormField::new("name",          "form.project.name",        FormTab::Project, true,  FormFieldType::Text)
                .hint("form.project.name.hint").default_val(&p.name).dirty(),
            FormField::new("domain",        "form.project.domain",      FormTab::Project, true,  FormFieldType::Text)
                .hint("form.project.domain.hint").default_val(&p.domain).dirty(),
            FormField::new("description",   "form.project.description", FormTab::Project, false, FormFieldType::Text)
                .hint("form.project.description.hint").default_val(desc).dirty(),
            FormField::new("contact_email", "form.project.email",       FormTab::Project, true,  FormFieldType::Email)
                .hint("form.project.email.hint").default_val(handle.email()).dirty(),
            FormField::new("language", "form.options.language",  FormTab::Options, false, FormFieldType::Select)
                .opts(vec!["de", "en", "fr", "es", "it", "pt"]).default_val(&p.language).dirty(),
            FormField::new("path",     "form.project.path",      FormTab::Options, true,  FormFieldType::Path)
                .hint("form.project.path.hint").default_val(handle.install_dir()).dirty(),
            FormField::new("version",  "form.options.version",   FormTab::Options, false, FormFieldType::Text)
                .default_val(&p.version).dirty(),
        ];
        Self { active_tab: 0, active_field: 0, fields, error: None, edit_slug: Some(handle.slug.clone()) }
    }

    /// Indices of fields belonging to the active tab.
    pub fn tab_field_indices(&self) -> Vec<usize> {
        let tab = FormTab::from_index(self.active_tab);
        self.fields.iter().enumerate()
            .filter(|(_, f)| f.tab == tab)
            .map(|(i, _)| i)
            .collect()
    }

    /// The currently focused field (global index).
    pub fn focused_field_idx(&self) -> Option<usize> {
        let indices = self.tab_field_indices();
        indices.get(self.active_field).copied()
    }

    /// Move focus to next field in tab; returns true if wrapped (stay in tab).
    pub fn focus_next(&mut self) {
        let count = self.tab_field_indices().len();
        if count == 0 { return; }
        self.active_field = (self.active_field + 1) % count;
    }

    pub fn focus_prev(&mut self) {
        let count = self.tab_field_indices().len();
        if count == 0 { return; }
        self.active_field = self.active_field.checked_sub(1).unwrap_or(count - 1);
    }

    pub fn next_tab(&mut self) {
        self.active_tab = (self.active_tab + 1) % FormTab::count();
        self.active_field = 0;
    }
    pub fn prev_tab(&mut self) {
        self.active_tab = self.active_tab.checked_sub(1).unwrap_or(FormTab::count() - 1);
        self.active_field = 0;
    }

    /// Insert char at cursor of focused field.
    pub fn insert_char(&mut self, c: char) {
        if let Some(idx) = self.focused_field_idx() {
            {
                let f = &mut self.fields[idx];
                f.value.insert(f.cursor, c);
                f.cursor += c.len_utf8();
                f.dirty = true;
            }
            self.on_field_changed(idx);
        }
    }

    /// Delete char before cursor.
    pub fn backspace(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let changed = {
                let f = &mut self.fields[idx];
                if f.cursor > 0 {
                    let prev = f.value[..f.cursor]
                        .char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
                    f.value.remove(prev);
                    f.cursor = prev;
                    f.dirty = true;
                    true
                } else { false }
            };
            if changed { self.on_field_changed(idx); }
        }
    }

    pub fn cursor_left(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            if f.cursor > 0 {
                f.cursor = f.value[..f.cursor]
                    .char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
            }
        }
    }

    pub fn cursor_right(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            if f.cursor < f.value.len() {
                let next = f.value[f.cursor..].chars().next().map(|c| f.cursor + c.len_utf8()).unwrap_or(f.cursor);
                f.cursor = next;
            }
        }
    }

    /// Delete char at cursor position (forward delete).
    pub fn delete_char(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let changed = {
                let f = &mut self.fields[idx];
                if f.cursor < f.value.len() {
                    let next = f.value[f.cursor..].chars().next()
                        .map(|c| f.cursor + c.len_utf8())
                        .unwrap_or(f.cursor);
                    f.value.drain(f.cursor..next);
                    f.dirty = true;
                    true
                } else { false }
            };
            if changed { self.on_field_changed(idx); }
        }
    }

    pub fn cursor_home(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            self.fields[idx].cursor = 0;
        }
    }

    pub fn cursor_end(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            f.cursor = f.value.len();
        }
    }

    /// Cycle a Select field's value forward.
    pub fn select_next(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            if matches!(f.field_type, FormFieldType::Select) && !f.options.is_empty() {
                let cur = f.options.iter().position(|&o| o == f.value).unwrap_or(0);
                let next = (cur + 1) % f.options.len();
                f.value = f.options[next].to_string();
            }
        }
    }

    /// Cycle a Select field's value backward.
    pub fn select_prev(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            if matches!(f.field_type, FormFieldType::Select) && !f.options.is_empty() {
                let cur = f.options.iter().position(|&o| o == f.value).unwrap_or(0);
                let prev = if cur == 0 { f.options.len() - 1 } else { cur - 1 };
                f.value = f.options[prev].to_string();
            }
        }
    }

    /// Called after any user edit — propagates smart defaults to dependent fields.
    fn on_field_changed(&mut self, idx: usize) {
        match self.fields[idx].key {
            "name" => {
                let slug = slugify(&self.fields[idx].value.clone());
                if let Some(d) = self.fields.iter().position(|f| f.key == "domain") {
                    if !self.fields[d].dirty {
                        let len = slug.len();
                        self.fields[d].value = slug;
                        self.fields[d].cursor = len;
                    }
                }
                self.sync_email_from_domain();
            }
            "domain" => self.sync_email_from_domain(),
            _ => {}
        }
    }

    fn sync_email_from_domain(&mut self) {
        let domain = self.fields.iter()
            .find(|f| f.key == "domain")
            .map(|f| f.value.clone())
            .unwrap_or_default();
        if domain.is_empty() { return; }
        if let Some(e) = self.fields.iter().position(|f| f.key == "contact_email") {
            if !self.fields[e].dirty {
                let email = format!("admin@{}", domain);
                let len = email.len();
                self.fields[e].value = email;
                self.fields[e].cursor = len;
            }
        }
    }

    /// How many required fields on the given tab are still empty.
    pub fn tab_missing_count(&self, tab_idx: usize) -> usize {
        let tab = FormTab::from_index(tab_idx);
        self.fields.iter()
            .filter(|f| f.tab == tab && f.required && f.value.trim().is_empty())
            .count()
    }

    /// Get the current value of a field by config key.
    pub fn field_value(&self, key: &str) -> String {
        self.fields.iter()
            .find(|f| f.key == key)
            .map(|f| f.value.clone())
            .unwrap_or_default()
    }

    /// Set a Select field's value by option index (for mouse click on dropdown).
    pub fn set_select_by_index(&mut self, option_idx: usize) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            if matches!(f.field_type, FormFieldType::Select) && option_idx < f.options.len() {
                f.value = f.options[option_idx].to_string();
            }
        }
    }

    /// Validate — returns list of missing required fields (all tabs).
    pub fn missing_required(&self) -> Vec<&'static str> {
        self.fields.iter()
            .filter(|f| f.required && f.value.trim().is_empty())
            .map(|f| f.label_key)
            .collect()
    }
}

// ── Slugify helper ────────────────────────────────────────────────────────────

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        match c {
            'a'..='z' | '0'..='9' | '.' => out.push(c),
            ' ' | '_' | '-' => { if !out.ends_with('-') { out.push('-'); } }
            _ => {}
        }
    }
    out.trim_matches('-').to_string()
}

// ── Full application state ────────────────────────────────────────────────────

pub struct AppState {
    pub screen:             Screen,
    pub lang:               Lang,
    pub sysinfo:            SysInfo,
    pub services:           Vec<ServiceRow>,
    pub selected:           usize,
    pub logs_overlay:       Option<LogsState>,
    pub lang_dropdown_open: bool,
    pub should_quit:        bool,
    /// Focused button on welcome screen (0=New, 1=Open)
    pub welcome_focus:      usize,
    pub new_project:        Option<NewProjectForm>,
    /// True when last keypress included CONTROL — switches hint bar to Ctrl shortcuts.
    pub ctrl_hint:          bool,
    /// Loaded projects from disk.
    pub projects:           Vec<ProjectHandle>,
    pub selected_project:   usize,
    pub dash_focus:         DashFocus,
    /// True = waiting for delete-confirm (J/N).
    pub dash_confirm:       bool,
    last_refresh:           Instant,
}

#[derive(Debug, Clone)]
pub struct LogsState {
    pub service_name: String,
    pub lines:        Vec<String>,
    pub scroll:       usize,
}

impl AppState {
    pub fn new(sysinfo: SysInfo, services: Vec<ServiceRow>, projects: Vec<ProjectHandle>) -> Self {
        let screen = if services.is_empty() { Screen::Welcome } else { Screen::Dashboard };
        Self {
            screen, lang: Lang::De, sysinfo, services,
            selected: 0, logs_overlay: None, lang_dropdown_open: false,
            should_quit: false, welcome_focus: 0, new_project: None,
            ctrl_hint: false, projects, selected_project: 0,
            dash_focus: DashFocus::Sidebar, dash_confirm: false,
            last_refresh: Instant::now(),
        }
    }

    pub fn t<'a>(&self, key: &'a str) -> &'a str {
        crate::i18n::t(self.lang, key)
    }
}

// ── Main loop ─────────────────────────────────────────────────────────────────

pub fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    state:    &mut AppState,
    root:     &Path,
) -> Result<()> {
    const POLL_MS:      u64 = 250;
    const REFRESH_SECS: u64 = 5;

    loop {
        terminal.draw(|f| ui::render(f, state))?;

        if event::poll(Duration::from_millis(POLL_MS))? {
            match event::read()? {
                Event::Key(key) => crate::events::handle(key, state, root)?,
                Event::Mouse(mouse) => crate::events::handle_mouse(mouse, state)?,
                _ => {}
            }
        }

        if state.should_quit { break; }

        if state.last_refresh.elapsed() >= Duration::from_secs(REFRESH_SECS) {
            state.sysinfo = SysInfo::collect();
            state.last_refresh = Instant::now();
        }
    }

    Ok(())
}
