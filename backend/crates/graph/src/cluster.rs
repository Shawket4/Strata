//! The `cluster` job (PLAN §9.2, §10, D10): community detection over the user's graph,
//! stable cluster IDs, AI cluster names, persisted to `.meta/clusters.json` (§6.5) and the
//! `clusters` / `cluster_names` tables in one `ai: cluster .meta/clusters.json` commit, with
//! change-log rows (`cluster_assignment`, `cluster_name`) for `/sync/changes` and one
//! `cluster.updated` event.
//!
//! 1. **Leiden** (`graph_algo::leiden_from`) on the undirected weighted projection of every
//!    non-similarity edge (user edges 3, AI ≥ 0.85 weight 2, other AI 1), seeded
//!    ([`ClusterConfig::seed`]) and started from the previous assignment, with the
//!    resolution from the user's preference `graph.cluster_resolution` (default 1.0).
//!    Communities smaller than [`ClusterConfig::min_size`] are left unclustered.
//! 2. **Stable IDs**: `graph_algo::match_clusters` against the previous file (Jaccard ≥
//!    0.25); new clusters get IDs from the file's `next_id` counter, so a retired ID is
//!    never reused.
//! 3. **Names**: only new clusters, clusters whose members changed, and clusters still
//!    waiting for a name go to the `cluster_naming` prompt (central titles, frequent
//!    concepts, previous name). A name the user gave is never replaced. When the AI is
//!    unavailable (disabled, paused, failing), a cluster keeps its previous name, or gets
//!    `Cluster <id>`, and is listed under `unnamed` in the file so a later run names it.
//! 4. Nothing is written when the result equals the previous file (apart from `generated`).
//!
//! [`rename`] is the user's rename (`PATCH /graph/clusters/{id}`): `named_by: user` in one
//! `user:` commit. The rows follow the file through `strata_vault::clusters`, which
//! `stratad reindex` also uses to reload them.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use chrono::{DateTime, Utc};
use domain::{GraphEdgeKind, NoteKind};
use graph_algo::{
    EdgeInput, EdgeWeights, GraphBuilder, LeidenConfig, MatchConfig, PreviousCluster,
    WeightedGraph, leiden_from, match_clusters,
};
use serde::Serialize;
use strata_ai::outputs::ClusterNames;
use strata_ai::prompts::{self, ids};
use strata_ai::{AiCaller, AiService};
use strata_common::{IdGenerator, JobId, NoteId, UserId};
use strata_index::repo::jobs::{Job, NewJob};
use strata_index::{AppDb, ScopedTx, UserScope};
use strata_jobs::{JobClass, JobContext, JobError, JobHandler};
use strata_vault::clusters::{self as vault_clusters, ClusterRows};
use strata_vault::ops::files::{Expect, FileWrite};
use strata_vault::{Author, VaultError, VaultService, fsio};
use ulid::Ulid;
use vault_format::clusters::{CLUSTERS_PATH, Cluster, Clusters};
use vault_format::sidecar::By;

use crate::error::GraphError;
use crate::load::{self, GraphData};

/// The job kind.
pub const CLUSTER: &str = "cluster";

/// Preference key (`PATCH /me` `preferences`) holding the Leiden resolution.
pub const RESOLUTION_PREFERENCE: &str = "graph.cluster_resolution";

/// `extra` key of `.meta/clusters.json`: the next fresh cluster ID.
pub const NEXT_ID_KEY: &str = "next_id";

/// `extra` key of `.meta/clusters.json`: clusters whose name is a placeholder waiting for
/// the AI.
pub const UNNAMED_KEY: &str = "unnamed";

