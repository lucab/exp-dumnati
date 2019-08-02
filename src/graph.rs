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
                        "org.fedoraproject.coreos.metadata.updates.deadend".to_string(),
                        true.to_string(),
                    );
                    current.metadata.insert(
                        "org.fedoraproject.coreos.metadata.updates.deadend.reason".to_string(),
                        reason,
                    );
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

    pub fn filter_deadends(self) -> Self {
        use std::collections::HashSet;
        static KEY: &str = "org.fedoraproject.coreos.metadata.updates.deadend";

        let mut graph = self;
        let mut deadends = HashSet::new();
        for (index, release) in graph.nodes.iter().enumerate() {
            if release.metadata.get(KEY) == Some(&"true".into()) {
                deadends.insert(index);
            }
        }

        graph.edges = graph
            .edges
            .into_iter()
            .filter_map(|(from, to)| {
                let src = from as usize;
                if deadends.contains(&src) {
                    None
                } else {
                    Some((from, to))
                }
            })
            .collect();
        graph
    }

    pub fn throttle_rollouts(self, client_wariness: f64) -> Self {
        use std::collections::HashSet;
        static START_EPOCH: &str = "org.fedoraproject.coreos.metadata.updates.start_epoch";
        static START_VALUE: &str = "org.fedoraproject.coreos.metadata.updates.start_value";
        static DURATION: &str = "org.fedoraproject.coreos.metadata.updates.duration_minutes";

        let now = chrono::Utc::now().timestamp();
        let mut graph = self;
        let mut hidden = HashSet::new();
        for (index, release) in graph.nodes.iter().enumerate() {
            let start_epoch: i64;
            if let Some(epoch) = release.metadata.get(START_EPOCH) {
                start_epoch = epoch.parse::<i64>().unwrap_or(0);
            } else {
                continue;
            }

            let start_value: f64;
            if let Some(val) = release.metadata.get(START_VALUE) {
                start_value = val.parse::<f64>().unwrap_or(0f64);
            } else {
                continue;
            }

            let mut minutes: Option<u64> = None;
            if let Some(mins) = release.metadata.get(DURATION) {
                if let Ok(m) = mins.parse::<u64>() {
                    minutes = Some(m);
                }
            }

            let throttling: f64;
            if let Some(mins) = minutes {
                let end = start_epoch + (mins * 60) as i64;
                let rate = (1.0 - start_value) / (end - start_epoch) as f64;
                if now < start_epoch {
                    throttling = 0.0;
                } else if now > end {
                    throttling = 1.0;
                } else {
                    throttling = start_value + rate * (now - start_epoch) as f64;
                }
            } else {
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

fn inject_throttling_params(updates: &Updates, release: &mut CincinnatiPayload) {
    for entry in &updates.rollouts {
        if entry.version != release.version {
            continue;
        }

        release.metadata.insert(
            "org.fedoraproject.coreos.metadata.updates.start_epoch".to_string(),
            entry.start_epoch.clone(),
        );
        release.metadata.insert(
            "org.fedoraproject.coreos.metadata.updates.start_value".to_string(),
            entry.start_value.clone(),
        );
        if let Some(minutes) = &entry.duration_minutes {
            release.metadata.insert(
                "org.fedoraproject.coreos.metadata.updates.duration_minutes".to_string(),
                minutes.clone(),
            );
        }
    }
}
