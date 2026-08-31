use urbanflow::demand::Demand;
use urbanflow::metrics::Metrics;
use urbanflow::observation::Observation;
use urbanflow::step_result::StepResult;
use urbanflow::world::{Edge, EdgeId, EdgeKind, NodeId};

#[test]
fn step_result_owns_step_outputs() {
    let observation = Observation {
        nodes: vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)],
        edges: vec![Edge {
            id: EdgeId(0),
            from: NodeId(0),
            to: NodeId(1),
            kind: EdgeKind::Road,
        }],
        demands: vec![Demand::new(NodeId(0), NodeId(3), 10)],
        budget: 100.0,
        step_count: 2,
    };
    let metrics = Metrics::new(8, 2, 0.25, 12.5);
    let result = StepResult {
        observation: observation.clone(),
        reward: 1.25,
        done: true,
        metrics,
    };

    assert_eq!(result.clone(), result);
    assert_eq!(result.observation, observation);
    assert_eq!(result.reward, 1.25);
    assert!(result.done);
    assert_eq!(result.metrics, metrics);
}
