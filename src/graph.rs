// GROVE — Knowledge graph construction and traversal.

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::HashMap;

use crate::parser::Triple;

/// A node in the knowledge graph.
#[derive(Debug, Clone)]
pub struct KnowledgeNode {
    pub name: String,
    /// Whether a corresponding .md file exists (false = stub node).
    pub has_file: bool,
    /// Degree centrality (normalized 0.0 - 1.0).
    pub centrality: f32,
    /// Tags from the ## tags section, in order.
    pub tags: Vec<String>,
}

/// An edge in the knowledge graph.
#[derive(Debug, Clone)]
pub struct KnowledgeEdge {
    /// Original-cased verb (retained for per-edge access if needed).
    #[allow(dead_code)]
    pub verb: String,
    /// Lowercase verb for grouping.
    pub verb_key: String,
}

/// The knowledge graph built from parsed triples.
#[derive(Debug)]
pub struct KnowledgeGraph {
    pub graph: DiGraph<KnowledgeNode, KnowledgeEdge>,
    /// Map from node name (case-sensitive) to NodeIndex.
    pub node_indices: HashMap<String, NodeIndex>,
    /// Unique verb groups (lowercase) with their display form.
    pub verb_groups: HashMap<String, String>,
}

impl KnowledgeGraph {
    /// Build the graph from a set of triples, known file subjects, and per-subject tags.
    pub fn build(triples: &[Triple], file_subjects: &[String], subject_tags: &HashMap<String, Vec<String>>) -> Self {
        let mut graph = DiGraph::new();
        let mut node_indices: HashMap<String, NodeIndex> = HashMap::new();
        let mut verb_groups: HashMap<String, String> = HashMap::new();

        // Helper: get or create a node
        let get_or_create = |graph: &mut DiGraph<KnowledgeNode, KnowledgeEdge>,
                                  indices: &mut HashMap<String, NodeIndex>,
                                  name: &str,
                                  has_file: bool| -> NodeIndex {
            if let Some(&idx) = indices.get(name) {
                if has_file {
                    graph[idx].has_file = true;
                }
                idx
            } else {
                let idx = graph.add_node(KnowledgeNode {
                    name: name.to_string(),
                    has_file,
                    centrality: 0.0,
                    tags: Vec::new(),
                });
                indices.insert(name.to_string(), idx);
                idx
            }
        };

        // Register all file-backed subjects
        for subject in file_subjects {
            let idx = get_or_create(&mut graph, &mut node_indices, subject, true);
            if let Some(tags) = subject_tags.get(subject) {
                graph[idx].tags = tags.clone();
            }
        }

        // Add edges from triples
        for triple in triples {
            let subj_idx = get_or_create(&mut graph, &mut node_indices, &triple.subject, false);
            let obj_idx = get_or_create(&mut graph, &mut node_indices, &triple.object, false);

            let verb_key = triple.verb.to_lowercase();
            verb_groups
                .entry(verb_key.clone())
                .or_insert_with(|| triple.verb.clone());

            graph.add_edge(
                subj_idx,
                obj_idx,
                KnowledgeEdge {
                    verb: triple.verb.clone(),
                    verb_key,
                },
            );
        }

        // Compute centrality
        let node_count = graph.node_count();
        if node_count > 1 {
            let max_degree = (node_count - 1) as f32;
            for idx in graph.node_indices() {
                let degree = graph.neighbors_undirected(idx).count() as f32;
                graph[idx].centrality = degree / max_degree;
            }
        }

        KnowledgeGraph {
            graph,
            node_indices,
            verb_groups,
        }
    }

