//! View-models (PLAN §12.1 `view/`): the structs streamed to Dart ([`model`]), their builders
//! from the local database ([`build`]) and the watcher hub that re-emits them when the data
//! behind them changes ([`hub`]).

pub mod build;
pub mod hub;
pub mod model;

use std::ops::{BitOr, BitOrAssign};

pub use hub::{ViewCtx, ViewHub, ViewSink, WatchId};

/// Areas of data a write changed; watchers subscribe to the topics their view reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Topics(u32);

impl Topics {
    /// Nothing.
    pub const NONE: Self = Self(0);
    /// Notes, links, relations, tags, full-text.
    pub const NOTES: Self = Self(1);
    /// Tasks and reminders.
    pub const TASKS: Self = Self(1 << 1);
    /// Inbox captures.
    pub const INBOX: Self = Self(1 << 2);
    /// Suggestions.
    pub const SUGGESTIONS: Self = Self(1 << 3);
    /// Entities, documents, places, custody.
    pub const ENTITIES: Self = Self(1 << 4);
    /// Outbox, sync state, conflicts, connectivity.
    pub const SYNC: Self = Self(1 << 5);
    /// Device settings, notification state.
    pub const SETTINGS: Self = Self(1 << 6);
    /// The account and session.
    pub const ACCOUNT: Self = Self(1 << 7);
    /// The clock moved (date-dependent views: task sections, days remaining).
    pub const TIME: Self = Self(1 << 8);
    /// The Ask conversation.
    pub const ASK: Self = Self(1 << 9);
    /// Cached online reads (devices, history, AI status, integrity, export).
    pub const REMOTE: Self = Self(1 << 10);
    /// Everything.
    pub const ALL: Self = Self(u32::MAX);

    /// Whether no topic is set.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether any topic is shared.
    pub fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl BitOr for Topics {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for Topics {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topics_combine() {
        let t = Topics::NOTES | Topics::TASKS;
        assert!(t.intersects(Topics::TASKS));
        assert!(!t.intersects(Topics::SYNC));
        assert!(Topics::NONE.is_empty());
        assert!(Topics::ALL.intersects(Topics::TIME));
    }
}
