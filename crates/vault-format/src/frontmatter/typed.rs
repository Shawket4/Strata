//! Typed access to the known frontmatter keys.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, FixedOffset, NaiveDate, SecondsFormat};
use domain::{CopyKind, DocType, DocumentStatus, Lang, NoteKind};
use ulid::Ulid;

use super::{Frontmatter, FrontmatterError, KnownKey, PropertyValue, RelationKey};
use crate::wikilink::WikiLink;

/// A `domain` vocabulary enum whose canonical spelling can be read back.
pub trait Vocabulary: Copy + FromStr {
    /// The canonical spelling.
    fn name(self) -> &'static str;
}

macro_rules! vocabulary {
    ($($t:ty),+) => {
        $(impl Vocabulary for $t {
            fn name(self) -> &'static str {
                self.as_str()
            }
        })+
    };
}

vocabulary!(
    domain::NoteKind,
    domain::Lang,
    domain::DocType,
    domain::CopyKind,
    domain::DocumentStatus
);

/// A frontmatter value from a shared `domain` vocabulary, or any other text the user wrote
/// (kept verbatim). Only the canonical spelling reads as [`Open::Known`], so every value
/// renders back exactly as written.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Open<T> {
    /// A value of the shared vocabulary.
    Known(T),
    /// Any other value, verbatim.
    Other(String),
}

impl<T: Vocabulary> Open<T> {
    /// Parses a frontmatter value.
    pub fn parse(s: &str) -> Self {
        match s.parse::<T>() {
            Ok(t) if t.name() == s => Self::Known(t),
            _ => Self::Other(s.to_owned()),
        }
    }

    /// The frontmatter spelling.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Known(t) => t.name(),
            Self::Other(s) => s,
        }
    }

    /// The vocabulary value, if known.
    pub fn known(&self) -> Option<T> {
        match self {
            Self::Known(t) => Some(*t),
            Self::Other(_) => None,
        }
    }
}

impl<T: Vocabulary> fmt::Display for Open<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<T> From<T> for Open<T> {
    fn from(t: T) -> Self {
        Self::Known(t)
    }
}

