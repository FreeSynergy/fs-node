// Module registry – scans modules/ directory and loads all module class TOMLs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::config::module::ModuleClass;
use crate::error::FsnError;

/// In-memory index of all available module classes.
/// Key = "{type}/{name}" (e.g. "auth/kanidm", "git/forgejo")
#[derive(Debug, Default)]
pub struct ModuleRegistry {
    classes: HashMap<String, ModuleClass>,
    /// Base path of the modules/ directory
    modules_dir: PathBuf,
}

impl ModuleRegistry {
    /// Scan a modules/ directory and load all class TOMLs.
    /// Expected layout: modules/{type}/{name}/{name}.toml
    pub fn load(modules_dir: &Path) -> Result<Self, FsnError> {
        let mut registry = Self {
            classes: HashMap::new(),
            modules_dir: modules_dir.to_path_buf(),
        };

        for entry in WalkDir::new(modules_dir)
            .min_depth(3) // skip modules/ and type dirs
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }

            // Extract module class key from path: modules/{type}/{name}/{name}.toml
            // Verify the file name matches the parent directory name
            let file_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            let parent_name = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or_default();

            if file_stem != parent_name {
                continue; // skip hooks, templates, etc.
            }

            // Build the class key: {type}/{name}
            let type_name = path
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or_default();

            let class_key = format!("{}/{}", type_name, file_stem);

            match Self::load_class(path) {
                Ok(class) => {
                    registry.classes.insert(class_key, class);
                }
                Err(e) => {
                    eprintln!("Warning: skipping {}: {}", path.display(), e);
                }
            }
        }

        Ok(registry)
    }

    fn load_class(path: &Path) -> Result<ModuleClass, FsnError> {
        let content = std::fs::read_to_string(path).map_err(FsnError::Io)?;
        toml::from_str(&content).map_err(|e| FsnError::ConfigParse {
            path: path.display().to_string(),
            source: e,
        })
    }

    /// Look up a module class by its "{type}/{name}" key.
    pub fn get(&self, class_key: &str) -> Option<&ModuleClass> {
        self.classes.get(class_key)
    }

    /// All loaded module classes.
    pub fn all(&self) -> impl Iterator<Item = (&str, &ModuleClass)> {
        self.classes.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn modules_dir(&self) -> &Path {
        &self.modules_dir
    }
}
