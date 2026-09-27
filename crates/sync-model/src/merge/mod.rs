//! 3-way merge of note documents (D19).
//!
//! [`merge`] takes the base version (from git), "ours" and "theirs" and returns either the
//! merged text or the conflict hunks for the conflict UI:
//!
//! - **Frontmatter** is merged key by key ([`frontmatter`] rules: set union for list keys
//!   with deletions respected, scalars conflict only when both sides changed them
//!   differently, unknown keys preserved in order, `updated` takes the later time).
//! - **Body** is merged line by line with diff3: non-overlapping changes apply cleanly,
//!   identical changes collapse, overlapping changes become conflict hunks carrying the
//!   base, ours and theirs text. Line-ending conversions and a missing final newline are
//!   merged as separate one-sided changes (see [`lines`]).
//! - **Task lines are atomic**: a line is never merged within itself, so a checkbox toggle
//!   on one side and a text edit on the other is a conflict on that line. In addition, a
//!   task (by block ID) that would appear more often in a clean result than on either side
//!   — both sides moved it to different places — becomes a
//!   [`ConflictKind::TaskPlacement`] conflict instead of a silent duplicate.
//!
//! Which side is "ours" is up to the caller: on the server, ours = the server's current
//! version and theirs = the pushed edit ([`decide_update`]); on the device, ours = the local
//! edit.

mod diff3;
pub mod frontmatter;
mod lines;
mod text;

use serde::{Deserialize, Serialize};
use vault_format::Document;

pub use frontmatter::FmValue;
use frontmatter::{FmConflict, merge_frontmatter};
use text::{Segment, TextMerge, merge_text};

use crate::Version;

/// Where a hunk or an automatic resolution is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Location {
    /// A frontmatter key.
    Frontmatter {
        /// The key.
        key: String,
    },
    /// Body lines (0-based line numbers of the region's first line on each side).
    Body {
        /// Line in the base body.
        base_line: u32,
        /// Line in ours' body.
        ours_line: u32,
        /// Line in theirs' body.
        theirs_line: u32,
    },
    /// The file's line endings.
    LineEndings,
}

/// How a change was merged automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoResolution {
    /// Only ours changed it.
    TookOurs,
    /// Only theirs changed it.
    TookTheirs,
    /// Both made the same change.
    Identical,
    /// A list key merged as a set union with deletions respected.
    ListUnion,
    /// `updated` set to the later of the two timestamps.
    LatestTimestamp,
}

/// An automatically merged change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoResolved {
    /// Where.
    pub location: Location,
    /// How.
    pub resolution: AutoResolution,
}

/// What kind of conflict a hunk is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    /// Both sides changed the same lines (or the same frontmatter key) differently.
    BothModified,
    /// Both sides inserted different lines at the same place.
    BothAdded,
    /// One side changed lines the other deleted.
    ModifyDelete,
    /// The overlapping lines include a task line (edited on both sides).
    TaskLine,
    /// A task (block ID) was placed on both sides; keeping both would duplicate it.
    TaskPlacement,
}

/// One conflict for the conflict UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictHunk {
    /// Index in [`Conflicted::hunks`] (used to resolve it).
    pub id: u32,
    /// Where.
    pub location: Location,
    /// Kind.
    pub kind: ConflictKind,
    /// Base text (exact bytes; for frontmatter the raw YAML entry, empty when absent).
    pub base: String,
    /// Ours' text.
    pub ours: String,
    /// Theirs' text.
    pub theirs: String,
}

/// How the user resolves one hunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Choice {
    /// Keep ours.
    Ours,
    /// Keep theirs.
    Theirs,
    /// Keep the base.
    Base,
    /// Body only: ours' lines, then theirs'.
    OursThenTheirs,
    /// Body only: theirs' lines, then ours'.
    TheirsThenOurs,
    /// Body only: this text instead (should end with a line terminator).
    Text(String),
    /// Frontmatter only: this value.
    Value(FmValue),
}

/// What the merged document is built from, so hunks can be resolved later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolutionTemplate {
    /// BOM and frontmatter block (ours' values for conflicting keys); `None` when the whole
    /// file was merged as text.
    pub prefix: Option<String>,
    /// Body pieces in order.
    pub body: Vec<TemplatePiece>,
    /// Remove the final line terminator after assembling (the merged file has no final
    /// newline).
    pub strip_final_newline: bool,
    /// Per hunk: the frontmatter values (`None` for body hunks).
    pub values: Vec<Option<HunkValues>>,
}

/// Frontmatter values of a conflicting key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HunkValues {
    /// Base.
    pub base: FmValue,
    /// Ours.
    pub ours: FmValue,
    /// Theirs.
    pub theirs: FmValue,
}

