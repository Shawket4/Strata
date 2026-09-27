//! Custody events of document notes (PLAN §6.12): the lines of the `## Custody` section and
//! the frontmatter state they imply.
//!
//! Line format (one bullet per event, newest first):
//!
//! ```text
//! - 2026-09-20 — returned-by [[Shady]] to [[Safe — Nasr City office]] — [[Capture 2026-09-20#^c1d2]]
//! ```
//!
//! `- <date> — <type> <arguments>[ — <citation> [<citation> …]]`. Arguments are wikilinks: the
//! type's primary argument without a keyword, others after `at`/`to`/`in` (place), `by`
//! (person) or `with`/`from` (counterparty).
//!
//! Citations are required for AI-produced events (PLAN §6.12: "each cited"; enforced by
//! [`crate::sections::validate_content`], which every AI write goes through). An event the
//! user recorded (`by: user`, e.g. `POST /documents/{id}/custody` without a source note) may
//! have none and is written without the trailing ` — ` part:
//!
//! ```text
//! - 2026-09-21 — handed-to [[Shady]]
//! ```

use std::fmt::Write as _;

use chrono::NaiveDate;

use domain::DocumentStatus;

use crate::frontmatter::{Frontmatter, FrontmatterError, KnownKey};
use crate::wikilink::{self, WikiLink};

/// The kind of custody event (shared vocabulary, PLAN L16).
pub use domain::CustodyEventType;

/// The role an event argument plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// A place note.
    Place,
    /// A person note.
    Person,
    /// A third party (person or company).
    Counterparty,
}

/// The argument written right after the event type, if the type takes one (required).
pub fn primary(kind: CustodyEventType) -> Option<Role> {
    use CustodyEventType as T;
    match kind {
        T::StoredAt | T::MovedTo => Some(Role::Place),
        T::HandedTo | T::ReturnedBy => Some(Role::Person),
        T::SentTo | T::ReceivedFrom => Some(Role::Counterparty),
        T::Lost | T::Found | T::Destroyed => None,
    }
}

/// One custody event. Link fields hold complete wikilinks (`[[Shady]]`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CustodyEvent {
    /// Date of the event (resolved, never relative).
    pub date: NaiveDate,
    /// Type.
    pub kind: CustodyEventType,
    /// Place involved.
    pub place: Option<String>,
    /// Person involved.
    pub person: Option<String>,
    /// Third party involved.
    pub counterparty: Option<String>,
    /// Citations (wikilinks to the notes/blocks stating the event). Empty only for an event
    /// the user recorded ([`Self::is_user_recorded`]); AI events always cite.
    pub citations: Vec<String>,
}

/// Why a custody line did not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CustodyParseError {
    /// Lines start with `- `.
    #[error("custody lines start with `- `")]
    NotABullet,
    /// The date is missing or invalid.
    #[error("missing or invalid date (YYYY-MM-DD)")]
    BadDate,
    /// The ` — ` separators are missing.
    #[error("expected `<date> — <event> — <citations>`")]
    MissingSeparator,
    /// Unknown event type.
    #[error("unknown custody event type `{0}`")]
    UnknownType(String),
    /// The type's primary argument is missing.
    #[error("`{0}` needs a [[link]] right after it")]
    MissingPrimary(CustodyEventType),
    /// Unexpected text in the event part.
    #[error("unexpected `{0}` in custody event")]
    Unexpected(String),
    /// The same role was given twice.
    #[error("role given twice")]
    DuplicateRole,
    /// No citation (only user-recorded events may omit it; AI events must cite).
    #[error("AI custody events must cite at least one note")]
    NoCitation,
}

const SEPARATORS: [&str; 3] = [" — ", " – ", " - "];

impl CustodyEvent {
    fn role(&self, role: Role) -> Option<&String> {
        match role {
            Role::Place => self.place.as_ref(),
            Role::Person => self.person.as_ref(),
            Role::Counterparty => self.counterparty.as_ref(),
        }
    }

    fn role_mut(&mut self, role: Role) -> &mut Option<String> {
        match role {
            Role::Place => &mut self.place,
            Role::Person => &mut self.person,
            Role::Counterparty => &mut self.counterparty,
        }
    }

