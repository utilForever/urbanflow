use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailRoute, RailTrace, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, Node, NodeId, World};

/// A small recorded service; all simulation decisions stay in the core API.
pub fn scenario() -> (World, RailTrace) {
    let mut network = Network::new();

    for (from, to) in [(8, 3), (3, 8), (5, 8), (8, 21)] {
        network
            .add_edge(NodeId(from), NodeId(to), EdgeKind::Road)
            .expect("demo edges are valid");
    }

    let edges = [(8, 3), (3, 21), (21, 5)]
        .map(|(from, to)| {
            network
                .add_edge(NodeId(from), NodeId(to), EdgeKind::Rail)
                .expect("demo edges are valid")
        })
        .to_vec();
    let route = RailRoute::new(&network, edges).expect("demo route is valid");

    let mut vehicle = RailVehicle::new(route, 6, 4, 2).expect("demo settings are positive");
    let mut passengers = RailPassengers::new(&[
        Demand::new(NodeId(8), NodeId(21), 8),
        Demand::new(NodeId(3), NodeId(5), 3),
    ]);

    let trace = vehicle
        .record_trace(&mut SimulationClock::default(), &mut passengers, 32)
        .expect("bounded demo service succeeds");
    let world = World {
        nodes: [8, 3, 21, 5].map(|id| Node { id: NodeId(id) }).to_vec(),
        network,
    };

    (world, trace)
}
