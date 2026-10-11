#[path = "operational_analysis/passenger_outcomes.rs"]
mod passenger_outcomes;
#[path = "operational_analysis/passenger_times.rs"]
mod passenger_times;
#[path = "operational_analysis/stop_activity.rs"]
mod stop_activity;
#[path = "operational_analysis/support.rs"]
mod support;
#[path = "operational_analysis/vehicle_occupancy.rs"]
mod vehicle_occupancy;

use rstest::rstest;
use support::{assert_trace_error, completed_trace};
use urbanflow::analysis::{
    AnalysisError, OperationalAnalysis, PassengerOutcomes, PassengerTimes, RouteTiming,
    StopActivity, VehicleOccupancy, analyze,
};
use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailRoute, RailTrace, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, NodeId};

#[test]
fn analysis_errors_support_standard_error_propagation_and_diagnostics() {
    fn summarize(trace: &RailTrace) -> Result<OperationalAnalysis, Box<dyn std::error::Error>> {
        Ok(analyze(trace)?)
    }

    let error = summarize(&RailTrace {
        snapshots: vec![],
        completed: false,
    })
    .unwrap_err();

    assert_eq!(
        error.downcast_ref::<AnalysisError>(),
        Some(&AnalysisError::EmptyTrace)
    );
    assert!(error.to_string().contains("empty"));

    for (error, detail) in [
        (AnalysisError::IncompleteTrace, "incomplete"),
        (AnalysisError::UnfinishedPassengers { demand_index: 7 }, "7"),
        (
            AnalysisError::InvalidPassengerCounts { demand_index: 11 },
            "11",
        ),
        (AnalysisError::CountOverflow, "overflow"),
        (AnalysisError::InvalidTickOrder { snapshot_index: 4 }, "4"),
        (
            AnalysisError::InconsistentDemands { snapshot_index: 5 },
            "5",
        ),
        (
            AnalysisError::InvalidPassengerTransition {
                snapshot_index: 6,
                demand_index: 9,
            },
            "9",
        ),
        (AnalysisError::TimeOverflow, "overflow"),
        (AnalysisError::InvalidCapacity { snapshot_index: 2 }, "2"),
        (AnalysisError::InvalidOccupancy { snapshot_index: 3 }, "3"),
        (AnalysisError::MissingInitialState, "initial"),
        (AnalysisError::NonContiguousTicks { snapshot_index: 2 }, "2"),
        (
            AnalysisError::InvalidStopSequence { snapshot_index: 1 },
            "1",
        ),
    ] {
        assert!(error.to_string().contains(detail));
    }
}

#[rstest]
#[case::empty(vec![])]
#[case::zero_demand(vec![Demand::new(NodeId(8), NodeId(3), 0)])]
#[case::unserved(vec![Demand::new(NodeId(3), NodeId(8), 2)])]
fn analyze_matches_individual_summaries_for_empty_zero_and_unserved_demand(
    #[case] demands: Vec<Demand>,
) {
    let mut trace = completed_trace(&demands, 6);

    for snapshot in &mut trace.snapshots {
        snapshot.tick += u64::MAX - 2;
    }

    assert_eq!(
        analyze(&trace),
        Ok(OperationalAnalysis {
            passenger_outcomes: PassengerOutcomes::from_trace(&trace).unwrap(),
            passenger_times: PassengerTimes::from_trace(&trace).unwrap(),
            vehicle_occupancy: VehicleOccupancy::from_trace(&trace).unwrap(),
            stops: StopActivity::from_trace(&trace).unwrap(),
            route_timing: RouteTiming::from_trace(&trace).unwrap(),
        })
    );
}

#[rstest]
#[case::one_tick_durations(1, 1, 4, 2, 2)]
#[case::longer_durations(3, 2, 10, 6, 4)]
fn route_timing_counts_initial_and_intermediate_dwell_without_final_dwell(
    #[case] travel: u64,
    #[case] dwell: u64,
    #[case] elapsed: u64,
    #[case] traveling: u64,
    #[case] dwelling: u64,
) {
    let mut network = Network::new();
    let outbound = network
        .add_edge(NodeId(8), NodeId(3), EdgeKind::Rail)
        .unwrap();
    let inbound = network
        .add_edge(NodeId(3), NodeId(8), EdgeKind::Rail)
        .unwrap();
    let route = RailRoute::new(&network, vec![outbound, inbound]).unwrap();

    let mut vehicle = RailVehicle::new(route, 6, travel, dwell).unwrap();
    let trace = vehicle
        .record_trace(
            &mut SimulationClock::default(),
            &mut RailPassengers::new(&[Demand::new(NodeId(8), NodeId(3), 8)]),
            elapsed,
        )
        .unwrap();
    let before = trace.clone();
    let expected = RouteTiming {
        elapsed_ticks: elapsed,
        traveling_ticks: traveling,
        dwelling_ticks: dwelling,
        completion_tick: elapsed,
    };

    assert_eq!(RouteTiming::from_trace(&trace), Ok(expected));
    assert_eq!(RouteTiming::from_trace(&trace), Ok(expected));
    assert_eq!(trace, before);
}

