use super::support::{assert_trace_error, completed_trace};
use rstest::rstest;
use urbanflow::analysis::{AnalysisError, RouteTiming};
use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailRoute, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, NodeId};

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
