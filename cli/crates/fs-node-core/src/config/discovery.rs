// Config file discovery — find project and host TOML files by scanning
// the FSN directory layout.
//
// These helpers are used by multiple CLI commands (deploy, config, init, sync).
// They are pure filesystem operations: no I/O beyond directory iteration.

use std::path::{Path, PathBuf};

use crate::config::HostConfig;

// ── ConfigDiscovery ───────────────────────────────────────────────────────────

pub struct ConfigDiscovery<'a> {
    root: &'a Path,
}

impl<'a> ConfigDiscovery<'a> {
    pub fn new(root: &'a Path) -> Self {
        Self { root }
    }

    /// Find the project config file.
    ///
    /// If `explicit` is provided it is returned as-is.
    /// Otherwise scans `{root}/projects/**/*.project.toml` and returns the first match.
    pub fn find_project(&self, explicit: Option<&Path>) -> Option<PathBuf> {
        if let Some(p) = explicit {
            return Some(p.to_path_buf());
        }
        self.scan_project_files()
            .find(|p| p.to_string_lossy().ends_with(".project.toml"))
    }

    /// Find the host config file.
    ///
    /// Search order:
    ///   1. `{root}/projects/**/*.host.toml`   (TUI layout)
    ///   2. `{root}/hosts/*.host.toml`          (legacy layout)
    ///
    /// `example.host.toml` files are always ignored.
    pub fn find_host(&self) -> Option<PathBuf> {
        self.find_host_by(Self::is_real_host_toml)
    }

    /// Find a host config file whose `[host].name` field (or filename prefix)
    /// matches `host_name`.
    pub fn find_host_by_name(&self, host_name: &str) -> Option<PathBuf> {
        // 1. Projects tree
        let projects_dir = self.root.join("projects");
        for proj_dir in Self::read_subdirs(&projects_dir) {
            for path in Self::read_dir_files(&proj_dir) {
                let fname = Self::file_name(&path);
                if !Self::is_real_host_toml(fname) {
                    continue;
                }
                if fname.starts_with(&format!("{host_name}.")) {
                    return Some(path);
                }
                if let Ok(h) = HostConfig::load(&path) {
                    if h.host.name() == host_name {
                        return Some(path);
                    }
                }
            }
        }

        // 2. Legacy hosts/ directory
        let hosts_dir = self.root.join("hosts");
        for path in Self::read_dir_files(&hosts_dir) {
            let fname = Self::file_name(&path);
            if Self::is_real_host_toml(fname) && fname.starts_with(&format!("{host_name}.")) {
                return Some(path);
            }
        }

        None
    }

    fn find_host_by<F>(&self, pred: F) -> Option<PathBuf>
    where
        F: Fn(&str) -> bool,
    {
        let projects_dir = self.root.join("projects");
        for proj_dir in Self::read_subdirs(&projects_dir) {
            if let Some(path) = Self::read_dir_files(&proj_dir).find(|p| pred(Self::file_name(p))) {
                return Some(path);
            }
        }

        let hosts_dir = self.root.join("hosts");
        Self::read_dir_files(&hosts_dir).find(|p| pred(Self::file_name(p)))
    }

    fn scan_project_files(&self) -> impl Iterator<Item = PathBuf> {
        Self::read_subdirs(&self.root.join("projects"))
            .flat_map(|d| Self::read_dir_files(&d).collect::<Vec<_>>())
    }

    fn is_real_host_toml(fname: &str) -> bool {
        fname.ends_with(".host.toml") && fname != "example.host.toml"
    }

    fn read_subdirs(dir: &Path) -> impl Iterator<Item = PathBuf> {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
    }

    fn read_dir_files(dir: &Path) -> impl Iterator<Item = PathBuf> {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
    }

    fn file_name(path: &Path) -> &str {
        path.file_name().and_then(|n| n.to_str()).unwrap_or("")
    }
}

// ── Public shims (used by config/mod.rs re-exports and existing callers) ─────

pub fn find_project(root: &Path, explicit: Option<&Path>) -> Option<PathBuf> {
    ConfigDiscovery::new(root).find_project(explicit)
}

pub fn find_host(root: &Path) -> Option<PathBuf> {
    ConfigDiscovery::new(root).find_host()
}

pub fn find_host_by_name(root: &Path, host_name: &str) -> Option<PathBuf> {
    ConfigDiscovery::new(root).find_host_by_name(host_name)
}
