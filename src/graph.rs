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
            .scan(String::new(), |parent, entry| {
                // XXX(lucab): may panic, this should match on arch instead.
                let payload = entry.commits[0].checksum.clone();
                let mut current = CincinnatiPayload {
                    version: entry.version,
                    payload,
                    metadata: hashmap! {
                        "org.fedoraproject.coreos.scheme".to_string() => "checksum".to_string(),
                    },
                };
                // Augment with child->parent metadata.
                if !parent.is_empty() {
                    current.metadata.insert(
                        "org.fedoraproject.coreos.metadata.releases.parent_version".to_string(),
                        parent.to_string(),
                    );
                }
                // Augment with deadends metadata.
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
                *parent = current.version.clone();
                Some(current)
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
