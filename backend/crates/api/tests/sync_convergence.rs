//! Sync as a state machine (PLAN §16.4): random op sequences from two offline devices of one
//! user and the server converge. Each device is a small client model on `sync-model` (a
//! bootstrapped record cache, an outbox of ops based on the last pulled version, coalesced per
//! note like the client core's outbox), driven over the HTTP API. After a final sync of both
//! devices, both caches equal a fresh bootstrap, every note record equals its file, every
//! conflict copy exists with the device's content, no op was rejected for anything but a note
//! the other device deleted meanwhile, and a replayed push answers the same bytes.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::assigning_clones,
    clippy::default_trait_access,
    clippy::float_cmp
)]

mod sync_harness;

use std::collections::BTreeMap;

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use sync_harness::{Device, H, User};
use sync_model::changes::NoteRecord;
use sync_model::ops::{self as o, Op};
use sync_model::{ConflictResolution, EntityType, OpResult, Record, SyncOp, Version};
use ulid::Ulid;

#[derive(Debug, Clone)]
enum Action {
    /// Create a note.
    Create { dev: usize },
    /// Replace line `line` of a note (picked by `pick`) with a marker.
    Edit {
        dev: usize,
        pick: usize,
        line: usize,
    },
    /// Append a marker line to a note.
    Append { dev: usize, pick: usize },
    /// Delete a note.
    Delete { dev: usize, pick: usize },
    /// Relate two notes.
    Relate { dev: usize, a: usize, b: usize },
    /// Push the outbox, then pull.
    Sync { dev: usize },
}

fn action() -> impl Strategy<Value = Action> {
    let dev = 0..2_usize;
    prop_oneof![
        2 => dev.clone().prop_map(|dev| Action::Create { dev }),
        4 => (dev.clone(), 0..8_usize, 0..4_usize)
            .prop_map(|(dev, pick, line)| Action::Edit { dev, pick, line }),
        2 => (dev.clone(), 0..8_usize).prop_map(|(dev, pick)| Action::Append { dev, pick }),
        1 => (dev.clone(), 0..8_usize).prop_map(|(dev, pick)| Action::Delete { dev, pick }),
        1 => (dev.clone(), 0..8_usize, 0..8_usize)
            .prop_map(|(dev, a, b)| Action::Relate { dev, a, b }),
        3 => dev.prop_map(|dev| Action::Sync { dev }),
    ]
}

/// One device: its cache, outbox and the local content of notes with a pending edit.
struct Dev {
    user: User,
    cache: Device,
    outbox: Vec<SyncOp>,
    /// Note → (base version the edit started from, local content).
    local: BTreeMap<Ulid, (Version, String)>,
    /// Notes created locally and not pushed yet: (id, content).
    created: BTreeMap<Ulid, String>,
    deleted: Vec<Ulid>,
    last_push: Option<(Vec<SyncOp>, Vec<u8>)>,
}

struct World {
    ops_seen: Vec<(SyncOp, OpResult)>,
    counter: u128,
}

impl World {
    fn next(&mut self) -> u128 {
        self.counter += 1;
        self.counter
    }
}

impl Dev {
    fn notes(&self) -> Vec<(Ulid, String)> {
        let mut out: Vec<(Ulid, String)> = self
            .cache
            .notes()
            .values()
            .filter(|n| !self.deleted.contains(&n.id))
            .map(|n| {
                let text = self
                    .local
                    .get(&n.id)
                    .map_or_else(|| n.content.clone(), |(_, t)| t.clone());
                (n.id, text)
            })
            .collect();
        out.extend(self.created.iter().map(|(id, t)| (*id, t.clone())));
        out
    }

    fn server_note(&self, id: Ulid) -> Option<NoteRecord> {
        match self.cache.records.get(&(EntityType::Note, id.to_string())) {
            Some(Record::Note(n)) => Some(n.clone()),
            _ => None,
        }
    }

