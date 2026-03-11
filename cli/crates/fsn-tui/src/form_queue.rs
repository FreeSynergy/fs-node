// Form queue — unified form management with editor-style tabs.
//
// Design Pattern: Queue + Tab View
//
//   FormQueue replaces both `state.current_form` (single standalone form)
//   and `state.task_queue` (progressive wizard).  Every open form lives here,
//   displayed as a tab in the editor-style tab bar at the top of the form screen.
//
//   Behaviour:
//     - Free tab selection: the user can click any tab at any time.
//     - Auto-advance: after submitting a tab it is marked Done; the queue
//       advances automatically to the next pending tab.
//     - Dependency expansion: submit_* functions push additional forms onto
//       the queue when a saved resource references new dependencies.
//
//   Adding a new resource type: no changes here — add to ResourceKind and
//   the matching submit_* function in submit.rs.

use crate::resource_form::ResourceForm;
use crate::task_queue::TaskKind;

// ── QueuedForm ────────────────────────────────────────────────────────────────

/// One tab in the FormQueue: a form and its completion state.
pub struct QueuedForm {
    pub form: ResourceForm,
    pub done: bool,
    /// The dependency kind that spawned this tab, if any.
    /// `None` for manually opened forms (Edit, New from menu).
    pub kind: Option<TaskKind>,
}

// ── FormQueue ─────────────────────────────────────────────────────────────────

/// Editor-style tab queue for ResourceForms.
///
/// Invariant: `tabs` is non-empty while the queue exists in AppState.
/// The queue is removed from AppState (set to `None`) when all tabs are done.
pub struct FormQueue {
    pub tabs:   Vec<QueuedForm>,
    pub active: usize,
}

impl FormQueue {
    /// Create with a single form (the most common case — opening one form).
    pub fn single(form: ResourceForm) -> Self {
        Self { tabs: vec![QueuedForm { form, done: false, kind: None }], active: 0 }
    }

    /// Create from a TaskKind — builds the form from the kind.
    pub fn from_kind(kind: TaskKind, state: &crate::app::AppState) -> Self {
        let form = kind.build_form(state);
        Self { tabs: vec![QueuedForm { form, done: false, kind: Some(kind) }], active: 0 }
    }

    /// Push an additional form to the end of the queue.
    pub fn push(&mut self, form: ResourceForm, kind: Option<TaskKind>) {
        self.tabs.push(QueuedForm { form, done: false, kind });
    }

    /// Push a TaskKind — builds the form on the spot.
    pub fn push_kind(&mut self, kind: TaskKind, state: &crate::app::AppState) {
        let form = kind.build_form(state);
        self.tabs.push(QueuedForm { form, done: false, kind: Some(kind) });
    }

    // ── Accessors ─────────────────────────────────────────────────────────

    pub fn active_form(&self) -> &ResourceForm {
        &self.tabs[self.active].form
    }

    pub fn active_form_mut(&mut self) -> &mut ResourceForm {
        &mut self.tabs[self.active].form
    }

    /// Whether there are multiple tabs (drives queue tab bar visibility).
    pub fn has_multiple(&self) -> bool {
        self.tabs.len() > 1
    }

    // ── Navigation ────────────────────────────────────────────────────────

    /// Switch to a tab by index (mouse click on queue tab bar).
    pub fn switch_to(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active = idx;
        }
    }

    // ── Queue progression ─────────────────────────────────────────────────

    /// Mark the active tab as Done and advance to the next pending tab.
    ///
    /// Returns `true` if more pending tabs remain; `false` if all tabs are done.
    pub fn mark_done_and_advance(&mut self) -> bool {
        self.tabs[self.active].done = true;
        // Prefer tabs after active first, then wrap around (browser-tab semantics).
        let len = self.tabs.len();
        let next = (self.active + 1..len)
            .chain(0..self.active)
            .find(|&i| !self.tabs[i].done);

        if let Some(idx) = next {
            self.active = idx;
            true
        } else {
            false
        }
    }

    /// True if every tab has been submitted.
    pub fn all_done(&self) -> bool {
        self.tabs.iter().all(|t| t.done)
    }
}
