#[path = "../examples/baseline/mod.rs"]
mod baseline;

use urbanflow::action::Action;
use urbanflow::world::{EdgeKind, NodeId};

#[test]
fn baseline_scenario_offers_one_unique_best_action() {
    let mut env = baseline::scenario();
    let actions = env.available_actions();
    let best_action = Action::AddEdge {
        from: NodeId(0),
        to: NodeId(1),
        kind: EdgeKind::Rail,
    };

    assert_eq!(actions.len(), 4);
    assert!(actions.contains(&best_action));

    for action in actions {
        env.reset();

        let result = env.step(action).unwrap();

        assert!(result.done);

        if action == best_action {
            assert_eq!(result.reward, 12.25);
        } else {
            assert!(result.reward < 12.25);
        }
    }
}

#[test]
fn random_policy_baseline_repeats_fixed_reward() {
    assert_eq!(
        (
            baseline::random_policy_reward(),
            baseline::random_policy_reward()
        ),
        (-530.0, -530.0)
    );
}

#[test]
fn tabular_q_learning_repeats_the_best_action() {
    let best_action = Action::AddEdge {
        from: NodeId(0),
        to: NodeId(1),
        kind: EdgeKind::Rail,
    };

    assert_eq!(
        (baseline::q_learning_action(), baseline::q_learning_action()),
        (best_action, best_action)
    );
}

#[test]
fn tabular_q_learning_outperforms_random_policy() {
    let mut env = baseline::scenario();
    let learned_action = baseline::q_learning_action();
    let learned_total_reward: f64 = (0..baseline::RANDOM_POLICY_EPISODES)
        .map(|_| {
            env.reset();
            env.step(learned_action).unwrap().reward
        })
        .sum();
    let episode_count = baseline::RANDOM_POLICY_EPISODES as f64;

    assert!(
        learned_total_reward / episode_count > baseline::random_policy_reward() / episode_count
    );
}