/// A piece of the merged body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TemplatePiece {
    /// Merged text.
    Text {
        /// The text.
        text: String,
    },
    /// A body conflict: the three texts in output form (line endings as the merged file).
    Hunk {
        /// Hunk ID.
        id: u32,
        /// Base.
        base: String,
        /// Ours.
        ours: String,
        /// Theirs.
        theirs: String,
    },
}

/// A merge with conflicts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflicted {
    /// The merged file with git-style diff3 markers in the body. `None` when a frontmatter
    /// key conflicts (markers cannot go into YAML): build the result with
    /// [`Conflicted::resolve`] instead.
    pub merged_with_markers: Option<String>,
    /// The conflicts, frontmatter first, then body in order.
    pub hunks: Vec<ConflictHunk>,
    /// Changes merged automatically.
    pub auto_resolved: Vec<AutoResolved>,
    /// How to rebuild the file from choices.
    pub template: ResolutionTemplate,
}

/// Result of [`merge`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum MergeOutcome {
    /// Merged without conflicts.
    Clean(String),
    /// Needs the user.
    Conflicted(Conflicted),
}

impl MergeOutcome {
    /// The merged text of a clean merge.
    pub fn clean(&self) -> Option<&str> {
        match self {
            Self::Clean(t) => Some(t),
            Self::Conflicted(_) => None,
        }
    }
}

/// Why a set of choices cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    /// A hunk has no choice.
    #[error("hunk {0} is not resolved")]
    Unresolved(u32),
    /// A choice names no hunk.
    #[error("no hunk {0}")]
    UnknownHunk(u32),
    /// The choice does not fit the hunk (e.g. `Value` for a body hunk).
    #[error("choice does not fit hunk {0}")]
    WrongChoice(u32),
    /// The chosen frontmatter value cannot be written.
    #[error("hunk {0}: the chosen frontmatter value cannot be written")]
    Unwritable(u32),
}

impl Conflicted {
    /// Builds the final file from one choice per hunk.
    pub fn resolve(&self, choices: &[(u32, Choice)]) -> Result<String, ResolveError> {
        let n = self.hunks.len();
        let mut chosen: Vec<Option<&Choice>> = vec![None; n];
        for (id, choice) in choices {
            let slot = chosen
                .get_mut(*id as usize)
                .ok_or(ResolveError::UnknownHunk(*id))?;
            *slot = Some(choice);
        }
        let choice_for = |id: u32| chosen[id as usize].ok_or(ResolveError::Unresolved(id));
        let t = &self.template;

        let mut prefix = t.prefix.clone().unwrap_or_default();
        let mut fm_doc: Option<Document> = None;
        for (i, values) in t.values.iter().enumerate() {
            let Some(values) = values else { continue };
            let id = u32::try_from(i).unwrap_or(u32::MAX);
            let value = match choice_for(id)? {
                Choice::Ours => &values.ours,
                Choice::Theirs => &values.theirs,
                Choice::Base => &values.base,
                Choice::Value(v) => v,
                _ => return Err(ResolveError::WrongChoice(id)),
            };
            if *value == values.ours {
                continue;
            }
            let Location::Frontmatter { key } = &self.hunks[i].location else {
                return Err(ResolveError::WrongChoice(id));
            };
            let doc = fm_doc.get_or_insert_with(|| Document::parse(&prefix));
            value
                .write(doc.frontmatter_mut(), key)
                .map_err(|()| ResolveError::Unwritable(id))?;
        }
        if let Some(doc) = fm_doc {
            prefix = doc.render();
        }

        let mut body = String::new();
        for piece in &t.body {
            match piece {
                TemplatePiece::Text { text } => body.push_str(text),
                TemplatePiece::Hunk {
                    id,
                    base,
                    ours,
                    theirs,
                } => match choice_for(*id)? {
                    Choice::Ours => body.push_str(ours),
                    Choice::Theirs => body.push_str(theirs),
                    Choice::Base => body.push_str(base),
                    Choice::OursThenTheirs => {
                        body.push_str(ours);
                        body.push_str(theirs);
                    }
                    Choice::TheirsThenOurs => {
                        body.push_str(theirs);
                        body.push_str(ours);
                    }
                    Choice::Text(text) => body.push_str(text),
                    Choice::Value(_) => return Err(ResolveError::WrongChoice(*id)),
                },
            }
        }
        if t.strip_final_newline {
            lines::strip_final_terminator(&mut body);
        }
        Ok(prefix + &body)
    }
}

