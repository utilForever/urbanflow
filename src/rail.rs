use crate::world::EdgeId;

/// An ordered sequence of Rail edges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RailRoute {
    pub edges: Vec<EdgeId>,
}
