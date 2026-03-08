// schema_form – build a Vec<Box<dyn FormNode>> from a FormSchema.
//
// This is the Ratatui renderer bridge: it reads the static FormSchema
// produced by #[derive(Form)] and instantiates the correct FormNode
// implementation for each field.
//
// Usage:
//   let nodes = schema_form::build_nodes(MyForm::schema(), &prefill, &display_fns, &dynamics);
//   let form  = ResourceForm::new(kind, TABS, nodes, edit_id, on_change);
//
// This replaces all the manual `vec![Box::new(TextInputNode::new(...))]` boilerplate.

use std::collections::HashMap;

use fsn_form::{FieldMeta, FormSchema, WidgetType};

use crate::ui::form_node::FormNode;
use crate::ui::nodes::{SelectInputNode, TextInputNode};

/// Build form nodes from a static schema.
///
/// # Arguments
/// * `schema`       — Static `FormSchema` from `YourForm::schema()`
/// * `prefill`      — Field-key → value map for edit forms (empty for new forms)
/// * `display_fns`  — Optional human-label mappers for `Select` fields
///                    (key → fn(option_code) -> display_label)
/// * `dynamics`     — Runtime-computed default values (override schema `default_val`)
///                    e.g. `[("install_dir", "$HOME/fsn")]`
pub fn build_nodes(
    schema:      &FormSchema,
    prefill:     &HashMap<&str, &str>,
    display_fns: &[(&'static str, fn(&str) -> &'static str)],
    dynamics:    &[(&str, String)],
) -> Vec<Box<dyn FormNode>> {
    schema.fields.iter().map(|field| build_node(field, prefill, display_fns, dynamics)).collect()
}

fn build_node(
    field:       &FieldMeta,
    prefill:     &HashMap<&str, &str>,
    display_fns: &[(&'static str, fn(&str) -> &'static str)],
    dynamics:    &[(&str, String)],
) -> Box<dyn FormNode> {
    // Resolve value: prefill > dynamic default > schema default
    let pre_val: Option<&str> = prefill.get(field.key).copied();
    let dyn_val: Option<&str> = dynamics.iter().find(|(k, _)| *k == field.key).map(|(_, v)| v.as_str());

    match field.widget {
        WidgetType::Select => {
            // Select: options come from the schema; display fn is registered separately
            let display_fn = display_fns.iter().find(|(k, _)| *k == field.key).map(|(_, f)| *f);
            let mut node = SelectInputNode::new(
                field.key,
                field.label_key,
                field.tab,
                field.required,
                field.options.clone(),
            );
            if let Some(f) = display_fn { node = node.display(f); }
            // Priority: prefill > dynamic > schema default
            if let Some(v) = pre_val.or(dyn_val).or(field.default_val) {
                node = node.default_val(v);
            }
            Box::new(node)
        }

        WidgetType::Password => {
            let mut node = TextInputNode::new(field.key, field.label_key, field.tab, field.required)
                .secret();
            if let Some(h) = field.hint_key { node = node.hint(h); }
            node = apply_value(node, pre_val, dyn_val, field.default_val);
            Box::new(node)
        }

        // Text, Email, IpAddress, Number, Toggle — all use TextInputNode for now.
        // Future: dedicated nodes per widget type (DateNode, ToggleNode, …).
        _ => {
            let mut node = TextInputNode::new(field.key, field.label_key, field.tab, field.required);
            if let Some(h) = field.hint_key { node = node.hint(h); }
            if let Some(n) = field.max_len  { node = node.max_len(n); }
            node = apply_value(node, pre_val, dyn_val, field.default_val);
            Box::new(node)
        }
    }
}

/// Apply the highest-priority value to a TextInputNode (pre-fill > dynamic > schema default).
fn apply_value(
    node:        TextInputNode,
    pre_val:     Option<&str>,
    dyn_val:     Option<&str>,
    schema_def:  Option<&'static str>,
) -> TextInputNode {
    if let Some(v) = pre_val {
        // Non-empty pre-fill → edit mode; marks the field as dirty so
        // on_change hooks won't overwrite it.
        node.pre_filled(v)
    } else if let Some(v) = dyn_val {
        // Runtime default (e.g. $HOME/fsn) — not yet dirty.
        node.default_val(v)
    } else if let Some(v) = schema_def {
        // Static schema default (e.g. "0.1.0").
        node.default_val(v)
    } else {
        node
    }
}
