use urbanflow::demand::Demand;
use urbanflow::env::Env;
use urbanflow::metrics::Metrics;
use urbanflow::observation::Observation;
use urbanflow::world::{EdgeKind, NodeId, ToyCity, World, toy_city};

#[test]
fn env_holds_current_state() {
    let ToyCity { nodes, network } = toy_city();
    let world: World = ToyCity { nodes, network };
    let demand = Demand::new(NodeId(0), NodeId(3), 10);
    let metrics = Metrics::new(8, 2, 0.25, 12.5);
    let env = Env {
        world,
        demands: vec![demand],
        metrics,
        budget: 100.0,
        step_count: 3,
    };

    assert_eq!(env.world.nodes.len(), 4);
    assert_eq!(env.world.network.edges().len(), 2);
    assert_eq!(env.demands, vec![demand]);
    assert_eq!(env.metrics, metrics);
    assert_eq!(env.budget, 100.0);
    assert_eq!(env.step_count, 3);
}

#[test]
fn reset_restores_the_fixed_initial_state() {
    let mut world = toy_city();
    world.nodes[0].id = NodeId(99);
    world
        .network
        .add_edge(NodeId(1), NodeId(2), EdgeKind::Road)
        .unwrap();
    let mut env = Env {
        world,
        demands: vec![Demand::new(NodeId(3), NodeId(0), 99)],
        metrics: Metrics::new(1, 2, 3.0, 4.0),
        budget: 1.0,
        step_count: 7,
    };

    let observation = env.reset();
    let expected_world = toy_city();
    let nodes = expected_world.nodes.map(|node| node.id);
    let edges = expected_world.network.edges().to_vec();
    let demands = vec![Demand::new(NodeId(0), NodeId(3), 10)];

    assert_eq!(
        observation,
        Observation {
            nodes,
            edges: edges.clone(),
            demands: demands.clone(),
            budget: 100.0,
            step_count: 0,
        }
    );
    assert_eq!(env.world.nodes.map(|node| node.id), nodes);
    assert_eq!(env.world.network.edges(), edges);
    assert_eq!(env.demands, demands);
    assert_eq!(env.metrics, Metrics::default());
    assert_eq!(env.budget, 100.0);
    assert_eq!(env.step_count, 0);
}
