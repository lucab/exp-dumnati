use crate::graph::Graph;
use crate::metadata;

/// Prune outgoing edges from "deadend" nodes.
pub fn filter_deadends(input: Graph) -> Graph {
    use std::collections::HashSet;

    let mut graph = input;
    let mut deadends = HashSet::new();
    for (index, release) in graph.nodes.iter().enumerate() {
        if release.metadata.get(metadata::DEADEND) == Some(&"true".into()) {
            deadends.insert(index);
        }
    }

    graph.edges.retain(|(from, _to)| {
        let index = *from as usize;
        !deadends.contains(&index)
    });
    graph.edges.shrink_to_fit();

    graph
}
