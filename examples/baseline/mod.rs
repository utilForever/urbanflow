use urbanflow::demand::Demand;
use urbanflow::env::Env;
use urbanflow::world::{Network, Node, NodeId, World};

pub const RANDOM_POLICY_EPISODES: usize = 100;

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
    let mut random = 0x5eed_u64;
    let mut total_reward = 0.0;

    for _ in 0..RANDOM_POLICY_EPISODES {
        env.reset();

        loop {
            let actions = env.available_actions();
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;

            let action = actions[(random % actions.len() as u64) as usize];
            let result = env.step(action).expect("available action is valid");

            total_reward += result.reward;

            if result.done {
                break;
            }
        }
    }

    total_reward
}
