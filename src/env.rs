use crate::action::Action;
use crate::demand::Demand;
use crate::metrics::Metrics;
use crate::network::ConnectivityIndex;
use crate::observation::Observation;
use crate::step_result::StepResult;
use crate::world::{AddEdgeError, NodeId, World, toy_city};

#[derive(Debug)]
pub struct Env {
    pub world: World,
    pub demands: Vec<Demand>,
    pub metrics: Metrics,
    pub budget: f64,
    pub step_count: usize,
    pub max_steps: usize,
}

const CONSTRUCTION_COST: f64 = 1.0;

fn reward(metrics: Metrics) -> f64 {
    let demand = i128::from(metrics.served_demand) - i128::from(metrics.unserved_demand);
    demand as f64 - metrics.congestion - metrics.cost
}

impl Env {
    fn observation(&self) -> Observation {
        Observation {
            nodes: self.world.nodes.map(|node| node.id),
            edges: self.world.network.edges().to_vec(),
            demands: self.demands.clone(),
            budget: self.budget,
            step_count: self.step_count,
        }
    }

    pub fn reset(&mut self) -> Observation {
        let max_steps = self.max_steps;
        *self = Self {
            world: toy_city(),
            demands: vec![Demand::new(NodeId(0), NodeId(3), 10)],
            metrics: Metrics::default(),
            budget: 100.0,
            step_count: 0,
            max_steps,
        };

        self.observation()
    }

    pub fn step(&mut self, action: Action) -> Result<StepResult, AddEdgeError> {
        let Action::AddEdge { from, to, kind } = action;
        let total_demand = self
            .demands
            .iter()
            .try_fold(0_u64, |total, demand| {
                total.checked_add(u64::from(demand.amount))
            })
            .expect("total demand exceeds metric capacity");
        let next_step_count = self
            .step_count
            .checked_add(1)
            .expect("step count exceeds environment capacity");

        self.world.network.add_edge(from, to, kind)?;
        self.budget -= CONSTRUCTION_COST;
        self.step_count = next_step_count;

        let connectivity = ConnectivityIndex::from_network(&self.world.network);
        let served = self
            .demands
            .iter()
            .filter(|demand| connectivity.can_serve(demand))
            .map(|demand| u64::from(demand.amount))
            .sum();
        let unserved = total_demand - served;

        self.metrics = Metrics::new(
            served,
            unserved,
            0.0,
            self.world.network.edges().len() as f64,
        );

        let reward = reward(self.metrics);

        Ok(StepResult {
            observation: self.observation(),
            reward,
            done: false,
            metrics: self.metrics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reward_uses_all_metrics() {
        assert_eq!(reward(Metrics::new(8, 2, 0.25, 12.5)), -6.75);
    }

    #[test]
    fn reward_preserves_small_differences_between_large_totals() {
        let limit = 1_u64 << 53;

        assert_eq!(reward(Metrics::new(limit + 1, limit, 0.0, 0.0)), 1.0);
    }
}
