use crate::metrics::Metrics;
use crate::observation::Observation;

#[derive(Clone, Debug, PartialEq)]
pub struct StepResult {
    pub observation: Observation,
    pub reward: f64,
    pub done: bool,
    pub metrics: Metrics,
}
