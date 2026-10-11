use super::support::{assert_trace_error, completed_trace};
use rstest::rstest;
use urbanflow::analysis::{AnalysisError, OperationalAnalysis, PassengerOutcomes, analyze};
use urbanflow::demand::Demand;
use urbanflow::rail::RailTrace;
use urbanflow::world::NodeId;

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
