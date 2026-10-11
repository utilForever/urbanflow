use std::collections::{HashMap, VecDeque};

use crate::demand::Demand;
use crate::world::{EdgeId, Network, NodeId};

/// A directed reachability index derived from a [`Network`].
///
/// [`Network`] stores the city graph as a flat list of typed, directed edges.
/// Answering "can node A reach node B?" against that shape would mean scanning
/// every edge on each step, so this index materializes an adjacency list once
/// and runs breadth-first searches against it.
///
/// Node ids are interned into contiguous positions rather than used as offsets
/// directly, because [`Network`] accepts any [`NodeId`]. Storage therefore grows
/// with the number of nodes an edge actually mentions, and even `usize::MAX` is
/// just one more entry.
///
/// Links are one-way, matching both the stored edges and the `AddEdge` action:
/// a two-way link is two edges, and costs the builder twice. Traversal never
/// walks an edge backwards, so serving a demand always reflects infrastructure
/// that was actually built.
///
/// This is a read-only view: rebuild it whenever the underlying `Network`
/// changes. Only nodes referenced by an edge are covered, because `Network`
/// derives its node set from edges; an isolated node cannot be represented.
#[derive(Debug, Default, Clone)]
pub struct ConnectivityIndex {
    /// Interned position of each node. Looked up by key only, never iterated,
    /// so traversal order stays deterministic.
    position_of: HashMap<NodeId, usize>,
    /// `successors[p]` holds each interned successor and the edge used to reach
    /// it, in network insertion order.
    successors: Vec<Vec<(usize, EdgeId)>>,
}

/// Return the interned position of `node`, assigning the next one if this is
/// the first time the node is seen.
fn intern(
    node: NodeId,
    position_of: &mut HashMap<NodeId, usize>,
    successors: &mut Vec<Vec<(usize, EdgeId)>>,
) -> usize {
    *position_of.entry(node).or_insert_with(|| {
        successors.push(Vec::new());
        successors.len() - 1
    })
}

impl ConnectivityIndex {
    /// Build the index from `network`, following each edge in its `from -> to`
    /// direction only.
    pub fn from_network(network: &Network) -> Self {
        let mut position_of = HashMap::new();
        let mut successors: Vec<Vec<(usize, EdgeId)>> = Vec::new();

        for edge in network.edges() {
            let from = intern(edge.from, &mut position_of, &mut successors);
            let to = intern(edge.to, &mut position_of, &mut successors);
            successors[from].push((to, edge.id));
        }

        Self {
            position_of,
            successors,
        }
    }

    /// Number of distinct nodes the index covers, i.e. how many nodes are named
    /// by at least one edge.
    pub fn node_count(&self) -> usize {
        self.successors.len()
    }

    /// Return the first shortest path from `origin` to `destination`.
    ///
    /// Equal-hop paths are resolved by network edge insertion order. A known
    /// node has an empty path to itself; unknown or unreachable nodes return
    /// `None`.
    pub fn path(&self, origin: NodeId, destination: NodeId) -> Option<Vec<EdgeId>> {
        let &origin = self.position_of.get(&origin)?;
        let &destination = self.position_of.get(&destination)?;

        if origin == destination {
            return Some(Vec::new());
        }

        let mut visited = vec![false; self.successors.len()];
        let mut predecessors = vec![None; self.successors.len()];
        let mut queue = VecDeque::new();

        visited[origin] = true;
        queue.push_back(origin);

        while let Some(node) = queue.pop_front() {
            for &(next, edge) in &self.successors[node] {
                if visited[next] {
                    continue;
                }

                visited[next] = true;
                predecessors[next] = Some((node, edge));

                if next == destination {
                    let mut path = Vec::new();
                    let mut current = destination;

                    while current != origin {
                        let (previous, edge) = predecessors[current]?;
                        path.push(edge);
                        current = previous;
                    }

                    path.reverse();
                    return Some(path);
                }

                queue.push_back(next);
            }
        }

        None
    }

    /// Whether `destination` can be reached from `origin` by following edges in
    /// their forward direction.
    ///
    /// A node always reaches itself (as long as an edge mentions it); nodes the
    /// network never mentions are not reachable, whatever their id.
    pub fn is_reachable(&self, origin: NodeId, destination: NodeId) -> bool {
        self.path(origin, destination).is_some()
    }

