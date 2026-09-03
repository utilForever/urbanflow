use urbanflow::demand::Demand;
use urbanflow::env::Env;
use urbanflow::world::{Network, Node, NodeId, World};

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
