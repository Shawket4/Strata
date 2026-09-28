//! Task transitions the task line does not allow (PLAN §6.11) are refused with a readable
//! `task_state_conflict` reason and leave the note untouched: completing a done task, a
//! recurring task whose rule is not understood or that has no date to count from, and
//! reopening an open task.
#![allow(clippy::expect_used)]

mod common;

use common::World;
use pretty_assertions::assert_eq;
use strata_vault::VaultError;
use strata_vault::ops::notes::CreateNote;
use strata_vault::ops::tasks::Transition;

#[tokio::test]
async fn transitions_the_line_does_not_allow_are_refused() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    w.vault
        .create_note(
            &s,
            CreateNote {
                created: strata_common::clock::default_test_epoch(),
                path: "notes/Chores.md".into(),
                content: "- [x] Sweep ✅ 2026-09-01 ^t-done\n\
                          - [ ] Water plants 🔁 every blue moon 📅 2026-10-01 ^t-moon\n\
                          - [ ] Pay rent 🔁 every month ^t-undated\n\
                          - [ ] Call Mona ^t-open\n"
                    .into(),
                id: None,
                force: false,
            },
        )
        .await
        .expect("note");
    let before = w.read(u, "notes/Chores.md");
    let commits = w.log(u).len();
    for (id, transition, reason) in [
        ("t-done", Transition::Complete, "the task is not open"),
        (
            "t-moon",
            Transition::Complete,
            "the recurrence is not understood; edit it first",
        ),
        (
            "t-undated",
            Transition::Complete,
            "a recurring task needs a due, scheduled or start date",
        ),
        ("t-open", Transition::Reopen, "the task is already open"),
    ] {
        let result = w
            .vault
            .transition_task(&s, id.into(), transition, None)
            .await;
        assert!(
            matches!(&result, Err(VaultError::TaskState(r)) if r == reason),
            "{id}: {result:?}"
        );
    }
    assert!(matches!(
        w.vault
            .transition_task(&s, "t-missing".into(), Transition::Complete, None)
            .await,
        Err(VaultError::NotFound)
    ));
    assert_eq!(w.read(u, "notes/Chores.md"), before);
    assert_eq!(w.log(u).len(), commits);
    w.finish().await;
}
