use urbanflow::demand::Demand;
use urbanflow::observation::Observation;
use urbanflow::world::{Edge, EdgeId, EdgeKind, NodeId};

#[test]
fn observation_is_an_owned_agent_facing_snapshot() {
    let edge = Edge {
        id: EdgeId(0),
        from: NodeId(0),
        to: NodeId(1),
        kind: EdgeKind::Road,
    };
    let demand = Demand::new(NodeId(0), NodeId(3), 10);
    let observation = Observation {
        nodes: vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)],
        edges: vec![edge],
        demands: vec![demand],
        budget: 100.0,
        step_count: 2,
    };

    assert_eq!(observation.clone(), observation);
    assert_eq!(observation.nodes[3], NodeId(3));
    assert_eq!(observation.edges, vec![edge]);
    assert_eq!(observation.demands, vec![demand]);
    assert_eq!(observation.budget, 100.0);
    assert_eq!(observation.step_count, 2);
}
