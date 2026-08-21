use crate::action::Action;
use crate::demand::Demand;
use crate::metrics::Metrics;
use crate::observation::Observation;
use crate::simulation::tick;
use crate::step_result::StepResult;
use crate::world::{AddEdgeError, EdgeKind, NodeId, World, toy_city};

#[derive(Debug)]
pub struct Env {
    pub world: World,
    pub demands: Vec<Demand>,
    pub metrics: Metrics,
    pub budget: f64,
    pub step_count: usize,
    pub max_steps: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepError {
    InvalidEdge(AddEdgeError),
    UnknownNode(NodeId),
    InsufficientBudget,
    EpisodeComplete,
}

const MIN_CONSTRUCTION_COST: f64 = EdgeKind::Road.construction_cost();

fn reward(metrics: Metrics) -> f64 {
    let demand = i128::from(metrics.served_demand) - i128::from(metrics.unserved_demand);
    demand as f64 - metrics.congestion - metrics.cost
}

impl Env {
    fn is_done(&self) -> bool {
        self.step_count >= self.max_steps || self.budget < MIN_CONSTRUCTION_COST
    }

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

    pub fn step(&mut self, action: Action) -> Result<StepResult, StepError> {
        if self.is_done() {
            return Err(StepError::EpisodeComplete);
        }

        let Action::AddEdge { from, to, kind } = action;

        for node in [from, to] {
            if !self
                .world
                .nodes
                .iter()
                .any(|candidate| candidate.id == node)
            {
                return Err(StepError::UnknownNode(node));
            }
        }

        self.world
            .network
            .validate_add_edge(from, to, kind)
            .map_err(StepError::InvalidEdge)?;

        if self.budget < kind.construction_cost() {
            return Err(StepError::InsufficientBudget);
        }

        // Validate before mutating the environment so overflow remains atomic.
        self.demands
            .iter()
            .try_fold(0_u64, |total, demand| {
                total.checked_add(u64::from(demand.amount))
            })
            .expect("total demand exceeds metric capacity");
        let next_step_count = self
            .step_count
            .checked_add(1)
            .expect("step count exceeds environment capacity");

        self.world
            .network
            .add_edge(from, to, kind)
            .expect("edge was validated before mutation");
        self.budget -= kind.construction_cost();
        self.step_count = next_step_count;

        self.metrics = tick(&self.world.network, &self.demands);

        let reward = reward(self.metrics);

        Ok(StepResult {
            observation: self.observation(),
            reward,
            done: self.is_done(),
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
