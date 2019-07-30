use crate::scraper::{Release, Updates};
use failure::Fallible;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CincinnatiPayload {
    pub(crate) version: String,
    pub(crate) metadata: HashMap<String, String>,
    pub(crate) payload: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Graph {
    pub(crate) nodes: Vec<CincinnatiPayload>,
    pub(crate) edges: Vec<(u64, u64)>,
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            nodes: vec![],
            edges: vec![],
        }
    }
}

impl Graph {
    pub fn from_metadata(releases: Vec<Release>, updates: Updates) -> Fallible<Self> {
        let nodes = releases
            .into_iter()
            .enumerate()
            .map(|(age_index, entry)| {
                // XXX(lucab): may panic, this should match on arch instead.
                let payload = entry.commits[0].checksum.clone();
                let mut current = CincinnatiPayload {
                    version: entry.version,
                    payload,
                    metadata: hashmap! {
                        "org.fedoraproject.coreos.scheme".to_string() => "checksum".to_string(),
                        "org.fedoraproject.coreos.metadata.releases.age_index".to_string() => age_index.to_string(),
                    },
                };

                // Augment with dead-ends metadata.
                if let Some(reason) = deadend_reason(&updates, &current) {
                    current.metadata.insert(
                        "org.fedoraproject.coreos.metadata.stream.deadend".to_string(),
                        true.to_string(),
                    );
                    current.metadata.insert(
                        "org.fedoraproject.coreos.metadata.stream.deadend.reason".to_string(),
                        reason,
                    );
                }

                // Augment with rollout throttling.
                if let Some(throttling) = compute_throttling(&updates, &current) {
                    current.metadata.insert(
                        "org.fedoraproject.coreos.metadata.stream.throttling".to_string(),
                        throttling,
                    );
                }

                current
            })
            .collect();

        // Synthesize an empty update graph.
        let edges = vec![];

        let graph = Graph { nodes, edges };
        Ok(graph)
    }
}

fn deadend_reason(updates: &Updates, release: &CincinnatiPayload) -> Option<String> {
    updates.deadends.iter().find_map(|dead| {
        if dead.version != release.version {
            return None;
        }

        if dead.reason.is_empty() {
            return Some(String::from("generic"));
        }

        Some(dead.reason.clone())
    })
}

fn compute_throttling(updates: &Updates, release: &CincinnatiPayload) -> Option<String> {
    updates.rollouts.iter().find_map(|rollout| {
        if rollout.version != release.version {
            return None;
        }

        rollout.policy.compute_throttling()
    })
}
