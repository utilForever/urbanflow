use urbanflow::demand::Demand;
use urbanflow::rail::{
    RailPassengerError, RailPassengers, RailRoute, RailStepError, RailVehicle, RailVehicleState,
};
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

#[test]
fn one_tick_durations_do_not_skip_travel_or_add_a_final_dwell() {
    let mut vehicle = vehicle(&[0, 1], 1, 1, 1);
    let mut clock = SimulationClock::default();
    let mut passengers = RailPassengers::new(&[]);

    assert_eq!(
        vehicle.advance(&mut clock, &mut passengers),
        Ok(RailVehicleState::Traveling {
            edge_index: 0,
            travel_ticks_elapsed: 0
        })
    );
    assert_eq!(clock.tick(), 1);
    assert_eq!(
        vehicle.advance(&mut clock, &mut passengers),
        Ok(RailVehicleState::Complete)
    );
    assert_eq!(clock.tick(), 2);
    assert!(passengers.records().is_empty());
}

#[test]
fn repeated_nodes_process_passengers_once_per_visit() {
    let mut vehicle = vehicle(&[0, 1, 0, 2], 2, 1, 2);
    let mut clock = SimulationClock::default();
    let mut passengers = RailPassengers::new(&[
        Demand::new(NodeId(0), NodeId(0), 2),
        Demand::new(NodeId(0), NodeId(2), 1),
    ]);

    for tick in 1..=9 {
        vehicle.advance(&mut clock, &mut passengers).unwrap();

        assert_eq!(
            passengers.records()[0].arrived,
            if tick < 6 { 0 } else { 2 }
        );
        assert_eq!(
            passengers.records()[1].onboard,
            if (6..9).contains(&tick) { 1 } else { 0 }
        );
    }

    assert_eq!(vehicle.state(), RailVehicleState::Complete);
    assert_eq!(passengers.records()[1].arrived, 1);
}

#[test]
fn stop_errors_preserve_vehicle_clock_and_passengers() {
    for (capacity, amounts, error) in [
        (1, [1, 1], RailPassengerError::CapacityExceeded),
        (u32::MAX, [u32::MAX, 1], RailPassengerError::CountOverflow),
    ] {
        // Reject invalid occupancy both before boarding and on final arrival.
        for at_arrival in [false, true] {
            let mut vehicle = vehicle(&[0, 1], capacity, 1, 1);
            let mut clock = SimulationClock::default();
            let mut passengers = RailPassengers::new(&[
                Demand::new(NodeId(0), NodeId(1), amounts[0]),
                Demand::new(NodeId(0), NodeId(1), amounts[1]),
            ]);

            if at_arrival {
                vehicle.advance(&mut clock, &mut passengers).unwrap();
            } else {
                passengers.board(0, amounts[0]).unwrap();
            }

            passengers.board(1, amounts[1]).unwrap();

            let before = (vehicle.clone(), clock.tick(), passengers.clone());

            assert_eq!(
                vehicle.advance(&mut clock, &mut passengers),
                Err(RailStepError::Passengers(error))
            );
            assert_eq!(
                (&vehicle, clock.tick(), &passengers),
                (&before.0, before.1, &before.2)
            );

            passengers.alight(1, amounts[1]).unwrap();

            assert!(vehicle.advance(&mut clock, &mut passengers).is_ok());
            assert_eq!(clock.tick(), before.1 + 1);
        }
    }
}
