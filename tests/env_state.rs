use urbanflow::demand::Demand;
use urbanflow::env::Env;
use urbanflow::metrics::Metrics;
use urbanflow::world::{NodeId, ToyCity, World, toy_city};

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