    /// Whether the network can serve `demand`, i.e. a directed path runs from
    /// its origin to its destination.
    pub fn can_serve(&self, demand: &Demand) -> bool {
        self.is_reachable(demand.origin, demand.destination)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::world::EdgeKind;

    /// Builds a directed line: 0 -> 1 -> 2 -> 3.
    fn line_index() -> ConnectivityIndex {
        let mut net = Network::new();
        net.add_edge(NodeId(0), NodeId(1), EdgeKind::Road).unwrap();
        net.add_edge(NodeId(1), NodeId(2), EdgeKind::Road).unwrap();
        net.add_edge(NodeId(2), NodeId(3), EdgeKind::Road).unwrap();

        ConnectivityIndex::from_network(&net)
    }

    #[rstest]
    #[case::direct_edge(NodeId(0), NodeId(1), true)]
    #[case::multiple_hops(NodeId(0), NodeId(3), true)]
    #[case::reverse_direction(NodeId(3), NodeId(0), false)]
    #[case::same_node(NodeId(2), NodeId(2), true)]
    #[case::unknown_destination(NodeId(0), NodeId(99), false)]
    #[case::unknown_same_node(NodeId(99), NodeId(99), false)]
    fn test_reachability_on_a_directed_line(
        #[case] from: NodeId,
        #[case] to: NodeId,
        #[case] reachable: bool,
    ) {
        assert_eq!(line_index().is_reachable(from, to), reachable);
    }

    #[test]
    fn test_path_returns_ordered_edge_ids() {
        let mut net = Network::new();
        let first = net.add_edge(NodeId(0), NodeId(1), EdgeKind::Road).unwrap();
        let second = net.add_edge(NodeId(1), NodeId(2), EdgeKind::Rail).unwrap();

        assert_eq!(
            ConnectivityIndex::from_network(&net).path(NodeId(0), NodeId(2)),
            Some(vec![first, second])
        );
    }

    #[test]
    fn test_path_breaks_equal_hop_ties_by_edge_insertion_order() {
        let mut net = Network::new();
        let first_route_start = net.add_edge(NodeId(0), NodeId(2), EdgeKind::Road).unwrap();
        net.add_edge(NodeId(0), NodeId(1), EdgeKind::Road).unwrap();
        let first_route_end = net.add_edge(NodeId(2), NodeId(3), EdgeKind::Rail).unwrap();
        net.add_edge(NodeId(1), NodeId(3), EdgeKind::Rail).unwrap();

        assert_eq!(
            ConnectivityIndex::from_network(&net).path(NodeId(0), NodeId(3)),
            Some(vec![first_route_start, first_route_end])
        );
    }

    #[rstest]
    #[case::same_node(NodeId(2), NodeId(2), Some(Vec::new()))]
    #[case::reverse_direction(NodeId(3), NodeId(0), None)]
    #[case::unknown_destination(NodeId(0), NodeId(99), None)]
    #[case::unknown_same_node(NodeId(99), NodeId(99), None)]
    fn test_path_distinguishes_empty_path_from_no_path(
        #[case] from: NodeId,
        #[case] to: NodeId,
        #[case] path: Option<Vec<EdgeId>>,
    ) {
        let idx = line_index();
        assert_eq!(idx.path(from, to), path);
    }

    #[test]
    fn test_two_way_link_needs_both_edges() {
        let mut net = Network::new();
        net.add_edge(NodeId(0), NodeId(1), EdgeKind::Rail).unwrap();
        assert!(!ConnectivityIndex::from_network(&net).is_reachable(NodeId(1), NodeId(0)));

        net.add_edge(NodeId(1), NodeId(0), EdgeKind::Rail).unwrap();
        assert!(ConnectivityIndex::from_network(&net).is_reachable(NodeId(1), NodeId(0)));
    }

    #[test]
    fn test_disconnected_is_unreachable() {
        let mut net = Network::new();
        net.add_edge(NodeId(0), NodeId(1), EdgeKind::Road).unwrap();
        net.add_edge(NodeId(2), NodeId(3), EdgeKind::Rail).unwrap(); // separate component
        assert!(!ConnectivityIndex::from_network(&net).is_reachable(NodeId(0), NodeId(3)));
    }

    #[test]
    fn test_node_count_covers_referenced_nodes() {
        assert_eq!(line_index().node_count(), 4);
    }

    /// `Network` puts no bound on a `NodeId`, so the index must cope with ids
    /// that are huge or far apart without overflowing or sizing storage to the
    /// id itself.
    #[test]
    fn test_sparse_node_ids_are_handled() {
        let highest = NodeId(usize::MAX);
        let middling = NodeId(usize::MAX / 2);

        let mut net = Network::new();
        net.add_edge(highest, NodeId(0), EdgeKind::Road).unwrap();
        net.add_edge(NodeId(0), middling, EdgeKind::Rail).unwrap();
        let idx = ConnectivityIndex::from_network(&net);

        assert_eq!(idx.node_count(), 3);
        assert!(idx.is_reachable(highest, middling));
        assert!(!idx.is_reachable(middling, highest));
    }

    #[rstest]
    #[case::outbound(Demand::new(NodeId(0), NodeId(3), 10), true)]
    #[case::return_trip(Demand::new(NodeId(3), NodeId(0), 10), false)]
    #[case::unknown_destination(Demand::new(NodeId(0), NodeId(99), 10), false)]
    fn test_can_serve_demand(#[case] demand: Demand, #[case] served: bool) {
        let idx = line_index();
        assert_eq!(idx.can_serve(&demand), served);
    }
}