/// Clustering parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterConfig {
    /// Leiden seed (fixed, so equal graphs give equal clusters).
    pub seed: u64,
    /// Resolution when the user has not set one.
    pub default_resolution: f64,
    /// Smallest community that becomes a cluster.
    pub min_size: usize,
    /// Titles per cluster sent for naming.
    pub max_titles: usize,
    /// Concepts per cluster sent for naming.
    pub max_concepts: usize,
    /// Clusters per naming call.
    pub names_per_call: usize,
    /// Output tokens per naming call.
    pub max_tokens: u32,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            seed: 0x5354_5241_5441_434c,
            default_resolution: 1.0,
            min_size: 3,
            max_titles: 10,
            max_concepts: 5,
            names_per_call: 40,
            max_tokens: 2_000,
        }
    }
}

/// Where `cluster.updated` goes (the API's event bus in production).
pub trait ClusterEvents: Send + Sync + fmt::Debug {
    /// The clusters with these IDs appeared, changed members or names, or were retired.
    fn clusters_updated(&self, user: UserId, cluster_ids: &[String]);
}

/// Drops events.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoClusterEvents;

impl ClusterEvents for NoClusterEvents {
    fn clusters_updated(&self, _: UserId, _: &[String]) {}
}

/// Records events (tests).
#[derive(Debug, Default)]
pub struct RecordedClusterEvents(Mutex<Vec<(UserId, Vec<String>)>>);

impl RecordedClusterEvents {
    /// Everything recorded so far.
    pub fn take(&self) -> Vec<(UserId, Vec<String>)> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

impl ClusterEvents for RecordedClusterEvents {
    fn clusters_updated(&self, user: UserId, cluster_ids: &[String]) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((user, cluster_ids.to_vec()));
    }
}

/// One cluster of a [`Plan`].
#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    /// Stable ID.
    pub id: u32,
    /// Members, sorted.
    pub members: Vec<Ulid>,
    /// The cluster it continues in the previous file.
    pub previous: Option<Cluster>,
    /// New, or members differ from the previous cluster.
    pub changed: bool,
    /// Goes to the naming prompt.
    pub needs_name: bool,
    /// Most central member titles (by weighted degree, then title).
    pub titles: Vec<String>,
    /// Most frequent concepts of the members (by count, then name).
    pub concepts: Vec<String>,
}

/// The clustering of one run, before naming.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// Clusters, by smallest member.
    pub clusters: Vec<Planned>,
    /// Previous IDs no cluster continues.
    pub retired: Vec<u32>,
    /// The next fresh ID after this run.
    pub next_id: u32,
}

