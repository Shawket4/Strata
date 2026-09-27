//! `.meta/clusters.json`: the latest cluster assignment and cluster names, persisted so names
//! stay stable across rebuilds (PLAN §6.5).

use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::sidecar::{By, to_pretty_json};

/// Vault path of the clusters file.
pub const CLUSTERS_PATH: &str = ".meta/clusters.json";

/// One cluster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cluster {
    /// Stable cluster ID.
    pub id: u32,
    /// Display name.
    pub name: String,
    /// Who named it (a user rename is never overwritten by the AI).
    pub named_by: By,
    /// Member note IDs, sorted.
    pub notes: Vec<Ulid>,
}

/// `.meta/clusters.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clusters {
    /// Format version (currently 1).
    pub version: u32,
    /// When the assignment was computed.
    pub generated: DateTime<FixedOffset>,
    /// Algorithm (`leiden`).
    pub algorithm: String,
    /// Clusters, sorted by ID.
    pub clusters: Vec<Cluster>,
    /// Fields this version does not know.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Clusters {
    /// Parses the JSON file.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialises to the on-disk form (clusters and members sorted for stable diffs).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let mut sorted = self.clone();
        sorted.clusters.sort_by_key(|c| c.id);
        for c in &mut sorted.clusters {
            c.notes.sort();
        }
        to_pretty_json(&sorted)
    }

    /// The cluster a note belongs to.
    pub fn cluster_of(&self, note: Ulid) -> Option<&Cluster> {
        self.clusters.iter().find(|c| c.notes.contains(&note))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_sorted() {
        let a = Ulid::from_parts(1, 1);
        let b = Ulid::from_parts(2, 2);
        let c = Clusters {
            version: 1,
            generated: DateTime::parse_from_rfc3339("2026-09-27T03:00:00+03:00")
                .unwrap_or_default(),
            algorithm: "leiden".into(),
            clusters: vec![
                Cluster {
                    id: 2,
                    name: "التسعير".into(),
                    named_by: By::User,
                    notes: vec![b, a],
                },
                Cluster {
                    id: 1,
                    name: "Logistics".into(),
                    named_by: By::Ai,
                    notes: vec![],
                },
            ],
            extra: BTreeMap::new(),
        };
        let json = c.to_json().unwrap_or_default();
        assert_eq!(
            json,
            format!(
                "{{\n  \"version\": 1,\n  \"generated\": \"2026-09-27T03:00:00+03:00\",\n  \"algorithm\": \"leiden\",\n  \"clusters\": [\n    {{\n      \"id\": 1,\n      \"name\": \"Logistics\",\n      \"named_by\": \"ai\",\n      \"notes\": []\n    }},\n    {{\n      \"id\": 2,\n      \"name\": \"التسعير\",\n      \"named_by\": \"user\",\n      \"notes\": [\n        \"{a}\",\n        \"{b}\"\n      ]\n    }}\n  ]\n}}\n"
            )
        );
        let back = Clusters::from_json(&json).ok();
        assert_eq!(
            back.as_ref().and_then(|c| c.cluster_of(a)).map(|c| c.id),
            Some(2)
        );
        assert_eq!(back.as_ref().and_then(|c| c.cluster_of(Ulid::nil())), None);
    }
}
