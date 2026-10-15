use super::support::{assert_trace_error, completed_trace};
use urbanflow::analysis::{AnalysisError, VehicleOccupancy};
use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailRoute, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, NodeId};

#[test]
fn vehicle_occupancy_weights_travel_and_dwell_without_mutating_the_trace() {
    let mut network = Network::new();
    let outbound = network
        .add_edge(NodeId(8), NodeId(3), EdgeKind::Rail)
        .unwrap();
    let inbound = network
        .add_edge(NodeId(3), NodeId(8), EdgeKind::Rail)
        .unwrap();
    let route = RailRoute::new(&network, vec![outbound, inbound]).unwrap();

    let mut vehicle = RailVehicle::new(route, 6, 2, 2).unwrap();
    let mut trace = vehicle
        .record_trace(
            &mut SimulationClock::default(),
            &mut RailPassengers::new(&[
                Demand::new(NodeId(8), NodeId(3), 4),
                Demand::new(NodeId(8), NodeId(3), 4),
                Demand::new(NodeId(3), NodeId(8), 2),
            ]),
            8,
        )
        .unwrap();

    let expected = VehicleOccupancy {
        capacity: 6,
        max_occupancy: 6,
        // Six ride [1, 4), then two ride [4, 8), including dwell.
        occupied_passenger_ticks: 26,
        mean_occupancy: Some(3.25),
        max_load_factor: Some(1.0),
        mean_load_factor: Some(3.25 / 6.0),
    };
    let before = trace.clone();

    assert_eq!(VehicleOccupancy::from_trace(&trace), Ok(expected));
    assert_eq!(trace, before);

    // Moving the initial tick preserves the same occupancy intervals.
    for frame in &mut trace.snapshots {
        frame.tick += 100;
    }
    assert_eq!(VehicleOccupancy::from_trace(&trace), Ok(expected));

    trace
        .snapshots
        .retain(|frame| [100, 101, 104, 108].contains(&frame.tick));

    assert_trace_error(
        &trace,
        AnalysisError::NonContiguousTicks { snapshot_index: 2 },
    );
}

#[test]
fn vehicle_occupancy_distinguishes_empty_service_from_no_active_intervals() {
    for demands in [vec![], vec![Demand::new(NodeId(3), NodeId(8), 2)]] {
        let trace = completed_trace(&demands, 6);

        assert_eq!(
            VehicleOccupancy::from_trace(&trace),
            Ok(VehicleOccupancy {
                capacity: 6,
                max_occupancy: 0,
                occupied_passenger_ticks: 0,
                mean_occupancy: Some(0.0),
                max_load_factor: Some(0.0),
                mean_load_factor: Some(0.0),
            })
        );
    }

    let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    trace.snapshots.remove(0);

    assert_eq!(
        VehicleOccupancy::from_trace(&trace),
        Ok(VehicleOccupancy {
            capacity: 6,
            max_occupancy: 2,
            occupied_passenger_ticks: 2,
            mean_occupancy: Some(2.0),
            max_load_factor: Some(2.0 / 6.0),
            mean_load_factor: Some(2.0 / 6.0),
        })
    );

    trace.snapshots.remove(0);

    let expected = VehicleOccupancy {
        capacity: 6,
        max_occupancy: 0,
        occupied_passenger_ticks: 0,
        mean_occupancy: None,
        max_load_factor: None,
        mean_load_factor: None,
    };

    assert_eq!(VehicleOccupancy::from_trace(&trace), Ok(expected));

    let mut later = trace.snapshots[0].clone();
    later.tick += 1;

    // A caller-supplied load after completion must match onboard passengers.
    trace.snapshots[0].occupancy = 6;
    trace.snapshots.push(later);

    assert_trace_error(
        &trace,
        AnalysisError::InvalidOccupancy { snapshot_index: 0 },
    );
}

#[test]
fn vehicle_occupancy_rejects_tick_gaps_before_accumulating_time() {
    let mut product = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    product.snapshots[2].tick = u64::MAX;

    let mut sum = product.clone();
    sum.snapshots[1].tick = 1 << 62;

    let mut initial = sum.snapshots[1].clone();
    initial.tick = 0;

    sum.snapshots[0] = initial;
    sum.snapshots[2].tick = 1 << 63;

    for (trace, snapshot_index) in [(product, 2), (sum, 1)] {
        assert_trace_error(&trace, AnalysisError::NonContiguousTicks { snapshot_index });
    }

    let mut boundary = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 1)], 1);
    boundary.snapshots.remove(0);
    boundary.snapshots[0].tick = u64::MAX - 1;
    boundary.snapshots[1].tick = u64::MAX;

    assert_eq!(
        VehicleOccupancy::from_trace(&boundary),
        Ok(VehicleOccupancy {
            capacity: 1,
            max_occupancy: 1,
            occupied_passenger_ticks: 1,
            mean_occupancy: Some(1.0),
            max_load_factor: Some(1.0),
            mean_load_factor: Some(1.0),
        })
    );
}

#[test]
fn vehicle_occupancy_uses_exact_tick_differences_above_float_precision() {
    let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 3)], 3);
    trace.snapshots.remove(0);
    trace.snapshots[0].tick = 9_007_199_254_740_992;
    trace.snapshots[1].tick = 9_007_199_254_740_993;

    assert_eq!(
        VehicleOccupancy::from_trace(&trace),
        Ok(VehicleOccupancy {
            capacity: 3,
            max_occupancy: 3,
            occupied_passenger_ticks: 3,
            mean_occupancy: Some(3.0),
            max_load_factor: Some(1.0),
            mean_load_factor: Some(1.0),
        })
    );
}
