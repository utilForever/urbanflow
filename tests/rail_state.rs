use urbanflow::rail::{RailRoute, RailVehicle, RailVehicleState};
use urbanflow::world::{EdgeId, EdgeKind, Network, NodeId};

#[test]
fn rail_route_preserves_edge_order() {
    let route = RailRoute {
        edges: vec![EdgeId(2), EdgeId(0), EdgeId(1)],
    };

    assert_eq!(route.edges, vec![EdgeId(2), EdgeId(0), EdgeId(1)]);
}

#[test]
fn rail_vehicle_represents_stop_travel_and_completion_state() {
    let mut network = Network::new();
    let last = network
        .add_edge(NodeId(1), NodeId(2), EdgeKind::Rail)
        .unwrap();
    let first = network
        .add_edge(NodeId(0), NodeId(1), EdgeKind::Rail)
        .unwrap();
    let mut vehicle = RailVehicle {
        route: RailRoute {
            edges: vec![first, last],
        },
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

    let RailVehicleState::AtStop { stop_index, .. } = vehicle.state else {
        panic!("vehicle must start at a stop");
    };
    let edge = network.edges()[vehicle.route.edges[stop_index].0];
    assert_eq!(edge.from, NodeId(0));

    vehicle.state = RailVehicleState::Traveling {
        edge_index: 1,
        travel_ticks_elapsed: 1,
    };

    assert_eq!(
        vehicle.state,
        RailVehicleState::Traveling {
            edge_index: 1,
            travel_ticks_elapsed: 1,
        }
    );

    let RailVehicleState::Traveling { edge_index, .. } = vehicle.state else {
        panic!("vehicle must be traveling");
    };
    let edge = network.edges()[vehicle.route.edges[edge_index].0];
    assert_eq!((edge.from, edge.to), (NodeId(1), NodeId(2)));

    vehicle.state = RailVehicleState::AtStop {
        stop_index: 2,
        dwell_ticks_remaining: 2,
    };
    let RailVehicleState::AtStop { stop_index, .. } = vehicle.state else {
        panic!("vehicle must be at the final stop");
    };
    let edge = network.edges()[vehicle.route.edges[stop_index - 1].0];
    assert_eq!(edge.to, NodeId(2));

    vehicle.state = RailVehicleState::Complete;

    assert_eq!(vehicle.state, RailVehicleState::Complete);
}
