use urbanflow::demand::Demand;
use urbanflow::rail::{
    RailPassengers, RailPosition, RailRoute, RailSnapshot, RailVehicle, RailVehicleState,
};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeId, EdgeKind, Network, NodeId};

fn vehicle(capacity: u32) -> RailVehicle {
    let mut network = Network::new();
    let second = network
        .add_edge(NodeId(3), NodeId(8), EdgeKind::Rail)
        .unwrap();
    let first = network
        .add_edge(NodeId(8), NodeId(3), EdgeKind::Rail)
        .unwrap();
    let route = RailRoute::new(&network, vec![first, second]).unwrap();

    RailVehicle::new(route, capacity, 2, 2).unwrap()
}

#[test]
fn retained_snapshots_repeat_exact_positions_and_passenger_counts() {
    let demands = [
        Demand::new(NodeId(8), NodeId(3), 2),
        Demand::new(NodeId(8), NodeId(8), 2),
        Demand::new(NodeId(3), NodeId(8), 4),
        Demand::new(NodeId(8), NodeId(3), 2),
        Demand::new(NodeId(99), NodeId(8), 0),
    ];
    let run = || {
        let mut vehicle = vehicle(3);
        let mut clock = SimulationClock::default();
        let mut passengers = RailPassengers::new(&demands);
        let mut snapshots: Vec<RailSnapshot> = Vec::new();

        loop {
            let before = (vehicle.clone(), clock.tick(), passengers.clone());
            let snapshot = vehicle.snapshot(&clock, &passengers).unwrap();

            assert_eq!(vehicle.snapshot(&clock, &passengers), Ok(snapshot.clone()));
            assert_eq!(
                (&vehicle, clock.tick(), &passengers),
                (&before.0, before.1, &before.2)
            );

            snapshots.push(snapshot);

            if vehicle.state() == RailVehicleState::Complete {
                vehicle.advance(&mut clock, &mut passengers).unwrap();
                assert_eq!(
                    vehicle.snapshot(&clock, &passengers).unwrap(),
                    *snapshots.last().unwrap()
                );
                break;
            }

            vehicle.advance(&mut clock, &mut passengers).unwrap();
        }

        snapshots
    };

    // The service inputs have been dropped; each frame still owns its data.
    let snapshots = run();

    assert_eq!(snapshots, run());
    assert_eq!(snapshots.len(), 9);

    for (tick, snapshot) in snapshots.iter().enumerate() {
        let position = match tick {
            0..=1 => RailPosition::AtStop {
                stop_index: 0,
                node: NodeId(8),
                dwell_ticks_remaining: 2 - tick as u64,
            },
            2..=3 => RailPosition::Traveling {
                edge_index: 0,
                edge: EdgeId(1),
                from: NodeId(8),
                to: NodeId(3),
                travel_ticks_elapsed: (tick - 2) as u64,
                travel_ticks_total: 2,
            },
            4..=5 => RailPosition::AtStop {
                stop_index: 1,
                node: NodeId(3),
                dwell_ticks_remaining: (6 - tick) as u64,
            },
            6..=7 => RailPosition::Traveling {
                edge_index: 1,
                edge: EdgeId(0),
                from: NodeId(3),
                to: NodeId(8),
                travel_ticks_elapsed: (tick - 6) as u64,
                travel_ticks_total: 2,
            },
            8 => RailPosition::Complete {
                stop_index: 2,
                node: NodeId(8),
            },
            _ => unreachable!(),
        };

        assert_eq!(snapshot.tick, tick as u64);
        assert_eq!(snapshot.position, position);
        assert_eq!(snapshot.capacity, 3);
        assert_eq!(
            snapshot.occupancy,
            if tick == 0 || tick == 8 { 0 } else { 3 }
        );

        let counts = match tick {
            0 => [(2, 0, 0, 0), (2, 0, 0, 0), (4, 0, 0, 0), (2, 0, 0, 0)],
            1..=3 => [(0, 2, 0, 0), (1, 1, 0, 0), (4, 0, 0, 0), (2, 0, 0, 0)],
            4..=7 => [(0, 0, 2, 0), (1, 1, 0, 0), (2, 2, 0, 0), (2, 0, 0, 0)],
            8 => [(0, 0, 2, 0), (0, 0, 1, 1), (0, 0, 2, 2), (0, 0, 0, 2)],
            _ => unreachable!(),
        };

        assert_eq!(snapshot.passengers.len(), demands.len());

        for ((record, demand), expected) in snapshot
            .passengers
            .iter()
            .zip(demands)
            .zip(counts.into_iter().chain([(0, 0, 0, 0)]))
        {
            assert_eq!(record.demand, demand);
            assert_eq!(
                (
                    record.waiting,
                    record.onboard,
                    record.arrived,
                    record.unserved
                ),
                expected
            );
        }
    }
}