    /// Renders the canonical line (without line terminator).
    pub fn to_line(&self) -> String {
        let mut out = format!("- {} — {}", self.date.format("%Y-%m-%d"), self.kind);
        let primary = primary(self.kind);
        if let Some(p) = primary.and_then(|r| self.role(r)) {
            out.push(' ');
            out.push_str(p);
        }
        for role in [Role::Place, Role::Person, Role::Counterparty] {
            if Some(role) == primary {
                continue;
            }
            if let Some(link) = self.role(role) {
                let kw = match (role, self.kind) {
                    (Role::Place, CustodyEventType::ReturnedBy) => "to",
                    (Role::Place, _) => "at",
                    (Role::Person, _) => "by",
                    (Role::Counterparty, _) => "with",
                };
                let _ = write!(out, " {kw} {link}");
            }
        }
        if !self.citations.is_empty() {
            out.push_str(" —");
            for c in &self.citations {
                out.push(' ');
                out.push_str(c);
            }
        }
        out
    }

    /// Whether the event was recorded by the user (`by: user`): it carries no citation. AI
    /// events always cite the note stating them.
    pub fn is_user_recorded(&self) -> bool {
        self.citations.is_empty()
    }

    /// Checks the AI rule: at least one citation ([`CustodyParseError::NoCitation`]).
    pub fn require_citation(&self) -> Result<(), CustodyParseError> {
        if self.citations.is_empty() {
            Err(CustodyParseError::NoCitation)
        } else {
            Ok(())
        }
    }

    /// Parses a custody line (canonical or with `–`/`-` separators). The citation part is
    /// optional (a user-recorded event); see [`Self::require_citation`] for AI content.
    pub fn parse(line: &str) -> Result<Self, CustodyParseError> {
        let rest = line
            .trim_end()
            .strip_prefix("- ")
            .or_else(|| line.trim_end().strip_prefix("* "))
            .ok_or(CustodyParseError::NotABullet)?;
        let date_text = rest.get(..10).ok_or(CustodyParseError::BadDate)?;
        let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d")
            .map_err(|_| CustodyParseError::BadDate)?;
        let after_date = &rest[10..];
        let sep = SEPARATORS
            .iter()
            .find(|s| after_date.starts_with(**s))
            .ok_or(CustodyParseError::MissingSeparator)?;
        let body = &after_date[sep.len()..];
        let links = wikilink::find_all(body);
        // No second separator: a user-recorded event without citations.
        let (event, citations) = match last_separator_outside(body, &links) {
            Some((end, start)) => (&body[..end], parse_citations(&body[start..])?),
            None => (body, Vec::new()),
        };
        let mut words = event.splitn(2, ' ');
        let kind_text = words.next().unwrap_or("");
        let kind = kind_text
            .parse::<CustodyEventType>()
            .ok()
            .ok_or_else(|| CustodyParseError::UnknownType(kind_text.to_owned()))?;
        let mut ev = Self {
            date,
            kind,
            place: None,
            person: None,
            counterparty: None,
            citations,
        };
        ev.parse_arguments(words.next().unwrap_or(""))?;
        if let Some(p) = primary(kind)
            && ev.role(p).is_none()
        {
            return Err(CustodyParseError::MissingPrimary(kind));
        }
        Ok(ev)
    }

    fn parse_arguments(&mut self, args: &str) -> Result<(), CustodyParseError> {
        let links = wikilink::find_all(args);
        let mut pos = 0;
        let mut first = true;
        for link in &links {
            let between = args[pos..link.span.start].trim();
            let role = match between {
                "" if first => primary(self.kind),
                "at" | "to" | "in" => Some(Role::Place),
                "by" => Some(Role::Person),
                "with" | "from" => Some(Role::Counterparty),
                _ => None,
            }
            .ok_or_else(|| {
                CustodyParseError::Unexpected(if between.is_empty() {
                    args[link.span.clone()].to_owned()
                } else {
                    between.to_owned()
                })
            })?;
            let slot = self.role_mut(role);
            if slot.is_some() {
                return Err(CustodyParseError::DuplicateRole);
            }
            *slot = Some(args[link.span.clone()].to_owned());
            pos = link.span.end;
            first = false;
        }
        let tail = args[pos..].trim();
        if tail.is_empty() {
            Ok(())
        } else {
            Err(CustodyParseError::Unexpected(tail.to_owned()))
        }
    }
}

