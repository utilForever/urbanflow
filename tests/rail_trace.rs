use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailPosition, RailRoute, RailVehicle};
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
