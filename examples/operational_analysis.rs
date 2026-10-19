// Run with `cargo run --example operational_analysis`.

use urbanflow::analysis::analyze;
use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailRoute, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, NodeId};

fn main() {
    let mut network = Network::new();
    let edge = network
        .add_edge(NodeId(0), NodeId(2), EdgeKind::Rail)
        .unwrap();
    let route = RailRoute::new(&network, vec![edge]).unwrap();

    // Capacity 6, one travel tick per edge, one dwell tick per stop.
    let mut vehicle = RailVehicle::new(route, 6, 1, 1).unwrap();
    let mut clock = SimulationClock::default();
    let mut passengers = RailPassengers::new(&[Demand::new(NodeId(0), NodeId(2), 10)]);

    let trace = vehicle
        .record_trace(&mut clock, &mut passengers, 2)
        .unwrap();

    assert!(trace.completed);

    let analysis = analyze(&trace).unwrap();
    let outcomes = analysis.passenger_outcomes;

    assert_eq!(
        (outcomes.requested, outcomes.arrived, outcomes.unserved),
        (10, 6, 4)
    );
    assert_eq!(outcomes.served_share, Some(0.6));

    let times = analysis.passenger_times;

    assert_eq!(times.waiting_passenger_ticks, 14); // Includes unserved waiting.
    assert_eq!(times.onboard_passenger_ticks, 6);
    assert_eq!(times.arrived_waiting_passenger_ticks, 6);
    assert_eq!(times.arrived_onboard_passenger_ticks, 6);
    assert_eq!(times.mean_waiting_ticks, Some(1.0));
    assert_eq!(times.mean_onboard_ticks, Some(1.0));
    assert_eq!(times.mean_journey_ticks, Some(2.0));

    let occupancy = analysis.vehicle_occupancy;

    assert_eq!(occupancy.capacity, 6);
    assert_eq!(occupancy.max_occupancy, 6);
    assert_eq!(occupancy.occupied_passenger_ticks, 6);
    assert_eq!(occupancy.mean_occupancy, Some(3.0));
    assert_eq!(occupancy.max_load_factor, Some(1.0));
    assert_eq!(occupancy.mean_load_factor, Some(0.5));
    assert_eq!(analysis.stops.len(), 2);
    assert_eq!(
        (
            analysis.stops[0].boarded,
            analysis.stops[0].remaining_waiting
        ),
        (6, 4)
    );
    assert_eq!(analysis.stops[1].alighted, 6);

    let timing = analysis.route_timing;

    assert_eq!(timing.elapsed_ticks, 2);
    assert_eq!((timing.traveling_ticks, timing.dwelling_ticks), (1, 1));
    assert_eq!(timing.completion_tick, 2);

    println!(
        "Arrived: {}, unserved: {}, elapsed ticks: {}",
        outcomes.arrived, outcomes.unserved, timing.elapsed_ticks
    );
}
