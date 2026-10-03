use urbanflow::analysis::{
    AnalysisError, OperationalAnalysis, PassengerOutcomes, PassengerTimes, RouteTiming,
    StopActivity, VehicleOccupancy,
};
use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailPosition, RailRoute, RailTrace, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, NodeId};

#[test]
fn analysis_errors_support_standard_error_propagation_and_diagnostics() {
    fn summarize(trace: &RailTrace) -> Result<PassengerOutcomes, Box<dyn std::error::Error>> {
        Ok(PassengerOutcomes::from_trace(trace)?)
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
fn route_timing_counts_initial_and_intermediate_dwell_without_final_dwell() {
    let mut network = Network::new();
    let outbound = network
        .add_edge(NodeId(8), NodeId(3), EdgeKind::Rail)
        .unwrap();
    let inbound = network
        .add_edge(NodeId(3), NodeId(8), EdgeKind::Rail)
        .unwrap();
    let route = RailRoute::new(&network, vec![outbound, inbound]).unwrap();

    for (travel, dwell, elapsed, traveling, dwelling) in [(1, 1, 4, 2, 2), (3, 2, 10, 6, 4)] {
        let mut vehicle = RailVehicle::new(route.clone(), 6, travel, dwell).unwrap();
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
}

#[test]
fn route_timing_rejects_invalid_recordings_without_mutation() {
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

    for tick in [0, 2, 3] {
        let mut invalid = trace.clone();
        invalid.snapshots[1].tick = tick;

        cases.push((
            invalid,
            AnalysisError::InvalidTickOrder {
                snapshot_index: if tick == 0 { 1 } else { 2 },
            },
        ));
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
        let before = invalid.clone();
        assert_eq!(RouteTiming::from_trace(&invalid), Err(error));
        assert_eq!(invalid, before);
    }
}

#[test]
fn stop_activity_counts_duplicate_demands_and_final_waiting_before_completion() {
    let trace = completed_trace(
        &[
            Demand::new(NodeId(8), NodeId(3), 4),
            Demand::new(NodeId(8), NodeId(3), 4),
            Demand::new(NodeId(8), NodeId(99), 2),
            Demand::new(NodeId(3), NodeId(8), 3),
            Demand::new(NodeId(21), NodeId(3), 2),
            Demand::new(NodeId(8), NodeId(3), 0),
        ],
        6,
    );
    let before = trace.clone();
    let expected = vec![
        StopActivity {
            stop_index: 0,
            node: NodeId(8),
            boarded: 6,
            alighted: 0,
            remaining_waiting: 4,
        },
        StopActivity {
            stop_index: 1,
            node: NodeId(3),
            boarded: 0,
            alighted: 6,
            remaining_waiting: 3,
        },
    ];

    assert_eq!(StopActivity::from_trace(&trace), Ok(expected.clone()));
    assert_eq!(StopActivity::from_trace(&trace), Ok(expected));
    assert_eq!(trace, before);
}

#[test]
fn stop_activity_preserves_repeated_visits_and_simultaneous_boarding_and_alighting() {
    for dwell in [1, 3] {
        let mut network = Network::new();
        let outbound = network
            .add_edge(NodeId(8), NodeId(3), EdgeKind::Rail)
            .unwrap();
        let inbound = network
            .add_edge(NodeId(3), NodeId(8), EdgeKind::Rail)
            .unwrap();
        let route = RailRoute::new(&network, vec![outbound, inbound, outbound, inbound]).unwrap();
        let mut vehicle = RailVehicle::new(route, 2, 2, dwell).unwrap();
        let trace = vehicle
            .record_trace(
                &mut SimulationClock::default(),
                &mut RailPassengers::new(&[Demand::new(NodeId(8), NodeId(8), 5)]),
                20,
            )
            .unwrap();

        // At the second visit to 8, two alight and two board: occupancy stays 2.
        let expected = [
            (8, 2, 0, 3),
            (3, 0, 0, 0),
            (8, 2, 2, 1),
            (3, 0, 0, 0),
            (8, 0, 2, 1),
        ]
        .into_iter()
        .enumerate()
        .map(
            |(stop_index, (node, boarded, alighted, remaining_waiting))| StopActivity {
                stop_index,
                node: NodeId(node),
                boarded,
                alighted,
                remaining_waiting,
            },
        )
        .collect();

        assert_eq!(StopActivity::from_trace(&trace), Ok(expected));
    }
}

#[test]
fn stop_activity_keeps_empty_visits_and_checked_wide_waiting_counts() {
    for (demands, waiting) in [
        (vec![], 0),
        (vec![Demand::new(NodeId(8), NodeId(3), 0)], 0),
        (
            vec![Demand::new(NodeId(8), NodeId(99), u32::MAX); 2],
            8_589_934_590,
        ),
    ] {
        let mut trace = completed_trace(&demands, 1);

        // A full service can start at a nonzero clock, even near its limit.
        for snapshot in &mut trace.snapshots {
            snapshot.tick += u64::MAX - 2;
        }

        assert_eq!(
            StopActivity::from_trace(&trace),
            Ok(vec![
                StopActivity {
                    stop_index: 0,
                    node: NodeId(8),
                    boarded: 0,
                    alighted: 0,
                    remaining_waiting: waiting
                },
                StopActivity {
                    stop_index: 1,
                    node: NodeId(3),
                    boarded: 0,
                    alighted: 0,
                    remaining_waiting: 0
                },
            ])
        );
    }
}

#[test]
fn stop_activity_requires_full_recording_and_consistent_stop_and_passenger_data() {
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

    for first in [1, 2] {
        let mut invalid = trace.clone();
        invalid.snapshots.drain(..first);

        cases.push((invalid, AnalysisError::MissingInitialState));
    }

    let mut invalid = trace.clone();
    invalid.snapshots[0].passengers[0].waiting = 1;
    invalid.snapshots[0].passengers[0].onboard = 1;

    cases.push((invalid, AnalysisError::MissingInitialState));

    let mut invalid = trace.clone();
    invalid.snapshots[1].tick = 0;

    cases.push((
        invalid,
        AnalysisError::InvalidTickOrder { snapshot_index: 1 },
    ));

    let mut invalid = trace.clone();
    invalid.snapshots.remove(1);

    cases.push((
        invalid,
        AnalysisError::NonContiguousTicks { snapshot_index: 1 },
    ));

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
    invalid.snapshots[1].passengers[0].onboard = 1;

    cases.push((
        invalid,
        AnalysisError::InvalidPassengerCounts { demand_index: 0 },
    ));

    for position in [
        RailPosition::Complete {
            stop_index: 2,
            node: NodeId(3),
        },
        RailPosition::Complete {
            stop_index: 1,
            node: NodeId(99),
        },
    ] {
        let mut invalid = trace.clone();
        invalid.snapshots[2].position = position;

        cases.push((
            invalid,
            AnalysisError::InvalidStopSequence { snapshot_index: 2 },
        ));
    }

    let mut invalid = trace.clone();

    if let RailPosition::Traveling {
        ref mut edge_index, ..
    } = invalid.snapshots[1].position
    {
        *edge_index = usize::MAX;
    }

    cases.push((
        invalid,
        AnalysisError::InvalidStopSequence { snapshot_index: 1 },
    ));

    for (change_origin, snapshot_index) in [(true, 1), (false, 2)] {
        let mut invalid = trace.clone();

        for snapshot in &mut invalid.snapshots {
            let demand = &mut snapshot.passengers[0].demand;

            if change_origin {
                demand.origin = NodeId(99);
            } else {
                demand.destination = NodeId(99);
            }
        }

        cases.push((
            invalid,
            AnalysisError::InvalidPassengerTransition {
                snapshot_index,
                demand_index: 0,
            },
        ));
    }

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

    let mut invalid = trace.clone();
    invalid.snapshots[1].passengers[0].onboard = 0;
    invalid.snapshots[1].passengers[0].unserved = 2;
    invalid.snapshots[2].passengers[0].arrived = 0;
    invalid.snapshots[2].passengers[0].unserved = 2;

    cases.push((
        invalid,
        AnalysisError::InvalidPassengerTransition {
            snapshot_index: 1,
            demand_index: 0,
        },
    ));

    for (invalid, error) in cases {
        let before = invalid.clone();
        assert_eq!(StopActivity::from_trace(&invalid), Err(error));
        assert_eq!(invalid, before);
    }
}

#[test]
fn stop_activity_rejects_passenger_changes_during_travel_or_later_dwell() {
    for dwell in [false, true] {
        let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
        trace.snapshots.insert(2, trace.snapshots[1].clone());

        for (tick, snapshot) in trace.snapshots.iter_mut().enumerate() {
            snapshot.tick = tick as u64;
        }

        if dwell {
            trace.snapshots[1].position = RailPosition::AtStop {
                stop_index: 0,
                node: NodeId(8),
                dwell_ticks_remaining: 1,
            };
        }

        trace.snapshots[1].passengers[0].waiting = 1;
        trace.snapshots[1].passengers[0].onboard = 1;
        trace.snapshots[1].occupancy = 1;

        assert_eq!(
            StopActivity::from_trace(&trace),
            Err(AnalysisError::InvalidPassengerTransition {
                snapshot_index: 2,
                demand_index: 0,
            })
        );
    }
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
fn passenger_outcomes_handle_empty_zero_all_served_and_all_unserved_demand() {
    for (demands, requested, arrived, unserved, served_share) in [
        (vec![], 0, 0, 0, None),
        (vec![Demand::new(NodeId(8), NodeId(3), 0)], 0, 0, 0, None),
        (
            vec![Demand::new(NodeId(8), NodeId(3), 2)],
            2,
            2,
            0,
            Some(1.0),
        ),
        (
            vec![Demand::new(NodeId(3), NodeId(8), 2)],
            2,
            0,
            2,
            Some(0.0),
        ),
    ] {
        assert_eq!(
            PassengerOutcomes::from_trace(&completed_trace(&demands, 6)),
            Ok(PassengerOutcomes {
                requested,
                arrived,
                unserved,
                served_share,
            })
        );
    }
}

#[test]
fn passenger_outcomes_preserve_totals_larger_than_one_demand() {
    let trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), u32::MAX); 2], 1);

    assert_eq!(
        PassengerOutcomes::from_trace(&trace),
        Ok(PassengerOutcomes {
            requested: 8_589_934_590,
            arrived: 1,
            unserved: 8_589_934_589,
            served_share: Some(1.0 / 8_589_934_590.0),
        })
    );
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
fn passenger_times_exclude_unserved_waiting_from_arrived_means() {
    let trace = completed_trace(
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

    // All 13 wait during [0, 1); six board at tick 1 and arrive at tick 2.
    assert_eq!(
        PassengerTimes::from_trace(&trace),
        Ok(PassengerTimes {
            waiting_passenger_ticks: 20,
            onboard_passenger_ticks: 6,
            arrived_waiting_passenger_ticks: 6,
            arrived_onboard_passenger_ticks: 6,
            mean_waiting_ticks: Some(1.0),
            mean_onboard_ticks: Some(1.0),
            mean_journey_ticks: Some(2.0),
        })
    );
    assert_eq!(trace, before);
}

#[test]
fn passenger_times_follow_separate_boarding_batches_and_include_onboard_dwell() {
    let mut network = Network::new();
    let outbound = network
        .add_edge(NodeId(8), NodeId(3), EdgeKind::Rail)
        .unwrap();
    let inbound = network
        .add_edge(NodeId(3), NodeId(8), EdgeKind::Rail)
        .unwrap();
    let route = RailRoute::new(&network, vec![outbound, inbound, outbound]).unwrap();
    let mut vehicle = RailVehicle::new(route, 2, 2, 2).unwrap();
    let trace = vehicle
        .record_trace(
            &mut SimulationClock::default(),
            &mut RailPassengers::new(&[Demand::new(NodeId(8), NodeId(3), 5)]),
            12,
        )
        .unwrap();

    // Two board at 1 and arrive at 4; two board at 8 and arrive at 12.
    // One passenger waits all 12 ticks and is unserved.
    let expected = PassengerTimes {
        waiting_passenger_ticks: 30,
        onboard_passenger_ticks: 14,
        arrived_waiting_passenger_ticks: 18,
        arrived_onboard_passenger_ticks: 14,
        mean_waiting_ticks: Some(4.5),
        mean_onboard_ticks: Some(3.5),
        mean_journey_ticks: Some(8.0),
    };
    assert_eq!(PassengerTimes::from_trace(&trace), Ok(expected));

    let mut shifted = trace.clone();

    for snapshot in &mut shifted.snapshots {
        snapshot.tick += 100;
    }

    assert_eq!(PassengerTimes::from_trace(&shifted), Ok(expected));
}

#[test]
fn passenger_times_handle_alighting_and_boarding_the_same_demand_at_one_tick() {
    let mut network = Network::new();
    let outbound = network
        .add_edge(NodeId(8), NodeId(3), EdgeKind::Rail)
        .unwrap();
    let inbound = network
        .add_edge(NodeId(3), NodeId(8), EdgeKind::Rail)
        .unwrap();
    let route = RailRoute::new(&network, vec![outbound, inbound, outbound, inbound]).unwrap();
    let mut vehicle = RailVehicle::new(route, 2, 2, 2).unwrap();
    let trace = vehicle
        .record_trace(
            &mut SimulationClock::default(),
            &mut RailPassengers::new(&[Demand::new(NodeId(8), NodeId(8), 5)]),
            16,
        )
        .unwrap();

    // Two ride [1, 8); two more ride [8, 16); one waits until completion.
    assert_eq!(
        PassengerTimes::from_trace(&trace),
        Ok(PassengerTimes {
            waiting_passenger_ticks: 34,
            onboard_passenger_ticks: 30,
            arrived_waiting_passenger_ticks: 18,
            arrived_onboard_passenger_ticks: 30,
            mean_waiting_ticks: Some(4.5),
            mean_onboard_ticks: Some(7.5),
            mean_journey_ticks: Some(12.0),
        })
    );
}

#[test]
fn passenger_times_attribute_partial_arrivals_to_earliest_boardings() {
    let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 3)], 6);
    let active = trace.snapshots[1].clone();

    trace.snapshots.splice(2..2, [active.clone(), active]);

    for (tick, (snapshot, (waiting, onboard, arrived, unserved))) in trace
        .snapshots
        .iter_mut()
        .zip([
            (3, 0, 0, 0),
            (2, 1, 0, 0),
            (1, 2, 0, 0),
            (1, 1, 1, 0),
            (0, 0, 1, 2),
        ])
        .enumerate()
    {
        snapshot.tick = tick as u64;
        snapshot.occupancy = onboard;

        let record = &mut snapshot.passengers[0];
        record.waiting = waiting;
        record.onboard = onboard;
        record.arrived = arrived;
        record.unserved = unserved;
    }

    // Explicit accounting can leave onboard passengers unserved. The arrival
    // at 3 belongs to the first boarding at 1, not the second boarding at 2.
    assert_eq!(
        PassengerTimes::from_trace(&trace),
        Ok(PassengerTimes {
            waiting_passenger_ticks: 7,
            onboard_passenger_ticks: 4,
            arrived_waiting_passenger_ticks: 1,
            arrived_onboard_passenger_ticks: 2,
            mean_waiting_ticks: Some(1.0),
            mean_onboard_ticks: Some(2.0),
            mean_journey_ticks: Some(3.0),
        })
    );

    trace.snapshots.drain(..3);

    assert_eq!(
        PassengerTimes::from_trace(&trace),
        Ok(PassengerTimes {
            waiting_passenger_ticks: 1,
            onboard_passenger_ticks: 1,
            arrived_waiting_passenger_ticks: 0,
            arrived_onboard_passenger_ticks: 0,
            mean_waiting_ticks: Some(0.0),
            mean_onboard_ticks: Some(0.0),
            mean_journey_ticks: Some(0.0),
        })
    );
}

