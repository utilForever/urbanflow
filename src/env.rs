use crate::demand::Demand;
use crate::metrics::Metrics;
use crate::observation::Observation;
use crate::world::{NodeId, World, toy_city};

#[derive(Debug)]
pub struct Env {
    pub world: World,
    pub demands: Vec<Demand>,
    pub metrics: Metrics,
    pub budget: f64,
    pub step_count: usize,
}

impl Env {
    pub fn reset(&mut self) -> Observation {
        *self = Self {
            world: toy_city(),
            demands: vec![Demand::new(NodeId(0), NodeId(3), 10)],
            metrics: Metrics::default(),
            budget: 100.0,
            step_count: 0,
        };

        Observation {
            nodes: self.world.nodes.map(|node| node.id),
            edges: self.world.network.edges().to_vec(),
            demands: self.demands.clone(),
            budget: self.budget,
            step_count: self.step_count,
        }
    }
}
