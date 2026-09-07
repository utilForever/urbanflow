use urbanflow::rail::{RailInitError, RailRoute, RailVehicle, RailVehicleState};
use urbanflow::world::{EdgeId, EdgeKind, Network, NodeId};

#[test]
fn rail_route_preserves_edge_order() {
    let mut network = Network::new();
    let second = network
        .add_edge(NodeId(0), NodeId(1), EdgeKind::Rail)
        .unwrap();
    let first = network
        .add_edge(NodeId(2), NodeId(0), EdgeKind::Rail)
        .unwrap();
    let route = RailRoute::new(&network, vec![first, second]).unwrap();

    assert_eq!(route.edges(), &[first, second]);
}

#[test]
fn rail_route_rejects_an_empty_edge_sequence() {
    assert_eq!(
        RailRoute::new(&Network::new(), Vec::new()),
        Err(RailInitError::EmptyRoute)
    );
}

#[test]
fn rail_route_rejects_the_first_unknown_edge() {
    assert_eq!(
        RailRoute::new(&Network::new(), vec![EdgeId(7), EdgeId(8)]),
        Err(RailInitError::UnknownEdge(EdgeId(7)))
    );
}

#[test]
fn rail_route_rejects_the_first_non_rail_edge() {
    let mut network = Network::new();
    let road = network
        .add_edge(NodeId(0), NodeId(1), EdgeKind::Road)
        .unwrap();

    assert_eq!(
        RailRoute::new(&network, vec![road]),
        Err(RailInitError::NonRailEdge(road))
    );
}

#[test]
fn rail_route_rejects_the_first_disconnected_edge_pair() {
    let mut network = Network::new();
    let previous = network
        .add_edge(NodeId(0), NodeId(1), EdgeKind::Rail)
        .unwrap();
    let next = network
        .add_edge(NodeId(2), NodeId(3), EdgeKind::Rail)
        .unwrap();

    assert_eq!(
        RailRoute::new(&network, vec![previous, next]),
        Err(RailInitError::DisconnectedEdges { previous, next })
    );
}

#[test]
fn rail_vehicle_starts_at_the_first_stop() {
    let mut network = Network::new();
    let last = network
        .add_edge(NodeId(1), NodeId(2), EdgeKind::Rail)
        .unwrap();
    let first = network
        .add_edge(NodeId(0), NodeId(1), EdgeKind::Rail)
        .unwrap();
    let route = RailRoute::new(&network, vec![first, last]).unwrap();
    let vehicle = RailVehicle::new(route, 30, 4, 2).unwrap();

    assert_eq!(vehicle.route().edges(), &[first, last]);
    assert_eq!(vehicle.capacity(), 30);
    assert_eq!(vehicle.travel_ticks_per_edge(), 4);
    assert_eq!(vehicle.dwell_ticks_per_stop(), 2);
    assert_eq!(
        vehicle.state(),
        RailVehicleState::AtStop {
            stop_index: 0,
            dwell_ticks_remaining: 2,
        }
    );

    let RailVehicleState::AtStop { stop_index, .. } = vehicle.state() else {
        panic!("vehicle must start at a stop");
    };
    let edge = network.edges()[vehicle.route().edges()[stop_index].0];
    assert_eq!(edge.from, NodeId(0));
}

#[test]
fn rail_vehicle_rejects_zero_configuration_values() {
    let mut network = Network::new();
    let edge = network
        .add_edge(NodeId(0), NodeId(1), EdgeKind::Rail)
        .unwrap();
    let route = RailRoute::new(&network, vec![edge]).unwrap();

    for (capacity, travel, dwell, error) in [
        (0, 1, 1, RailInitError::InvalidCapacity),
        (1, 0, 1, RailInitError::InvalidTravelTicks),
        (1, 1, 0, RailInitError::InvalidDwellTicks),
    ] {
        assert_eq!(
            RailVehicle::new(route.clone(), capacity, travel, dwell),
            Err(error)
        );
    }
}