fn next_id_of(previous: Option<&Clusters>) -> u32 {
    let Some(p) = previous else { return 1 };
    let stored = p
        .extra
        .get(NEXT_ID_KEY)
        .and_then(serde_json::Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(1);
    let above_max = p
        .clusters
        .iter()
        .map(|c| c.id.saturating_add(1))
        .max()
        .unwrap_or(1);
    stored.max(above_max).max(1)
}

fn unnamed_of(previous: Option<&Clusters>) -> BTreeSet<u32> {
    previous
        .and_then(|p| p.extra.get(UNNAMED_KEY))
        .and_then(serde_json::Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(serde_json::Value::as_u64)
                .filter_map(|n| u32::try_from(n).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Clusters `data` (pure and deterministic: equal inputs give equal plans).
#[allow(clippy::too_many_lines)] // one pass: graph, Leiden, matching, naming inputs
pub fn plan(
    data: &GraphData,
    previous: Option<&Clusters>,
    resolution: f64,
    config: &ClusterConfig,
) -> Plan {
    let mut b = GraphBuilder::new();
    for n in &data.nodes {
        let _ = b.add_node(&n.id.to_string(), n.kind.into());
    }
    for e in data
        .edges
        .iter()
        .filter(|e| e.kind != GraphEdgeKind::Similarity)
    {
        let _ = b.add_edge(&EdgeInput {
            source: e.source.to_string(),
            target: e.target.to_string(),
            kind: e.kind,
            by: e.by,
            confidence: e.confidence,
        });
    }
    let graph = b.build();
    let weighted = WeightedGraph::project(&graph, &EdgeWeights::default());

    // Start from the previous assignment (stability); new notes start alone.
    let prev_of: BTreeMap<Ulid, u32> = previous
        .map(|p| {
            p.clusters
                .iter()
                .flat_map(|c| c.notes.iter().map(move |n| (*n, c.id)))
                .collect()
        })
        .unwrap_or_default();
    let mut compact: BTreeMap<u32, u32> = BTreeMap::new();
    let mut next_index = 0_u32;
    let initial: Vec<u32> = data
        .nodes
        .iter()
        .map(|n| {
            let fresh = |next: &mut u32| {
                let v = *next;
                *next += 1;
                v
            };
            match prev_of.get(&n.id.as_ulid()) {
                Some(c) => *compact.entry(*c).or_insert_with(|| fresh(&mut next_index)),
                None => fresh(&mut next_index),
            }
        })
        .collect();
    let partition = leiden_from(
        &weighted,
        Some(&initial),
        &LeidenConfig {
            resolution,
            seed: config.seed,
            ..LeidenConfig::default()
        },
    );
    let communities: Vec<Vec<u32>> = partition
        .communities()
        .into_iter()
        .filter(|c| c.len() >= config.min_size.max(1))
        .collect();
    let new: Vec<Vec<String>> = communities
        .iter()
        .map(|c| {
            c.iter()
                .map(|&i| data.nodes[i as usize].id.to_string())
                .collect()
        })
        .collect();
    let previous_clusters: Vec<PreviousCluster> = previous
        .map(|p| {
            p.clusters
                .iter()
                .map(|c| PreviousCluster {
                    id: c.id.to_string(),
                    members: c.notes.iter().map(Ulid::to_string).collect(),
                })
                .collect()
        })
        .unwrap_or_default();
    let mut next_id = next_id_of(previous);
    let matching = match_clusters(&previous_clusters, &new, &MatchConfig::default(), || {
        let id = next_id;
        next_id = next_id.saturating_add(1);
        id.to_string()
    });
    let unnamed = unnamed_of(previous);
    let by_id: BTreeMap<u32, &Cluster> = previous
        .map(|p| p.clusters.iter().map(|c| (c.id, c)).collect())
        .unwrap_or_default();

    let title_of: BTreeMap<NoteId, &str> = data
        .nodes
        .iter()
        .map(|n| (n.id, n.title.as_str()))
        .collect();
    let concept_edges: Vec<(NoteId, NoteId)> = data
        .edges
        .iter()
        .filter(|e| e.kind == GraphEdgeKind::Concept)
        .map(|e| (e.source, e.target))
        .collect();
    let is_concept: BTreeSet<NoteId> = data
        .nodes
        .iter()
        .filter(|n| n.kind == NoteKind::Concept)
        .map(|n| n.id)
        .collect();

    let clusters = matching
        .clusters
        .into_iter()
        .zip(&communities)
        .map(|(m, indices)| {
            let id: u32 = m.id.parse().unwrap_or(0);
            let members: Vec<Ulid> = m
                .members
                .iter()
                .filter_map(|s| Ulid::from_string(s).ok())
                .collect();
            let prev = by_id.get(&id).map(|c| (*c).clone());
            let changed = prev.as_ref().is_none_or(|p| {
                let mut old = p.notes.clone();
                old.sort();
                old != members
            });
            let needs_name = match &prev {
                None => true,
                Some(p) if p.named_by == By::User => false,
                Some(p) => changed || unnamed.contains(&p.id),
            };
            let mut ranked: Vec<(f64, &str)> = indices
                .iter()
                .map(|&i| {
                    (
                        weighted.strength(i),
                        title_of
                            .get(&data.nodes[i as usize].id)
                            .copied()
                            .unwrap_or(""),
                    )
                })
                .collect();
            ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(b.1)));
            let titles = ranked
                .into_iter()
                .take(config.max_titles)
                .map(|(_, t)| t.to_owned())
                .collect();
            let member_set: BTreeSet<NoteId> =
                members.iter().map(|u| NoteId::from_ulid(*u)).collect();
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for (s, t) in &concept_edges {
                if member_set.contains(s) && is_concept.contains(t) {
                    *counts
                        .entry(title_of.get(t).copied().unwrap_or(""))
                        .or_default() += 1;
                }
            }
            let mut concepts: Vec<(usize, &str)> =
                counts.into_iter().map(|(k, v)| (v, k)).collect();
            concepts.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
            Planned {
                id,
                members,
                previous: prev,
                changed,
                needs_name,
                titles,
                concepts: concepts
                    .into_iter()
                    .take(config.max_concepts)
                    .map(|(_, c)| c.to_owned())
                    .collect(),
            }
        })
        .collect();
    Plan {
        clusters,
        retired: matching
            .retired
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect(),
        next_id,
    }
}

/// One cluster of the `cluster_naming` input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NamingCluster {
    /// Stable ID.
    pub cluster_id: String,
    /// Most central titles.
    pub titles: Vec<String>,
    /// Most frequent concepts.
    pub concepts: Vec<String>,
    /// The name it had, if any.
    pub previous_name: Option<String>,
}

/// The `cluster_naming` input (serialised as the prompt's user message).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NamingInput {
    /// Clusters to name.
    pub clusters: Vec<NamingCluster>,
}

