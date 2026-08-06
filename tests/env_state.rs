use std::panic::{AssertUnwindSafe, catch_unwind};

use urbanflow::action::Action;
use urbanflow::demand::Demand;
use urbanflow::env::Env;
use urbanflow::metrics::Metrics;
use urbanflow::observation::Observation;
use urbanflow::world::{AddEdgeError, Edge, EdgeId, EdgeKind, NodeId, ToyCity, World, toy_city};

fn reset_env() -> Env {
    let mut env = Env {
        world: toy_city(),
        demands: Vec::new(),
        metrics: Metrics::default(),
        budget: 0.0,
        step_count: 0,
    };
    env.reset();
    env
}

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

#[test]
fn step_simulates_reachable_demand() {
    let mut env = reset_env();

    let result = env
        .step(Action::AddEdge {
            from: NodeId(1),
            to: NodeId(2),
            kind: EdgeKind::Road,
        })
        .unwrap();

    assert_eq!(
        result.observation.edges.last(),
        Some(&Edge {
            id: EdgeId(2),
            from: NodeId(1),
            to: NodeId(2),
            kind: EdgeKind::Road,
        })
    );
    assert_eq!(env.step_count, 1);
    assert_eq!(result.observation.step_count, 1);
    assert_eq!(result.observation.budget, 100.0);
    assert_eq!(result.metrics, Metrics::new(10, 0, 0.0, 3.0));
    assert_eq!(env.metrics, result.metrics);
    assert_eq!(result.reward, 7.0);
    assert!(!result.done);
}

#[test]
fn step_penalizes_unserved_demand() {
    let mut env = reset_env();

    let result = env
        .step(Action::AddEdge {
            from: NodeId(3),
            to: NodeId(0),
            kind: EdgeKind::Rail,
        })
        .unwrap();

    assert_eq!(result.metrics, Metrics::new(0, 10, 0.0, 3.0));
    assert_eq!(env.metrics, result.metrics);
    assert_eq!(result.reward, -13.0);
}

#[test]
fn step_preserves_large_demand_totals() {
    let mut env = reset_env();
    env.demands = vec![Demand::new(NodeId(0), NodeId(3), u32::MAX); 2];

    let result = env
        .step(Action::AddEdge {
            from: NodeId(1),
            to: NodeId(2),
            kind: EdgeKind::Road,
        })
        .unwrap();
    let served = u64::from(u32::MAX) * 2;

    assert_eq!(result.metrics.served_demand, served);
    assert_eq!(result.reward, served as f64 - 3.0);
}

#[test]
fn step_count_overflow_does_not_mutate_the_network() {
    let mut env = reset_env();
    env.step_count = usize::MAX;

    let edges = env.world.network.edges().to_vec();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = env.step(Action::AddEdge {
            from: NodeId(1),
            to: NodeId(2),
            kind: EdgeKind::Road,
        });
    }));

    assert!(result.is_err());
    assert_eq!(env.world.network.edges(), edges);
    assert_eq!(env.step_count, usize::MAX);
}

#[test]
fn invalid_action_does_not_advance_the_environment() {
    let mut env = reset_env();
    let metrics = Metrics::new(4, 6, 0.5, 2.0);
    env.metrics = metrics;

    let edges = env.world.network.edges().to_vec();

    assert_eq!(
        env.step(Action::AddEdge {
            from: NodeId(0),
            to: NodeId(1),
            kind: EdgeKind::Road,
        })
        .unwrap_err(),
        AddEdgeError::DuplicateEdge
    );
    assert_eq!(env.world.network.edges(), edges);
    assert_eq!(env.metrics, metrics);
    assert_eq!(env.step_count, 0);
}
