//! Line diffs for display: the history panel's revision diff and the conflict screen's
//! per-line annotations of the base, local and server columns (Myers diff from `similar`, the
//! same algorithm `sync-model`'s merge uses).

use std::collections::HashSet;

use similar::{DiffOp, TextDiff};

use crate::format::direction::dir_of;
use crate::format::labels::{LINES, Lang, tr};
use crate::view::model::{AnnotatedLine, DiffLine, DiffLineKind, LineChange};

fn lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n')
        .map(|l| l.trim_end_matches(['\n', '\r']))
        .collect()
}

fn n32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The diff of `old` → `new`, one entry per line.
pub fn line_diff(old: &str, new: &str) -> Vec<DiffLine> {
    let (a, b) = (lines(old), lines(new));
    let diff = TextDiff::from_slices(&a, &b);
    let mut out = Vec::new();
    for op in diff.ops() {
        match *op {
            DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                for k in 0..len {
                    out.push(DiffLine {
                        kind: DiffLineKind::Same,
                        old_line: Some(n32(old_index + k + 1)),
                        new_line: Some(n32(new_index + k + 1)),
                        text: a[old_index + k].to_owned(),
                        dir: dir_of(a[old_index + k]),
                    });
                }
            }
            DiffOp::Delete {
                old_index, old_len, ..
            } => removed(&mut out, &a, old_index, old_len),
            DiffOp::Insert {
                new_index, new_len, ..
            } => added(&mut out, &b, new_index, new_len),
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                removed(&mut out, &a, old_index, old_len);
                added(&mut out, &b, new_index, new_len);
            }
        }
    }
    out
}

fn removed(out: &mut Vec<DiffLine>, a: &[&str], from: usize, len: usize) {
    for (k, line) in a.iter().enumerate().skip(from).take(len) {
        out.push(DiffLine {
            kind: DiffLineKind::Removed,
            old_line: Some(n32(k + 1)),
            new_line: None,
            text: (*line).to_owned(),
            dir: dir_of(line),
        });
    }
}

fn added(out: &mut Vec<DiffLine>, b: &[&str], from: usize, len: usize) {
    for (k, line) in b.iter().enumerate().skip(from).take(len) {
        out.push(DiffLine {
            kind: DiffLineKind::Added,
            old_line: None,
            new_line: Some(n32(k + 1)),
            text: (*line).to_owned(),
            dir: dir_of(line),
        });
    }
}

/// "+2 lines, 1 changed, 1 removed" (replaced lines count as changed).
pub fn summary(old: &str, new: &str, lang: Lang) -> String {
    let (a, b) = (lines(old), lines(new));
    let diff = TextDiff::from_slices(&a, &b);
    let (mut plus, mut changed, mut minus) = (0usize, 0usize, 0usize);
    for op in diff.ops() {
        match *op {
            DiffOp::Equal { .. } => {}
            DiffOp::Delete { old_len, .. } => minus += old_len,
            DiffOp::Insert { new_len, .. } => plus += new_len,
            DiffOp::Replace {
                old_len, new_len, ..
            } => {
                changed += old_len.min(new_len);
                plus += new_len.saturating_sub(old_len);
                minus += old_len.saturating_sub(new_len);
            }
        }
    }
    let mut parts = Vec::new();
    if plus > 0 {
        parts.push(format!(
            "+{}",
            LINES.of(i64::try_from(plus).unwrap_or(i64::MAX), lang)
        ));
    }
    if changed > 0 {
        parts.push(match lang {
            Lang::En => format!("{changed} changed"),
            Lang::Ar => format!("{changed} معدّل"),
        });
    }
    if minus > 0 {
        parts.push(match lang {
            Lang::En => format!("{minus} removed"),
            Lang::Ar => format!("{minus} محذوف"),
        });
    }
    if parts.is_empty() {
        return tr(lang, "No changes", "لا تغييرات");
    }
    parts.join(match lang {
        Lang::En => ", ",
        Lang::Ar => "، ",
    })
}

/// Base lines a side touched: (replaced, deleted).
struct Touched {
    replaced: HashSet<usize>,
    deleted: HashSet<usize>,
}

fn touched(base: &[&str], side: &[&str]) -> Touched {
    let diff = TextDiff::from_slices(base, side);
    let mut t = Touched {
        replaced: HashSet::new(),
        deleted: HashSet::new(),
    };
    for op in diff.ops() {
        match *op {
            DiffOp::Delete {
                old_index, old_len, ..
            } => t.deleted.extend(old_index..old_index + old_len),
            DiffOp::Replace {
                old_index, old_len, ..
            } => t.replaced.extend(old_index..old_index + old_len),
            _ => {}
        }
    }
    t
}

