use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailPosition, RailRoute, RailVehicle, RailVehicleState};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeId, EdgeKind, Network, NodeId};

fn service() -> (RailVehicle, SimulationClock, RailPassengers) {
    let mut network = Network::new();
    let first = network
        .add_edge(NodeId(8), NodeId(3), EdgeKind::Rail)
        .unwrap();
    let second = network
        .add_edge(NodeId(3), NodeId(8), EdgeKind::Rail)
        .unwrap();
    let vehicle = RailVehicle::new(
        RailRoute::new(&network, vec![first, second]).unwrap(),
        2,
        2,
        1,
    )
    .unwrap();
    let passengers = RailPassengers::new(&[
        Demand::new(NodeId(8), NodeId(3), 3),
        Demand::new(NodeId(3), NodeId(8), 1),
        Demand::new(NodeId(8), NodeId(3), 1),
        Demand::new(NodeId(8), NodeId(8), 0),
    ]);

    (vehicle, SimulationClock::default(), passengers)
}

#[test]
fn trace_retains_initial_and_each_tick_in_deterministic_order() {
    let run = || {
        let (mut vehicle, mut clock, mut passengers) = service();
        vehicle
            .record_trace(&mut clock, &mut passengers, 20)
            .unwrap()
    };
    let trace = run();

    // The service inputs have been dropped; all frames still own their data.
    assert_eq!(trace, run());
    assert!(trace.completed);
    assert_eq!(trace.snapshots.len(), 7);

    let positions = [
        RailPosition::AtStop {
            stop_index: 0,
            node: NodeId(8),
            dwell_ticks_remaining: 1,
        },
        RailPosition::Traveling {
            edge_index: 0,
            edge: EdgeId(0),
            from: NodeId(8),
            to: NodeId(3),
            travel_ticks_elapsed: 0,
            travel_ticks_total: 2,
        },
        RailPosition::Traveling {
            edge_index: 0,
            edge: EdgeId(0),
            from: NodeId(8),
            to: NodeId(3),
            travel_ticks_elapsed: 1,
            travel_ticks_total: 2,
        },
        RailPosition::AtStop {
            stop_index: 1,
            node: NodeId(3),
            dwell_ticks_remaining: 1,
        },
        RailPosition::Traveling {
            edge_index: 1,
            edge: EdgeId(1),
            from: NodeId(3),
            to: NodeId(8),
            travel_ticks_elapsed: 0,
            travel_ticks_total: 2,
        },
        RailPosition::Traveling {
            edge_index: 1,
            edge: EdgeId(1),
            from: NodeId(3),
            to: NodeId(8),
            travel_ticks_elapsed: 1,
            travel_ticks_total: 2,
        },
        RailPosition::Complete {
            stop_index: 2,
            node: NodeId(8),
        },
    ];

    for (tick, (snapshot, position)) in trace.snapshots.iter().zip(positions).enumerate() {
        assert_eq!(snapshot.tick, tick as u64);
        assert_eq!(snapshot.position, position);
        assert_eq!(snapshot.capacity, 2);
        assert_eq!(snapshot.occupancy, [0, 2, 2, 1, 1, 1, 0][tick]);

        let counts = match tick {
            0 => [(3, 0, 0, 0), (1, 0, 0, 0), (1, 0, 0, 0), (0, 0, 0, 0)],
            1..=2 => [(1, 2, 0, 0), (1, 0, 0, 0), (1, 0, 0, 0), (0, 0, 0, 0)],
            3..=5 => [(1, 0, 2, 0), (0, 1, 0, 0), (1, 0, 0, 0), (0, 0, 0, 0)],
            6 => [(0, 0, 2, 1), (0, 0, 1, 0), (0, 0, 0, 1), (0, 0, 0, 0)],
            _ => unreachable!(),
        };

        assert_eq!(snapshot.passengers.len(), 4);

        for ((record, initial), counts) in snapshot
            .passengers
            .iter()
            .zip(&trace.snapshots[0].passengers)
            .zip(counts)
        {
            assert_eq!(record.demand, initial.demand);
            assert_eq!(
                (
                    record.waiting,
                    record.onboard,
                    record.arrived,
                    record.unserved
                ),
                counts
            );
        }
    }
}

#[test]
fn tick_limits_include_initial_frame_and_report_completion_at_the_boundary() {
    for (limit, last_tick, completed) in [
        (0, 0, false),
        (1, 1, false),
        (5, 5, false),
        (6, 6, true),
        (7, 6, true),
        (u64::MAX, 6, true),
    ] {
        let (mut vehicle, mut clock, mut passengers) = service();
        let initial = vehicle.snapshot(&clock, &passengers).unwrap();
        let trace = vehicle
            .record_trace(&mut clock, &mut passengers, limit)
            .unwrap();

        assert_eq!(trace.completed, completed);
        assert_eq!(trace.snapshots.len(), last_tick as usize + 1);
        assert_eq!(trace.snapshots[0], initial);
        assert_eq!(clock.tick(), last_tick);
        assert_eq!(vehicle.state() == RailVehicleState::Complete, completed);
        assert_eq!(
            trace.snapshots.last().unwrap(),
            &vehicle.snapshot(&clock, &passengers).unwrap()
        );

        if !completed {
            assert!(
                passengers
                    .records()
                    .iter()
                    .all(|record| record.unserved == 0)
            );
        }
    }
}

#[test]
fn recording_can_continue_from_a_nonzero_tick_without_changing_retained_frames() {
    let (mut vehicle, mut clock, mut passengers) = service();
    let first = vehicle
        .record_trace(&mut clock, &mut passengers, 2)
        .unwrap();
    let retained = first.clone();
    let second = vehicle
        .record_trace(&mut clock, &mut passengers, 1)
        .unwrap();

    assert!(!second.completed);
    assert_eq!(second.snapshots.len(), 2);
    assert_eq!(second.snapshots[0], first.snapshots[2]);
    assert_eq!(second.snapshots[1].tick, 3);
    assert_eq!(clock.tick(), 3);
    assert_eq!(first, retained);

    let final_trace = vehicle
        .record_trace(&mut clock, &mut passengers, 3)
        .unwrap();

    assert!(final_trace.completed);
    assert_eq!(final_trace.snapshots.last().unwrap().tick, 6);

    for limit in [0, 10] {
        let completed = vehicle
            .record_trace(&mut clock, &mut passengers, limit)
            .unwrap();

        assert!(completed.completed);
        assert_eq!(completed.snapshots.len(), 1);
        assert_eq!(completed.snapshots.last(), final_trace.snapshots.last());
        assert_eq!(clock.tick(), 6);
    }
}