#[test]
fn route_timing_measures_only_recorded_active_intervals() {
    let mut trace = completed_trace(&[], 6);

    for (snapshot, tick) in trace.snapshots.iter_mut().zip([100, 101, 102]) {
        snapshot.tick = tick;
    }

    let expected = RouteTiming {
        elapsed_ticks: 2,
        traveling_ticks: 1,
        dwelling_ticks: 1,
        completion_tick: 102,
    };

    assert_eq!(RouteTiming::from_trace(&trace), Ok(expected));

    // A caller-added completed interval contributes no travel or dwell time.
    let mut later = trace.snapshots.last().unwrap().clone();
    later.tick = 103;

    trace.snapshots.push(later);

    assert_eq!(
        RouteTiming::from_trace(&trace),
        Ok(RouteTiming {
            completion_tick: 103,
            ..expected
        })
    );

    trace.snapshots.pop();
    trace.snapshots.remove(0);

    assert_eq!(
        RouteTiming::from_trace(&trace),
        Ok(RouteTiming {
            elapsed_ticks: 1,
            traveling_ticks: 1,
            dwelling_ticks: 0,
            completion_tick: 102,
        })
    );

    trace.snapshots.remove(0);

    assert_eq!(
        RouteTiming::from_trace(&trace),
        Ok(RouteTiming {
            elapsed_ticks: 0,
            traveling_ticks: 0,
            dwelling_ticks: 0,
            completion_tick: 102,
        })
    );
}

#[test]
fn route_timing_preserves_full_width_completion_ticks() {
    let mut trace = completed_trace(&[], 6);
    trace.snapshots[2].tick = u64::MAX;

    assert_trace_error(
        &trace,
        AnalysisError::NonContiguousTicks { snapshot_index: 2 },
    );

    trace.snapshots[0].tick = u64::MAX - 2;
    trace.snapshots[1].tick = u64::MAX - 1;

    assert_eq!(
        RouteTiming::from_trace(&trace),
        Ok(RouteTiming {
            elapsed_ticks: 2,
            traveling_ticks: 1,
            dwelling_ticks: 1,
            completion_tick: u64::MAX,
        })
    );
}

#[test]
fn analyses_reject_invalid_recordings_without_mutation() {
    let trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    let mut cases = vec![(
        RailTrace {
            snapshots: vec![],
            completed: true,
        },
        AnalysisError::EmptyTrace,
    )];

    let mut invalid = trace.clone();
    invalid.completed = false;

    cases.push((invalid, AnalysisError::IncompleteTrace));

    let mut invalid = trace.clone();
    invalid.snapshots.pop();

    cases.push((invalid, AnalysisError::IncompleteTrace));

    for (snapshot_index, tick, error) in [
        (1, 0, AnalysisError::InvalidTickOrder { snapshot_index: 1 }),
        (2, 0, AnalysisError::InvalidTickOrder { snapshot_index: 2 }),
        (2, 1, AnalysisError::InvalidTickOrder { snapshot_index: 2 }),
        (
            1,
            3,
            AnalysisError::NonContiguousTicks { snapshot_index: 1 },
        ),
        (
            2,
            u64::MAX,
            AnalysisError::NonContiguousTicks { snapshot_index: 2 },
        ),
    ] {
        let mut invalid = trace.clone();
        invalid.snapshots[snapshot_index].tick = tick;

        cases.push((invalid, error));
    }

    let mut invalid = trace.clone();
    invalid.snapshots[2].passengers[0].waiting = 1;

    cases.push((
        invalid,
        AnalysisError::UnfinishedPassengers { demand_index: 0 },
    ));

    let mut invalid = trace.clone();
    invalid.snapshots[2].passengers[0].arrived = 1;

    cases.push((
        invalid,
        AnalysisError::InvalidPassengerCounts { demand_index: 0 },
    ));

    for (invalid, error) in cases {
        assert_trace_error(&invalid, error);
    }
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

#[rstest]
#[case::waiting(1, 0)]
#[case::onboard(0, 1)]
fn passenger_outcomes_reject_remaining_waiting_or_onboard_passengers(
    #[case] waiting: u32,
    #[case] onboard: u32,
) {
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

#[rstest]
#[case::too_few_arrivals(1, 0)]
#[case::too_many_arrivals(3, 0)]
#[case::wide_terminal_total(u32::MAX, u32::MAX)]
fn passenger_outcomes_require_conservation_per_terminal_demand(
    #[case] arrived: u32,
    #[case] unserved: u32,
) {
    let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2); 2], 6);

    let records = &mut trace.snapshots.last_mut().unwrap().passengers;
    records[0].arrived = arrived;
    records[0].unserved = unserved;
    // Aggregate conservation alone would accept too few or too many arrivals.
    records[1].arrived = 4u32.saturating_sub(arrived);

    let before = trace.clone();

    assert_eq!(
        PassengerOutcomes::from_trace(&trace),
        Err(AnalysisError::InvalidPassengerCounts { demand_index: 0 })
    );
    assert_eq!(trace, before);
}