/// Formats a timestamp the way Strata writes `created`/`updated`: RFC 3339, with `Z` for UTC
/// (Strata writes every time in UTC; a value read with another offset keeps it).
pub fn format_timestamp(ts: &DateTime<FixedOffset>) -> String {
    ts.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

impl Frontmatter {
    /// Scalar text of a known key (`None` if absent, empty-valued, or not a scalar).
    pub fn text(&self, key: KnownKey) -> Option<&str> {
        self.get(key.as_str()).and_then(PropertyValue::as_text)
    }

    /// A known key as a list (scalars become one-element lists).
    pub fn list(&self, key: KnownKey) -> Vec<String> {
        self.get(key.as_str())
            .map(PropertyValue::to_list)
            .unwrap_or_default()
    }

    /// Sets a known key to a scalar.
    pub fn set_text(
        &mut self,
        key: KnownKey,
        value: impl Into<String>,
    ) -> Result<(), FrontmatterError> {
        self.set(key.as_str(), PropertyValue::Text(value.into()))
    }

    /// Sets a known key to a list.
    pub fn set_list(&mut self, key: KnownKey, items: Vec<String>) -> Result<(), FrontmatterError> {
        self.set(key.as_str(), PropertyValue::List(items))
    }

    /// Removes a known key.
    pub fn remove_key(&mut self, key: KnownKey) -> Result<bool, FrontmatterError> {
        self.remove(key.as_str())
    }

    /// `id` as a ULID.
    pub fn id(&self) -> Result<Option<Ulid>, FrontmatterError> {
        self.text(KnownKey::Id)
            .map(|s| {
                Ulid::from_string(s).map_err(|e| FrontmatterError::InvalidValue {
                    key: "id".into(),
                    reason: e.to_string(),
                })
            })
            .transpose()
    }

    /// Sets `id`.
    pub fn set_id(&mut self, id: Ulid) -> Result<(), FrontmatterError> {
        self.set_text(KnownKey::Id, id.to_string())
    }

    /// `kind`.
    pub fn kind(&self) -> Option<Open<NoteKind>> {
        self.text(KnownKey::Kind).map(Open::parse)
    }

    /// Sets `kind`.
    pub fn set_kind(&mut self, kind: NoteKind) -> Result<(), FrontmatterError> {
        if kind == NoteKind::Note {
            // Plain notes carry no `kind` (PLAN §6.4).
            return self.remove_key(KnownKey::Kind).map(|_| ());
        }
        self.set_text(KnownKey::Kind, kind.as_str())
    }

    /// `title`.
    pub fn title(&self) -> Option<&str> {
        self.text(KnownKey::Title)
    }

    /// `aliases`.
    pub fn aliases(&self) -> Vec<String> {
        self.list(KnownKey::Aliases)
    }

    /// `tags`, without leading `#`. A legacy comma/space separated string is split.
    pub fn tags(&self) -> Vec<String> {
        let raw = match self.get(KnownKey::Tags.as_str()) {
            Some(PropertyValue::Text(s)) => {
                s.split([',', ' ']).map(str::to_owned).collect::<Vec<_>>()
            }
            Some(v) => v.to_list(),
            None => Vec::new(),
        };
        raw.iter()
            .map(|t| t.trim().trim_start_matches('#').to_owned())
            .filter(|t| !t.is_empty())
            .collect()
    }

    fn timestamp(&self, key: KnownKey) -> Result<Option<DateTime<FixedOffset>>, FrontmatterError> {
        self.text(key)
            .map(|s| {
                DateTime::parse_from_rfc3339(s).map_err(|e| FrontmatterError::InvalidValue {
                    key: key.as_str().into(),
                    reason: e.to_string(),
                })
            })
            .transpose()
    }

    /// `created` (RFC 3339).
    pub fn created(&self) -> Result<Option<DateTime<FixedOffset>>, FrontmatterError> {
        self.timestamp(KnownKey::Created)
    }

    /// `updated` (RFC 3339).
    pub fn updated(&self) -> Result<Option<DateTime<FixedOffset>>, FrontmatterError> {
        self.timestamp(KnownKey::Updated)
    }

    /// Sets `created`.
    pub fn set_created(&mut self, ts: &DateTime<FixedOffset>) -> Result<(), FrontmatterError> {
        self.set_text(KnownKey::Created, format_timestamp(ts))
    }

    /// Sets `updated`.
    pub fn set_updated(&mut self, ts: &DateTime<FixedOffset>) -> Result<(), FrontmatterError> {
        self.set_text(KnownKey::Updated, format_timestamp(ts))
    }

    /// `lang`.
    pub fn lang(&self) -> Option<Open<Lang>> {
        self.text(KnownKey::Lang).map(Open::parse)
    }

    /// `doc-type`.
    pub fn doc_type(&self) -> Option<Open<DocType>> {
        self.text(KnownKey::DocType).map(Open::parse)
    }

    /// `copy`.
    pub fn copy_kind(&self) -> Option<Open<CopyKind>> {
        self.text(KnownKey::Copy).map(Open::parse)
    }

    /// `status`.
    pub fn status(&self) -> Option<Open<DocumentStatus>> {
        self.text(KnownKey::Status).map(Open::parse)
    }

    /// `expires` (`YYYY-MM-DD`).
    pub fn expires(&self) -> Result<Option<NaiveDate>, FrontmatterError> {
        self.text(KnownKey::Expires)
            .filter(|s| !s.is_empty())
            .map(|s| {
                NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| {
                    FrontmatterError::InvalidValue {
                        key: "expires".into(),
                        reason: e.to_string(),
                    }
                })
            })
            .transpose()
    }

    /// The wikilink held by a single-link key (`source`, `location`, `holder`, `last-holder`).
    /// `None` when absent, empty, or not a wikilink.
    pub fn link(&self, key: KnownKey) -> Option<WikiLink> {
        self.text(key).and_then(WikiLink::parse_exact)
    }

    /// Raw entries of a relation list (normally `"[[Target]]"` strings).
    pub fn relation(&self, relation: RelationKey) -> Vec<String> {
        self.list(relation.key())
    }

    /// Parsed wikilinks of a relation list; entries that are not wikilinks are skipped.
    pub fn relation_links(&self, relation: RelationKey) -> Vec<WikiLink> {
        self.relation(relation)
            .iter()
            .filter_map(|s| WikiLink::parse_exact(s))
            .collect()
    }

    /// Replaces a relation list.
    pub fn set_relation(
        &mut self,
        relation: RelationKey,
        items: Vec<String>,
    ) -> Result<(), FrontmatterError> {
        self.set_list(relation.key(), items)
    }

    /// Adds `[[target]]` to a relation unless a link to the same target is already there.
    /// Returns whether it was added.
    pub fn add_relation_link(
        &mut self,
        relation: RelationKey,
        target: &str,
    ) -> Result<bool, FrontmatterError> {
        let mut items = self.relation(relation);
        let exists = items
            .iter()
            .filter_map(|s| WikiLink::parse_exact(s))
            .any(|l| l.path.eq_ignore_ascii_case(target) || l.path == target);
        if exists {
            return Ok(false);
        }
        items.push(format!("[[{target}]]"));
        self.set_relation(relation, items)?;
        Ok(true)
    }

    /// Removes every link to `target` from a relation. Returns how many were removed.
    pub fn remove_relation_link(
        &mut self,
        relation: RelationKey,
        target: &str,
    ) -> Result<usize, FrontmatterError> {
        let items = self.relation(relation);
        let kept: Vec<String> = items
            .iter()
            .filter(|s| WikiLink::parse_exact(s).is_none_or(|l| l.path != target))
            .cloned()
            .collect();
        let removed = items.len() - kept.len();
        if removed > 0 {
            self.set_relation(relation, kept)?;
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{MentionType, RelationType};

    fn fm(inner: &str) -> Frontmatter {
        Frontmatter::from_parts("---\n", inner, "---\n")
    }

    #[test]
    fn timestamps_are_written_with_z_in_utc() {
        let utc = DateTime::parse_from_rfc3339("2026-09-27T11:32:00+00:00").expect("ts");
        assert_eq!(format_timestamp(&utc), "2026-09-27T11:32:00Z");
        let cairo = DateTime::parse_from_rfc3339("2026-09-27T14:32:00+03:00").expect("ts");
        assert_eq!(format_timestamp(&cairo), "2026-09-27T14:32:00+03:00");
        let frac = DateTime::parse_from_rfc3339("2026-09-27T11:32:00.25Z").expect("ts");
        assert_eq!(format_timestamp(&frac), "2026-09-27T11:32:00.250Z");
    }

    #[test]
    fn typed_getters() {
        let f = fm(concat!(
            "id: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\n",
            "kind: person\n",
            "tags: \"#a, b\"\n",
            "created: 2026-09-27T14:32:00+03:00\n",
            "lang: ar\n",
            "expires: 2027-03-31\n",
            "status: checked-out\n",
            "copy: certified copy\n",
            "doc-type: passport\n",
            "location: \"[[Safe — Nasr City office]]\"\n",
            "holder: \"\"\n",
            "people: [\"[[Ahmed Samir]]\", plain]\n",
        ));
        assert_eq!(f.error(), None);
        assert_eq!(
            f.id().ok().flatten().map(|u| u.to_string()),
            Some("01J8ZK3M4X7Q9W2E5R6T8Y0V1H".into())
        );
        assert_eq!(f.kind(), Some(Open::Known(NoteKind::Person)));
        assert_eq!(f.tags(), vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(
            f.created().ok().flatten().map(|t| format_timestamp(&t)),
            Some("2026-09-27T14:32:00+03:00".into())
        );
        assert_eq!(f.lang(), Some(Open::Known(Lang::Ar)));
        assert_eq!(f.expires(), Ok(NaiveDate::from_ymd_opt(2027, 3, 31)));
        assert_eq!(f.status(), Some(Open::Known(DocumentStatus::CheckedOut)));
        assert_eq!(f.copy_kind(), Some(Open::Known(CopyKind::CertifiedCopy)));
        assert_eq!(f.doc_type(), Some(Open::Other("passport".into())));
        assert_eq!(
            f.link(KnownKey::Location).map(|l| l.path),
            Some("Safe — Nasr City office".into())
        );
        assert_eq!(f.link(KnownKey::Holder), None);
        assert_eq!(
            f.relation(RelationKey::Mention(MentionType::People)),
            vec!["[[Ahmed Samir]]".to_owned(), "plain".to_owned()]
        );
        assert_eq!(
            f.relation_links(RelationKey::Mention(MentionType::People))
                .len(),
            1
        );
    }

    #[test]
    fn invalid_typed_values() {
        let f = fm("id: nope\ncreated: yesterday\nexpires: 2027-02-30\n");
        assert!(
            matches!(f.id(), Err(FrontmatterError::InvalidValue { ref key, .. }) if key == "id")
        );
        assert!(
            matches!(f.created(), Err(FrontmatterError::InvalidValue { ref key, .. }) if key == "created")
        );
        assert!(
            matches!(f.expires(), Err(FrontmatterError::InvalidValue { ref key, .. }) if key == "expires")
        );
    }

    #[test]
    fn relation_editing() {
        let mut f = fm("related: [\"[[A]]\"]\n");
        assert_eq!(
            f.add_relation_link(RelationKey::Note(RelationType::Related), "A"),
            Ok(false)
        );
        assert_eq!(
            f.add_relation_link(RelationKey::Note(RelationType::Related), "B"),
            Ok(true)
        );
        assert_eq!(f.render(), "---\nrelated: [\"[[A]]\", \"[[B]]\"]\n---\n");
        assert_eq!(
            f.remove_relation_link(RelationKey::Note(RelationType::Related), "A"),
            Ok(1)
        );
        assert_eq!(
            f.remove_relation_link(RelationKey::Note(RelationType::Related), "Z"),
            Ok(0)
        );
        assert_eq!(f.render(), "---\nrelated: [\"[[B]]\"]\n---\n");
    }

    #[test]
    fn open_values_round_trip() {
        for s in [
            "concept", "person", "company", "document", "place", "note", "meeting", "Person",
        ] {
            assert_eq!(Open::<NoteKind>::parse(s).as_str(), s);
        }
        assert_eq!(
            Open::<NoteKind>::parse("Person"),
            Open::Other("Person".into())
        );
        // Aliases accepted by `domain` stay verbatim here so the file round-trips.
        assert_eq!(
            Open::<CopyKind>::parse("certified-copy"),
            Open::Other("certified-copy".into())
        );
        assert_eq!(Open::<DocType>::parse("other"), Open::Known(DocType::Other));
        assert_eq!(
            Open::from(DocumentStatus::WithThirdParty).to_string(),
            "with-third-party"
        );
        assert_eq!(Open::Known(Lang::Mixed).known(), Some(Lang::Mixed));
    }

    #[test]
    fn set_kind_writes_or_removes() {
        let mut f = fm("kind: concept\n");
        f.set_kind(NoteKind::Person).unwrap_or_default();
        assert_eq!(f.render(), "---\nkind: person\n---\n");
        f.set_kind(NoteKind::Note).unwrap_or_default();
        assert_eq!(f.render(), "---\n---\n");
    }
}
