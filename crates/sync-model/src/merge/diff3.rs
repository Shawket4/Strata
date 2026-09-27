//! diff3 over lines: two 2-way diffs from the base (via the `similar` crate's Myers
//! implementation) combined into regions of the base.
//!
//! Changes of the two sides are grouped when they **overlap** in the base:
//! - two changes that replace or delete lines overlap when their base ranges intersect;
//! - an insertion at base position `p` overlaps a change of lines `s..e` when `s < p < e`
//!   (an insertion exactly at the start or end of the other side's change does not: its
//!   place in the result is unambiguous);
//! - two insertions overlap when they are at the same position.
//!
//! A group with changes from one side only takes that side; a group where both sides made
//! the same change collapses; anything else is a conflict. The grouping depends only on the
//! two diffs, never on which side is called "ours", so clean merges are symmetric.

use std::collections::HashMap;
use std::ops::Range;

use similar::{Algorithm, DiffOp, capture_diff_slices};

/// Which input a change comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Who {
    Ours,
    Theirs,
}

/// A change of one side: `base` lines replaced by `side` lines.
#[derive(Debug, Clone)]
struct Change {
    who: Who,
    base: Range<usize>,
    side: Range<usize>,
}

/// A region of the base and what each side has there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Region {
    /// Nobody changed these base lines.
    Unchanged { base: Range<usize> },
    /// Only ours changed.
    Ours {
        base: Range<usize>,
        ours: Range<usize>,
        theirs: Range<usize>,
    },
    /// Only theirs changed.
    Theirs {
        base: Range<usize>,
        ours: Range<usize>,
        theirs: Range<usize>,
    },
    /// Both made the same change.
    Both {
        base: Range<usize>,
        ours: Range<usize>,
        theirs: Range<usize>,
    },
    /// Both changed, differently.
    Conflict {
        base: Range<usize>,
        ours: Range<usize>,
        theirs: Range<usize>,
    },
}

fn changes(who: Who, base: &[u32], side: &[u32]) -> Vec<Change> {
    let mut out: Vec<Change> = Vec::new();
    for op in capture_diff_slices(Algorithm::Myers, base, side) {
        let (base_range, side_range) = match op {
            DiffOp::Equal { .. } => continue,
            DiffOp::Delete {
                old_index,
                old_len,
                new_index,
            } => (old_index..old_index + old_len, new_index..new_index),
            DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => (old_index..old_index, new_index..new_index + new_len),
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => (old_index..old_index + old_len, new_index..new_index + new_len),
        };
        match out.last_mut() {
            Some(prev) if prev.base.end == base_range.start && prev.side.end == side_range.start => {
                prev.base.end = base_range.end;
                prev.side.end = side_range.end;
            }
            _ => out.push(Change {
                who,
                base: base_range,
                side: side_range,
            }),
        }
    }
    out
}

/// Interns lines so the diffs compare integers.
fn intern<'a>(table: &mut HashMap<&'a str, u32>, lines: &'a [String]) -> Vec<u32> {
    lines
        .iter()
        .map(|l| {
            let next = u32::try_from(table.len()).expect("fewer than 2^32 distinct lines");
            *table.entry(l.as_str()).or_insert(next)
        })
        .collect()
}