/// One conflict column (`side` = local or server) annotated against `base`; lines this side
/// changed where the `other` side changed the same base lines are `ChangedBoth`.
pub fn annotate_side(base: &str, side: &str, other: &str) -> Vec<AnnotatedLine> {
    let (b, s, o) = (lines(base), lines(side), lines(other));
    let theirs = touched(&b, &o);
    let diff = TextDiff::from_slices(&b, &s);
    let mut out = Vec::new();
    let mut push = |k: usize, change: LineChange| {
        out.push(AnnotatedLine {
            line: n32(k + 1),
            text: s[k].to_owned(),
            change,
            dir: dir_of(s[k]),
        });
    };
    for op in diff.ops() {
        match *op {
            DiffOp::Equal { new_index, len, .. } => {
                for k in new_index..new_index + len {
                    push(k, LineChange::Same);
                }
            }
            DiffOp::Insert {
                new_index, new_len, ..
            } => {
                for k in new_index..new_index + new_len {
                    push(k, LineChange::Added);
                }
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                let both = (old_index..old_index + old_len)
                    .any(|i| theirs.replaced.contains(&i) || theirs.deleted.contains(&i));
                for k in new_index..new_index + new_len {
                    push(
                        k,
                        if both {
                            LineChange::ChangedBoth
                        } else {
                            LineChange::Changed
                        },
                    );
                }
            }
            DiffOp::Delete { .. } => {}
        }
    }
    out
}

/// The base column: lines changed or removed by either side.
pub fn annotate_base(base: &str, local: &str, server: &str) -> Vec<AnnotatedLine> {
    let b = lines(base);
    let ours = touched(&b, &lines(local));
    let theirs = touched(&b, &lines(server));
    b.iter()
        .enumerate()
        .map(|(i, text)| {
            let o = ours.replaced.contains(&i) || ours.deleted.contains(&i);
            let t = theirs.replaced.contains(&i) || theirs.deleted.contains(&i);
            let change = match (o, t) {
                (true, true) => LineChange::ChangedBoth,
                (false, false) => LineChange::Same,
                _ if ours.deleted.contains(&i) || theirs.deleted.contains(&i) => {
                    LineChange::Removed
                }
                _ => LineChange::Changed,
            };
            AnnotatedLine {
                line: n32(i + 1),
                text: (*text).to_owned(),
                change,
                dir: dir_of(text),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::model::TextDir;

    #[test]
    fn revision_diff_numbers_lines() {
        let old = "a\nb\nc\n";
        let new = "a\nB\nc\nd\n";
        let diff = line_diff(old, new);
        let kinds: Vec<(DiffLineKind, Option<u32>, Option<u32>, &str)> = diff
            .iter()
            .map(|l| (l.kind, l.old_line, l.new_line, l.text.as_str()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                (DiffLineKind::Same, Some(1), Some(1), "a"),
                (DiffLineKind::Removed, Some(2), None, "b"),
                (DiffLineKind::Added, None, Some(2), "B"),
                (DiffLineKind::Same, Some(3), Some(3), "c"),
                (DiffLineKind::Added, None, Some(4), "d"),
            ]
        );
        assert_eq!(summary(old, new, Lang::En), "+1 line, 1 changed");
        assert_eq!(summary("a\nb\n", "a\n", Lang::En), "1 removed");
        assert_eq!(summary("a\n", "a\n", Lang::En), "No changes");
        assert_eq!(summary("a\n", "a\nb\nc\n", Lang::Ar), "+سطران");
    }

    #[test]
    fn conflict_columns_mark_both_sides() {
        let base = "title\nprice 10\nnote\n";
        let local = "title\nprice 12\nnote\nextra\n";
        let server = "title\nسعر 15\nnote\n";
        let l = annotate_side(base, local, server);
        assert_eq!(
            l.iter().map(|a| (a.line, a.change)).collect::<Vec<_>>(),
            vec![
                (1, LineChange::Same),
                (2, LineChange::ChangedBoth),
                (3, LineChange::Same),
                (4, LineChange::Added),
            ]
        );
        let s = annotate_side(base, server, local);
        assert_eq!(s[1].change, LineChange::ChangedBoth);
        assert_eq!(s[1].dir, TextDir::Rtl);
        let b = annotate_base(base, local, "title\nprice 10\n");
        assert_eq!(
            b.iter().map(|a| a.change).collect::<Vec<_>>(),
            vec![LineChange::Same, LineChange::Changed, LineChange::Removed]
        );
    }
}