    /// Replaces the note's content locally, coalescing into one pending update or create.
    fn set_content(&mut self, id: Ulid, text: String) {
        if let Some(c) = self.created.get_mut(&id) {
            *c = text.clone();
            for op in &mut self.outbox {
                if let Op::NoteCreate(p) = &mut op.op
                    && p.id == id
                {
                    p.content = text.clone();
                }
            }
            return;
        }
        let Some(server) = self.server_note(id) else {
            return;
        };
        let base = server.version.clone();
        self.local.insert(id, (base.clone(), text.clone()));
        let existing = self
            .outbox
            .iter_mut()
            .find(|op| matches!(&op.op, Op::NoteUpdate(p) if p.id == id));
        match existing {
            Some(op) => {
                if let Op::NoteUpdate(p) = &mut op.op {
                    p.content = text;
                }
            }
            None => self.outbox.push(SyncOp::new(
                Ulid::nil(),
                Some(base),
                Op::NoteUpdate(o::NoteUpdate { id, content: text }),
            )),
        }
    }
}

fn marker(dev: usize, n: u128) -> String {
    format!("d{dev}-m{n}")
}

async fn run(actions: Vec<Action>) {
    let h = H::new().await;
    let first = h.user("alice").await;
    let second = h.login(&first, "alice", "phone").await;
    // A shared starting point: two notes, both devices bootstrapped.
    let seed: Vec<SyncOp> = (1..=2_u128)
        .map(|n| {
            SyncOp::new(
                Ulid(0x0199_0000_0000_0000_0000_0000_0000_0000 + n),
                None,
                Op::NoteCreate(o::NoteCreate {
                    created: strata_common::clock::default_test_epoch(),
                    id: Ulid(0x0199_1111_0000_0000_0000_0000_0000_0000 + n),
                    path: format!("notes/Seed {n}.md"),
                    content: format!("# Seed {n}\n\none\ntwo\nthree\n"),
                    force: true,
                }),
            )
        })
        .collect();
    h.push(&first, seed).await;
    let mut devs = Vec::new();
    for user in [first, second] {
        let cache = h.bootstrap(&user, Some(3)).await;
        devs.push(Dev {
            user,
            cache,
            outbox: Vec::new(),
            local: BTreeMap::new(),
            created: BTreeMap::new(),
            deleted: Vec::new(),
            last_push: None,
        });
    }
    let mut world = World {
        ops_seen: Vec::new(),
        counter: 0,
    };
    let finals = [
        Action::Sync { dev: 0 },
        Action::Sync { dev: 1 },
        Action::Sync { dev: 0 },
        Action::Sync { dev: 1 },
    ];
    for action in actions.iter().chain(finals.iter()) {
        let n = world.next();
        match action {
            Action::Create { dev } => {
                let d = &mut devs[*dev];
                let id = Ulid(0x0199_2222_0000_0000_0000_0000_0000_0000 + n);
                let text = format!("# Note {n}\n\n{}\n", marker(*dev, n));
                d.created.insert(id, text.clone());
                d.outbox.push(SyncOp::new(
                    Ulid::nil(),
                    None,
                    Op::NoteCreate(o::NoteCreate {
                        created: strata_common::clock::default_test_epoch(),
                        id,
                        path: format!("notes/Note {n}.md"),
                        content: text,
                        force: true,
                    }),
                ));
            }
            Action::Edit { dev, pick, .. } | Action::Append { dev, pick }
                if !devs[*dev].notes().is_empty() =>
            {
                let d = &mut devs[*dev];
                let notes = d.notes();
                let (id, text) = notes[pick % notes.len()].clone();
                let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
                let body_start = lines.iter().rposition(|l| l == "---").map_or(0, |i| i + 1);
                let m = marker(*dev, n);
                match action {
                    Action::Edit { line, .. } if lines.len() > body_start => {
                        let span = lines.len() - body_start;
                        lines[body_start + line % span] = m;
                    }
                    _ => lines.push(m),
                }
                let mut new = lines.join("\n");
                new.push('\n');
                d.set_content(id, new);
            }
            Action::Delete { dev, pick } => {
                let d = &mut devs[*dev];
                let candidates: Vec<NoteRecord> = d
                    .cache
                    .notes()
                    .into_values()
                    .filter(|n| !d.local.contains_key(&n.id) && !d.deleted.contains(&n.id))
                    .collect();
                if let Some(note) = candidates.get(pick % candidates.len().max(1)) {
                    d.deleted.push(note.id);
                    d.outbox.push(SyncOp::new(
                        Ulid::nil(),
                        Some(note.version.clone()),
                        Op::NoteDelete(o::NoteRef { id: note.id }),
                    ));
                }
            }
            Action::Relate { dev, a, b } => {
                let d = &mut devs[*dev];
                let notes: Vec<Ulid> = d
                    .cache
                    .notes()
                    .values()
                    .map(|n| n.id)
                    .filter(|i| !d.deleted.contains(i))
                    .collect();
                if notes.len() >= 2 {
                    let (x, y) = (notes[a % notes.len()], notes[b % notes.len()]);
                    if x != y {
                        d.outbox.push(SyncOp::new(
                            Ulid::nil(),
                            None,
                            Op::RelationAdd(o::RelationRef {
                                src_id: x,
                                dst_id: y,
                                relation: "related".parse().expect("rel"),
                            }),
                        ));
                    }
                }
            }
            Action::Sync { dev } => {
                let d = &mut devs[*dev];
                let mut ops_ = std::mem::take(&mut d.outbox);
                for op in &mut ops_ {
                    op.op_id = Ulid(0x0199_3333_0000_0000_0000_0000_0000_0000 + world.next());
                }
                if !ops_.is_empty() {
                    let (res, bytes) = h.push(&d.user, ops_.clone()).await;
                    for (op, r) in ops_.iter().zip(res.results) {
                        world.ops_seen.push((op.clone(), r.result));
                    }
                    d.last_push = Some((ops_, bytes));
                }
                d.local.clear();
                d.created.clear();
                d.deleted.clear();
                h.pull(&d.user, &mut d.cache).await;
            }
            _ => {}
        }
    }

    // Convergence.
    let fresh = h.bootstrap(&devs[0].user, None).await;
    assert_eq!(devs[0].cache.state(), fresh.state());
    assert_eq!(devs[1].cache.state(), fresh.state());
    assert_eq!(devs[0].cache.cursor, fresh.cursor);
    // Records are the files.
    for note in fresh.notes().values() {
        assert_eq!(h.read(devs[0].user.id, &note.path), note.content);
        assert_eq!(note.version, Version::of_text(&note.content));
    }
    // Results.
    for (op, result) in &world.ops_seen {
        match result {
            OpResult::Applied { .. } => {}
            OpResult::Conflict {
                resolution:
                    ConflictResolution::ConflictCopy {
                        note_id,
                        path,
                        version,
                    },
                ..
            } => {
                let Op::NoteUpdate(p) = &op.op else {
                    panic!("conflict copy for {op:?}");
                };
                let Some(Record::Note(copy)) =
                    fresh.records.get(&(EntityType::Note, note_id.to_string()))
                else {
                    // A device deleted the copy later.
                    assert!(
                        world
                            .ops_seen
                            .iter()
                            .any(|(o, _)| matches!(&o.op, Op::NoteDelete(d) if d.id == *note_id))
                    );
                    continue;
                };
                assert_eq!((&copy.path, &copy.version), (path, version));
                // The device's text is kept in full (only the note ID differs).
                assert_eq!(
                    copy.content,
                    p.content
                        .replace(&format!("id: {}", p.id), &format!("id: {note_id}"))
                );
            }
            // The other device deleted the note first.
            OpResult::Conflict {
                resolution: ConflictResolution::ServerKept { .. },
                ..
            } => assert!(matches!(op.op, Op::NoteUpdate(_) | Op::NoteDelete(_))),
            OpResult::Rejected { problem } => {
                assert_eq!(problem.status, 404, "{op:?} {problem:?}");
                assert!(matches!(op.op, Op::RelationAdd(_)), "{op:?}");
            }
            OpResult::Duplicate { .. } => panic!("forced creates never answer duplicate"),
        }
    }
    // A replayed push answers the stored bytes.
    for d in &devs {
        if let Some((ops_, bytes)) = &d.last_push {
            let (_, again) = h.push(&d.user, ops_.clone()).await;
            assert_eq!(&again, bytes);
        }
    }
    h.finish().await;
}

#[test]
fn two_devices_and_the_server_converge() {
    let config = Config {
        cases: 8,
        failure_persistence: None,
        ..Config::default()
    };
    let mut runner =
        TestRunner::new_with_rng(config, TestRng::deterministic_rng(RngAlgorithm::ChaCha));
    runner
        .run(&proptest::collection::vec(action(), 4..18), |actions| {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("runtime");
            rt.block_on(run(actions));
            Ok(())
        })
        .expect("converges");
}
