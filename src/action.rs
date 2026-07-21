use crate::world::{EdgeKind, NodeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    AddEdge {
        from: NodeId,
        to: NodeId,
        kind: EdgeKind,
    },
}
