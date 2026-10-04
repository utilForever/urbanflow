#[path = "../examples/rail_viewer/mod.rs"]
mod rail_viewer;

use urbanflow::analysis::{
    AnalysisError, OperationalAnalysis, PassengerOutcomes, PassengerTimes, RouteTiming,
    StopActivity, VehicleOccupancy, analyze,
};
use urbanflow::demand::Demand;
use urbanflow::rail::{
    RailPassengers, RailPosition, RailRoute, RailTrace, RailVehicle, RailVehicleState,
};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeId, EdgeKind, Network, Node, NodeId, World};

fn embedded_data(html: &str) -> &str {
    html.split_once("<script id=\"trace-data\" type=\"application/json\">")
        .unwrap()
        .1
        .split_once("</script>")
        .unwrap()
        .0
        .trim()
}

#[test]
fn fixed_route_service_reaches_the_viewer_end_to_end() {
    let mut network = Network::new();
    network
        .add_edge(NodeId(8), NodeId(21), EdgeKind::Road)
        .unwrap();

    let edges = [(8, 3), (3, 21)]
        .map(|(from, to)| {
            network
                .add_edge(NodeId(from), NodeId(to), EdgeKind::Rail)
                .unwrap()
        })
        .to_vec();
    let route = RailRoute::new(&network, edges).unwrap();
    let world = World {
        nodes: [8, 3, 21].map(|id| Node { id: NodeId(id) }).to_vec(),
        network,
    };

    let mut vehicle = RailVehicle::new(route, 2, 2, 1).unwrap();
    let mut clock = SimulationClock::default();
    // The middle-stop demand comes first: arrivals must free capacity before
    // boarding it. At the origin, earlier demand takes the two available seats.
    let demands = [
        Demand::new(NodeId(3), NodeId(21), 2),
        Demand::new(NodeId(8), NodeId(3), 3),
        Demand::new(NodeId(8), NodeId(21), 1),
    ];
    let mut passengers = RailPassengers::new(&demands);

    let initial = vehicle.snapshot(&clock, &passengers).unwrap();
    let trace = vehicle
        .record_trace(&mut clock, &mut passengers, 20)
        .unwrap();
    let positions = [
        RailPosition::AtStop {
            stop_index: 0,
            node: NodeId(8),
            dwell_ticks_remaining: 1,
        },
        RailPosition::Traveling {
            edge_index: 0,
            edge: EdgeId(1),
            from: NodeId(8),
            to: NodeId(3),
            travel_ticks_elapsed: 0,
            travel_ticks_total: 2,
        },
        RailPosition::Traveling {
            edge_index: 0,
            edge: EdgeId(1),
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
            edge: EdgeId(2),
            from: NodeId(3),
            to: NodeId(21),
            travel_ticks_elapsed: 0,
            travel_ticks_total: 2,
        },
        RailPosition::Traveling {
            edge_index: 1,
            edge: EdgeId(2),
            from: NodeId(3),
            to: NodeId(21),
            travel_ticks_elapsed: 1,
            travel_ticks_total: 2,
        },
        RailPosition::Complete {
            stop_index: 2,
            node: NodeId(21),
        },
    ];

    assert!(trace.completed);
    assert_eq!(trace.snapshots.len(), positions.len());
    assert_eq!(trace.snapshots[0], initial);

    for (tick, (snapshot, position)) in trace.snapshots.iter().zip(positions).enumerate() {
        assert_eq!(snapshot.tick, tick as u64);
        assert_eq!(snapshot.position, position, "position at tick {tick}");
        assert_eq!(snapshot.capacity, 2);
        assert_eq!(snapshot.occupancy, [0, 2, 2, 2, 2, 2, 0][tick]);

        // Counts are (waiting, onboard, arrived, unserved), in demand order.
        let counts = match tick {
            0 => [(2, 0, 0, 0), (3, 0, 0, 0), (1, 0, 0, 0)],
            1..=2 => [(2, 0, 0, 0), (1, 2, 0, 0), (1, 0, 0, 0)],
            3..=5 => [(0, 2, 0, 0), (1, 0, 2, 0), (1, 0, 0, 0)],
            6 => [(0, 0, 2, 0), (0, 0, 2, 1), (0, 0, 0, 1)],
            _ => unreachable!(),
        };

        assert_eq!(snapshot.passengers.len(), demands.len());

        for ((record, demand), counts) in snapshot.passengers.iter().zip(demands).zip(counts) {
            assert_eq!(record.demand, demand);
            assert_eq!(
                (
                    record.waiting,
                    record.onboard,
                    record.arrived,
                    record.unserved
                ),
                counts,
                "passengers at tick {tick}: {demand:?}"
            );
        }
    }

    // The generous recording limit must stop at final arrival, with no extra
    // dwell or frames; advancing after completion must preserve the final state.
    assert_eq!(clock.tick(), 6);
    assert_eq!(vehicle.state(), RailVehicleState::Complete);
    assert_eq!(
        vehicle.advance(&mut clock, &mut passengers).unwrap(),
        RailVehicleState::Complete
    );
    assert_eq!(
        trace.snapshots.last().unwrap(),
        &vehicle.snapshot(&clock, &passengers).unwrap()
    );

    let before = trace.clone();
    let analysis = analyze(&trace).unwrap();

    // Two passengers board at tick 1 and arrive at 3; two board at 3 and
    // arrive at 6, including one onboard dwell tick. Two wait all six ticks.
    assert_eq!(
        analysis,
        OperationalAnalysis {
            passenger_outcomes: PassengerOutcomes {
                requested: 6,
                arrived: 4,
                unserved: 2,
                served_share: Some(4.0 / 6.0),
            },
            passenger_times: PassengerTimes {
                waiting_passenger_ticks: 20,
                onboard_passenger_ticks: 10,
                arrived_waiting_passenger_ticks: 8,
                arrived_onboard_passenger_ticks: 10,
                mean_waiting_ticks: Some(2.0),
                mean_onboard_ticks: Some(2.5),
                mean_journey_ticks: Some(4.5),
            },
            vehicle_occupancy: VehicleOccupancy {
                capacity: 2,
                max_occupancy: 2,
                occupied_passenger_ticks: 10,
                mean_occupancy: Some(10.0 / 6.0),
                max_load_factor: Some(1.0),
                mean_load_factor: Some(5.0 / 6.0),
            },
            stops: vec![
                StopActivity {
                    stop_index: 0,
                    node: NodeId(8),
                    boarded: 2,
                    alighted: 0,
                    remaining_waiting: 2,
                },
                StopActivity {
                    stop_index: 1,
                    node: NodeId(3),
                    boarded: 2,
                    alighted: 2,
                    remaining_waiting: 0,
                },
                StopActivity {
                    stop_index: 2,
                    node: NodeId(21),
                    boarded: 0,
                    alighted: 2,
                    remaining_waiting: 0,
                },
            ],
            route_timing: RouteTiming {
                elapsed_ticks: 6,
                traveling_ticks: 4,
                dwelling_ticks: 2,
                completion_tick: 6,
            },
        }
    );

    let html = rail_viewer::render(&world, &trace).unwrap();
    // This hand-checked fixture covers the entire ordered payload, not just
    // isolated fields. Its schema contains no whitespace inside string values.
    let expected = include_str!("fixtures/rail_service.json")
        .split_ascii_whitespace()
        .collect::<String>();

    assert_eq!(embedded_data(&html), expected);
    assert!(!html.contains("__TRACE_DATA__"));

    for control in ["play", "pause", "reset", "speed", "snapshot"] {
        assert!(
            html.contains(&format!("id=\"{control}\"")),
            "missing viewer control: {control}"
        );
    }

    assert_eq!(trace, before);
}

