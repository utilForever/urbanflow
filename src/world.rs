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
}
