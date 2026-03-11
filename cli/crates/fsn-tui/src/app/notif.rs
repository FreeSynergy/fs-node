// Notification (toast) system.
//
// Pattern: Value Object — Notification carries immutable data about a single
// toast message. AppState owns the Vec<Notification> and manages lifecycle.

use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifKind { Success, Warning, Error, Info }

#[derive(Debug, Clone)]
pub struct Notification {
    pub message:   String,
    pub kind:      NotifKind,
    pub born:      Instant,
    /// Tick at creation — used by Anim::notif_width() for slide-in effect.
    pub born_tick: u32,
}