#[test]
fn viewer_embeds_ordered_network_and_core_positions_without_losing_integer_precision() {
    let mut network = Network::new();
    network
        .add_edge(NodeId(7), NodeId(3), EdgeKind::Road)
        .unwrap();

    let edge = network
        .add_edge(NodeId(7), NodeId(3), EdgeKind::Rail)
        .unwrap();

    let mut vehicle =
        RailVehicle::new(RailRoute::new(&network, vec![edge]).unwrap(), 2, 2, 1).unwrap();
    let world = World {
        nodes: [7, 3, usize::MAX]
            .map(|id| Node { id: NodeId(id) })
            .to_vec(),
        network,
    };
    let mut passengers = RailPassengers::new(&[Demand::new(NodeId(7), NodeId(3), 3)]);
    let mut trace = vehicle
        .record_trace(&mut SimulationClock::default(), &mut passengers, 3)
        .unwrap();

    // Presentation must preserve large integer labels as well as sparse IDs.
    for (index, snapshot) in trace.snapshots.iter_mut().enumerate() {
        snapshot.tick = u64::MAX - 3 + index as u64;
    }

    let html = rail_viewer::render(&world, &trace).unwrap();
    let data = embedded_data(&html);

    assert!(data.starts_with(&format!(
        "{{\"nodes\":[\"7\",\"3\",\"{}\"],\"edges\":[{{\"id\":\"0\",\"from\":\"7\",\"to\":\"3\",\"kind\":\"Road\"}},{{\"id\":\"1\",\"from\":\"7\",\"to\":\"3\",\"kind\":\"Rail\"}}],\"trace\":{{\"completed\":true,\"snapshots\":[",
        usize::MAX
    )));

    for position in [
        r#""position":{"kind":"AtStop","stop_index":"0","node":"7","dwell_ticks_remaining":"1"}"#,
        r#""position":{"kind":"Traveling","edge_index":"0","edge":"1","from":"7","to":"3","travel_ticks_elapsed":"0","travel_ticks_total":"2"}"#,
        r#""position":{"kind":"Traveling","edge_index":"0","edge":"1","from":"7","to":"3","travel_ticks_elapsed":"1","travel_ticks_total":"2"}"#,
        r#""position":{"kind":"Complete","stop_index":"1","node":"3"}"#,
    ] {
        assert!(data.contains(position), "missing {position}");
    }

    assert_eq!(data.matches("\"tick\":").count(), 4);
    assert!(data.contains("\"tick\":\"18446744073709551615\""));
    assert!(data.contains("\"completion_tick\":\"18446744073709551615\""));
    assert!(data.contains(r#""occupancy":2,"capacity":2"#));
    assert!(data.contains(r#""passengers":[{"from":"7","to":"3","amount":3,"waiting":0,"onboard":0,"arrived":2,"unserved":1}]"#));
    assert!(!html.contains("<script src="));
    assert!(!html.contains("<link "));
}

#[test]
fn demo_html_is_repeatable_and_preserves_partial_and_empty_traces() {
    let (world, trace) = rail_viewer::scenario();

    assert!(trace.completed);

    let html = rail_viewer::render(&world, &trace).unwrap();
    let (repeated_world, repeated_trace) = rail_viewer::scenario();

    assert_eq!(
        html,
        rail_viewer::render(&repeated_world, &repeated_trace).unwrap()
    );

    for snapshots in [vec![], vec![trace.snapshots[0].clone()]] {
        let partial = RailTrace {
            snapshots,
            completed: false,
        };
        let html = rail_viewer::render(&world, &partial).unwrap();
        let data = embedded_data(&html);

        assert!(data.contains("\"completed\":false"));
        assert!(data.ends_with("\"analysis\":null}"));
        assert_eq!(data.matches("\"tick\":").count(), partial.snapshots.len());
    }
}

#[test]
fn viewer_preserves_undefined_analysis_and_repeated_stop_visits() {
    let mut network = Network::new();
    let edges = [(7, 3), (3, 7)].map(|(from, to)| {
        network
            .add_edge(NodeId(from), NodeId(to), EdgeKind::Rail)
            .unwrap()
    });
    let mut vehicle =
        RailVehicle::new(RailRoute::new(&network, edges.to_vec()).unwrap(), 2, 1, 1).unwrap();
    let trace = vehicle
        .record_trace(
            &mut SimulationClock::default(),
            &mut RailPassengers::new(&[]),
            4,
        )
        .unwrap();
    let world = World {
        nodes: [7, 3].map(|id| Node { id: NodeId(id) }).to_vec(),
        network,
    };
    let html = rail_viewer::render(&world, &trace).unwrap();
    let data = embedded_data(&html);

    assert!(data.contains(
        r#""passenger_outcomes":{"requested":"0","arrived":"0","unserved":"0","served_share":null}"#
    ));
    assert!(data.contains(
        r#""mean_waiting_ticks":null,"mean_onboard_ticks":null,"mean_journey_ticks":null"#
    ));
    assert!(data.contains(r#""mean_occupancy":0,"max_load_factor":0,"mean_load_factor":0"#));
    assert!(data.contains(r#""stops":[{"stop_index":"0","node":"7","boarded":"0","alighted":"0","remaining_waiting":"0"},{"stop_index":"1","node":"3","boarded":"0","alighted":"0","remaining_waiting":"0"},{"stop_index":"2","node":"7","boarded":"0","alighted":"0","remaining_waiting":"0"}]"#));
}

#[test]
fn viewer_preserves_resumed_and_completion_only_recordings_without_analysis() {
    let mut network = Network::new();
    let edge = network
        .add_edge(NodeId(7), NodeId(3), EdgeKind::Rail)
        .unwrap();
    let mut vehicle =
        RailVehicle::new(RailRoute::new(&network, vec![edge]).unwrap(), 2, 2, 1).unwrap();
    let world = World {
        nodes: [7, 3].map(|id| Node { id: NodeId(id) }).to_vec(),
        network,
    };
    let mut clock = SimulationClock::default();
    let mut passengers = RailPassengers::new(&[Demand::new(NodeId(7), NodeId(3), 3)]);

    vehicle.advance(&mut clock, &mut passengers).unwrap();

    for expected_ticks in [&[1, 2, 3][..], &[3][..]] {
        let trace = vehicle
            .record_trace(&mut clock, &mut passengers, 3)
            .unwrap();

        assert!(trace.completed);

        let before = trace.clone();
        let html = rail_viewer::render(&world, &trace).unwrap();
        let data = embedded_data(&html);

        assert_eq!(trace, before);
        assert!(data.contains(r#""completed":true"#));
        assert!(data.ends_with(r#""analysis":null}"#));
        assert_eq!(data.matches("\"tick\":").count(), expected_ticks.len());

        for tick in expected_ticks {
            assert!(data.contains(&format!(r#""tick":"{tick}""#)));
        }

        assert!(data.contains(r#""waiting":0,"onboard":0,"arrived":2,"unserved":1"#));

        let mut invalid = trace;
        invalid.snapshots[0].occupancy = 3;

        assert_eq!(
            rail_viewer::render(&world, &invalid),
            Err(AnalysisError::InvalidOccupancy { snapshot_index: 0 }),
        );
    }
}

#[test]
fn viewer_propagates_completed_trace_analysis_errors_without_mutation() {
    let (world, mut trace) = rail_viewer::scenario();
    trace.snapshots[1].occupancy = 0;

    let before = trace.clone();

    assert_eq!(
        rail_viewer::render(&world, &trace),
        Err(AnalysisError::InvalidOccupancy { snapshot_index: 1 }),
    );
    assert_eq!(trace, before);
}
