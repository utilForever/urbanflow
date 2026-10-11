use super::support::{assert_trace_error, completed_trace};
use rstest::rstest;
use urbanflow::analysis::{AnalysisError, PassengerTimes};
use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailRoute, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, NodeId};

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

#[rstest]
#[case::empty(vec![], 0)]
#[case::zero_demand(vec![Demand::new(NodeId(8), NodeId(3), 0)], 0)]
#[case::unserved(vec![Demand::new(NodeId(3), NodeId(8), 2)], 4)]
fn passenger_times_have_no_means_without_arrivals(
    #[case] demands: Vec<Demand>,
    #[case] waiting: u64,
) {
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
fn passenger_times_reject_tick_gaps_before_accumulating_time() {
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

    for (trace, snapshot_index) in [
        (waiting_product, 1),
        (onboard_product, 2),
        (waiting_sum, 1),
        (journey_sum, 1),
    ] {
        assert_trace_error(&trace, AnalysisError::NonContiguousTicks { snapshot_index });
    }

    let mut boundary = completed_trace(&[Demand::new(NodeId(3), NodeId(8), 1)], 6);
    boundary.snapshots[0].tick = u64::MAX - 2;
    boundary.snapshots[1].tick = u64::MAX - 1;
    boundary.snapshots[2].tick = u64::MAX;

    assert_eq!(
        PassengerTimes::from_trace(&boundary)
            .unwrap()
            .waiting_passenger_ticks,
        2
    );
}
