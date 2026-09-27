//! diff3 table tests for the line merge (`merge_text_only`, no fast paths).

use pretty_assertions::assert_eq;
use sync_model::{ConflictKind, MergeOutcome, merge_text_only};

#[derive(Debug, PartialEq)]
enum Want {
    Clean(&'static str),
    /// Text with markers, and the kind of each hunk.
    Conflict(&'static str, &'static [ConflictKind]),
}

use ConflictKind::{BothAdded, BothModified, ModifyDelete, TaskLine, TaskPlacement};
use Want::{Clean, Conflict};

#[rustfmt::skip]
const CASES: &[(&str, &str, &str, &str, Want)] = &[
    // name, base, ours, theirs, expected
    ("all empty", "", "", "", Clean("")),
    ("ours adds to empty", "", "a\n", "", Clean("a\n")),
    ("both add same to empty", "", "a\nb\n", "a\nb\n", Clean("a\nb\n")),
    ("both add different to empty", "", "a\n", "b\n",
        Conflict("<<<<<<< ours\na\n||||||| base\n=======\nb\n>>>>>>> theirs\n", &[BothAdded])),
    ("non-overlapping edits", "a\nb\nc\nd\ne\n", "A\nb\nc\nd\ne\n", "a\nb\nc\nd\nE\n", Clean("A\nb\nc\nd\nE\n")),
    ("same line changed differently", "a\nb\nc\n", "a\nX\nc\n", "a\nY\nc\n",
        Conflict("a\n<<<<<<< ours\nX\n||||||| base\nb\n=======\nY\n>>>>>>> theirs\nc\n", &[BothModified])),
    ("identical change collapses", "a\nb\nc\n", "a\nX\nc\n", "a\nX\nc\n", Clean("a\nX\nc\n")),
    ("delete vs modify", "a\nb\nc\n", "a\nc\n", "a\nB\nc\n",
        Conflict("a\n<<<<<<< ours\n||||||| base\nb\n=======\nB\n>>>>>>> theirs\nc\n", &[ModifyDelete])),
    ("delete one line, edit another", "a\nb\nc\nd\n", "a\nc\nd\n", "a\nb\nc\nD\n", Clean("a\nc\nD\n")),
    ("both delete same line", "a\nb\nc\n", "a\nc\n", "a\nc\n", Clean("a\nc\n")),
    ("insert at start, edit at end", "a\nb\n", "0\na\nb\n", "a\nB\n", Clean("0\na\nB\n")),
    ("both append different", "a\n", "a\nx\n", "a\ny\n",
        Conflict("a\n<<<<<<< ours\nx\n||||||| base\n=======\ny\n>>>>>>> theirs\n", &[BothAdded])),
    ("adjacent line edits are clean", "a\nb\nc\nd\n", "a\nB\nc\nd\n", "a\nb\nC\nd\n", Clean("a\nB\nC\nd\n")),
    ("insertion before the other's change", "a\nb\nc\n", "a\nb\nX\nc\n", "a\nb\nC\n", Clean("a\nb\nX\nC\n")),
    ("insertion after the other's change", "a\nb\nc\n", "a\nB\nc\n", "a\nb\nY\nc\n", Clean("a\nB\nY\nc\n")),
    ("insertion inside the other's change conflicts", "a\nb\nc\nd\n", "a\nb\nX\nc\nd\n", "a\nd\n",
        Conflict("a\n<<<<<<< ours\nb\nX\nc\n||||||| base\nb\nc\n=======\n>>>>>>> theirs\nd\n", &[ModifyDelete])),
    ("crlf non-overlapping edits", "a\r\nb\r\nc\r\n", "A\r\nb\r\nc\r\n", "a\r\nb\r\nC\r\n", Clean("A\r\nb\r\nC\r\n")),
    ("crlf conversion on one side merges", "a\nb\nc\n", "a\r\nb\r\nc\r\n", "a\nb\nC\n", Clean("a\r\nb\r\nC\r\n")),
    ("crlf conversion plus edits", "a\nb\nc\n", "A\r\nb\r\nc\r\n", "a\nb\nC\n", Clean("A\r\nb\r\nC\r\n")),
    ("both convert to crlf", "a\nb\nc\n", "A\r\nb\r\nc\r\n", "a\r\nb\r\nC\r\n", Clean("A\r\nb\r\nC\r\n")),
    ("crlf conflict markers use crlf", "a\r\nb\r\n", "a\r\nX\r\n", "a\r\nY\r\n",
        Conflict("a\r\n<<<<<<< ours\r\nX\r\n||||||| base\r\nb\r\n=======\r\nY\r\n>>>>>>> theirs\r\n", &[BothModified])),
    ("mixed endings compare exactly", "a\nb\r\nc\n", "A\nb\r\nc\n", "a\nb\r\nC\n", Clean("A\nb\r\nC\n")),
    ("ours drops final newline", "a\nb\n", "a\nb", "A\nb\n", Clean("A\nb")),
    ("ours adds final newline, theirs appends", "a", "a\n", "a\nb", Clean("a\nb\n")),
    ("append to file without final newline", "a\nb", "A\nb", "a\nb\nc", Clean("A\nb\nc")),
    ("both append without final newline", "a", "a\nx", "a\ny",
        Conflict("a\n<<<<<<< ours\nx\n||||||| base\n=======\ny\n>>>>>>> theirs", &[BothAdded])),
    ("arabic non-overlapping", "سطر أول\nسطر ثاني\nسطر ثالث\n", "سطر أول معدل\nسطر ثاني\nسطر ثالث\n",
        "سطر أول\nسطر ثاني\nسطر ثالث — تعديل\n", Clean("سطر أول معدل\nسطر ثاني\nسطر ثالث — تعديل\n")),
    ("arabic conflict", "مرحبا\nعالم\n", "مرحبا\nدنيا\n", "مرحبا\nكون\n",
        Conflict("مرحبا\n<<<<<<< ours\nدنيا\n||||||| base\nعالم\n=======\nكون\n>>>>>>> theirs\n", &[BothModified])),
    ("moved block, edit elsewhere", "A1\nA2\nm\nn\no\n", "m\nn\no\nA1\nA2\n", "A1\nA2\nm\nN\no\n", Clean("m\nN\no\nA1\nA2\n")),
    // The cheapest diff of ours moves `m n` up (not `A1 A2` down), so theirs' edit inside the
    // block still applies.
    ("moved block, edit inside it", "A1\nA2\nm\nn\n", "m\nn\nA1\nA2\n", "A1\nA2x\nm\nn\n", Clean("m\nn\nA1\nA2x\n")),
    // A move is a delete plus an insert: both deleted `a`, ours re-inserted it (as git does).
    ("moved line, other side deletes it", "a\nb\nc\nd\n", "b\nc\nd\na\n", "b\nc\nd\n", Clean("b\nc\nd\na\n")),
    ("both move the same block", "A\nm\nn\n", "m\nn\nA\n", "m\nn\nA\n", Clean("m\nn\nA\n")),
    ("task toggle vs task text edit", "# T\n- [ ] pay rent ^t-1\nend\n", "# T\n- [x] pay rent ✅ 2026-09-27 ^t-1\nend\n",
        "# T\n- [ ] pay the rent ^t-1\nend\n",
        Conflict("# T\n<<<<<<< ours\n- [x] pay rent ✅ 2026-09-27 ^t-1\n||||||| base\n- [ ] pay rent ^t-1\n=======\n- [ ] pay the rent ^t-1\n>>>>>>> theirs\nend\n", &[TaskLine])),
    ("different tasks toggled", "- [ ] a ^t-1\nx\n- [ ] b ^t-2\n", "- [x] a ^t-1\nx\n- [ ] b ^t-2\n",
        "- [ ] a ^t-1\nx\n- [x] b ^t-2\n", Clean("- [x] a ^t-1\nx\n- [x] b ^t-2\n")),
    ("task moved to two places", "- [ ] t ^t-1\na\nb\nc\nd\n", "a\nb\n- [ ] t ^t-1\nc\nd\n", "a\nb\nc\nd\n- [ ] t ^t-1\n",
        Conflict("a\nb\n<<<<<<< ours\n- [ ] t ^t-1\n||||||| base\n=======\n>>>>>>> theirs\nc\nd\n<<<<<<< ours\n||||||| base\n=======\n- [ ] t ^t-1\n>>>>>>> theirs\n", &[TaskPlacement, TaskPlacement])),
    // Here ours' cheapest diff moves `a` instead of the task: no duplicate, theirs' place wins.
    ("task moved one line on one side", "- [ ] t ^t-1\na\nb\nc\n", "a\n- [ ] t ^t-1\nb\nc\n", "a\nb\nc\n- [ ] t ^t-1\n",
        Clean("a\nb\nc\n- [ ] t ^t-1\n")),
    ("ours empties the file", "a\nb\n", "", "a\nb\n", Clean("")),
    ("ours empties, theirs edits", "a\nb\n", "", "a\nB\n",
        Conflict("<<<<<<< ours\n||||||| base\na\nb\n=======\na\nB\n>>>>>>> theirs\n", &[ModifyDelete])),
    ("shared insertion edges are trimmed", "a\nz\n", "a\n1\n2\nX\n3\nz\n", "a\n1\n2\nY\n3\nz\n",
        Conflict("a\n1\n2\n<<<<<<< ours\nX\n||||||| base\n=======\nY\n>>>>>>> theirs\n3\nz\n", &[BothAdded])),
    ("emoji and bare cr are text", "a\rb\n🙂\n", "a\rb\n🙂🙂\n", "A\rb\n🙂\n", Clean("A\rb\n🙂🙂\n")),
    ("theirs only", "a\nb\n", "a\nb\n", "a\nb\nc\n", Clean("a\nb\nc\n")),
    ("repeated lines", "x\nx\nx\n", "x\nx\nx\nx\n", "x\nx\n", Clean("x\nx\nx\n")),
];

fn actual(o: &MergeOutcome) -> Want {
    fn leak(s: &str) -> &'static str {
        Box::leak(s.to_owned().into_boxed_str())
    }
    match o {
        MergeOutcome::Clean(t) => Clean(leak(t)),
        MergeOutcome::Conflicted(c) => {
            let kinds: Vec<ConflictKind> = c.hunks.iter().map(|h| h.kind).collect();
            Conflict(
                leak(c.merged_with_markers.as_deref().expect("body-only conflicts have markers")),
                Box::leak(kinds.into_boxed_slice()),
            )
        }
    }
}

#[test]
fn table() {
    assert!(CASES.len() >= 30);
    let mut failures = Vec::new();
    for (name, base, ours, theirs, want) in CASES {
        let got = actual(&merge_text_only(base, ours, theirs));
        if &got != want {
            failures.push(format!("{name}:\n  want {want:?}\n  got  {got:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn clean_merges_are_symmetric_in_the_table() {
    for (name, base, ours, theirs, want) in CASES {
        if let Clean(text) = want {
            assert_eq!(merge_text_only(base, theirs, ours), MergeOutcome::Clean((*text).to_owned()), "{name}");
        }
    }
}