impl NamingInput {
    /// The input for `clusters`.
    pub fn of(clusters: &[&Planned]) -> Self {
        Self {
            clusters: clusters
                .iter()
                .map(|p| NamingCluster {
                    cluster_id: p.id.to_string(),
                    titles: p.titles.clone(),
                    concepts: p.concepts.clone(),
                    previous_name: p.previous.as_ref().map(|c| c.name.clone()),
                })
                .collect(),
        }
    }
}

/// The file content for `plan` with the AI's `names` (`generated` = `now`).
pub fn clusters_file(
    plan: &Plan,
    names: &BTreeMap<u32, String>,
    previous: Option<&Clusters>,
    now: DateTime<Utc>,
) -> Clusters {
    let mut unnamed = Vec::new();
    let clusters = plan
        .clusters
        .iter()
        .map(|p| {
            let (name, named_by) = match (&p.previous, names.get(&p.id)) {
                (Some(prev), _) if !p.needs_name => (prev.name.clone(), prev.named_by),
                (_, Some(n)) => (n.clone(), By::Ai),
                (Some(prev), None) => {
                    unnamed.push(p.id);
                    (prev.name.clone(), prev.named_by)
                }
                (None, None) => {
                    unnamed.push(p.id);
                    (format!("Cluster {}", p.id), By::Ai)
                }
            };
            Cluster {
                id: p.id,
                name,
                named_by,
                notes: p.members.clone(),
            }
        })
        .collect();
    let mut extra = previous.map(|p| p.extra.clone()).unwrap_or_default();
    extra.insert(NEXT_ID_KEY.into(), serde_json::json!(plan.next_id));
    extra.remove(UNNAMED_KEY);
    if !unnamed.is_empty() {
        unnamed.sort_unstable();
        extra.insert(UNNAMED_KEY.into(), serde_json::json!(unnamed));
    }
    let mut out = Clusters {
        version: 1,
        generated: now.fixed_offset(),
        algorithm: "leiden".into(),
        clusters,
        extra,
    };
    out.clusters.sort_by_key(|c| c.id);
    out
}

/// Whether two files hold the same clustering (ignoring `generated`).
pub fn same_clustering(a: &Clusters, b: &Clusters) -> bool {
    let norm = |c: &Clusters| {
        let mut c = c.clone();
        c.clusters.sort_by_key(|x| x.id);
        for x in &mut c.clusters {
            x.notes.sort();
        }
        (c.version, c.algorithm, c.clusters, c.extra)
    };
    norm(a) == norm(b)
}

