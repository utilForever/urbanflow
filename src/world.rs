use std::collections::HashSet;

use crate::env::InitError;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EdgeId(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Node {
    pub id: NodeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Edge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgeKind {
    Road,
    Rail,
}

impl EdgeKind {
    pub const fn capacity(self) -> u32 {
        match self {
            Self::Road => 10,
            Self::Rail => 20,
        }
    }

    pub const fn construction_cost(self) -> f64 {
        match self {
            Self::Road => 1.0,
            Self::Rail => 2.0,
        }
    }
}

#[derive(Debug, Default)]
pub struct Network {
    edges: Vec<Edge>,
}

#[derive(Debug)]
pub struct World {
    pub nodes: Vec<Node>,
    pub network: Network,
}

pub type ToyCity = World;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddEdgeError {
    DuplicateEdge,
    SelfConnection,
}

impl World {
    pub fn validate_topology(&self) -> Result<(), InitError> {
        let mut node_ids = HashSet::with_capacity(self.nodes.len());

        for node in self.nodes.iter() {
            if !node_ids.insert(node.id) {
                return Err(InitError::DuplicateNode(node.id));
            }
        }

        for edge in self.network.edges() {
            for endpoint in [edge.from, edge.to] {
                if !node_ids.contains(&endpoint) {
                    return Err(InitError::UnknownEdgeEndpoint(endpoint));
                }
            }
        }

        Ok(())
    }
}

impl Network {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub(crate) fn validate_add_edge(
        &self,
        from: NodeId,
        to: NodeId,
        kind: EdgeKind,
    ) -> Result<(), AddEdgeError> {
        if from == to {
            return Err(AddEdgeError::SelfConnection);
        }

        if self
            .edges
            .iter()
            .any(|edge| edge.from == from && edge.to == to && edge.kind == kind)
        {
            return Err(AddEdgeError::DuplicateEdge);
        }

        Ok(())
    }

    pub fn add_edge(
        &mut self,
        from: NodeId,
        to: NodeId,
        kind: EdgeKind,
    ) -> Result<EdgeId, AddEdgeError> {
        self.validate_add_edge(from, to, kind)?;

        let id = EdgeId(self.edges.len());
        self.edges.push(Edge { id, from, to, kind });
        Ok(id)
    }
}

pub fn toy_city() -> ToyCity {
    let nodes = vec![
        Node { id: NodeId(0) },
        Node { id: NodeId(1) },
        Node { id: NodeId(2) },
        Node { id: NodeId(3) },
    ];
    let mut network = Network::new();

    network
        .add_edge(nodes[0].id, nodes[1].id, EdgeKind::Road)
        .expect("toy city edges are valid");
    network
        .add_edge(nodes[2].id, nodes[3].id, EdgeKind::Rail)
        .expect("toy city edges are valid");

    World { nodes, network }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_model_types_compose() {
        let node = Node { id: NodeId(1) };
        let edge = Edge {
            id: EdgeId(2),
            from: node.id,
            to: NodeId(3),
            kind: EdgeKind::Rail,
        };

        assert_eq!(edge.from, node.id);
    }

    #[test]
    fn world_preserves_caller_defined_node_order() {
        let world = World {
            nodes: vec![
                Node {
                    id: NodeId(usize::MAX),
                },
                Node { id: NodeId(7) },
                Node { id: NodeId(0) },
            ],
            network: Network::new(),
        };

        assert_eq!(
            world.nodes.iter().map(|node| node.id).collect::<Vec<_>>(),
            vec![NodeId(usize::MAX), NodeId(7), NodeId(0)]
        );
    }

    #[test]
    fn world_topology_rejects_the_first_duplicate_node() {
        let mut network = Network::new();
        network
            .add_edge(NodeId(99), NodeId(3), EdgeKind::Road)
            .unwrap();

        let world = World {
            nodes: vec![
                Node { id: NodeId(7) },
                Node { id: NodeId(3) },
                Node { id: NodeId(7) },
                Node { id: NodeId(3) },
            ],
            network,
        };

        assert_eq!(
            world.validate_topology(),
            Err(crate::env::InitError::DuplicateNode(NodeId(7)))
        );
    }

    #[test]
    fn world_topology_rejects_unknown_edge_endpoints() {
        for (from, to, unknown) in [
            (NodeId(99), NodeId(1), NodeId(99)),
            (NodeId(1), NodeId(99), NodeId(99)),
            (NodeId(98), NodeId(99), NodeId(98)),
        ] {
            let mut network = Network::new();
            network.add_edge(from, to, EdgeKind::Road).unwrap();

            let world = World {
                nodes: vec![Node { id: NodeId(1) }, Node { id: NodeId(2) }],
                network,
            };

            assert_eq!(
                world.validate_topology(),
                Err(crate::env::InitError::UnknownEdgeEndpoint(unknown))
            );
        }
    }

    #[test]
    fn world_topology_preserves_valid_node_and_edge_order() {
        let mut network = Network::new();
        network
            .add_edge(NodeId(usize::MAX), NodeId(7), EdgeKind::Rail)
            .unwrap();
        network
            .add_edge(NodeId(0), NodeId(usize::MAX), EdgeKind::Road)
            .unwrap();

        let world = World {
            nodes: vec![
                Node {
                    id: NodeId(usize::MAX),
                },
                Node { id: NodeId(7) },
                Node { id: NodeId(0) },
            ],
            network,
        };

        assert_eq!(world.validate_topology(), Ok(()));
        assert_eq!(
            world.nodes.iter().map(|node| node.id).collect::<Vec<_>>(),
            vec![NodeId(usize::MAX), NodeId(7), NodeId(0)]
        );
        assert_eq!(
            world
                .network
                .edges()
                .iter()
                .map(|edge| edge.id)
                .collect::<Vec<_>>(),
            vec![EdgeId(0), EdgeId(1)]
        );
    }

    #[test]
    fn edge_kinds_expose_capacity_and_construction_cost() {
        assert_eq!(EdgeKind::Road.capacity(), 10);
        assert_eq!(EdgeKind::Rail.capacity(), 20);
        assert_eq!(EdgeKind::Road.construction_cost(), 1.0);
        assert_eq!(EdgeKind::Rail.construction_cost(), 2.0);
    }

    #[test]
    fn network_adds_valid_edges_and_rejects_invalid_ones() {
        let mut network = Network::new();

        assert_eq!(
            network.add_edge(NodeId(1), NodeId(2), EdgeKind::Rail),
            Ok(EdgeId(0))
        );
        assert_eq!(
            network.add_edge(NodeId(1), NodeId(2), EdgeKind::Road),
            Ok(EdgeId(1))
        );
        assert_eq!(
            network.add_edge(NodeId(1), NodeId(2), EdgeKind::Rail),
            Err(AddEdgeError::DuplicateEdge)
        );
        assert_eq!(
            network.add_edge(NodeId(1), NodeId(1), EdgeKind::Road),
            Err(AddEdgeError::SelfConnection)
        );
        assert_eq!(network.edges().len(), 2);
    }

    #[test]
    fn toy_city_is_fixed() {
        let city = toy_city();

        assert_eq!(
            city.nodes.iter().map(|node| node.id).collect::<Vec<_>>(),
            vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)]
        );
        assert_eq!(
            city.network.edges(),
            &[
                Edge {
                    id: EdgeId(0),
                    from: NodeId(0),
                    to: NodeId(1),
                    kind: EdgeKind::Road,
                },
                Edge {
                    id: EdgeId(1),
                    from: NodeId(2),
                    to: NodeId(3),
                    kind: EdgeKind::Rail,
                },
            ]
        );
    }
}
