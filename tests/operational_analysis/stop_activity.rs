use super::support::completed_trace;
use rstest::rstest;
use urbanflow::analysis::{AnalysisError, StopActivity, analyze};
use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailPosition, RailRoute, RailTrace, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, NodeId};

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

#[rstest]
#[case::one_tick_dwell(1)]
#[case::three_tick_dwell(3)]
fn stop_activity_preserves_repeated_visits_and_simultaneous_boarding_and_alighting(
    #[case] dwell: u64,
) {
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

#[rstest]
#[case::empty(vec![], 0)]
#[case::zero_demand(vec![Demand::new(NodeId(8), NodeId(3), 0)], 0)]
#[case::wide_waiting_counts(vec![Demand::new(NodeId(8), NodeId(99), u32::MAX); 2], 8_589_934_590)]
fn stop_activity_keeps_empty_visits_and_checked_wide_waiting_counts(
    #[case] demands: Vec<Demand>,
    #[case] waiting: u64,
) {
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

#[rstest]
#[case::empty(
    |trace: &mut RailTrace| trace.snapshots.clear(),
    AnalysisError::EmptyTrace,
)]
#[case::incomplete(
    |trace: &mut RailTrace| trace.completed = false,
    AnalysisError::IncompleteTrace,
)]
#[case::starts_during_service(
    |trace: &mut RailTrace| {
        trace.snapshots.drain(..1);
    },
    AnalysisError::MissingInitialState,
)]
#[case::completion_only(
    |trace: &mut RailTrace| {
        trace.snapshots.drain(..2);
    },
    AnalysisError::MissingInitialState,
)]
#[case::initial_passengers_onboard(
    |trace: &mut RailTrace| {
        trace.snapshots[0].passengers[0].waiting = 1;
        trace.snapshots[0].passengers[0].onboard = 1;
        trace.snapshots[0].occupancy = 1;
    },
    AnalysisError::MissingInitialState,
)]
#[case::duplicate_tick(
    |trace: &mut RailTrace| trace.snapshots[1].tick = 0,
    AnalysisError::InvalidTickOrder { snapshot_index: 1 },
)]
#[case::missing_tick(
    |trace: &mut RailTrace| {
        trace.snapshots.remove(1);
    },
    AnalysisError::NonContiguousTicks { snapshot_index: 1 },
)]
#[case::changed_demand(
    |trace: &mut RailTrace| trace.snapshots[1].passengers[0].demand.origin = NodeId(99),
    AnalysisError::InconsistentDemands { snapshot_index: 1 },
)]
#[case::missing_demand(
    |trace: &mut RailTrace| trace.snapshots[1].passengers.clear(),
    AnalysisError::InconsistentDemands { snapshot_index: 1 },
)]
#[case::invalid_passenger_counts(
    |trace: &mut RailTrace| trace.snapshots[1].passengers[0].onboard = 1,
    AnalysisError::InvalidPassengerCounts { demand_index: 0 },
)]
#[case::wrong_final_stop_index(
    |trace: &mut RailTrace| {
        trace.snapshots[2].position = RailPosition::Complete {
            stop_index: 2,
            node: NodeId(3),
        };
    },
    AnalysisError::InvalidStopSequence { snapshot_index: 2 },
)]
#[case::wrong_final_node(
    |trace: &mut RailTrace| {
        trace.snapshots[2].position = RailPosition::Complete {
            stop_index: 1,
            node: NodeId(99),
        };
    },
    AnalysisError::InvalidStopSequence { snapshot_index: 2 },
)]
#[case::wrong_edge_index(
    |trace: &mut RailTrace| {
        if let RailPosition::Traveling { ref mut edge_index, .. } = trace.snapshots[1].position {
            *edge_index = usize::MAX;
        }
    },
    AnalysisError::InvalidStopSequence { snapshot_index: 1 },
)]
#[case::boarding_at_wrong_origin(
    |trace: &mut RailTrace| {
        for snapshot in &mut trace.snapshots {
            snapshot.passengers[0].demand.origin = NodeId(99);
        }
    },
    AnalysisError::InvalidPassengerTransition { snapshot_index: 1, demand_index: 0 },
)]
#[case::alighting_at_wrong_destination(
    |trace: &mut RailTrace| {
        for snapshot in &mut trace.snapshots {
            snapshot.passengers[0].demand.destination = NodeId(99);
        }
    },
    AnalysisError::InvalidPassengerTransition { snapshot_index: 2, demand_index: 0 },
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
        trace.snapshots[1].passengers[0].onboard = 0;
        trace.snapshots[1].passengers[0].unserved = 2;
        trace.snapshots[2].passengers[0].arrived = 0;
        trace.snapshots[2].passengers[0].unserved = 2;
    },
    AnalysisError::InvalidPassengerTransition { snapshot_index: 1, demand_index: 0 },
)]
fn stop_activity_requires_full_recording_and_consistent_stop_and_passenger_data(
    #[case] invalidate: fn(&mut RailTrace),
    #[case] error: AnalysisError,
) {
    let mut trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), 2)], 6);
    invalidate(&mut trace);

    let before = trace.clone();

    assert_eq!(analyze(&trace), Err(error));
    assert_eq!(StopActivity::from_trace(&trace), Err(error));
    assert_eq!(trace, before);
}

#[rstest]
#[case::travel(false)]
#[case::later_dwell(true)]
fn stop_activity_rejects_passenger_changes_during_travel_or_later_dwell(#[case] dwell: bool) {
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