/// The IDs whose cluster (members or name) differs between `old` and `new`, or that exist
/// in only one of them; sorted numerically.
pub fn changed_ids(old: Option<&Clusters>, new: &Clusters) -> Vec<String> {
    let index = |c: &Clusters| -> BTreeMap<u32, (String, Vec<Ulid>)> {
        c.clusters
            .iter()
            .map(|x| {
                let mut n = x.notes.clone();
                n.sort();
                (x.id, (x.name.clone(), n))
            })
            .collect()
    };
    let a = old.map(index).unwrap_or_default();
    let b = index(new);
    let ids: BTreeSet<u32> = a.keys().chain(b.keys()).copied().collect();
    ids.into_iter()
        .filter(|id| a.get(id) != b.get(id))
        .map(|id| id.to_string())
        .collect()
}

/// What one run did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterOutcome {
    /// The commit (`None` when nothing changed).
    pub commit: Option<String>,
    /// Clusters that appeared, changed or were retired.
    pub changed: Vec<String>,
    /// Naming calls made.
    pub ai_calls: usize,
}

/// Enqueues a `cluster` run for the scope's user (a queued run with the same `key` is
/// re-timed instead of duplicated).
pub async fn enqueue(
    tx: &mut ScopedTx,
    ids: &dyn IdGenerator,
    key: &str,
    now: DateTime<Utc>,
) -> Result<Job, strata_index::IndexError> {
    strata_index::repo::jobs::enqueue(
        tx,
        &NewJob {
            id: JobId::generate(ids),
            kind: CLUSTER.to_owned(),
            note_id: None,
            payload: Vec::new(),
            run_after: now,
            max_attempts: 3,
            dedupe_key: Some(key.to_owned()),
        },
        now,
    )
    .await
}

/// The `cluster` job handler.
#[derive(Debug, Clone)]
pub struct ClusterHandler {
    db: AppDb,
    vault: VaultService,
    ai: Arc<AiService>,
    events: Arc<dyn ClusterEvents>,
    config: ClusterConfig,
}

impl ClusterHandler {
    /// The handler.
    pub fn new(
        db: AppDb,
        vault: VaultService,
        ai: Arc<AiService>,
        events: Arc<dyn ClusterEvents>,
        config: ClusterConfig,
    ) -> Self {
        Self {
            db,
            vault,
            ai,
            events,
            config,
        }
    }

    /// Names the clusters that need a name; failures leave them out (the caller falls
    /// back). Returns the names and the number of calls made.
    async fn names(
        &self,
        scope: &UserScope,
        username: &str,
        plan: &Plan,
    ) -> (BTreeMap<u32, String>, usize) {
        let mut out = BTreeMap::new();
        let mut calls = 0;
        let Some(prompt) = prompts::latest(ids::CLUSTER_NAMING) else {
            return (out, calls);
        };
        let wanted: Vec<&Planned> = plan.clusters.iter().filter(|p| p.needs_name).collect();
        for chunk in wanted.chunks(self.config.names_per_call.max(1)) {
            let input = NamingInput::of(chunk);
            let caller = AiCaller {
                scope: *scope,
                username: username.to_owned(),
            };
            calls += 1;
            match self
                .ai
                .complete::<ClusterNames>(caller, prompt, &input, self.config.max_tokens)
                .await
            {
                Ok(named) => {
                    let asked: BTreeSet<u32> = chunk.iter().map(|p| p.id).collect();
                    for n in named.value.names {
                        let name = n.name.split_whitespace().collect::<Vec<_>>().join(" ");
                        if let Ok(id) = n.cluster_id.parse::<u32>()
                            && asked.contains(&id)
                            && !name.is_empty()
                        {
                            out.entry(id).or_insert(name);
                        }
                    }
                }
                Err(e) => {
                    // AI down, disabled or paused: keep previous names (principle 6).
                    tracing::warn!(error = %e, "cluster naming unavailable; keeping previous names");
                    if matches!(
                        e,
                        strata_ai::AiError::Disabled | strata_ai::AiError::ProviderNotConfigured(_)
                    ) {
                        calls -= 1;
                        break;
                    }
                }
            }
        }
        (out, calls)
    }

