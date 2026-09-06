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