fn hunk_id(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

fn build(
    prefix: Option<String>,
    fm_conflicts: Vec<FmConflict>,
    mut auto: Vec<AutoResolved>,
    body: &TextMerge,
) -> MergeOutcome {
    auto.extend(body.auto.iter().cloned());
    if fm_conflicts.is_empty() && body.hunks.is_empty() {
        let text = body.clean_text().unwrap_or_default();
        return MergeOutcome::Clean(prefix.unwrap_or_default() + &text);
    }
    let offset = fm_conflicts.len();
    let mut hunks = Vec::new();
    let mut values = Vec::new();
    for (i, c) in fm_conflicts.into_iter().enumerate() {
        hunks.push(ConflictHunk {
            id: hunk_id(i),
            location: Location::Frontmatter { key: c.key },
            kind: ConflictKind::BothModified,
            base: c.base_raw,
            ours: c.ours_raw,
            theirs: c.theirs_raw,
        });
        values.push(Some(HunkValues {
            base: c.base,
            ours: c.ours,
            theirs: c.theirs,
        }));
    }
    for (i, h) in body.hunks.iter().enumerate() {
        hunks.push(ConflictHunk {
            id: hunk_id(offset + i),
            location: Location::Body {
                base_line: hunk_id(h.lines.0),
                ours_line: hunk_id(h.lines.1),
                theirs_line: hunk_id(h.lines.2),
            },
            kind: h.kind,
            base: h.base.clone(),
            ours: h.ours.clone(),
            theirs: h.theirs.clone(),
        });
        values.push(None);
    }
    let pieces = body
        .segments
        .iter()
        .map(|s| match s {
            Segment::Text(text) => TemplatePiece::Text { text: text.clone() },
            Segment::Hunk(i) => {
                let h = &body.hunks[*i];
                TemplatePiece::Hunk {
                    id: hunk_id(offset + i),
                    base: h.base_out.clone(),
                    ours: h.ours_out.clone(),
                    theirs: h.theirs_out.clone(),
                }
            }
        })
        .collect();
    let merged_with_markers =
        (offset == 0).then(|| prefix.clone().unwrap_or_default() + &body.with_markers());
    MergeOutcome::Conflicted(Conflicted {
        merged_with_markers,
        hunks,
        auto_resolved: auto,
        template: ResolutionTemplate {
            prefix,
            body: pieces,
            strip_final_newline: body.strip_final_newline,
            values,
        },
    })
}

/// Merges plain text line by line (no frontmatter handling).
pub fn merge_text_only(base: &str, ours: &str, theirs: &str) -> MergeOutcome {
    build(
        None,
        Vec::new(),
        Vec::new(),
        &merge_text(base, ours, theirs),
    )
}

/// 3-way merges a note file (see the module docs).
pub fn merge(base: &str, ours: &str, theirs: &str) -> MergeOutcome {
    if ours == theirs || theirs == base {
        return MergeOutcome::Clean(ours.to_owned());
    }
    if ours == base {
        return MergeOutcome::Clean(theirs.to_owned());
    }
    let (bd, od, td) = (
        Document::parse(base),
        Document::parse(ours),
        Document::parse(theirs),
    );
    let Some(fm) = merge_frontmatter(&bd, &od, &td) else {
        return merge_text_only(base, ours, theirs);
    };
    let body = merge_text(bd.body(), od.body(), td.body());
    build(Some(fm.prefix), fm.conflicts, fm.auto, &body)
}

/// What to do with a pushed `note.update` (server) or a pulled change over a local pending
/// edit (device), per D19.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDecision {
    /// The edit was based on the current version: write it as is.
    FastForward,
    /// The current content already equals the edit: nothing to write.
    AlreadyApplied,
    /// Stale base, merged cleanly: write this text.
    Merged(String),
    /// Stale base, overlapping edits: keep the current content and surface the conflict
    /// (the server also saves the edit as a conflict copy).
    Conflict(Box<Conflicted>),
}

/// Decides a note update. `current` is the version that stands (server content, or the
/// pulled server content on the device), `edit` the incoming content, `base` the content
/// at `base_version` (from git; `None` if it is unavailable, which merges against an empty
/// base).
pub fn decide_update(
    base_version: &Version,
    base: Option<&str>,
    current: &str,
    edit: &str,
) -> UpdateDecision {
    if current == edit {
        return UpdateDecision::AlreadyApplied;
    }
    if base_version.matches(current) {
        return UpdateDecision::FastForward;
    }
    match merge(base.unwrap_or(""), current, edit) {
        MergeOutcome::Clean(text) if text == current => UpdateDecision::AlreadyApplied,
        MergeOutcome::Clean(text) => UpdateDecision::Merged(text),
        MergeOutcome::Conflicted(c) => UpdateDecision::Conflict(Box::new(c)),
    }
}