#[test]
fn passenger_times_reject_invalid_recordings_without_mutation() {
    let trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    let mut cases = vec![(
        RailTrace {
            snapshots: vec![],
            completed: true,
        },
        AnalysisError::EmptyTrace,
    )];

    let mut incomplete = trace.clone();
    incomplete.completed = false;

    cases.push((incomplete, AnalysisError::IncompleteTrace));

    for missing in [false, true] {
        let mut invalid = trace.clone();

        if missing {
            invalid.snapshots[1].passengers.clear();
        } else {
            invalid.snapshots[1].passengers[0].demand.origin = NodeId(99);
        }

        cases.push((
            invalid,
            AnalysisError::InconsistentDemands { snapshot_index: 1 },
        ));
    }

    let mut invalid = trace.clone();
    invalid.snapshots[0].passengers[0].waiting = 1;

    cases.push((
        invalid,
        AnalysisError::InvalidPassengerCounts { demand_index: 0 },
    ));

    let mut invalid = trace.clone();
    invalid.snapshots[1].passengers[0].onboard = 1;
    invalid.snapshots[1].passengers[0].arrived = 1;

    cases.push((
        invalid,
        AnalysisError::InvalidPassengerTransition {
            snapshot_index: 1,
            demand_index: 0,
        },
    ));

    for (waiting, onboard, arrived, unserved) in [(1, 1, 0, 0), (1, 0, 1, 0), (1, 0, 0, 1)] {
        let mut invalid = trace.clone();

        let record = &mut invalid.snapshots[0].passengers[0];
        record.waiting = waiting;
        record.onboard = onboard;
        record.arrived = arrived;
        record.unserved = unserved;

        invalid.snapshots[0].occupancy = onboard;
        invalid.snapshots[1].passengers[0] = trace.snapshots[0].passengers[0];
        invalid.snapshots[1].occupancy = 0;

        cases.push((
            invalid,
            AnalysisError::InvalidPassengerTransition {
                snapshot_index: 1,
                demand_index: 0,
            },
        ));
    }

    let mut invalid = trace.clone();
    invalid.snapshots[1].passengers[0].onboard = 1;
    invalid.snapshots[1].passengers[0].unserved = 1;
    invalid.snapshots[2].passengers[0].arrived = 1;
    invalid.snapshots[2].passengers[0].unserved = 1;

    cases.push((
        invalid,
        AnalysisError::InvalidPassengerTransition {
            snapshot_index: 1,
            demand_index: 0,
        },
    ));

    let mut invalid = trace.clone();
    invalid.snapshots[2].passengers[0].arrived = 1;
    invalid.snapshots[2].passengers[0].waiting = 1;

    cases.push((
        invalid,
        AnalysisError::UnfinishedPassengers { demand_index: 0 },
    ));

    for (invalid, error) in cases {
        assert_trace_error(&invalid, error);
    }
}

