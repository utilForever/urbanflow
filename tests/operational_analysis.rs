#[path = "operational_analysis/passenger_outcomes.rs"]
mod passenger_outcomes;
#[path = "operational_analysis/passenger_times.rs"]
mod passenger_times;
#[path = "operational_analysis/route_timing.rs"]
mod route_timing;
#[path = "operational_analysis/stop_activity.rs"]
mod stop_activity;
#[path = "operational_analysis/support.rs"]
mod support;
#[path = "operational_analysis/validation.rs"]
mod validation;
#[path = "operational_analysis/vehicle_occupancy.rs"]
mod vehicle_occupancy;

use rstest::rstest;
use support::completed_trace;
use urbanflow::analysis::{
    OperationalAnalysis, PassengerOutcomes, PassengerTimes, RouteTiming, StopActivity,
    VehicleOccupancy, analyze,
};
use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailRoute, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, NodeId};

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
