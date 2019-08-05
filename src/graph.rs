use crate::metadata;
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
    pub fn from_metadata(
        releases: Vec<metadata::Release>,
        updates: metadata::Updates,
    ) -> Fallible<Self> {
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
                        metadata::SCHEME.to_string() => "checksum".to_string(),
                        metadata::AGE_INDEX.to_string() => age_index.to_string(),
                    },
                };

                // Augment with dead-ends metadata.
                Self::inject_deadend_reason(&updates, &mut current);

                // Augment with rollouts metadata.
                Self::inject_throttling_params(&updates, &mut current);

                current
            })
            .collect();

        // Synthesize an update graph.
        let edges = vec![(0, 1), (0, 2), (1, 2)];

        let graph = Graph { nodes, edges };
        Ok(graph)
    }

    fn inject_deadend_reason(updates: &metadata::Updates, release: &mut CincinnatiPayload) {
        for entry in &updates.deadends {
            if entry.version != release.version {
                continue;
            }

            let reason = if entry.reason.is_empty() {
                "generic"
            } else {
                &entry.reason
            };

            release
                .metadata
                .insert(metadata::DEADEND.to_string(), true.to_string());
            release
                .metadata
                .insert(metadata::DEADEND_REASON.to_string(), reason.to_string());
        }
    }

    fn inject_throttling_params(updates: &metadata::Updates, release: &mut CincinnatiPayload) {
        for entry in &updates.rollouts {
            if entry.version != release.version {
                continue;
            }

            release
                .metadata
                .insert(metadata::START_EPOCH.to_string(), entry.start_epoch.clone());
            release
                .metadata
                .insert(metadata::START_VALUE.to_string(), entry.start_value.clone());
            if let Some(minutes) = &entry.duration_minutes {
                release
                    .metadata
                    .insert(metadata::DURATION.to_string(), minutes.clone());
            }
        }
    }
}
