use crate::demand::Demand;
use crate::world::{Edge, NodeId};

#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    pub nodes: [NodeId; 4],
    pub edges: Vec<Edge>,
    pub demands: Vec<Demand>,
    pub budget: f64,
    pub step_count: usize,
}
