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

#[derive(Debug, Default)]
pub struct Network {
    edges: Vec<Edge>,
}

#[derive(Debug)]
pub struct ToyCity {
    pub nodes: [Node; 4],
    pub network: Network,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddEdgeError {
    DuplicateEdge,
    SelfConnection,
}

impl Network {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub fn add_edge(
        &mut self,
        from: NodeId,
        to: NodeId,
        kind: EdgeKind,
    ) -> Result<EdgeId, AddEdgeError> {
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

        let id = EdgeId(self.edges.len());
        self.edges.push(Edge { id, from, to, kind });
        Ok(id)
    }
}

pub fn toy_city() -> ToyCity {
    let nodes = [
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

    ToyCity { nodes, network }
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
}
