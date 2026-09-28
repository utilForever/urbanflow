use urbanflow::analysis::{
    AnalysisError, OperationalAnalysis, PassengerOutcomes, PassengerTimes, RouteTiming,
    StopActivity, VehicleOccupancy,
};
use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailRoute, RailTrace, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, NodeId};

fn completed_trace(demands: &[Demand], capacity: u32) -> RailTrace {
    let mut network = Network::new();
    let edge = network
        .add_edge(NodeId(8), NodeId(3), EdgeKind::Rail)
        .unwrap();
    let route = RailRoute::new(&network, vec![edge]).unwrap();

    let mut vehicle = RailVehicle::new(route, capacity, 1, 1).unwrap();
    vehicle
        .record_trace(
            &mut SimulationClock::default(),
            &mut RailPassengers::new(demands),
            2,
        )
        .unwrap()
}

#[test]
fn passenger_outcomes_use_final_counts_including_duplicate_and_ineligible_demands() {
    let mut trace = completed_trace(
        &[
            Demand::new(NodeId(8), NodeId(3), 4),
            Demand::new(NodeId(8), NodeId(3), 4),
            Demand::new(NodeId(3), NodeId(8), 3),
            Demand::new(NodeId(21), NodeId(3), 2),
            Demand::new(NodeId(8), NodeId(3), 0),
        ],
        6,
    );
    let before = trace.clone();
    let expected = PassengerOutcomes {
        requested: 13,
        arrived: 6,
        unserved: 7,
        served_share: Some(6.0 / 13.0),
    };

    assert_eq!(PassengerOutcomes::from_trace(&trace), Ok(expected));
    assert_eq!(trace, before);

    // Recording an already completed service produces just its final frame.
    trace.snapshots.drain(..2);
    assert_eq!(PassengerOutcomes::from_trace(&trace), Ok(expected));
}

#[test]
fn passenger_outcomes_require_a_nonempty_completed_trace_and_terminal_position() {
    for completed in [false, true] {
        assert_eq!(
            PassengerOutcomes::from_trace(&RailTrace {
                snapshots: vec![],
                completed,
            }),
            Err(AnalysisError::EmptyTrace)
        );
    }

    let mut trace = completed_trace(&[], 6);
    trace.completed = false;

    assert_eq!(
        PassengerOutcomes::from_trace(&trace),
        Err(AnalysisError::IncompleteTrace)
    );

    trace.completed = true;
    trace.snapshots.pop();

    assert_eq!(
        PassengerOutcomes::from_trace(&trace),
        Err(AnalysisError::IncompleteTrace)
    );
}

#[test]
fn passenger_outcomes_reject_remaining_waiting_or_onboard_passengers() {
    for (waiting, onboard) in [(1, 0), (0, 1)] {
        let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2); 2], 6);

        let final_frame = trace.snapshots.last_mut().unwrap();
        final_frame.passengers[1].arrived = 1;
        final_frame.passengers[1].waiting = waiting;
        final_frame.passengers[1].onboard = onboard;
        final_frame.occupancy = onboard;

        let before = trace.clone();

        assert_eq!(
            PassengerOutcomes::from_trace(&trace),
            Err(AnalysisError::UnfinishedPassengers { demand_index: 1 })
        );
        assert_eq!(trace, before);
    }
}

#[test]
fn passenger_outcomes_require_conservation_per_terminal_demand() {
    for (arrived, unserved) in [(1, 0), (3, 0), (u32::MAX, u32::MAX)] {
        let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2); 2], 6);

        let records = &mut trace.snapshots.last_mut().unwrap().passengers;
        records[0].arrived = arrived;
        records[0].unserved = unserved;
        // Aggregate conservation alone would accept the first two cases.
        records[1].arrived = 4u32.saturating_sub(arrived);

        let before = trace.clone();

        assert_eq!(
            PassengerOutcomes::from_trace(&trace),
            Err(AnalysisError::InvalidPassengerCounts { demand_index: 0 })
        );
        assert_eq!(trace, before);
    }
}