    /// One run for the scope's user.
    pub async fn run_for(
        &self,
        scope: &UserScope,
        username: &str,
        now: DateTime<Utc>,
    ) -> Result<ClusterOutcome, JobError> {
        let mut tx = self.db.begin(scope).await?;
        let data = load::load(&mut tx)
            .await
            .map_err(|e| JobError::Retry(e.to_string()))?;
        let resolution = load::preference(&mut tx, RESOLUTION_PREFERENCE)
            .await
            .map_err(|e| JobError::Retry(e.to_string()))?
            .and_then(|s| s.trim().parse::<f64>().ok())
            .filter(|r| r.is_finite() && *r > 0.0 && *r <= 10.0)
            .unwrap_or(self.config.default_resolution);
        tx.commit().await?;

        let raw = self.vault.read_file_bytes(scope, CLUSTERS_PATH).await?;
        let previous_version = raw.as_deref().map(fsio::version_of);
        let previous = raw
            .as_deref()
            .and_then(|b| std::str::from_utf8(b).ok())
            .and_then(|s| Clusters::from_json(s).ok());

        let plan = plan(&data, previous.as_ref(), resolution, &self.config);
        let (names, ai_calls) = self.names(scope, username, &plan).await;
        let file = clusters_file(&plan, &names, previous.as_ref(), now);
        if previous.as_ref().is_some_and(|p| same_clustering(p, &file)) {
            return Ok(ClusterOutcome {
                commit: None,
                changed: Vec::new(),
                ai_calls,
            });
        }
        let changed = changed_ids(previous.as_ref(), &file);
        let json = file
            .to_json()
            .map_err(|e| JobError::Fatal(format!("clusters encoding: {e}")))?;
        let rows = ClusterRows::of_file(&file);
        let written = self
            .vault
            .write_file(
                scope,
                FileWrite {
                    path: CLUSTERS_PATH.to_owned(),
                    content: Some(json.into_bytes()),
                    expect: previous_version.map_or(Expect::Absent, Expect::Version),
                    author: Author::Ai(CLUSTER.to_owned()),
                    op: CLUSTER.to_owned(),
                    index: Some(Box::new(move |tx| Box::pin(store_tables(tx, rows, now)))),
                },
            )
            .await
            .map_err(|e| match e {
                VaultError::VersionConflict { .. } | VaultError::NotFound => {
                    JobError::Retry("clusters file changed during the run".into())
                }
                other => other.into(),
            })?;
        if written.commit.is_some() && !changed.is_empty() {
            self.events.clusters_updated(scope.user_id(), &changed);
        }
        Ok(ClusterOutcome {
            commit: written.commit,
            changed,
            ai_calls,
        })
    }
}

/// Replaces the `clusters` and `cluster_names` rows with `rows` and logs the differences
/// for sync (`strata_vault::clusters`).
async fn store_tables(
    tx: &mut ScopedTx,
    rows: ClusterRows,
    now: DateTime<Utc>,
) -> Result<(), VaultError> {
    let before = vault_clusters::snapshot(tx).await?;
    vault_clusters::replace(tx, &before, &rows, now).await?;
    Ok(())
}

/// Longest cluster name a user may give, in characters.
pub const MAX_NAME_CHARS: usize = 100;

/// A user's cluster rename (`PATCH /graph/clusters/{id}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renamed {
    /// Stable ID.
    pub id: String,
    /// The name now.
    pub name: String,
    /// Member notes.
    pub size: u32,
    /// The commit (`None` when the cluster already had this user-given name).
    pub commit: Option<String>,
}

/// The name a user gave, with whitespace runs collapsed; `422` when empty or longer than
/// [`MAX_NAME_CHARS`].
pub fn user_name(name: &str) -> crate::Result<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(GraphError::InvalidField {
            pointer: "/name",
            code: "empty_name",
            message: "the cluster name is empty".into(),
        });
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(GraphError::InvalidField {
            pointer: "/name",
            code: "name_too_long",
            message: format!("the cluster name is longer than {MAX_NAME_CHARS} characters"),
        });
    }
    Ok(name)
}

