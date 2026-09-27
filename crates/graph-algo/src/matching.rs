//! Stable cluster identities across runs (§10: "kept stable across runs by matching new
//! clusters to old ones by member overlap"), so AI cluster names persist.
//!
//! Matching is greedy on Jaccard similarity: all (new, old) pairs with overlap are sorted by
//! Jaccard (descending), then overlap size (descending), then old ID and new index; each pair
//! whose clusters are both still free and whose Jaccard is at least
//! [`MatchConfig::min_jaccard`] is matched. A split keeps the ID on the larger overlap; a
//! merge keeps the ID of the best-overlapping old cluster and retires the others. Unmatched
//! new clusters get fresh IDs from the caller's generator (injectable, per the ID rules).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// Matching parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MatchConfig {
    /// Minimum Jaccard similarity for a new cluster to inherit an old ID.
    pub min_jaccard: f64,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self { min_jaccard: 0.25 }
    }
}

/// A previous cluster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviousCluster {
    /// Stable ID (names are stored against it).
    pub id: String,
    /// Member node keys.
    pub members: Vec<String>,
}

/// A new cluster with its stable ID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchedCluster {
    /// Stable ID (inherited or fresh).
    pub id: String,
    /// Member node keys, sorted.
    pub members: Vec<String>,
    /// The old cluster it continues, if any.
    pub previous: Option<String>,
    /// Jaccard similarity with that old cluster (0 when fresh).
    pub jaccard: f64,
}

/// Result of [`match_clusters`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterMatching {
    /// New clusters in input order.
    pub clusters: Vec<MatchedCluster>,
    /// Old IDs that no new cluster continues (their names can be dropped), sorted.
    pub retired: Vec<String>,
}

/// Matches `new` clusters (member keys) to `previous` ones.
pub fn match_clusters(
    previous: &[PreviousCluster],
    new: &[Vec<String>],
    config: &MatchConfig,
    mut fresh_id: impl FnMut() -> String,
) -> ClusterMatching {
    let old_sets: Vec<BTreeSet<&str>> = previous
        .iter()
        .map(|c| c.members.iter().map(String::as_str).collect())
        .collect();
    // Old clusters are a partition, but tolerate overlaps: index every owner of a key.
    let mut owners: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, set) in old_sets.iter().enumerate() {
        for &k in set {
            owners.entry(k).or_default().push(i);
        }
    }
    let new_sets: Vec<BTreeSet<&str>> = new
        .iter()
        .map(|m| m.iter().map(String::as_str).collect())
        .collect();
    // (jaccard, overlap, old index, new index)
    let mut pairs: Vec<(f64, usize, usize, usize)> = Vec::new();
    for (j, set) in new_sets.iter().enumerate() {
        let candidates: BTreeSet<usize> = set
            .iter()
            .filter_map(|k| owners.get(k))
            .flatten()
            .copied()
            .collect();
        for i in candidates {
            let common = old_sets[i].intersection(set).count();
            let union = old_sets[i].len() + set.len() - common;
            #[expect(clippy::cast_precision_loss, reason = "cluster sizes are far below 2^52")]
            let jac = common as f64 / union as f64;
            pairs.push((jac, common, i, j));
        }
    }
    pairs.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then(b.1.cmp(&a.1))
            .then_with(|| previous[a.2].id.cmp(&previous[b.2].id))
            .then(a.3.cmp(&b.3))
    });
    let mut new_match: Vec<Option<(usize, f64)>> = vec![None; new.len()];
    let mut old_used = vec![false; previous.len()];
    for (jac, _, i, j) in pairs {
        if jac + 1e-12 < config.min_jaccard || old_used[i] || new_match[j].is_some() {
            continue;
        }
        old_used[i] = true;
        new_match[j] = Some((i, jac));
    }
    let clusters = new_sets
        .iter()
        .zip(new_match)
        .map(|(set, m)| {
            let members: Vec<String> = set.iter().map(|&s| s.to_owned()).collect();
            match m {
                Some((i, jaccard)) => MatchedCluster {
                    id: previous[i].id.clone(),
                    members,
                    previous: Some(previous[i].id.clone()),
                    jaccard,
                },
                None => MatchedCluster {
                    id: fresh_id(),
                    members,
                    previous: None,
                    jaccard: 0.0,
                },
            }
        })
        .collect();
    let mut retired: Vec<String> = previous
        .iter()
        .zip(&old_used)
        .filter(|(_, used)| !**used)
        .map(|(c, _)| c.id.clone())
        .collect();
    retired.sort();
    ClusterMatching { clusters, retired }
}
