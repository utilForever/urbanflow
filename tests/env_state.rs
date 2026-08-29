use urbanflow::action::Action;
use urbanflow::demand::Demand;
use urbanflow::env::{Env, InitError, StepError};
use urbanflow::metrics::Metrics;
use urbanflow::observation::Observation;
use urbanflow::world::{
    AddEdgeError, Edge, EdgeId, EdgeKind, Network, Node, NodeId, ToyCity, World, toy_city,
};

fn reset_env() -> Env {
    let mut env = Env {
        world: toy_city(),
        demands: Vec::new(),
        metrics: Metrics::default(),
        budget: 0.0,
        step_count: 0,
        max_steps: 100,
    };
    env.reset();
    env
}

fn assert_rejected_step_does_not_mutate(mut env: Env, action: Action, expected: StepError) {
    let edges = env.world.network.edges().to_vec();
    let metrics = env.metrics;
    let budget = env.budget;
    let step_count = env.step_count;

    assert_eq!(env.step(action).unwrap_err(), expected);
    assert_eq!(env.world.network.edges(), edges);
    assert_eq!(env.metrics, metrics);
    assert_eq!(env.budget, budget);
    assert_eq!(env.step_count, step_count);
}

fn assert_episode_complete_step_does_not_mutate(env: Env) {
    assert_rejected_step_does_not_mutate(
        env,
        Action::AddEdge {
            from: NodeId(1),
            to: NodeId(2),
            kind: EdgeKind::Road,
        },
        StepError::EpisodeComplete,
    );
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
        max_steps: 100,
    };

    assert_eq!(env.world.nodes.len(), 4);
    assert_eq!(env.world.network.edges().len(), 2);
    assert_eq!(env.demands, vec![demand]);
    assert_eq!(env.metrics, metrics);
    assert_eq!(env.budget, 100.0);
    assert_eq!(env.step_count, 3);
    assert_eq!(env.max_steps, 100);
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
        max_steps: 7,
    };

    let observation = env.reset();
    let expected_world = toy_city();
    let nodes = vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)];
    let edges = expected_world.network.edges().to_vec();
    let demands = vec![Demand::new(NodeId(0), NodeId(3), 10)];

    assert_eq!(
        observation,
        Observation {
            nodes: nodes.clone(),
            edges: edges.clone(),
            demands: demands.clone(),
            budget: 100.0,
            step_count: 0,
        }
    );
    assert_eq!(
        env.world
            .nodes
            .iter()
            .map(|node| node.id)
            .collect::<Vec<_>>(),
        nodes
    );
    assert_eq!(env.world.network.edges(), edges);
    assert_eq!(env.demands, demands);
    assert_eq!(env.metrics, Metrics::default());
    assert_eq!(env.budget, 100.0);
    assert_eq!(env.step_count, 0);
    assert_eq!(env.max_steps, 7);
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
    assert_eq!(result.observation.budget, 99.0);
    assert_eq!(result.metrics, Metrics::new(10, 0, 1.0, 4.0));
    assert_eq!(env.metrics, result.metrics);
    assert_eq!(result.reward, 5.0);
    assert!(!result.done);
}

#[test]
fn road_and_rail_produce_distinct_multimodal_trade_offs() {
    let run = |kind| {
        let mut env = reset_env();
        env.demands = vec![Demand::new(NodeId(1), NodeId(2), 15)];

        env.step(Action::AddEdge {
            from: NodeId(1),
            to: NodeId(2),
            kind,
        })
        .unwrap()
    };

    let road = run(EdgeKind::Road);
    let rail = run(EdgeKind::Rail);

    assert_eq!(road.metrics, Metrics::new(10, 5, 1.0, 4.0));
    assert_eq!(road.observation.budget, 99.0);
    assert_eq!(road.reward, 0.0);
    assert_eq!(rail.metrics, Metrics::new(15, 0, 0.75, 5.0));
    assert_eq!(rail.observation.budget, 98.0);
    assert_eq!(rail.reward, 9.25);
}

#[test]
fn useful_add_edge_improves_reward_over_disconnected_baseline() {
    let mut baseline_env = reset_env();
    let baseline = baseline_env
        .step(Action::AddEdge {
            from: NodeId(1),
            to: NodeId(0),
            kind: EdgeKind::Road,
        })
        .unwrap();
    let mut improved_env = reset_env();
    let improved = improved_env
        .step(Action::AddEdge {
            from: NodeId(1),
            to: NodeId(2),
            kind: EdgeKind::Road,
        })
        .unwrap();

    assert!(improved.reward > baseline.reward);
}

#[test]
fn step_finishes_at_max_steps() {
    let mut env = reset_env();
    env.max_steps = 1;

    let result = env
        .step(Action::AddEdge {
            from: NodeId(1),
            to: NodeId(2),
            kind: EdgeKind::Road,
        })
        .unwrap();

    assert!(result.done);
}

#[test]
fn step_finishes_when_budget_cannot_cover_another_edge() {
    let mut env = reset_env();
    env.budget = 1.0;

    let result = env
        .step(Action::AddEdge {
            from: NodeId(1),
            to: NodeId(2),
            kind: EdgeKind::Road,
        })
        .unwrap();

    assert_eq!(result.observation.budget, 0.0);
    assert!(result.done);
}

