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
                if let Some(reason) = deadend_reason(&updates, &current) {
                    current
                        .metadata
                        .insert(metadata::DEADEND.to_string(), true.to_string());
                    current
                        .metadata
                        .insert(metadata::DEADEND_REASON.to_string(), reason);
                }

                // Augment with rollouts metadata.
                inject_throttling_params(&updates, &mut current);

                current
            })
            .collect();

        // Synthesize an update graph.
        let edges = vec![(0, 1), (0, 2), (1, 2)];

        let graph = Graph { nodes, edges };
        Ok(graph)
    }

    pub fn throttle_rollouts(self, client_wariness: f64) -> Self {
        use std::collections::HashSet;

        let now = chrono::Utc::now().timestamp();
        let mut graph = self;
        let mut hidden = HashSet::new();
        for (index, release) in graph.nodes.iter().enumerate() {
            // Skip if this release is not being rolled out.
            if release.metadata.get(metadata::START_EPOCH).is_none()
                && release.metadata.get(metadata::START_VALUE).is_none()
            {
                continue;
            };

            // Start epoch defaults to 0.
            let start_epoch = match release.metadata.get(metadata::START_EPOCH) {
                Some(epoch) => epoch.parse::<i64>().unwrap_or(0),
                None => 0i64,
            };

            // Start value defaults to 0.0.
            let start_value = match release.metadata.get(metadata::START_VALUE) {
                Some(val) => val.parse::<f64>().unwrap_or(0f64),
                None => 0f64,
            };

            // Duration has no default (i.e. no progress).
            let mut minutes: Option<u64> = None;
            if let Some(mins) = release.metadata.get(metadata::DURATION) {
                if let Ok(m) = mins.parse::<u64>() {
                    minutes = Some(m.max(1));
                }
            }

            let throttling: f64;
            if let Some(mins) = minutes {
                let end = start_epoch + (mins.saturating_mul(60)) as i64;
                let rate = (1.0 - start_value) / (end.saturating_sub(start_epoch)) as f64;
                if now < start_epoch {
                    throttling = 0.0;
                } else if now > end {
                    throttling = 1.0;
                } else {
                    throttling = start_value + rate * (now - start_epoch) as f64;
                }
            } else {
                // Without duration, rollout does not progress past initial value.
                if now < start_epoch {
                    throttling = 0.0;
                } else {
                    throttling = start_value
                }
            }

            if client_wariness > throttling {
                hidden.insert(index);
            }
        }

        graph.edges = graph
            .edges
            .into_iter()
            .filter_map(|(from, to)| {
                let dest = to as usize;
                if hidden.contains(&dest) {
                    None
                } else {
                    Some((from, to))
                }
            })
            .collect();
        graph
    }
}

fn deadend_reason(updates: &metadata::Updates, release: &CincinnatiPayload) -> Option<String> {
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
