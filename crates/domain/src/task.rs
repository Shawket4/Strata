//! Tasks (§6.11, §7.4 `tasks`).

use crate::macros::string_enum;

string_enum! {
    /// Task line status: `- [ ]` open, `- [x]` done, `❌` cancelled.
    pub enum TaskStatus("task status") {
        /// Not done yet.
        Open => "open",
        /// Completed (`✅` date).
        Done => "done",
        /// Cancelled (`❌` date).
        Cancelled => "cancelled",
    }
}

string_enum! {
    /// Obsidian Tasks priority. A line without a priority signifier is [`Priority::Normal`].
    /// Variants are declared from highest to lowest, so the derived `Ord` sorts by urgency
    /// (`Highest < … < Lowest`).
    pub enum Priority("priority") {
        /// `🔺`
        Highest => "highest",
        /// `⏫`
        High => "high",
        /// `🔼`
        Medium => "medium",
        /// No signifier.
        Normal => "normal",
        /// `🔽`
        Low => "low",
        /// `⏬`
        Lowest => "lowest",
    }
}

impl Priority {
    /// The Tasks plugin signifier emoji; `None` for [`Priority::Normal`].
    pub const fn signifier(self) -> Option<&'static str> {
        match self {
            Self::Highest => Some("🔺"),
            Self::High => Some("⏫"),
            Self::Medium => Some("🔼"),
            Self::Normal => None,
            Self::Low => Some("🔽"),
            Self::Lowest => Some("⏬"),
        }
    }

    /// Parses a Tasks plugin signifier emoji.
    pub fn from_signifier(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|p| p.signifier() == Some(s))
    }
}
