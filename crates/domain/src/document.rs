//! Documents, copies and custody (§6.12, §7.4 `documents`, `custody_events`).

use crate::macros::string_enum;

string_enum! {
    /// A custody event recorded in a document's `## Custody` section.
    pub enum CustodyEventType("custody event type") {
        /// Stored at a place.
        StoredAt => "stored-at",
        /// Moved to another place.
        MovedTo => "moved-to",
        /// Handed to a person.
        HandedTo => "handed-to",
        /// Returned by a person.
        ReturnedBy => "returned-by",
        /// Sent to a third party.
        SentTo => "sent-to",
        /// Received from a third party.
        ReceivedFrom => "received-from",
        /// Lost.
        Lost => "lost",
        /// Found again.
        Found => "found",
        /// Destroyed.
        Destroyed => "destroyed",
    }
}

string_enum! {
    /// Current status of a document (`status:` frontmatter), always the result of the newest
    /// custody event.
    pub enum DocumentStatus("document status") {
        /// Stored at a place.
        Stored => "stored",
        /// Held by a person.
        CheckedOut => "checked-out",
        /// With a third party.
        WithThirdParty => "with-third-party",
        /// Lost.
        Lost => "lost",
        /// Destroyed.
        Destroyed => "destroyed",
    }
}

string_enum! {
    /// Which copy a document note describes (`copy:` frontmatter). The plan spells the second
    /// value with a space, `certified copy`; `certified-copy` is accepted when parsing.
    pub enum CopyKind("copy kind") {
        /// The original.
        Original => "original",
        /// A certified copy.
        CertifiedCopy => "certified copy" | "certified-copy",
        /// A plain copy.
        Copy => "copy",
        /// A digital copy (scan, PDF).
        Digital => "digital",
    }
}
