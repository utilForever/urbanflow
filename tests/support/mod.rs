use urbanflow::rail::{RailRoute, RailVehicle};
use urbanflow::world::{EdgeKind, Network, NodeId};

// Insert each directed Rail edge in stop order so edge IDs follow route order.
pub fn vehicle(stops: &[usize], capacity: u32, travel: u64, dwell: u64) -> RailVehicle {
    let mut network = Network::new();
    let edges = stops
        .windows(2)
        .map(|pair| {
            network
                .add_edge(NodeId(pair[0]), NodeId(pair[1]), EdgeKind::Rail)
                .unwrap()
        })
        .collect();

    RailVehicle::new(
        RailRoute::new(&network, edges).unwrap(),
        capacity,
        travel,
        dwell,
    )
    .unwrap()
}
