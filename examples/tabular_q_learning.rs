mod baseline;

fn main() {
    println!(
        "Tabular Q-learning action after {} episodes: {:?}",
        baseline::Q_LEARNING_EPISODES,
        baseline::q_learning_action()
    );
}
