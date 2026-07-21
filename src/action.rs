use crate::world::{EdgeKind, NodeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    AddEdge {
        from: NodeId,
        to: NodeId,
        kind: EdgeKind,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_edge_uses_world_types() {
        let action = Action::AddEdge {
            from: NodeId(1),
            to: NodeId(2),
            kind: EdgeKind::Rail,
        };

        assert_eq!(
            action,
            Action::AddEdge {
                from: NodeId(1),
                to: NodeId(2),
                kind: EdgeKind::Rail,
            }
        );
    }
}
