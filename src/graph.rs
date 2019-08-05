use crate::scraper::{Release, Updates};
use failure::Fallible;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

static SCHEME: &str = "org.fedoraproject.coreos.scheme";
static AGE_INDEX: &str = "org.fedoraproject.coreos.releases.age_index";
static DEADEND: &str = "org.fedoraproject.coreos.updates.deadend";
static DEADEND_REASON: &str = "org.fedoraproject.coreos.updates.deadend_reason";
static START_EPOCH: &str = "org.fedoraproject.coreos.updates.start_epoch";
static START_VALUE: &str = "org.fedoraproject.coreos.updates.start_value";
static DURATION: &str = "org.fedoraproject.coreos.updates.duration_minutes";

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
                        SCHEME.to_string() => "checksum".to_string(),
                        AGE_INDEX.to_string() => age_index.to_string(),
                    },
                };

                // Augment with dead-ends metadata.
                if let Some(reason) = deadend_reason(&updates, &current) {
                    current
                        .metadata
                        .insert(DEADEND.to_string(), true.to_string());
                    current.metadata.insert(DEADEND_REASON.to_string(), reason);
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

        let mut graph = self;
        let mut deadends = HashSet::new();
        for (index, release) in graph.nodes.iter().enumerate() {
            if release.metadata.get(DEADEND) == Some(&"true".into()) {
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

        let now = chrono::Utc::now().timestamp();
        let mut graph = self;
        let mut hidden = HashSet::new();
        for (index, release) in graph.nodes.iter().enumerate() {
            // Skip if this release is not being rolled out.
            if release.metadata.get(START_EPOCH).is_none()
                && release.metadata.get(START_VALUE).is_none()
            {
                continue;
            };

            // Start epoch defaults to 0.
            let start_epoch = match release.metadata.get(START_EPOCH) {
                Some(epoch) => epoch.parse::<i64>().unwrap_or(0),
                None => 0i64,
            };

            // Start epoch defaults to 1.0.
            let start_value = match release.metadata.get(START_VALUE) {
                Some(val) => val.parse::<f64>().unwrap_or(0f64),
                None => 1f64,
            };

            // Duration has no default (i.e. no progress).
            let mut minutes: Option<u64> = None;
            if let Some(mins) = release.metadata.get(DURATION) {
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
            "org.fedoraproject.coreos.updates.start_epoch".to_string(),
            entry.start_epoch.clone(),
        );
        release.metadata.insert(
            "org.fedoraproject.coreos.updates.start_value".to_string(),
            entry.start_value.clone(),
        );
        if let Some(minutes) = &entry.duration_minutes {
            release.metadata.insert(
                "org.fedoraproject.coreos.updates.duration_minutes".to_string(),
                minutes.clone(),
            );
        }
    }
}
