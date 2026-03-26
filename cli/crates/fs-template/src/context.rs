// fs-template — TemplateContext

use std::collections::HashMap;

use indexmap::IndexMap;
use serde::Serialize;

use fs_error::FsError;

/// Builder for a Tera template rendering context.
///
/// Variables are stored as JSON values and converted to `tera::Context` on render.
#[derive(Debug, Clone, Default)]
pub struct TemplateContext {
    vars: IndexMap<String, serde_json::Value>,
}

impl TemplateContext {
    /// Create an empty context.
    pub fn new() -> Self {
        Self {
            vars: IndexMap::new(),
        }
    }

    /// Insert a string variable.
    pub fn set_str(&mut self, key: impl Into<String>, val: impl Into<String>) -> &mut Self {
        self.vars
            .insert(key.into(), serde_json::Value::String(val.into()));
        self
    }

    /// Insert a boolean variable.
    pub fn set_bool(&mut self, key: impl Into<String>, val: bool) -> &mut Self {
        self.vars.insert(key.into(), serde_json::Value::Bool(val));
        self
    }

    /// Insert an integer variable.
    pub fn set_i64(&mut self, key: impl Into<String>, val: i64) -> &mut Self {
        self.vars.insert(
            key.into(),
            serde_json::Value::Number(serde_json::Number::from(val)),
        );
        self
    }

    /// Insert a u64 variable.
    pub fn set_u64(&mut self, key: impl Into<String>, val: u64) -> &mut Self {
        self.vars.insert(
            key.into(),
            serde_json::Value::Number(serde_json::Number::from(val)),
        );
        self
    }

    /// Insert any serializable value (structs, vecs, maps).
    pub fn set<T: Serialize>(
        &mut self,
        key: impl Into<String>,
        val: &T,
    ) -> Result<&mut Self, FsError> {
        let json = serde_json::to_value(val)
            .map_err(|e| FsError::internal(format!("context serialization error: {e}")))?;
        self.vars.insert(key.into(), json);
        Ok(self)
    }

    /// Merge a flat `HashMap<String, String>` into the context.
    ///
    /// Existing keys are overwritten.
    pub fn merge_str_map(&mut self, map: &HashMap<String, String>) -> &mut Self {
        for (k, v) in map {
            self.vars
                .insert(k.clone(), serde_json::Value::String(v.clone()));
        }
        self
    }

    /// Return `true` when `key` is set in this context.
    pub fn contains_key(&self, key: &str) -> bool {
        self.vars.contains_key(key)
    }

    /// Convert to a `tera::Context` for rendering.
    pub(crate) fn to_tera(&self) -> tera::Context {
        let mut ctx = tera::Context::new();
        for (key, val) in &self.vars {
            ctx.insert(key, val);
        }
        ctx
    }
}
