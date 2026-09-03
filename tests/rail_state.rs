use urbanflow::rail::{RailInitError, RailRoute, RailVehicle, RailVehicleState};
use urbanflow::world::{EdgeId, EdgeKind, Network, NodeId};

#[test]
fn rail_route_preserves_edge_order() {
    let mut network = Network::new();
    let first = network
        .add_edge(NodeId(2), NodeId(0), EdgeKind::Rail)
        .unwrap();
    let second = network
        .add_edge(NodeId(0), NodeId(1), EdgeKind::Rail)
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
fn rail_vehicle_represents_stop_travel_and_completion_state() {
    let mut vehicle = RailVehicle {
        capacity: 30,
        travel_ticks_per_edge: 4,
        dwell_ticks_per_stop: 2,
        state: RailVehicleState::AtStop {
            stop_index: 0,
            dwell_ticks_remaining: 2,
        },
    };

    assert_eq!(
        vehicle.state,
        RailVehicleState::AtStop {
            stop_index: 0,
            dwell_ticks_remaining: 2,
        }
    );

    vehicle.state = RailVehicleState::Traveling {
        edge_index: 0,
        travel_ticks_elapsed: 1,
    };

    assert_eq!(
        vehicle.state,
        RailVehicleState::Traveling {
            edge_index: 0,
            travel_ticks_elapsed: 1,
        }
    );

    vehicle.state = RailVehicleState::Complete;

    assert_eq!(vehicle.state, RailVehicleState::Complete);
}