/// Finds the last ` — ` (or `–`/`-`) outside wikilinks: (separator start, citations start).
fn last_separator_outside(body: &str, links: &[WikiLink]) -> Option<(usize, usize)> {
    let mut best = None;
    for sep in SEPARATORS {
        let mut from = 0;
        while let Some(rel) = body[from..].find(sep) {
            let at = from + rel;
            if !links.iter().any(|l| l.span.contains(&at)) && best.is_none_or(|(b, _)| at > b) {
                best = Some((at, at + sep.len()));
            }
            from = at + sep.len();
        }
        // Prefer the em dash when present.
        if best.is_some() {
            return best;
        }
    }
    best
}

fn parse_citations(text: &str) -> Result<Vec<String>, CustodyParseError> {
    let links = wikilink::find_all(text);
    let mut pos = 0;
    let mut out = Vec::new();
    for l in &links {
        if !text[pos..l.span.start].trim().is_empty() {
            return Err(CustodyParseError::Unexpected(
                text[pos..l.span.start].trim().to_owned(),
            ));
        }
        out.push(text[l.span.clone()].to_owned());
        pos = l.span.end;
    }
    if !text[pos..].trim().is_empty() {
        return Err(CustodyParseError::Unexpected(text[pos..].trim().to_owned()));
    }
    if out.is_empty() {
        return Err(CustodyParseError::NoCitation);
    }
    Ok(out)
}

/// A line of a custody section that is not an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadLine {
    /// 1-based line number within the section content.
    pub line: usize,
    /// Why.
    pub error: CustodyParseError,
}

/// Parses the content of a `## Custody` section (blank lines ignored).
pub fn parse_section(content: &str) -> (Vec<CustodyEvent>, Vec<BadLine>) {
    let mut events = Vec::new();
    let mut bad = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match CustodyEvent::parse(line) {
            Ok(e) => events.push(e),
            Err(error) => bad.push(BadLine { line: i + 1, error }),
        }
    }
    (events, bad)
}

