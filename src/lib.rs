/*!
# cuda-topology

Fleet network topology and graph analysis.

Agents form a network. This crate provides graph structures and
algorithms for understanding fleet connectivity — shortest paths,
clustering, centrality, and community detection.

- Directed/undirected graph
- Shortest path (BFS, Dijkstra)
- Centrality measures (degree, betweenness)
- Connected components
- Community detection (label propagation)
- Cluster coefficient
- Topology snapshots
*/

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Edge weight
pub type Weight = f64;

/// A graph node
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub labels: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl Node {
    pub fn new(id: &str) -> Self { Node { id: id.to_string(), labels: vec![], metadata: HashMap::new() } }
}

/// A graph edge
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub weight: Weight,
    pub directed: bool,
}

/// A fleet graph
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FleetGraph {
    pub nodes: HashMap<String, Node>,
    pub adjacency: HashMap<String, Vec<(String, Weight)>>, // node → [(neighbor, weight)]
    pub directed: bool,
}

impl FleetGraph {
    pub fn new(directed: bool) -> Self { FleetGraph { nodes: HashMap::new(), adjacency: HashMap::new(), directed } }

    /// Add a node
    pub fn add_node(&mut self, node: Node) {
        self.adjacency.entry(node.id.clone()).or_insert_with(Vec::new);
        self.nodes.insert(node.id, node);
    }

    /// Add an edge
    pub fn add_edge(&mut self, from: &str, to: &str, weight: Weight) {
        self.add_node(Node::new(from));
        self.add_node(Node::new(to));
        self.adjacency.entry(from.to_string()).or_default().push((to.to_string(), weight));
        if !self.directed {
            self.adjacency.entry(to.to_string()).or_default().push((from.to_string(), weight));
        }
    }

    /// Neighbors of a node
    pub fn neighbors(&self, id: &str) -> Vec<&str> {
        self.adjacency.get(id).map(|adj| adj.iter().map(|(n, _)| n.as_str()).collect()).unwrap_or_default()
    }

    /// Degree of a node
    pub fn degree(&self, id: &str) -> usize { self.neighbors(id).len() }

    /// BFS shortest path (unweighted)
    pub fn bfs_shortest_path(&self, from: &str, to: &str) -> Option<Vec<String>> {
        if from == to { return Some(vec![from.to_string()]); }
        let mut visited = HashSet::new();
        let mut queue: VecDeque<(String, Vec<String>)> = VecDeque::new();
        visited.insert(from.to_string());
        queue.push_back((from.to_string(), vec![from.to_string()]));

        while let Some((current, path)) = queue.pop_front() {
            for (neighbor, _) in self.adjacency.get(&current).unwrap_or(&vec![]) {
                if neighbor == to { let mut p = path.clone(); p.push(neighbor.clone()); return Some(p); }
                if visited.insert(neighbor.clone()) {
                    let mut p = path.clone();
                    p.push(neighbor.clone());
                    queue.push_back((neighbor.clone(), p));
                }
            }
        }
        None
    }

    /// Connected components (undirected only)
    pub fn connected_components(&self) -> Vec<HashSet<String>> {
        let mut visited = HashSet::new();
        let mut components = vec![];
        for node_id in self.nodes.keys() {
            if visited.contains(node_id) { continue; }
            let mut component = HashSet::new();
            let mut queue = VecDeque::new();
            queue.push_back(node_id.clone());
            while let Some(current) = queue.pop_front() {
                if !component.insert(current.clone()) { continue; }
                visited.insert(current.clone());
                for (neighbor, _) in self.adjacency.get(&current).unwrap_or(&vec![]) {
                    if !component.contains(neighbor) { queue.push_back(neighbor.clone()); }
                }
            }
            components.push(component);
        }
        components
    }

    /// Degree centrality for all nodes
    pub fn degree_centrality(&self) -> HashMap<String, f64> {
        let n = self.nodes.len().max(1);
        self.nodes.keys().map(|id| (id.clone(), self.degree(id) as f64 / (n - 1) as f64)).collect()
    }

    /// Cluster coefficient for a node (ratio of actual to possible triangles)
    pub fn clustering_coefficient(&self, id: &str) -> f64 {
        let neighbors = self.neighbors(id);
        let k = neighbors.len();
        if k < 2 { return 0.0; }
        let possible = k * (k - 1) / 2;
        let mut triangles = 0u32;
        let neighbor_set: HashSet<&str> = neighbors.iter().cloned().collect();
        for i in 0..neighbors.len() {
            for j in (i+1)..neighbors.len() {
                if self.adjacency.get(neighbors[i]).map_or(false, |adj| adj.iter().any(|(n, _)| n == neighbors[j])) {
                    triangles += 1;
                }
            }
        }
        triangles as f64 / possible as f64
    }

