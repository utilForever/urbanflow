#[path = "../examples/rail_viewer/mod.rs"]
mod rail_viewer;

use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailRoute, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, Node, NodeId, World};

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
    trace.snapshots[3].tick = u64::MAX;

    let html = rail_viewer::render(&world, &trace);
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
    assert!(data.contains(r#""occupancy":2,"capacity":2"#));
    assert!(data.contains(r#""passengers":[{"from":"7","to":"3","amount":3,"waiting":0,"onboard":0,"arrived":2,"unserved":1}]"#));
    assert!(!html.contains("<script src="));
    assert!(!html.contains("<link "));
}

#[test]
fn demo_html_is_repeatable() {
    let (world, trace) = rail_viewer::scenario();

    assert!(trace.completed);

    let html = rail_viewer::render(&world, &trace);
    let (repeated_world, repeated_trace) = rail_viewer::scenario();

    assert_eq!(html, rail_viewer::render(&repeated_world, &repeated_trace));
}
