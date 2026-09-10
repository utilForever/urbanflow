use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailRoute, RailVehicle, RailVehicleState};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, NodeId};

fn vehicle(stops: &[usize], capacity: u32, travel: u64, dwell: u64) -> RailVehicle {
    let mut network = Network::new();
    let edges = stops
        .windows(2)
        .map(|pair| {
            network
                .add_edge(NodeId(pair[0]), NodeId(pair[1]), EdgeKind::Rail)
                .unwrap()
        })
        .collect();

    RailVehicle::new(
        RailRoute::new(&network, edges).unwrap(),
        capacity,
        travel,
        dwell,
    )
    .unwrap()
}

#[test]
fn ticks_repeat_exact_dwell_travel_and_passenger_transitions() {
    use RailVehicleState::{AtStop, Complete, Traveling};

    let initial = vehicle(&[8, 3, 9], 3, 3, 2);
    for mut vehicle in [initial.clone(), initial] {
        let mut clock = SimulationClock::default();
        let mut passengers = RailPassengers::new(&[
            Demand::new(NodeId(8), NodeId(3), 2),
            Demand::new(NodeId(8), NodeId(9), 2),
            Demand::new(NodeId(3), NodeId(9), 4),
            Demand::new(NodeId(99), NodeId(9), 1),
        ]);
        let expected = [
            AtStop {
                stop_index: 0,
                dwell_ticks_remaining: 1,
            },
            Traveling {
                edge_index: 0,
                travel_ticks_elapsed: 0,
            },
            Traveling {
                edge_index: 0,
                travel_ticks_elapsed: 1,
            },
            Traveling {
                edge_index: 0,
                travel_ticks_elapsed: 2,
            },
            AtStop {
                stop_index: 1,
                dwell_ticks_remaining: 2,
            },
            AtStop {
                stop_index: 1,
                dwell_ticks_remaining: 1,
            },
            Traveling {
                edge_index: 1,
                travel_ticks_elapsed: 0,
            },
            Traveling {
                edge_index: 1,
                travel_ticks_elapsed: 1,
            },
            Traveling {
                edge_index: 1,
                travel_ticks_elapsed: 2,
            },
            Complete,
        ];

        for (tick, state) in (1..).zip(expected) {
            assert_eq!(vehicle.advance(&mut clock, &mut passengers), Ok(state));
            assert_eq!(vehicle.state(), state);
            assert_eq!(clock.tick(), tick);

            let counts = match tick {
                1..=4 => [(0, 2, 0, 0), (1, 1, 0, 0), (4, 0, 0, 0), (1, 0, 0, 0)],
                5..=9 => [(0, 0, 2, 0), (1, 1, 0, 0), (2, 2, 0, 0), (1, 0, 0, 0)],
                10 => [(0, 0, 2, 0), (0, 0, 1, 1), (0, 0, 2, 2), (0, 0, 0, 1)],
                _ => unreachable!(),
            };

            for (record, expected) in passengers.records().iter().zip(counts) {
                assert_eq!(
                    (
                        record.waiting,
                        record.onboard,
                        record.arrived,
                        record.unserved
                    ),
                    expected
                );
                assert_eq!(
                    record.waiting + record.onboard + record.arrived + record.unserved,
                    record.demand.amount
                );
            }
        }

        let completed = passengers.clone();

        for _ in 0..2 {
            assert_eq!(vehicle.advance(&mut clock, &mut passengers), Ok(Complete));
            assert_eq!(clock.tick(), 10);
            assert_eq!(passengers, completed);
        }
    }
}
