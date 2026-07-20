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
