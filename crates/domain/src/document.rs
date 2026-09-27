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

string_enum! {
    /// Kind of document (`doc-type:` frontmatter, §6.12). Free text is allowed in the vault;
    /// values outside this set are kept verbatim by the vault format.
    pub enum DocType("document type") {
        /// A contract.
        Contract => "contract",
        /// An identity document.
        Id => "id",
        /// A licence.
        Licence => "licence",
        /// A deed.
        Deed => "deed",
        /// An invoice.
        Invoice => "invoice",
        /// A certificate.
        Certificate => "certificate",
        /// Explicitly "other".
        Other => "other",
    }
}

string_enum! {
    /// Document-to-document relation (§6.12), a frontmatter wikilink list on the copy.
    pub enum DocumentRelationType("document relation type") {
        /// `copy-of`: this note describes a copy of the linked document.
        CopyOf => "copy-of",
    }
}

impl DocumentRelationType {
    /// The frontmatter key holding this relation's wikilink list.
    pub const fn frontmatter_key(self) -> &'static str {
        self.as_str()
    }
}