#[test]
fn passenger_times_have_no_means_without_arrivals() {
    for (demands, waiting) in [
        (vec![], 0),
        (vec![Demand::new(NodeId(8), NodeId(3), 0)], 0),
        (vec![Demand::new(NodeId(3), NodeId(8), 2)], 4),
    ] {
        assert_eq!(
            PassengerTimes::from_trace(&completed_trace(&demands, 6)),
            Ok(PassengerTimes {
                waiting_passenger_ticks: waiting,
                onboard_passenger_ticks: 0,
                arrived_waiting_passenger_ticks: 0,
                arrived_onboard_passenger_ticks: 0,
                mean_waiting_ticks: None,
                mean_onboard_ticks: None,
                mean_journey_ticks: None,
            })
        );
    }
}

#[test]
fn passenger_times_measure_only_recorded_intervals() {
    let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    trace.snapshots.remove(0);

    assert_eq!(
        PassengerTimes::from_trace(&trace),
        Ok(PassengerTimes {
            waiting_passenger_ticks: 0,
            onboard_passenger_ticks: 2,
            arrived_waiting_passenger_ticks: 0,
            arrived_onboard_passenger_ticks: 2,
            mean_waiting_ticks: Some(0.0),
            mean_onboard_ticks: Some(1.0),
            mean_journey_ticks: Some(1.0),
        })
    );

    trace.snapshots.remove(0);

    assert_eq!(
        PassengerTimes::from_trace(&trace),
        Ok(PassengerTimes {
            waiting_passenger_ticks: 0,
            onboard_passenger_ticks: 0,
            arrived_waiting_passenger_ticks: 0,
            arrived_onboard_passenger_ticks: 0,
            mean_waiting_ticks: Some(0.0),
            mean_onboard_ticks: Some(0.0),
            mean_journey_ticks: Some(0.0),
        })
    );
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

    for tick in [0, 2, 3] {
        let mut invalid = trace.clone();
        invalid.snapshots[1].tick = tick;

        cases.push((
            invalid,
            AnalysisError::InvalidTickOrder {
                snapshot_index: if tick == 0 { 1 } else { 2 },
            },
        ));
    }

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

        invalid.snapshots[1].passengers[0] = trace.snapshots[0].passengers[0];

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
        let before = invalid.clone();
        assert_eq!(PassengerTimes::from_trace(&invalid), Err(error));
        assert_eq!(invalid, before);
    }
}

