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

#[rstest]
#[case::empty(
    |trace: &mut RailTrace| trace.snapshots.clear(),
    AnalysisError::EmptyTrace,
)]
#[case::incomplete(
    |trace: &mut RailTrace| trace.completed = false,
    AnalysisError::IncompleteTrace,
)]
#[case::missing_terminal(
    |trace: &mut RailTrace| {
        trace.snapshots.pop();
    },
    AnalysisError::IncompleteTrace,
)]
#[case::duplicate_intermediate_tick(
    |trace: &mut RailTrace| trace.snapshots[1].tick = 0,
    AnalysisError::InvalidTickOrder { snapshot_index: 1 },
)]
#[case::backward_terminal_tick(
    |trace: &mut RailTrace| trace.snapshots[2].tick = 0,
    AnalysisError::InvalidTickOrder { snapshot_index: 2 },
)]
#[case::duplicate_terminal_tick(
    |trace: &mut RailTrace| trace.snapshots[2].tick = 1,
    AnalysisError::InvalidTickOrder { snapshot_index: 2 },
)]
#[case::intermediate_tick_gap(
    |trace: &mut RailTrace| trace.snapshots[1].tick = 3,
    AnalysisError::NonContiguousTicks { snapshot_index: 1 },
)]
#[case::terminal_tick_gap(
    |trace: &mut RailTrace| trace.snapshots[2].tick = u64::MAX,
    AnalysisError::NonContiguousTicks { snapshot_index: 2 },
)]
#[case::unfinished_passengers(
    |trace: &mut RailTrace| trace.snapshots[2].passengers[0].waiting = 1,
    AnalysisError::UnfinishedPassengers { demand_index: 0 },
)]
#[case::invalid_terminal_counts(
    |trace: &mut RailTrace| trace.snapshots[2].passengers[0].arrived = 1,
    AnalysisError::InvalidPassengerCounts { demand_index: 0 },
)]
fn analyses_reject_invalid_recordings_without_mutation(
    #[case] invalidate: fn(&mut RailTrace),
    #[case] error: AnalysisError,
) {
    let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    invalidate(&mut trace);

    assert_trace_error(&trace, error);
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

#[rstest]
#[case::empty(
    |trace: &mut RailTrace| trace.snapshots.clear(),
    AnalysisError::EmptyTrace,
)]
#[case::incomplete(
    |trace: &mut RailTrace| trace.completed = false,
    AnalysisError::IncompleteTrace,
)]
#[case::changed_demand(
    |trace: &mut RailTrace| trace.snapshots[1].passengers[0].demand.origin = NodeId(99),
    AnalysisError::InconsistentDemands { snapshot_index: 1 },
)]
#[case::missing_demand(
    |trace: &mut RailTrace| trace.snapshots[1].passengers.clear(),
    AnalysisError::InconsistentDemands { snapshot_index: 1 },
)]
#[case::invalid_initial_counts(
    |trace: &mut RailTrace| trace.snapshots[0].passengers[0].waiting = 1,
    AnalysisError::InvalidPassengerCounts { demand_index: 0 },
)]
#[case::arrival_before_boarding(
    |trace: &mut RailTrace| {
        trace.snapshots[1].passengers[0].onboard = 1;
        trace.snapshots[1].passengers[0].arrived = 1;
    },
    AnalysisError::InvalidPassengerTransition { snapshot_index: 1, demand_index: 0 },
)]
#[case::unserved_before_completion(
    |trace: &mut RailTrace| {
        trace.snapshots[1].passengers[0].onboard = 1;
        trace.snapshots[1].passengers[0].unserved = 1;
        trace.snapshots[2].passengers[0].arrived = 1;
        trace.snapshots[2].passengers[0].unserved = 1;
    },
    AnalysisError::InvalidPassengerTransition { snapshot_index: 1, demand_index: 0 },
)]
#[case::unfinished_passengers(
    |trace: &mut RailTrace| {
        trace.snapshots[2].passengers[0].arrived = 1;
        trace.snapshots[2].passengers[0].waiting = 1;
    },
    AnalysisError::UnfinishedPassengers { demand_index: 0 },
)]
fn passenger_times_reject_invalid_recordings_without_mutation(
    #[case] invalidate: fn(&mut RailTrace),
    #[case] error: AnalysisError,
) {
    let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    invalidate(&mut trace);

    assert_trace_error(&trace, error);
}