/// Renders events newest first (stable for equal dates), `\n`-separated, no trailing newline.
pub fn render_section(events: &[CustodyEvent]) -> String {
    let mut sorted: Vec<&CustodyEvent> = events.iter().collect();
    sorted.sort_by(|a, b| b.date.cmp(&a.date));
    sorted
        .iter()
        .map(|e| e.to_line())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Where a document is after its custody events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustodyState {
    /// `location` (a place link) or `None`.
    pub location: Option<String>,
    /// `holder` (a person/party link) or `None`.
    pub holder: Option<String>,
    /// `last-holder`.
    pub last_holder: Option<String>,
    /// `status`.
    pub status: DocumentStatus,
}

impl CustodyState {
    fn apply(&mut self, e: &CustodyEvent) {
        use CustodyEventType as T;
        let person = e.person.clone();
        match e.kind {
            T::StoredAt | T::MovedTo => {
                self.location.clone_from(&e.place);
                self.holder = None;
                self.status = DocumentStatus::Stored;
                if person.is_some() {
                    self.last_holder = person;
                }
            }
            T::HandedTo => {
                self.location.clone_from(&e.place);
                self.holder.clone_from(&person);
                self.last_holder = person;
                self.status = DocumentStatus::CheckedOut;
            }
            T::ReturnedBy => {
                self.location.clone_from(&e.place);
                self.holder = None;
                self.last_holder = person;
                self.status = DocumentStatus::Stored;
            }
            T::SentTo => {
                self.location = None;
                self.holder.clone_from(&e.counterparty);
                self.last_holder.clone_from(&e.counterparty);
                self.status = DocumentStatus::WithThirdParty;
            }
            T::ReceivedFrom | T::Found => {
                self.location.clone_from(&e.place);
                self.holder.clone_from(&person);
                if e.counterparty.is_some() {
                    self.last_holder.clone_from(&e.counterparty);
                }
                if person.is_some() {
                    self.last_holder.clone_from(&person);
                }
                self.status = if person.is_some() {
                    DocumentStatus::CheckedOut
                } else {
                    DocumentStatus::Stored
                };
            }
            T::Lost => {
                self.location = None;
                self.holder = None;
                if person.is_some() {
                    self.last_holder = person;
                }
                self.status = DocumentStatus::Lost;
            }
            T::Destroyed => {
                self.location = None;
                self.holder = None;
                self.status = DocumentStatus::Destroyed;
            }
        }
    }

    /// Folds events (in file order, newest first) into the resulting state; `None` when there
    /// are no events. Events on the same date apply in chronological (reverse file) order.
    pub fn derive(events_newest_first: &[CustodyEvent]) -> Option<Self> {
        if events_newest_first.is_empty() {
            return None;
        }
        let mut ordered: Vec<&CustodyEvent> = events_newest_first.iter().rev().collect();
        ordered.sort_by_key(|e| e.date);
        let mut state = Self {
            location: None,
            holder: None,
            last_holder: None,
            status: DocumentStatus::Stored,
        };
        for e in ordered {
            state.apply(e);
        }
        Some(state)
    }

    /// Writes `location`, `holder`, `last-holder` and `status` (empty values as `""`).
    pub fn write_to(&self, fm: &mut Frontmatter) -> Result<(), FrontmatterError> {
        fm.set_text(
            KnownKey::Location,
            self.location.clone().unwrap_or_default(),
        )?;
        fm.set_text(KnownKey::Holder, self.holder.clone().unwrap_or_default())?;
        fm.set_text(
            KnownKey::LastHolder,
            self.last_holder.clone().unwrap_or_default(),
        )?;
        fm.set_text(KnownKey::Status, self.status.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap_or_default()
    }

    #[test]
    fn parses_canonical_line() {
        let line = "- 2026-09-20 — returned-by [[Shady]] to [[Safe — Nasr City office]] — [[Capture 2026-09-20#^c1d2]]";
        let e = CustodyEvent::parse(line);
        assert_eq!(
            e,
            Ok(CustodyEvent {
                date: d(2026, 9, 20),
                kind: CustodyEventType::ReturnedBy,
                place: Some("[[Safe — Nasr City office]]".into()),
                person: Some("[[Shady]]".into()),
                counterparty: None,
                citations: vec!["[[Capture 2026-09-20#^c1d2]]".into()],
            })
        );
        assert_eq!(e.map(|e| e.to_line()).as_deref(), Ok(line));
    }

    #[test]
    fn every_type_round_trips() {
        for &kind in CustodyEventType::ALL {
            let mut e = CustodyEvent {
                date: d(2026, 1, 2),
                kind,
                place: Some("[[Office]]".into()),
                person: Some("[[شادي]]".into()),
                counterparty: Some("[[Watanya]]".into()),
                citations: vec!["[[A#^x]]".into(), "[[B]]".into()],
            };
            if primary(kind).is_none() {
                e.counterparty = None;
            }
            let line = e.to_line();
            assert_eq!(CustodyEvent::parse(&line), Ok(e.clone()), "{line}");
            // The same event recorded by the user, without citations.
            e.citations.clear();
            let line = e.to_line();
            assert!(!line.ends_with('—'), "{line}");
            assert_eq!(CustodyEvent::parse(&line), Ok(e), "{line}");
        }
    }

    #[test]
    fn parse_errors() {
        use CustodyParseError as E;
        let cases = [
            ("2026-01-01 — lost — [[a]]", E::NotABullet),
            ("- 2026-13-01 — lost — [[a]]", E::BadDate),
            ("- 2026-01-01 lost [[a]]", E::MissingSeparator),
            (
                "- 2026-01-01 — misplaced — [[a]]",
                E::UnknownType("misplaced".into()),
            ),
            (
                "- 2026-01-01 — stored-at — [[a]]",
                E::MissingPrimary(CustodyEventType::StoredAt),
            ),
            (
                "- 2026-01-01 — lost near [[x]] — [[a]]",
                E::Unexpected("near".into()),
            ),
            (
                "- 2026-01-01 — lost by [[x]] by [[y]] — [[a]]",
                E::DuplicateRole,
            ),
            ("- 2026-01-01 — lost —", E::Unexpected("—".into())),
            (
                "- 2026-01-01 — handed-to [[x]] - see notes",
                E::Unexpected("see notes".into()),
            ),
            (
                "- 2026-01-01 — lost — see notes",
                E::Unexpected("see notes".into()),
            ),
            (
                "- 2026-01-01 — lost [[x]] — [[a]]",
                E::Unexpected("[[x]]".into()),
            ),
        ];
        for (line, err) in cases {
            assert_eq!(CustodyEvent::parse(line), Err(err), "{line}");
        }
    }

    #[test]
    fn user_recorded_events_need_no_citation() {
        let line = "- 2026-09-21 — returned-by [[Shady]] to [[Safe — Nasr City office]]";
        let e = CustodyEvent::parse(line);
        assert_eq!(
            e,
            Ok(CustodyEvent {
                date: d(2026, 9, 21),
                kind: CustodyEventType::ReturnedBy,
                place: Some("[[Safe — Nasr City office]]".into()),
                person: Some("[[Shady]]".into()),
                counterparty: None,
                citations: vec![],
            })
        );
        let e = e.expect("parsed");
        assert!(e.is_user_recorded());
        assert_eq!(e.require_citation(), Err(CustodyParseError::NoCitation));
        assert_eq!(e.to_line(), line);
        for (line, kind) in [
            ("- 2026-01-01 — lost", CustodyEventType::Lost),
            ("- 2026-01-01 — destroyed by [[Shady]]", CustodyEventType::Destroyed),
            ("- 2026-01-01 - sent-to [[Bank]]", CustodyEventType::SentTo),
        ] {
            let e = CustodyEvent::parse(line).expect(line);
            assert_eq!((e.kind, e.citations.len()), (kind, 0), "{line}");
        }
        let cited = CustodyEvent::parse("- 2026-01-01 — lost — [[C]]").expect("cited");
        assert!(!cited.is_user_recorded());
        assert_eq!(cited.require_citation(), Ok(()));
        // A section may mix both; the state derives from all of them.
        let section = "- 2026-09-21 — handed-to [[Shady]]\n\
                       - 2026-01-01 — stored-at [[Safe]] — [[C0]]\n";
        let (events, bad) = parse_section(section);
        assert_eq!(bad, vec![]);
        assert_eq!(render_section(&events), section.trim_end());
        assert_eq!(
            CustodyState::derive(&events).map(|s| (s.holder, s.status)),
            Some((Some("[[Shady]]".into()), DocumentStatus::CheckedOut))
        );
    }

    #[test]
    fn tolerant_separators() {
        let e = CustodyEvent::parse("- 2026-01-01 - lost - [[a]]");
        assert_eq!(e.map(|e| e.kind), Ok(CustodyEventType::Lost));
    }

    #[test]
    fn derives_state_from_newest_event() {
        let section = "- 2026-09-20 — returned-by [[Shady]] to [[Safe — Nasr City office]] — [[C2]]\n\
                       - 2026-09-10 — handed-to [[Shady]] — [[C1]]\n\
                       - 2026-01-01 — stored-at [[Safe — Nasr City office]] — [[C0]]\n";
        let (events, bad) = parse_section(section);
        assert!(bad.is_empty());
        assert_eq!(
            CustodyState::derive(&events),
            Some(CustodyState {
                location: Some("[[Safe — Nasr City office]]".into()),
                holder: None,
                last_holder: Some("[[Shady]]".into()),
                status: DocumentStatus::Stored,
            })
        );
        assert_eq!(render_section(&events), section.trim_end());
        assert_eq!(
            CustodyState::derive(&events[1..]).map(|s| s.status),
            Some(DocumentStatus::CheckedOut)
        );
        assert_eq!(CustodyState::derive(&[]), None);
    }

    #[test]
    fn writes_state_to_frontmatter() {
        let mut fm = Frontmatter::new(crate::LineEnding::Lf);
        let state = CustodyState {
            location: Some("[[Safe]]".into()),
            holder: None,
            last_holder: Some("[[Shady]]".into()),
            status: DocumentStatus::Stored,
        };
        assert_eq!(state.write_to(&mut fm), Ok(()));
        assert_eq!(
            fm.render(),
            "---\nlocation: \"[[Safe]]\"\nholder: \"\"\nlast-holder: \"[[Shady]]\"\nstatus: stored\n---\n"
        );
    }
}