#[test]
fn passenger_times_check_products_totals_and_combined_journey_overflow() {
    let mut waiting_product = completed_trace(&[Demand::new(NodeId(3), NodeId(8), 2)], 6);
    waiting_product.snapshots[1].tick = u64::MAX - 1;
    waiting_product.snapshots[2].tick = u64::MAX;

    let mut onboard_product = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    onboard_product.snapshots[2].tick = u64::MAX;

    let mut waiting_sum = completed_trace(&[Demand::new(NodeId(3), NodeId(8), u32::MAX); 2], 6);
    waiting_sum.snapshots[1].tick = 1 << 32;
    waiting_sum.snapshots[2].tick = (1 << 32) + 1;

    let mut journey_sum = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    journey_sum.snapshots[1].tick = 1 << 62;
    journey_sum.snapshots[2].tick = 1 << 63;

    for trace in [waiting_product, onboard_product, waiting_sum, journey_sum] {
        let before = trace.clone();

        assert_eq!(
            PassengerTimes::from_trace(&trace),
            Err(AnalysisError::TimeOverflow)
        );
        assert_eq!(trace, before);
    }

    let mut boundary = completed_trace(&[Demand::new(NodeId(3), NodeId(8), 1)], 6);
    boundary.snapshots[1].tick = u64::MAX - 1;
    boundary.snapshots[2].tick = u64::MAX;

    assert_eq!(
        PassengerTimes::from_trace(&boundary)
            .unwrap()
            .waiting_passenger_ticks,
        u64::MAX
    );
}

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

    // Uneven intervals retain the same piecewise-constant occupancy.
    trace
        .snapshots
        .retain(|frame| [0, 1, 4, 8].contains(&frame.tick));

    for frame in &mut trace.snapshots {
        frame.tick += 100;
    }
    assert_eq!(VehicleOccupancy::from_trace(&trace), Ok(expected));
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
    later.tick += 10;

    // Even a caller-supplied load after completion cannot affect the maximum.
    trace.snapshots[0].occupancy = 6;
    trace.snapshots.push(later);

    assert_eq!(VehicleOccupancy::from_trace(&trace), Ok(expected));
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

    for tick in [0, 2, 3] {
        let mut invalid = trace.clone();
        invalid.snapshots[1].tick = tick;

        cases.push((
            invalid,
            AnalysisError::InvalidTickOrder {
                snapshot_index: if tick == 0 { 1 } else { 2 },
            },
        ));
    }

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
    }

    for (invalid, error) in cases {
        let before = invalid.clone();
        assert_eq!(VehicleOccupancy::from_trace(&invalid), Err(error));
        assert_eq!(invalid, before);
    }
}