#[test]
fn step_finishes_when_only_unaffordable_valid_edges_remain() {
    let mut env = reset_env();
    let nodes = toy_city().nodes;
    let mut network = Network::new();

    for from in nodes.iter() {
        for to in nodes.iter() {
            if from != to && (from.id, to.id) != (NodeId(1), NodeId(2)) {
                network.add_edge(from.id, to.id, EdgeKind::Road).unwrap();
            }
        }
    }

    env.world = World { nodes, network };
    env.budget = 2.0;

    let result = env
        .step(Action::AddEdge {
            from: NodeId(1),
            to: NodeId(2),
            kind: EdgeKind::Road,
        })
        .unwrap();

    assert_eq!(result.observation.budget, 1.0);
    assert!(result.done);
}

#[test]
fn episode_complete_when_no_edge_is_affordable() {
    let mut env = reset_env();
    env.budget = 0.5;

    assert_episode_complete_step_does_not_mutate(env);
}

#[test]
fn insufficient_budget_for_selected_mode_does_not_advance_the_environment() {
    let mut env = reset_env();
    env.budget = 1.0;

    assert_rejected_step_does_not_mutate(
        env,
        Action::AddEdge {
            from: NodeId(1),
            to: NodeId(2),
            kind: EdgeKind::Rail,
        },
        StepError::InsufficientBudget,
    );
}

#[test]
fn episode_complete_when_max_steps_is_zero() {
    let mut env = reset_env();
    env.max_steps = 0;

    assert_episode_complete_step_does_not_mutate(env);
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

    assert_eq!(result.observation.budget, 98.0);
    assert_eq!(result.metrics, Metrics::new(0, 10, 0.0, 5.0));
    assert_eq!(env.metrics, result.metrics);
    assert_eq!(result.reward, -15.0);
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
    let total = u64::from(u32::MAX) * 2;

    assert_eq!(result.metrics.served_demand, 10);
    assert_eq!(result.metrics.unserved_demand, total - 10);
    assert_eq!(result.reward, 20.0 - total as f64 - 1.0 - 4.0);
}

#[test]
fn invalid_action_does_not_advance_the_environment() {
    let mut env = reset_env();
    env.metrics = Metrics::new(4, 6, 0.5, 2.0);

    assert_rejected_step_does_not_mutate(
        env,
        Action::AddEdge {
            from: NodeId(0),
            to: NodeId(1),
            kind: EdgeKind::Road,
        },
        StepError::InvalidEdge(AddEdgeError::DuplicateEdge),
    );
}

#[test]
fn step_returns_an_owned_variable_size_node_snapshot() {
    let mut env = Env {
        world: World {
            nodes: vec![
                Node { id: NodeId(7) },
                Node { id: NodeId(3) },
                Node { id: NodeId(5) },
            ],
            network: Network::new(),
        },
        demands: Vec::new(),
        metrics: Metrics::default(),
        budget: 100.0,
        step_count: 0,
        max_steps: 100,
    };

    let result = env
        .step(Action::AddEdge {
            from: NodeId(7),
            to: NodeId(3),
            kind: EdgeKind::Road,
        })
        .unwrap();
    env.world.nodes[0].id = NodeId(99);

    assert_eq!(
        result.observation.nodes.as_slice(),
        &[NodeId(7), NodeId(3), NodeId(5)]
    );
}

#[test]
fn initialization_errors_distinguish_invalid_input_sources() {
    let node = NodeId(7);

    assert_eq!(
        InitError::DuplicateNode(node),
        InitError::DuplicateNode(node)
    );
    assert_ne!(
        InitError::UnknownEdgeEndpoint(node),
        InitError::UnknownDemandEndpoint(node)
    );
    assert_eq!(InitError::InvalidBudget, InitError::InvalidBudget);
}

#[test]
fn initial_inputs_reject_the_first_unknown_demand_endpoint() {
    let world = toy_city();

    for (demands, unknown) in [
        (
            vec![
                Demand::new(NodeId(99), NodeId(98), 10),
                Demand::new(NodeId(97), NodeId(0), 20),
            ],
            NodeId(99),
        ),
        (
            vec![
                Demand::new(NodeId(0), NodeId(99), 10),
                Demand::new(NodeId(98), NodeId(1), 20),
            ],
            NodeId(99),
        ),
    ] {
        assert_eq!(
            Env::validate_inputs(&world, &demands, 100.0),
            Err(InitError::UnknownDemandEndpoint(unknown))
        );
    }
}

#[test]
fn unknown_node_from_does_not_advance_the_environment() {
    let env = reset_env();

    assert_rejected_step_does_not_mutate(
        env,
        Action::AddEdge {
            from: NodeId(99),
            to: NodeId(2),
            kind: EdgeKind::Road,
        },
        StepError::UnknownNode(NodeId(99)),
    );
}

#[test]
fn unknown_node_to_does_not_advance_the_environment() {
    let env = reset_env();

    assert_rejected_step_does_not_mutate(
        env,
        Action::AddEdge {
            from: NodeId(1),
            to: NodeId(99),
            kind: EdgeKind::Road,
        },
        StepError::UnknownNode(NodeId(99)),
    );
}