#[rstest]
#[case::onboard_returns_to_waiting(1, 0, 0)]
#[case::arrived_returns_to_waiting(0, 1, 0)]
#[case::unserved_returns_to_waiting(0, 0, 1)]
fn analyses_reject_backward_passenger_transitions(
    #[case] onboard: u32,
    #[case] arrived: u32,
    #[case] unserved: u32,
) {
    let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    trace.snapshots[1].passengers[0] = trace.snapshots[0].passengers[0];
    trace.snapshots[1].occupancy = 0;

    let record = &mut trace.snapshots[0].passengers[0];
    record.waiting = 1;
    record.onboard = onboard;
    record.arrived = arrived;
    record.unserved = unserved;

    trace.snapshots[0].occupancy = onboard;

    assert_trace_error(
        &trace,
        AnalysisError::InvalidPassengerTransition {
            snapshot_index: 1,
            demand_index: 0,
        },
    );
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

#[rstest]
#[case::empty(
    |trace: &mut RailTrace| trace.snapshots.clear(),
    AnalysisError::EmptyTrace,
)]
#[case::incomplete(
    |trace: &mut RailTrace| trace.completed = false,
    AnalysisError::IncompleteTrace,
)]
#[case::missing_terminal(
    |trace: &mut RailTrace| {
        trace.snapshots.pop();
    },
    AnalysisError::IncompleteTrace,
)]
#[case::unfinished_passengers(
    |trace: &mut RailTrace| trace.snapshots[2].passengers[0].waiting = 1,
    AnalysisError::UnfinishedPassengers { demand_index: 0 },
)]
fn vehicle_occupancy_rejects_invalid_recordings_without_mutation(
    #[case] invalidate: fn(&mut RailTrace),
    #[case] error: AnalysisError,
) {
    let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    invalidate(&mut trace);

    assert_trace_error(&trace, error);
}

#[rstest]
#[case::zero_initial_capacity(0, 0, 0)]
#[case::changed_initial_capacity(0, 7, 1)]
#[case::zero_intermediate_capacity(1, 0, 1)]
#[case::changed_intermediate_capacity(1, 7, 1)]
#[case::zero_terminal_capacity(2, 0, 2)]
#[case::changed_terminal_capacity(2, 7, 2)]
fn analyses_reject_invalid_capacity(
    #[case] snapshot_index: usize,
    #[case] capacity: u32,
    #[case] error_index: usize,
) {
    let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    trace.snapshots[snapshot_index].capacity = capacity;

    assert_trace_error(
        &trace,
        AnalysisError::InvalidCapacity {
            snapshot_index: error_index,
        },
    );
}

#[rstest]
#[case::initial_above_capacity(0, 7)]
#[case::initial_load_one(0, 1)]
#[case::initial_load_three(0, 3)]
#[case::intermediate_above_capacity(1, 7)]
#[case::intermediate_below_onboard(1, 1)]
#[case::intermediate_above_onboard(1, 3)]
#[case::terminal_above_capacity(2, 7)]
#[case::terminal_load_one(2, 1)]
#[case::terminal_load_three(2, 3)]
fn analyses_reject_invalid_occupancy(#[case] snapshot_index: usize, #[case] occupancy: u32) {
    let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    trace.snapshots[snapshot_index].occupancy = occupancy;

    assert_trace_error(&trace, AnalysisError::InvalidOccupancy { snapshot_index });
}
