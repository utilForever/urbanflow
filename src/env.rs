use crate::demand::Demand;
use crate::metrics::Metrics;
use crate::world::World;

#[derive(Debug)]
pub struct Env {
    pub world: World,
    pub demands: Vec<Demand>,
    pub metrics: Metrics,
    pub budget: f64,
    pub step_count: usize,
}
