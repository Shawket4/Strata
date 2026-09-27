//! Merge properties (proptest).

use proptest::prelude::*;
use sync_model::{MergeOutcome, merge, merge_text_only};

fn line() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => "[a-d]{1,2}",
        1 => Just("- [ ] pay rent ^t-1".to_owned()),
        1 => Just("- [x] pay rent ✅ 2026-09-27 ^t-1".to_owned()),
        1 => Just("- [ ] call Shady ^t-2".to_owned()),
        1 => Just("سطر عربي".to_owned()),
        1 => Just(String::new()),
    ]
}

/// Edits `base` line by line: keep, delete, replace, or insert before; plus appended lines.
fn edited(base: Vec<String>) -> impl Strategy<Value = Vec<String>> {
    let n = base.len();
    prop::collection::vec((0..4_u8, line()), n + 1).prop_map(move |ops| {
        let mut out = Vec::new();
        for (i, (op, l)) in ops.into_iter().enumerate() {
            let Some(b) = base.get(i) else {
                if op == 3 {
                    out.push(l);
                }
                break;
            };
            match op {
                0 => out.push(b.clone()),
                1 => {}
                2 => out.push(l),
                _ => {
                    out.push(l);
                    out.push(b.clone());
                }
            }
        }
        out
    })
}

#[derive(Debug, Clone, Copy)]
struct Style {
    eol: u8,
    final_newline: bool,
}

fn style() -> impl Strategy<Value = Style> {
    (prop_oneof![6 => Just(0_u8), 3 => Just(1_u8), 1 => Just(2_u8)], prop::bool::weighted(0.8))
        .prop_map(|(eol, final_newline)| Style { eol, final_newline })
}

fn render(lines: &[String], s: Style) -> String {
    let mut out = String::new();
    for (i, l) in lines.iter().enumerate() {
        out.push_str(l);
        let last = i + 1 == lines.len();
        if last && !s.final_newline {
            break;
        }
        out.push_str(match s.eol {
            0 => "\n",
            1 => "\r\n",
            _ if i % 2 == 0 => "\r\n",
            _ => "\n",
        });
    }
    out
}

fn triple() -> impl Strategy<Value = (String, String, String)> {
    prop::collection::vec(line(), 0..8)
        .prop_flat_map(|base| (Just(base.clone()), edited(base.clone()), edited(base), style(), style(), style()))
        .prop_map(|(b, x, y, sb, sx, sy)| (render(&b, sb), render(&x, sx), render(&y, sy)))
}

fn content_lines(text: &str) -> Vec<String> {
    text.split_inclusive('\n')
        .map(|l| l.trim_end_matches(['\n', '\r']).to_owned())
        .collect()
}

/// Lines of `side` that are not in `base` must survive: in a clean result, or in the text
/// with conflict markers (which carries both sides of every hunk).
fn assert_no_loss(base: &str, side: &str, outcome: &MergeOutcome) -> Result<(), TestCaseError> {
    let base_lines = content_lines(base);
    let out = match outcome {
        MergeOutcome::Clean(t) => content_lines(t),
        MergeOutcome::Conflicted(c) => {
            let mut all = content_lines(c.merged_with_markers.as_deref().unwrap_or_default());
            for h in &c.hunks {
                all.extend(content_lines(&h.ours));
                all.extend(content_lines(&h.theirs));
            }
            all
        }
    };
    for l in content_lines(side) {
        if !base_lines.contains(&l) {
            prop_assert!(out.contains(&l), "line {l:?} lost: {outcome:?}");
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn theirs_unchanged_yields_ours((b, x, _y) in triple()) {
        prop_assert_eq!(merge_text_only(&b, &x, &b), MergeOutcome::Clean(x.clone()));
        prop_assert_eq!(merge(&b, &x, &b), MergeOutcome::Clean(x));
    }

    #[test]
    fn ours_unchanged_yields_theirs((b, _x, y) in triple()) {
        prop_assert_eq!(merge_text_only(&b, &b, &y), MergeOutcome::Clean(y.clone()));
        prop_assert_eq!(merge(&b, &b, &y), MergeOutcome::Clean(y));
    }

    #[test]
    fn identical_edits_collapse((b, x, _y) in triple()) {
        prop_assert_eq!(merge_text_only(&b, &x, &x), MergeOutcome::Clean(x.clone()));
        prop_assert_eq!(merge(&b, &x, &x), MergeOutcome::Clean(x));
    }

    #[test]
    fn clean_merges_commute((b, x, y) in triple()) {
        if let MergeOutcome::Clean(t) = merge_text_only(&b, &x, &y) {
            prop_assert_eq!(merge_text_only(&b, &y, &x), MergeOutcome::Clean(t));
        }
        if let MergeOutcome::Clean(t) = merge(&b, &x, &y) {
            prop_assert_eq!(merge(&b, &y, &x), MergeOutcome::Clean(t));
        }
    }

    #[test]
    fn conflicts_commute_up_to_side_labels((b, x, y) in triple()) {
        if let (MergeOutcome::Conflicted(xy), MergeOutcome::Conflicted(yx)) =
            (merge_text_only(&b, &x, &y), merge_text_only(&b, &y, &x))
        {
            let swapped: Vec<(String, String, String)> =
                yx.hunks.iter().map(|h| (h.base.clone(), h.theirs.clone(), h.ours.clone())).collect();
            let direct: Vec<(String, String, String)> =
                xy.hunks.iter().map(|h| (h.base.clone(), h.ours.clone(), h.theirs.clone())).collect();
            prop_assert_eq!(direct, swapped);
        } else {
            prop_assert!(merge_text_only(&b, &x, &y).clean().is_some() == merge_text_only(&b, &y, &x).clean().is_some());
        }
    }

    #[test]
    fn no_silent_data_loss((b, x, y) in triple()) {
        let outcome = merge_text_only(&b, &x, &y);
        assert_no_loss(&b, &x, &outcome)?;
        assert_no_loss(&b, &y, &outcome)?;
    }

    #[test]
    fn resolving_every_hunk_with_a_side_reproduces_that_side_when_the_other_is_base((b, x, _y) in triple()) {
        // Force conflicts by editing the same text on both sides differently, then choose.
        let y = format!("{x}zz-theirs\n");
        let x2 = format!("{x}zz-ours\n");
        if let MergeOutcome::Conflicted(c) = merge_text_only(&b, &x2, &y) {
            let ours: Vec<(u32, sync_model::Choice)> = c.hunks.iter().map(|h| (h.id, sync_model::Choice::Ours)).collect();
            prop_assert_eq!(c.resolve(&ours).map(|t| t.contains("zz-ours")), Ok(true));
        }
    }
}
