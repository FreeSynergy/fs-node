// Sidebar item — navigable rows in the left sidebar panel.
//
// Pattern: Composite (variant-based) — each SidebarItem variant carries its
// own data and knows its visual appearance, selectable status, context actions,
// and delete-confirm parameters. dashboard.rs composes items without branching.
//
// Single Source of Truth: context_actions() and delete_confirm() are the
// only places that declare what actions exist per item type.

use fsn_core::health::HealthLevel;
use crate::app::overlay::{ConfirmAction, ContextAction};
use crate::handles::RunState;

// ── SidebarAction ─────────────────────────────────────────────────────────────

/// The action triggered when a sidebar item is activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction { NewProject, NewHost, NewService }

// ── SidebarItem ───────────────────────────────────────────────────────────────

/// One navigable row in the sidebar.
///
/// Analogous to a DOM element — each variant knows its own visual appearance
/// and the action it triggers when selected.
#[derive(Debug, Clone)]
pub enum SidebarItem {
    Section(&'static str),
    /// A project entry — includes a pre-computed health level for the sidebar indicator.
    Project { slug: String, name: String, health: HealthLevel },
    /// A host entry — includes a pre-computed health level for the sidebar indicator.
    Host    { slug: String, name: String, health: HealthLevel },
    Service { name: String, class: String, status: RunState },
    Action  { label_key: &'static str, kind: SidebarAction },
}

impl SidebarItem {
    pub fn is_selectable(&self) -> bool {
        !matches!(self, SidebarItem::Section(_))
    }
    pub fn action_kind(&self) -> Option<SidebarAction> {
        if let SidebarItem::Action { kind, .. } = self { Some(*kind) } else { None }
    }
    pub fn hint_key(&self) -> &'static str {
        match self {
            SidebarItem::Host    { .. } => "dash.hint.host",
            SidebarItem::Service { .. } => "dash.hint.service",
            _                           => "dash.hint",
        }
    }

    /// Context menu actions available for this item type.
    ///
    /// Single source of truth — to add/remove actions per type: edit only here.
    /// Called by mouse.rs right-click handler; no duplicate lists anywhere else.
    pub fn context_actions(&self) -> Vec<ContextAction> {
        match self {
            SidebarItem::Project { .. } => vec![
                ContextAction::Edit,
                ContextAction::AddService,
                ContextAction::AddHost,
                ContextAction::Deploy,
                ContextAction::Delete,
            ],
            SidebarItem::Host { .. } => vec![
                ContextAction::Edit,
                ContextAction::Deploy,
                ContextAction::Delete,
            ],
            SidebarItem::Service { status, .. } => {
                let start_stop = if *status == RunState::Running { ContextAction::Stop } else { ContextAction::Start };
                vec![start_stop, ContextAction::Logs, ContextAction::Edit, ContextAction::Delete]
            }
            _ => vec![],
        }
    }

    /// Confirm-overlay parameters for deleting this item, if applicable.
    ///
    /// Returns `(message_key, optional_data, yes_action)`.
    /// Used by `execute_context_action` — add new resource types here only.
    pub fn delete_confirm(&self) -> Option<(String, Option<String>, ConfirmAction)> {
        match self {
            SidebarItem::Project { .. } =>
                Some(("confirm.delete.project".into(), None, ConfirmAction::DeleteProject)),
            SidebarItem::Host { slug, .. } =>
                Some(("confirm.delete.host".into(), Some(slug.clone()), ConfirmAction::DeleteHost)),
            SidebarItem::Service { name, .. } =>
                Some(("confirm.delete.service".into(), Some(name.clone()), ConfirmAction::DeleteService)),
            _ => None,
        }
    }
}

/// Options shown in the new-resource selector popup (label key + kind).
pub use crate::resource_form::ResourceKind;
pub const NEW_RESOURCE_ITEMS: &[(&str, ResourceKind)] = &[
    ("new.project", ResourceKind::Project),
    ("new.host",    ResourceKind::Host),
    ("new.service", ResourceKind::Service),
    ("new.bot",     ResourceKind::Bot),
];
