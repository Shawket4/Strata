//! Line-wise 3-way merge of text (note bodies, or whole files whose frontmatter cannot be
//! merged key by key).

use std::collections::HashMap;
use std::ops::Range;

use vault_format::tasks::TaskLine;

use super::diff3::{Region, regions};
use super::lines::{Prepared, prepare, strip_final_terminator};
use super::{AutoResolution, AutoResolved, ConflictKind, Location};

/// A piece of merged output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Segment {
    /// Merged text (output form).
    Text(String),
    /// A conflict, by index into [`TextMerge::hunks`].
    Hunk(usize),
}

/// A conflict region of a text merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextHunk {
    /// 0-based first line in base, ours, theirs.
    pub lines: (usize, usize, usize),
    /// Kind.
    pub kind: ConflictKind,
    /// Original bytes of each side (for display).
    pub base: String,
    pub ours: String,
    pub theirs: String,
    /// The same in output form (for resolving).
    pub base_out: String,
    pub ours_out: String,
    pub theirs_out: String,
}

/// Result of a text merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextMerge {
    pub segments: Vec<Segment>,
    pub hunks: Vec<TextHunk>,
    pub auto: Vec<AutoResolved>,
    /// Remove the final terminator from the assembled output.
    pub strip_final_newline: bool,
    /// Terminator for conflict marker lines.
    pub terminator: &'static str,
}

impl TextMerge {
    /// The merged text, if there are no conflicts.
    pub fn clean_text(&self) -> Option<String> {
        if !self.hunks.is_empty() {
            return None;
        }
        Some(self.assemble(|_| String::new()))
    }

    /// Assembles the output, rendering each hunk with `hunk`.
    pub fn assemble(&self, mut hunk: impl FnMut(usize) -> String) -> String {
        let mut out = String::new();
        for s in &self.segments {
            match s {
                Segment::Text(t) => out.push_str(t),
                Segment::Hunk(i) => out.push_str(&hunk(*i)),
            }
        }
        if self.strip_final_newline {
            strip_final_terminator(&mut out);
        }
        out
    }

    /// The output with git-style diff3 conflict markers.
    pub fn with_markers(&self) -> String {
        let eol = self.terminator;
        self.assemble(|i| {
            let h = &self.hunks[i];
            let mut s = format!("<<<<<<< ours{eol}");
            push_block(&mut s, &h.ours_out, eol);
            s.push_str(&format!("||||||| base{eol}"));
            push_block(&mut s, &h.base_out, eol);
            s.push_str(&format!("======={eol}"));
            push_block(&mut s, &h.theirs_out, eol);
            s.push_str(&format!(">>>>>>> theirs{eol}"));
            s
        })
    }
}

fn push_block(out: &mut String, block: &str, eol: &str) {
    out.push_str(block);
    if !block.is_empty() && !block.ends_with('\n') {
        out.push_str(eol);
    }
}

fn is_task_line(line: &str) -> bool {
    let content = line.trim_end_matches(['\n', '\r']);
    TaskLine::parse(content).is_some()
}

fn task_block_id(line: &str) -> Option<String> {
    let content = line.trim_end_matches(['\n', '\r']);
    TaskLine::parse(content).and_then(|t| t.block_id().map(str::to_owned))
}

fn count_task_ids<'a>(lines: impl IntoIterator<Item = &'a String>) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for l in lines {
        if let Some(id) = task_block_id(l) {
            *counts.entry(id).or_insert(0) += 1;
        }
    }
    counts
}

fn conflict_kind(p: &Prepared, base: &Range<usize>, ours: &Range<usize>, theirs: &Range<usize>) -> ConflictKind {
    let any_task = p.base.lines[base.clone()]
        .iter()
        .chain(&p.ours.lines[ours.clone()])
        .chain(&p.theirs.lines[theirs.clone()])
        .any(|l| is_task_line(l));
    if any_task {
        ConflictKind::TaskLine
    } else if base.is_empty() {
        ConflictKind::BothAdded
    } else if ours.is_empty() || theirs.is_empty() {
        ConflictKind::ModifyDelete
    } else {
        ConflictKind::BothModified
    }
}

