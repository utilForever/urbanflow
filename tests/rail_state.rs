use urbanflow::rail::{RailRoute, RailVehicle, RailVehicleState};
use urbanflow::world::EdgeId;

#[test]
fn rail_route_preserves_edge_order() {
    let route = RailRoute {
        edges: vec![EdgeId(2), EdgeId(0), EdgeId(1)],
    };

    assert_eq!(route.edges, vec![EdgeId(2), EdgeId(0), EdgeId(1)]);
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
