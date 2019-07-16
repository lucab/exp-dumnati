use crate::scraper::Release;
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
    pub fn from_releases(releases: Vec<Release>) -> Fallible<Self> {
        let mut nodes = Vec::with_capacity(releases.len());
        for entry in releases {
            // XXX(lucab): may panic, this should match on arch instead.
            let payload = entry.commits[0].checksum.clone();
            let current = CincinnatiPayload {
                version: entry.version,
                payload,
                metadata: hashmap! {
                    "org.fedoraproject.coreos.scheme".to_string() => "checksum".to_string(),
                },
            };
            nodes.push(current);
        }

        // Synthesize a linear update graph.
        let edges = nodes
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(i, _)| {
                let pos = i as u64;
                if pos > 0 {
                    Some((pos.saturating_sub(1), pos))
                } else {
                    None
                }
            })
            .collect();

        let graph = Graph { nodes, edges };
        Ok(graph)
    }
}