#[test]
fn operational_results_own_ordered_stop_visits_and_wide_totals() {
    let unserved = u64::from(u32::MAX) + 1;
    let analysis = {
        let stops = vec![
            StopActivity {
                stop_index: 0,
                node: NodeId(8),
                boarded: 2,
                alighted: 0,
                remaining_waiting: unserved,
            },
            StopActivity {
                stop_index: 1,
                node: NodeId(3),
                boarded: 0,
                alighted: 2,
                remaining_waiting: 0,
            },
            StopActivity {
                stop_index: 2,
                node: NodeId(8),
                boarded: 0,
                alighted: 0,
                remaining_waiting: unserved,
            },
        ];

        OperationalAnalysis {
            passenger_outcomes: PassengerOutcomes {
                requested: unserved + 2,
                arrived: 2,
                unserved,
                served_share: Some(2.0 / (unserved + 2) as f64),
            },
            passenger_times: PassengerTimes {
                waiting_passenger_ticks: unserved * 4 + 2,
                onboard_passenger_ticks: 2,
                arrived_waiting_passenger_ticks: 2,
                arrived_onboard_passenger_ticks: 2,
                mean_waiting_ticks: Some(1.0),
                mean_onboard_ticks: Some(1.0),
                mean_journey_ticks: Some(2.0),
            },
            vehicle_occupancy: VehicleOccupancy {
                capacity: 4,
                max_occupancy: 2,
                occupied_passenger_ticks: 2,
                mean_occupancy: Some(0.5),
                max_load_factor: Some(0.5),
                mean_load_factor: Some(0.125),
            },
            stops,
            route_timing: RouteTiming {
                elapsed_ticks: 4,
                traveling_ticks: 2,
                dwelling_ticks: 2,
                completion_tick: 4,
            },
        }
    };

    let mut changed = analysis.clone();
    changed.stops[0].boarded = 0;
    changed.stops.reverse();

    assert_ne!(changed, analysis);
    assert_eq!(analysis.stops[0].boarded, 2);
    assert_eq!(analysis.stops[0].remaining_waiting, 4_294_967_296);
    assert_eq!(analysis.passenger_outcomes.requested, 4_294_967_298);
    assert_eq!(
        analysis.passenger_times.waiting_passenger_ticks,
        17_179_869_186
    );
    assert_eq!(
        analysis
            .stops
            .iter()
            .map(|stop| (stop.stop_index, stop.node))
            .collect::<Vec<_>>(),
        vec![(0, NodeId(8)), (1, NodeId(3)), (2, NodeId(8))]
    );
}

#[test]
fn empty_denominators_are_representable_without_nan_or_infinity() {
    let outcomes = PassengerOutcomes {
        requested: 0,
        arrived: 0,
        unserved: 0,
        served_share: None,
    };
    let times = PassengerTimes {
        waiting_passenger_ticks: 0,
        onboard_passenger_ticks: 0,
        arrived_waiting_passenger_ticks: 0,
        arrived_onboard_passenger_ticks: 0,
        mean_waiting_ticks: None,
        mean_onboard_ticks: None,
        mean_journey_ticks: None,
    };
    let occupancy = VehicleOccupancy {
        capacity: 4,
        max_occupancy: 0,
        occupied_passenger_ticks: 0,
        mean_occupancy: None,
        max_load_factor: None,
        mean_load_factor: None,
    };

    assert_eq!(outcomes.served_share, None);
    assert_eq!(times.mean_waiting_ticks, None);
    assert_eq!(times.mean_onboard_ticks, None);
    assert_eq!(times.mean_journey_ticks, None);
    assert_eq!(occupancy.mean_occupancy, None);
    assert_eq!(occupancy.max_load_factor, None);
    assert_eq!(occupancy.mean_load_factor, None);
}