#[test]
fn analyses_check_every_passenger_record_and_sum_onboard_without_wrapping() {
    let trace = completed_trace(
        &[
            Demand::new(NodeId(8), NodeId(3), 2),
            Demand::new(NodeId(8), NodeId(3), 3),
        ],
        6,
    );

    for snapshot_index in 0..trace.snapshots.len() {
        let mut invalid = trace.clone();
        invalid.snapshots[snapshot_index].passengers.swap(0, 1);

        assert_trace_error(
            &invalid,
            AnalysisError::InconsistentDemands {
                // Ordered demands are compared against the final snapshot.
                snapshot_index: if snapshot_index == 2 {
                    0
                } else {
                    snapshot_index
                },
            },
        );
    }

    for snapshot_index in [0, 1] {
        for extra in [false, true] {
            let mut invalid = trace.clone();
            let record = &mut invalid.snapshots[snapshot_index].passengers[1];

            if extra {
                record.unserved = u32::MAX;
            } else if snapshot_index == 0 {
                record.waiting -= 1;
            } else {
                record.onboard -= 1;
            }

            assert_trace_error(
                &invalid,
                AnalysisError::InvalidPassengerCounts { demand_index: 1 },
            );
        }
    }

    let mut invalid = completed_trace(&[Demand::new(NodeId(8), NodeId(3), u32::MAX); 2], u32::MAX);
    invalid.snapshots[1].passengers[1].waiting = 0;
    invalid.snapshots[1].passengers[1].onboard = u32::MAX;
    invalid.snapshots[1].occupancy = u32::MAX - 1; // Wrapped sum would match.

    assert_trace_error(
        &invalid,
        AnalysisError::InvalidOccupancy { snapshot_index: 1 },
    );
}

#[test]
fn vehicle_occupancy_rejects_invalid_recordings_without_mutation() {
    let trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    let mut cases = vec![(
        RailTrace {
            snapshots: vec![],
            completed: true,
        },
        AnalysisError::EmptyTrace,
    )];

    let mut incomplete = trace.clone();
    incomplete.completed = false;

    cases.push((incomplete, AnalysisError::IncompleteTrace));

    let mut incomplete = trace.clone();
    incomplete.snapshots.pop();

    cases.push((incomplete, AnalysisError::IncompleteTrace));

    let mut unfinished = trace.clone();
    unfinished.snapshots[2].passengers[0].waiting = 1;

    cases.push((
        unfinished,
        AnalysisError::UnfinishedPassengers { demand_index: 0 },
    ));

    for snapshot_index in 0..trace.snapshots.len() {
        for capacity in [0, 7] {
            let mut invalid = trace.clone();
            invalid.snapshots[snapshot_index].capacity = capacity;

            cases.push((
                invalid,
                AnalysisError::InvalidCapacity {
                    snapshot_index: if snapshot_index == 0 && capacity != 0 {
                        1
                    } else {
                        snapshot_index
                    },
                },
            ));
        }

        let mut invalid = trace.clone();
        invalid.snapshots[snapshot_index].occupancy = 7;

        cases.push((invalid, AnalysisError::InvalidOccupancy { snapshot_index }));

        // Mismatches in both directions remain invalid even within capacity.
        for occupancy in [1, 3] {
            let mut invalid = trace.clone();
            invalid.snapshots[snapshot_index].occupancy = occupancy;

            cases.push((invalid, AnalysisError::InvalidOccupancy { snapshot_index }));
        }
    }

    for (invalid, error) in cases {
        assert_trace_error(&invalid, error);
    }
}

#[test]
fn analyze_returns_owned_ordered_summaries_and_wide_totals() {
    let mut network = Network::new();
    let outbound = network
        .add_edge(NodeId(8), NodeId(3), EdgeKind::Rail)
        .unwrap();
    let inbound = network
        .add_edge(NodeId(3), NodeId(8), EdgeKind::Rail)
        .unwrap();
    let route = RailRoute::new(&network, vec![outbound, inbound]).unwrap();

    let mut vehicle = RailVehicle::new(route, 4, 1, 1).unwrap();
    let mut trace = vehicle
        .record_trace(
            &mut SimulationClock::default(),
            &mut RailPassengers::new(&[
                Demand::new(NodeId(8), NodeId(3), 2),
                Demand::new(NodeId(8), NodeId(99), 1 << 31),
                Demand::new(NodeId(8), NodeId(99), 1 << 31),
            ]),
            4,
        )
        .unwrap();

    let before = trace.clone();
    let analysis = analyze(&trace).unwrap();
    let unserved = u64::from(u32::MAX) + 1;
    let expected = {
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

    assert_eq!(analysis, expected);
    assert_eq!(analyze(&trace), Ok(expected));
    assert_eq!(trace, before);

    trace.snapshots.clear();

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