/// Computes the regions covering all of `base`, in order.
pub(crate) fn regions(base: &[String], ours: &[String], theirs: &[String]) -> Vec<Region> {
    let mut table = HashMap::new();
    let b = intern(&mut table, base);
    let o = intern(&mut table, ours);
    let t = intern(&mut table, theirs);

    let mut all = changes(Who::Ours, &b, &o);
    all.extend(changes(Who::Theirs, &b, &t));
    all.sort_by(|x, y| {
        (x.base.start, x.base.end, x.who).cmp(&(y.base.start, y.base.end, y.who))
    });

    // Group overlapping changes (see the module docs for the overlap rule).
    let mut groups: Vec<Vec<Change>> = Vec::new();
    let mut max_end: Option<usize> = None; // max end of non-empty members of the open group
    let mut last_insert: Option<usize> = None; // position of the last insertion in the group
    for c in all {
        let joins = match groups.last() {
            None => false,
            Some(_) if c.base.is_empty() => {
                let p = c.base.start;
                max_end.is_some_and(|e| p < e) || last_insert == Some(p)
            }
            Some(_) => max_end.is_some_and(|e| c.base.start < e),
        };
        if !joins {
            groups.push(Vec::new());
            max_end = None;
            last_insert = None;
        }
        if c.base.is_empty() {
            last_insert = Some(c.base.start);
        } else {
            max_end = Some(max_end.map_or(c.base.end, |e| e.max(c.base.end)));
        }
        groups
            .last_mut()
            .expect("a group was pushed above")
            .push(c);
    }

    let mut out = Vec::new();
    let mut pos = 0; // next base line
    // Offsets (side index - base index) after the last change of each side.
    let (mut off_o, mut off_t): (isize, isize) = (0, 0);
    let shift = |i: usize, off: isize| i.checked_add_signed(off).expect("diff offsets stay in range");
    for group in groups {
        let gs = group.iter().map(|c| c.base.start).min().unwrap_or(pos);
        let ge = group.iter().map(|c| c.base.end).max().unwrap_or(gs);
        if pos < gs {
            out.push(Region::Unchanged { base: pos..gs });
        }
        let range_for = |who: Who, off: &mut isize| -> (Range<usize>, bool) {
            let mine: Vec<&Change> = group.iter().filter(|c| c.who == who).collect();
            match (mine.first(), mine.last()) {
                (Some(first), Some(last)) => {
                    let start = first.side.start - (first.base.start - gs);
                    let end = last.side.end + (ge - last.base.end);
                    *off = signed(last.side.end) - signed(last.base.end);
                    (start..end, true)
                }
                _ => (shift(gs, *off)..shift(ge, *off), false),
            }
        };
        let (ours, o_changed) = range_for(Who::Ours, &mut off_o);
        let (theirs, t_changed) = range_for(Who::Theirs, &mut off_t);
        let base_range = gs..ge;
        out.push(match (o_changed, t_changed) {
            (true, false) => Region::Ours {
                base: base_range,
                ours,
                theirs,
            },
            (false, true) => Region::Theirs {
                base: base_range,
                ours,
                theirs,
            },
            _ if o[ours.clone()] == t[theirs.clone()] => Region::Both {
                base: base_range,
                ours,
                theirs,
            },
            _ => Region::Conflict {
                base: base_range,
                ours,
                theirs,
            },
        });
        pos = ge;
    }
    if pos < base.len() {
        out.push(Region::Unchanged {
            base: pos..base.len(),
        });
    }
    out
}

fn signed(i: usize) -> isize {
    isize::try_from(i).expect("line counts fit in isize")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.chars().map(|c| format!("{c}\n")).collect()
    }

    #[test]
    fn non_overlapping_changes_are_separate_regions() {
        let r = regions(&lines("abcde"), &lines("aXcde"), &lines("abcdY"));
        assert_eq!(
            r,
            [
                Region::Unchanged { base: 0..1 },
                Region::Ours {
                    base: 1..2,
                    ours: 1..2,
                    theirs: 1..2
                },
                Region::Unchanged { base: 2..4 },
                Region::Theirs {
                    base: 4..5,
                    ours: 4..5,
                    theirs: 4..5
                },
            ]
        );
    }

    #[test]
    fn insertion_at_change_boundary_does_not_overlap() {
        // ours inserts X before c; theirs changes c.
        let r = regions(&lines("abc"), &lines("abXc"), &lines("abY"));
        assert_eq!(
            r,
            [
                Region::Unchanged { base: 0..2 },
                Region::Ours {
                    base: 2..2,
                    ours: 2..3,
                    theirs: 2..2
                },
                Region::Theirs {
                    base: 2..3,
                    ours: 3..4,
                    theirs: 2..3
                },
            ]
        );
    }

    #[test]
    fn same_point_insertions_conflict_unless_equal() {
        let r = regions(&lines("ab"), &lines("aXb"), &lines("aYb"));
        assert_eq!(
            r[1],
            Region::Conflict {
                base: 1..1,
                ours: 1..2,
                theirs: 1..2
            }
        );
        let r = regions(&lines("ab"), &lines("aXb"), &lines("aXb"));
        assert_eq!(
            r[1],
            Region::Both {
                base: 1..1,
                ours: 1..2,
                theirs: 1..2
            }
        );
    }
}
