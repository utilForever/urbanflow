use std::collections::VecDeque;

use crate::NodeId;
use crate::demand::Demand;

/// An unweighted, undirected graph of the city network.
///
/// Nodes are identified by contiguous `NodeId`s and stored as an adjacency
/// list. Sprint 1 only needs to answer reachability ("are two nodes connected?"),
/// so edges carry no weight, direction, or capacity.
#[derive(Debug, Default, Clone)]
pub struct Network {
    /// `adjacency[n]` holds the neighbors of node `n`.
    adjacency: Vec<Vec<NodeId>>,
}

impl Network {
    pub fn new() -> Self {
        Self {
            adjacency: Vec::new(),
        }
    }

    /// Number of nodes currently in the network.
    pub fn node_count(&self) -> usize {
        self.adjacency.len()
    }

    /// Add an undirected edge between `a` and `b`, growing the node set to
    /// fit the larger id if needed.
    ///
    /// A self-loop (`a == b`) is recorded once rather than as a duplicate.
    pub fn add_edge(&mut self, a: NodeId, b: NodeId) {
        let max = a.max(b);
        if max >= self.adjacency.len() {
            self.adjacency.resize(max + 1, Vec::new());
        }
        self.adjacency[a].push(b);
        if a != b {
            self.adjacency[b].push(a);
        }
    }

    /// Whether `destination` is reachable from `origin` via a breadth-first
    /// traversal.
    ///
    /// A node always reaches itself (as long as it exists); ids outside the
    /// current node set are never reachable.
    pub fn is_reachable(&self, origin: NodeId, destination: NodeId) -> bool {
        if origin >= self.adjacency.len() || destination >= self.adjacency.len() {
            return false;
        }
        if origin == destination {
            return true;
        }

        let mut visited = vec![false; self.adjacency.len()];
        let mut queue = VecDeque::new();
        visited[origin] = true;
        queue.push_back(origin);

        while let Some(node) = queue.pop_front() {
            for &next in &self.adjacency[node] {
                if next == destination {
                    return true;
                }
                if !visited[next] {
                    visited[next] = true;
                    queue.push_back(next);
                }
            }
        }
        false
    }

    /// Whether the network can serve `demand`, i.e. its origin and
    /// destination are connected.
    pub fn can_serve(&self, demand: &Demand) -> bool {
        self.is_reachable(demand.origin, demand.destination)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a simple line: 0 - 1 - 2 - 3.
    fn line_network() -> Network {
        let mut net = Network::new();
        net.add_edge(0, 1);
        net.add_edge(1, 2);
        net.add_edge(2, 3);
        net
    }

    #[test]
    fn test_direct_edge_is_reachable() {
        let net = line_network();
        assert!(net.is_reachable(0, 1));
    }

    #[test]
    fn test_multi_hop_is_reachable() {
        let net = line_network();
        assert!(net.is_reachable(0, 3));
    }

    #[test]
    fn test_undirected_reachable_both_ways() {
        let net = line_network();
        assert!(net.is_reachable(3, 0));
    }

    #[test]
    fn test_disconnected_is_unreachable() {
        let mut net = line_network();
        net.add_edge(5, 6); // separate component
        assert!(!net.is_reachable(0, 5));
    }

    #[test]
    fn test_node_reaches_itself() {
        let net = line_network();
        assert!(net.is_reachable(2, 2));
    }

    #[test]
    fn test_unknown_node_is_unreachable() {
        let net = line_network();
        assert!(!net.is_reachable(0, 99));
    }

    #[test]
    fn test_can_serve_demand() {
        let net = line_network();
        assert!(net.can_serve(&Demand::new(0, 3, 10)));
        assert!(!net.can_serve(&Demand::new(0, 99, 10)));
    }

    #[test]
    fn test_self_loop_recorded_once() {
        let mut net = Network::new();
        net.add_edge(2, 2);
        assert_eq!(net.adjacency[2].len(), 1);
    }
}
