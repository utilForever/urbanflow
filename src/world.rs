#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EdgeId(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Node {
    pub id: NodeId,
}