/// Renames cluster `id` to `name` for the scope's user: `.meta/clusters.json` gets the name
/// with `named_by: user` (so the `cluster` job never replaces it) and loses the cluster from
/// `unnamed`, in one `user: rename cluster .meta/clusters.json` commit whose index hook
/// updates `cluster_names` and logs the `cluster_name` change; then `cluster.updated
/// {[id]}` is published. A cluster that already has this user-given name writes nothing.
/// `404` when the file or the cluster does not exist. A concurrent write of the file (a
/// `cluster` run) is retried on the new content.
pub async fn rename(
    vault: &VaultService,
    events: &dyn ClusterEvents,
    scope: &UserScope,
    id: &str,
    name: &str,
    now: DateTime<Utc>,
) -> crate::Result<Renamed> {
    const ATTEMPTS: usize = 3;
    let cluster_id: u32 = id.parse().map_err(|_| GraphError::NotFound)?;
    let name = user_name(name)?;
    let mut attempt = 0;
    loop {
        attempt += 1;
        let raw = vault
            .read_file_bytes(scope, CLUSTERS_PATH)
            .await?
            .ok_or(GraphError::NotFound)?;
        let version = fsio::version_of(&raw);
        let mut file = std::str::from_utf8(&raw)
            .ok()
            .and_then(|s| Clusters::from_json(s).ok())
            .ok_or(GraphError::NotFound)?;
        let unnamed = unnamed_of(Some(&file));
        let cluster = file
            .clusters
            .iter_mut()
            .find(|c| c.id == cluster_id)
            .ok_or(GraphError::NotFound)?;
        let size = u32::try_from(cluster.notes.len()).unwrap_or(u32::MAX);
        if cluster.name == name && cluster.named_by == By::User && !unnamed.contains(&cluster_id) {
            return Ok(Renamed {
                id: cluster_id.to_string(),
                name,
                size,
                commit: None,
            });
        }
        cluster.name.clone_from(&name);
        cluster.named_by = By::User;
        let rest: Vec<u32> = unnamed.into_iter().filter(|u| *u != cluster_id).collect();
        if rest.is_empty() {
            file.extra.remove(UNNAMED_KEY);
        } else {
            file.extra
                .insert(UNNAMED_KEY.into(), serde_json::json!(rest));
        }
        let json = file.to_json().map_err(|e| {
            GraphError::Vault(VaultError::Internal(format!("clusters encoding: {e}")))
        })?;
        let rows = ClusterRows::of_file(&file);
        let written = vault
            .write_file(
                scope,
                FileWrite {
                    path: CLUSTERS_PATH.to_owned(),
                    content: Some(json.into_bytes()),
                    expect: Expect::Version(version),
                    author: Author::User,
                    op: "rename cluster".to_owned(),
                    index: Some(Box::new(move |tx| Box::pin(store_tables(tx, rows, now)))),
                },
            )
            .await;
        match written {
            Ok(w) => {
                if w.commit.is_some() {
                    events.clusters_updated(scope.user_id(), &[cluster_id.to_string()]);
                }
                return Ok(Renamed {
                    id: cluster_id.to_string(),
                    name,
                    size,
                    commit: w.commit,
                });
            }
            Err(VaultError::VersionConflict { .. } | VaultError::NotFound)
                if attempt < ATTEMPTS => {}
            Err(VaultError::NotFound) => return Err(GraphError::NotFound),
            Err(e) => return Err(e.into()),
        }
    }
}

#[async_trait::async_trait]
impl JobHandler for ClusterHandler {
    fn kind(&self) -> &'static str {
        CLUSTER
    }

    fn class(&self) -> JobClass {
        JobClass::Llm
    }

    async fn run(&self, ctx: JobContext) -> Result<(), JobError> {
        self.run_for(&ctx.scope, &ctx.username, ctx.now)
            .await
            .map(|_| ())
    }
}