/// Where a one-sided region ends up when the task guard turns it into a conflict.
#[derive(Debug, Clone)]
enum Planned {
    Region(Region),
    TaskPlacement(Region),
}

/// Turns one-sided regions that would duplicate a task (same block ID more often than in
/// either side) into conflicts: the task was placed on both sides (e.g. moved to two
/// different places), and silently keeping two copies would garble the task list.
fn guard_tasks(p: &Prepared, regions: Vec<Region>) -> Vec<Planned> {
    let ours_counts = count_task_ids(&p.ours.lines);
    let theirs_counts = count_task_ids(&p.theirs.lines);
    let mut result_counts: HashMap<String, usize> = HashMap::new();
    for r in &regions {
        let lines: &[String] = match r {
            Region::Unchanged { base } => &p.base.lines[base.clone()],
            Region::Ours { ours, .. } | Region::Both { ours, .. } => &p.ours.lines[ours.clone()],
            Region::Theirs { theirs, .. } => &p.theirs.lines[theirs.clone()],
            Region::Conflict { .. } => &[],
        };
        for (id, n) in count_task_ids(lines) {
            *result_counts.entry(id).or_insert(0) += n;
        }
    }
    let excess: Vec<String> = result_counts
        .into_iter()
        .filter(|(id, n)| {
            *n > ours_counts
                .get(id)
                .copied()
                .unwrap_or(0)
                .max(theirs_counts.get(id).copied().unwrap_or(0))
        })
        .map(|(id, _)| id)
        .collect();
    regions
        .into_iter()
        .map(|r| {
            let lines: &[String] = match &r {
                Region::Ours { ours, .. } => &p.ours.lines[ours.clone()],
                Region::Theirs { theirs, .. } => &p.theirs.lines[theirs.clone()],
                _ => return Planned::Region(r),
            };
            let hit = lines
                .iter()
                .filter_map(|l| task_block_id(l))
                .any(|id| excess.contains(&id));
            if hit {
                Planned::TaskPlacement(r)
            } else {
                Planned::Region(r)
            }
        })
        .collect()
}

fn body_location(base: &Range<usize>, ours: &Range<usize>, theirs: &Range<usize>) -> Location {
    Location::Body {
        base_line: line_number(base.start),
        ours_line: line_number(ours.start),
        theirs_line: line_number(theirs.start),
    }
}