#[test]
fn vehicle_occupancy_checks_passenger_tick_overflow() {
    let mut product = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    product.snapshots[2].tick = u64::MAX;

    let mut sum = product.clone();
    sum.snapshots[1].tick = 1 << 62;

    let mut initial = sum.snapshots[1].clone();
    initial.tick = 0;

    sum.snapshots[0] = initial;
    sum.snapshots[2].tick = 1 << 63;

    for trace in [product, sum] {
        let before = trace.clone();
        assert_eq!(
            VehicleOccupancy::from_trace(&trace),
            Err(AnalysisError::TimeOverflow)
        );
        assert_eq!(trace, before);
    }

    let mut boundary = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 1)], 1);
    boundary.snapshots.remove(0);
    boundary.snapshots[0].tick = 0;
    boundary.snapshots[1].tick = u64::MAX;

    assert_eq!(
        VehicleOccupancy::from_trace(&boundary),
        Ok(VehicleOccupancy {
            capacity: 1,
            max_occupancy: 1,
            occupied_passenger_ticks: u64::MAX,
            mean_occupancy: Some(1.0),
            max_load_factor: Some(1.0),
            mean_load_factor: Some(1.0),
        })
    );
}

#[test]
fn vehicle_occupancy_keeps_rounded_means_within_recorded_bounds() {
    let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 3)], 3);
    trace.snapshots.remove(0);
    trace.snapshots[0].tick = 0;
    trace.snapshots[1].tick = 9_007_199_254_740_993;

    assert_eq!(
        VehicleOccupancy::from_trace(&trace),
        Ok(VehicleOccupancy {
            capacity: 3,
            max_occupancy: 3,
            occupied_passenger_ticks: 27_021_597_764_222_979,
            mean_occupancy: Some(3.0),
            max_load_factor: Some(1.0),
            mean_load_factor: Some(1.0),
        })
    );
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
