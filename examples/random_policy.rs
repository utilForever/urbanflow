mod baseline;

fn main() {
    println!(
        "Random policy total reward over {} episodes: {:.2}",
        baseline::RANDOM_POLICY_EPISODES,
        baseline::random_policy_reward()
    );
}