fn line_number(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// Removes the lines ours and theirs share at both ends of a pure-insertion conflict (the
/// base is empty there, so they were added identically by both sides).
fn trim_insertion(
    p: &Prepared,
    ours: &mut Range<usize>,
    theirs: &mut Range<usize>,
) -> (Vec<String>, Vec<String>) {
    let mut prefix = Vec::new();
    while ours.start < ours.end && theirs.start < theirs.end && p.ours.lines[ours.start] == p.theirs.lines[theirs.start] {
        prefix.push(p.ours.lines[ours.start].clone());
        ours.start += 1;
        theirs.start += 1;
    }
    let mut suffix = Vec::new();
    while ours.start < ours.end && theirs.start < theirs.end && p.ours.lines[ours.end - 1] == p.theirs.lines[theirs.end - 1] {
        suffix.push(p.ours.lines[ours.end - 1].clone());
        ours.end -= 1;
        theirs.end -= 1;
    }
    suffix.reverse();
    (prefix, suffix)
}

/// Merges three texts line by line.
pub(crate) fn merge_text(base: &str, ours: &str, theirs: &str) -> TextMerge {
    let p = prepare(base, ours, theirs);
    let planned = guard_tasks(&p, regions(&p.base.lines, &p.ours.lines, &p.theirs.lines));
    let mut segments: Vec<Segment> = Vec::new();
    let mut hunks = Vec::new();
    let mut auto = Vec::new();
    let push_text = |segments: &mut Vec<Segment>, text: String| {
        if text.is_empty() {
            return;
        }
        if let Some(Segment::Text(prev)) = segments.last_mut() {
            prev.push_str(&text);
        } else {
            segments.push(Segment::Text(text));
        }
    };
    if p.line_endings_changed() {
        auto.push(AutoResolved {
            location: Location::LineEndings,
            resolution: if p.line_endings_from_ours() {
                AutoResolution::TookOurs
            } else {
                AutoResolution::TookTheirs
            },
        });
    }
    for plan in planned {
        let (region, forced) = match plan {
            Planned::Region(r) => (r, false),
            Planned::TaskPlacement(r) => (r, true),
        };
        match region {
            Region::Unchanged { base } => push_text(&mut segments, p.output(&p.base.lines[base])),
            Region::Ours { base, ours, theirs }
            | Region::Theirs { base, ours, theirs }
            | Region::Both { base, ours, theirs }
                if !forced =>
            {
                let (lines, resolution) = match &region_kind(&base, &ours, &theirs, &p) {
                    Taken::Ours => (&p.ours.lines[ours.clone()], AutoResolution::TookOurs),
                    Taken::Theirs => (&p.theirs.lines[theirs.clone()], AutoResolution::TookTheirs),
                    Taken::Identical => (&p.ours.lines[ours.clone()], AutoResolution::Identical),
                };
                push_text(&mut segments, p.output(lines));
                auto.push(AutoResolved {
                    location: body_location(&base, &ours, &theirs),
                    resolution,
                });
            }
            Region::Ours { base, ours, theirs }
            | Region::Theirs { base, ours, theirs }
            | Region::Both { base, ours, theirs }
            | Region::Conflict { base, ours, theirs } => {
                let (mut ours, mut theirs) = (ours, theirs);
                let kind = if forced {
                    ConflictKind::TaskPlacement
                } else {
                    conflict_kind(&p, &base, &ours, &theirs)
                };
                if base.is_empty() && !forced {
                    let (prefix, suffix) = trim_insertion(&p, &mut ours, &mut theirs);
                    push_text(&mut segments, p.output(&prefix));
                    push_hunk(&p, &mut segments, &mut hunks, kind, &base, &ours, &theirs);
                    push_text(&mut segments, p.output(&suffix));
                } else {
                    push_hunk(&p, &mut segments, &mut hunks, kind, &base, &ours, &theirs);
                }
            }
        }
    }
    TextMerge {
        segments,
        hunks,
        auto,
        strip_final_newline: p.missing_final_newline,
        terminator: p.terminator(),
    }
}

enum Taken {
    Ours,
    Theirs,
    Identical,
}

fn region_kind(base: &Range<usize>, ours: &Range<usize>, theirs: &Range<usize>, p: &Prepared) -> Taken {
    let o = &p.ours.lines[ours.clone()];
    let t = &p.theirs.lines[theirs.clone()];
    let b = &p.base.lines[base.clone()];
    if o == t {
        Taken::Identical
    } else if t == b {
        Taken::Ours
    } else {
        Taken::Theirs
    }
}

fn push_hunk(
    p: &Prepared,
    segments: &mut Vec<Segment>,
    hunks: &mut Vec<TextHunk>,
    kind: ConflictKind,
    base: &Range<usize>,
    ours: &Range<usize>,
    theirs: &Range<usize>,
) {
    segments.push(Segment::Hunk(hunks.len()));
    hunks.push(TextHunk {
        lines: (base.start, ours.start, theirs.start),
        kind,
        base: p.base.original(base.clone()),
        ours: p.ours.original(ours.clone()),
        theirs: p.theirs.original(theirs.clone()),
        base_out: p.output(&p.base.lines[base.clone()]),
        ours_out: p.output(&p.ours.lines[ours.clone()]),
        theirs_out: p.output(&p.theirs.lines[theirs.clone()]),
    });
}
