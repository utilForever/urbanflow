#![allow(dead_code)]

use urbanflow::demand::Demand;
use urbanflow::env::Env;
use urbanflow::world::{Network, Node, NodeId, World};

pub const RANDOM_POLICY_EPISODES: usize = 100;
pub const Q_LEARNING_EPISODES: usize = 100;

const RANDOM_SEED: u64 = 0x5eed;
const LEARNING_RATE: f64 = 0.1;
const EPSILON_PERCENT: u64 = 20;

fn next_random(random: &mut u64) -> u64 {
    *random ^= *random << 13;
    *random ^= *random >> 7;
    *random ^= *random << 17;
    *random
}

fn greedy_action(action_values: &[f64]) -> usize {
    let mut best = 0;

    for index in 1..action_values.len() {
        if action_values[index] > action_values[best] {
            best = index;
        }
    }

    best
}

pub fn scenario() -> Env {
    Env::new(
        World {
            nodes: vec![Node { id: NodeId(0) }, Node { id: NodeId(1) }],
            network: Network::new(),
        },
        vec![Demand::new(NodeId(0), NodeId(1), 15)],
        2.0,
        1,
    )
    .expect("baseline scenario inputs are valid")
}

pub fn random_policy_reward() -> f64 {
    let mut env = scenario();
    let mut random = RANDOM_SEED;
    let mut total_reward = 0.0;

    for _ in 0..RANDOM_POLICY_EPISODES {
        env.reset();

        loop {
            let actions = env.available_actions();
            let action = actions[(next_random(&mut random) % actions.len() as u64) as usize];
            let result = env.step(action).expect("available action is valid");

            total_reward += result.reward;

            if result.done {
                break;
            }
        }
    }

    total_reward
}

pub fn q_learning_action() -> urbanflow::action::Action {
    let mut env = scenario();
    let actions = env.available_actions();
    let mut action_values = vec![0.0; actions.len()];
    let mut random = RANDOM_SEED;

    for _ in 0..Q_LEARNING_EPISODES {
        let observation = env.reset();
        assert_eq!(observation.step_count, 0, "baseline has one decision state");

        let action_index = if next_random(&mut random) % 100 < EPSILON_PERCENT {
            (next_random(&mut random) % actions.len() as u64) as usize
        } else {
            greedy_action(&action_values)
        };

        let result = env
            .step(actions[action_index])
            .expect("available action is valid");
        assert!(result.done, "baseline episodes have one step");

        action_values[action_index] +=
            LEARNING_RATE * (result.reward - action_values[action_index]);
    }

    actions[greedy_action(&action_values)]
}