    /// Find the node with the highest centrality (default root).
    pub fn most_central_node(&self) -> Option<NodeIndex> {
        self.graph
            .node_indices()
            .max_by(|&a, &b| {
                self.graph[a]
                    .centrality
                    .partial_cmp(&self.graph[b].centrality)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get all unique verb keys (lowercase) sorted alphabetically.
    pub fn verb_keys_sorted(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.verb_groups.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// Get the display name for a verb key.
    pub fn verb_display<'a>(&'a self, key: &'a str) -> &'a str {
        self.verb_groups
            .get(key)
            .map(|s| s.as_str())
            .unwrap_or(key)
    }

    /// Get neighbors of a node up to a given depth, respecting verb filters.
    /// Returns set of visible node indices.
    pub fn visible_nodes(
        &self,
        root: NodeIndex,
        max_depth: usize,
        enabled_verbs: &HashMap<String, bool>,
    ) -> HashMap<NodeIndex, usize> {
        let mut visible: HashMap<NodeIndex, usize> = HashMap::new();
        let mut queue = std::collections::VecDeque::new();

        visible.insert(root, 0);
        queue.push_back((root, 0usize));

        while let Some((node, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            // Check outgoing edges
            for edge in self.graph.edges(node) {
                let verb_enabled = enabled_verbs
                    .get(&edge.weight().verb_key)
                    .copied()
                    .unwrap_or(true);
                if !verb_enabled {
                    continue;
                }
                let target = edge.target();
                if !visible.contains_key(&target) {
                    visible.insert(target, depth + 1);
                    queue.push_back((target, depth + 1));
                }
            }

            // Check incoming edges (undirected traversal for layout)
            for neighbor in self.graph.neighbors_directed(node, petgraph::Direction::Incoming) {
                // Check if any edge from neighbor to node has an enabled verb
                let has_enabled_edge = self
                    .graph
                    .edges_connecting(neighbor, node)
                    .any(|e| {
                        enabled_verbs
                            .get(&e.weight().verb_key)
                            .copied()
                            .unwrap_or(true)
                    });

                if has_enabled_edge && !visible.contains_key(&neighbor) {
                    visible.insert(neighbor, depth + 1);
                    queue.push_back((neighbor, depth + 1));
                }
            }
        }

        visible
    }

    /// Find shortest undirected path from `from` to `to`, returning node indices along the path.
    /// Returns None if no path exists.
    pub fn shortest_path(&self, from: NodeIndex, to: NodeIndex) -> Option<Vec<NodeIndex>> {
        if from == to {
            return Some(vec![from]);
        }
        let mut visited: HashMap<NodeIndex, NodeIndex> = HashMap::new();
        let mut queue = std::collections::VecDeque::new();
        visited.insert(from, from); // parent of root is itself
        queue.push_back(from);

        while let Some(node) = queue.pop_front() {
            for neighbor in self.graph.neighbors_undirected(node) {
                if visited.contains_key(&neighbor) {
                    continue;
                }
                visited.insert(neighbor, node);
                if neighbor == to {
                    // Reconstruct path
                    let mut path = vec![to];
                    let mut cur = to;
                    while cur != from {
                        cur = visited[&cur];
                        path.push(cur);
                    }
                    path.reverse();
                    return Some(path);
                }
                queue.push_back(neighbor);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_graph() {
        let triples = vec![
            Triple {
                subject: "A".to_string(),
                verb: "connects".to_string(),
                object: "B".to_string(),
            },
            Triple {
                subject: "A".to_string(),
                verb: "uses".to_string(),
                object: "C".to_string(),
            },
        ];
        let graph = KnowledgeGraph::build(&triples, &["A".to_string(), "B".to_string()], &HashMap::new());
        assert_eq!(graph.graph.node_count(), 3); // A, B, C
        assert_eq!(graph.graph.edge_count(), 2);
        assert!(graph.node_indices.contains_key("A"));
        assert!(graph.graph[graph.node_indices["C"]].has_file == false); // stub
    }

    #[test]
    fn test_most_central() {
        let triples = vec![
            Triple { subject: "Hub".to_string(), verb: "to".to_string(), object: "A".to_string() },
            Triple { subject: "Hub".to_string(), verb: "to".to_string(), object: "B".to_string() },
            Triple { subject: "Hub".to_string(), verb: "to".to_string(), object: "C".to_string() },
        ];
        let graph = KnowledgeGraph::build(&triples, &["Hub".to_string()], &HashMap::new());
        let root = graph.most_central_node().unwrap();
        assert_eq!(graph.graph[root].name, "Hub");
    }
}