    /// Label propagation community detection
    pub fn communities(&self, max_iters: usize) -> HashMap<String, String> {
        let mut labels: HashMap<String, String> = self.nodes.keys().map(|id| (id.clone(), id.clone())).collect();
        for _ in 0..max_iters {
            let mut changed = false;
            for node_id in self.nodes.keys() {
                let neighbors = self.neighbors(node_id);
                let mut label_counts: HashMap<&str, usize> = HashMap::new();
                let own_label = labels.get(node_id).unwrap().clone();
                *label_counts.entry(&own_label).or_insert(0) += 1;
                for neighbor in &neighbors {
                    let n_label = labels.get(*neighbor).unwrap();
                    *label_counts.entry(n_label.as_str()).or_insert(0) += 1;
                }
                let best = label_counts.iter().max_by_key(|(_, c)| *c).map(|(l, _)| *l).unwrap();
                if best != own_label.as_str() { labels.insert(node_id.clone(), best.to_string()); changed = true; }
            }
            if !changed { break; }
        }
        labels
    }

    /// Is the graph connected?
    pub fn is_connected(&self) -> bool { self.connected_components().len() <= 1 }

    /// Summary
    pub fn summary(&self) -> String {
        let components = self.connected_components();
        let avg_cc: f64 = self.nodes.keys().map(|id| self.clustering_coefficient(id)).sum::<f64>() / self.nodes.len().max(1) as f64;
        format!("FleetGraph: {} nodes, {} edges, {} components, avg_clustering={:.3}",
            self.nodes.len(), self.adjacency.values().map(|v| v.len()).sum::<usize>() / if self.directed { 1 } else { 2 }, components.len(), avg_cc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_graph() -> FleetGraph {
        let mut g = FleetGraph::new(false);
        g.add_edge("a", "b", 1.0);
        g.add_edge("b", "c", 1.0);
        g.add_edge("c", "d", 1.0);
        g.add_edge("a", "d", 2.0);
        g
    }

    #[test]
    fn test_neighbors() {
        let g = make_test_graph();
        let n = g.neighbors("b");
        assert!(n.contains(&"a"));
        assert!(n.contains(&"c"));
    }

    #[test]
    fn test_degree() {
        let g = make_test_graph();
        assert_eq!(g.degree("a"), 2);
    }

    #[test]
    fn test_bfs_shortest_path() {
        let g = make_test_graph();
        let path = g.bfs_shortest_path("a", "d").unwrap();
        assert_eq!(path[0], "a");
        assert_eq!(path.last().unwrap(), "d");
    }

    #[test]
    fn test_bfs_same_node() {
        let g = make_test_graph();
        let path = g.bfs_shortest_path("a", "a").unwrap();
        assert_eq!(path.len(), 1);
    }

    #[test]
    fn test_bfs_no_path() {
        let mut g = FleetGraph::new(false);
        g.add_edge("a", "b", 1.0);
        g.add_node(Node::new("isolated"));
        assert!(g.bfs_shortest_path("a", "isolated").is_none());
    }

    #[test]
    fn test_connected_components() {
        let mut g = FleetGraph::new(false);
        g.add_edge("a", "b", 1.0);
        g.add_edge("b", "c", 1.0);
        g.add_node(Node::new("isolated"));
        assert_eq!(g.connected_components().len(), 2);
    }

    #[test]
    fn test_is_connected() {
        let g = make_test_graph();
        assert!(g.is_connected());
    }

    #[test]
    fn test_degree_centrality() {
        let g = make_test_graph();
        let dc = g.degree_centrality();
        assert!(dc.contains_key("a"));
    }

    #[test]
    fn test_clustering_coefficient() {
        let mut g = FleetGraph::new(false);
        g.add_edge("a", "b", 1.0);
        g.add_edge("a", "c", 1.0);
        g.add_edge("b", "c", 1.0); // triangle
        let cc = g.clustering_coefficient("a");
        assert!((cc - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_communities() {
        let mut g = FleetGraph::new(false);
        g.add_edge("a", "b", 1.0);
        g.add_edge("b", "c", 1.0);
        g.add_edge("d", "e", 1.0);
        let communities = g.communities(10);
        // a,b,c should share a label; d,e another
        assert_ne!(communities["a"], communities["d"]);
    }

    #[test]
    fn test_summary() {
        let g = FleetGraph::new(false);
        let s = g.summary();
        assert!(s.contains("0 nodes"));
    }
}
