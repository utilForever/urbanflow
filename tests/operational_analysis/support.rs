use urbanflow::analysis::{
    AnalysisError, PassengerOutcomes, PassengerTimes, RouteTiming, StopActivity, VehicleOccupancy,
    analyze,
};
use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailRoute, RailTrace, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, NodeId};

pub(super) fn completed_trace(demands: &[Demand], capacity: u32) -> RailTrace {
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

pub(super) fn assert_trace_error(trace: &RailTrace, error: AnalysisError) {
    let before = trace.clone();

    assert_eq!(analyze(trace), Err(error));
    assert_eq!(PassengerOutcomes::from_trace(trace), Err(error));
    assert_eq!(PassengerTimes::from_trace(trace), Err(error));
    assert_eq!(VehicleOccupancy::from_trace(trace), Err(error));
    assert_eq!(StopActivity::from_trace(trace), Err(error));
    assert_eq!(RouteTiming::from_trace(trace), Err(error));
    assert_eq!(*trace, before);
}
