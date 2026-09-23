use std::fmt::Write;

use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailPosition, RailRoute, RailTrace, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, Node, NodeId, World};

/// A small recorded service; all simulation decisions stay in the core API.
pub fn scenario() -> (World, RailTrace) {
    let mut network = Network::new();

    for (from, to) in [(8, 3), (3, 8), (5, 8), (8, 21)] {
        network
            .add_edge(NodeId(from), NodeId(to), EdgeKind::Road)
            .expect("demo edges are valid");
    }

    let edges = [(8, 3), (3, 21), (21, 5)]
        .map(|(from, to)| {
            network
                .add_edge(NodeId(from), NodeId(to), EdgeKind::Rail)
                .expect("demo edges are valid")
        })
        .to_vec();
    let route = RailRoute::new(&network, edges).expect("demo route is valid");

    let mut vehicle = RailVehicle::new(route, 6, 4, 2).expect("demo settings are positive");
    let mut passengers = RailPassengers::new(&[
        Demand::new(NodeId(8), NodeId(21), 8),
        Demand::new(NodeId(3), NodeId(5), 3),
    ]);

    let trace = vehicle
        .record_trace(&mut SimulationClock::default(), &mut passengers, 32)
        .expect("bounded demo service succeeds");
    let world = World {
        nodes: [8, 3, 21, 5].map(|id| Node { id: NodeId(id) }).to_vec(),
        network,
    };

    (world, trace)
}

/// Embeds a world and its recorded trace in an offline HTML document.
///
/// Inputs come from the same valid scenario. The fixed JSON schema contains
/// only integers and enum labels, so no caller-supplied text needs escaping.
/// IDs and u64 values are strings to preserve precision in JavaScript; passenger
/// counts fit exactly in JavaScript numbers. No serialization dependency or
/// browser-side simulation is needed.
pub fn render(world: &World, trace: &RailTrace) -> String {
    let mut data = String::from("{\"nodes\":[");

    for (index, node) in world.nodes.iter().enumerate() {
        if index > 0 {
            data.push(',');
        }

        write!(data, "\"{}\"", node.id.0).unwrap();
    }

    data.push_str("],\"edges\":[");

    for (index, edge) in world.network.edges().iter().enumerate() {
        if index > 0 {
            data.push(',');
        }

        let kind = match edge.kind {
            EdgeKind::Road => "Road",
            EdgeKind::Rail => "Rail",
        };
        write!(
            data,
            r#"{{"id":"{}","from":"{}","to":"{}","kind":"{kind}"}}"#,
            edge.id.0, edge.from.0, edge.to.0
        )
        .unwrap();
    }

    write!(
        data,
        "],\"trace\":{{\"completed\":{},\"snapshots\":[",
        trace.completed
    )
    .unwrap();

    for (index, snapshot) in trace.snapshots.iter().enumerate() {
        if index > 0 {
            data.push(',');
        }

        write!(data, "{{\"tick\":\"{}\",\"position\":", snapshot.tick).unwrap();

        match snapshot.position {
            RailPosition::AtStop { stop_index, node, dwell_ticks_remaining } => write!(
                data,
                r#"{{"kind":"AtStop","stop_index":"{stop_index}","node":"{}","dwell_ticks_remaining":"{dwell_ticks_remaining}"}}"#,
                node.0
            ),
            RailPosition::Traveling { edge_index, edge, from, to, travel_ticks_elapsed, travel_ticks_total } => write!(
                data,
                r#"{{"kind":"Traveling","edge_index":"{edge_index}","edge":"{}","from":"{}","to":"{}","travel_ticks_elapsed":"{travel_ticks_elapsed}","travel_ticks_total":"{travel_ticks_total}"}}"#,
                edge.0, from.0, to.0
            ),
            RailPosition::Complete { stop_index, node } => write!(
                data,
                r#"{{"kind":"Complete","stop_index":"{stop_index}","node":"{}"}}"#,
                node.0
            ),
        }.unwrap();

        write!(
            data,
            ",\"occupancy\":{},\"capacity\":{},\"passengers\":[",
            snapshot.occupancy, snapshot.capacity
        )
        .unwrap();

        for (index, passenger) in snapshot.passengers.iter().enumerate() {
            if index > 0 {
                data.push(',');
            }

            write!(
                data,
                r#"{{"from":"{}","to":"{}","amount":{},"waiting":{},"onboard":{},"arrived":{},"unserved":{}}}"#,
                passenger.demand.origin.0, passenger.demand.destination.0, passenger.demand.amount,
                passenger.waiting, passenger.onboard, passenger.arrived, passenger.unserved
            ).unwrap();
        }

        data.push_str("]}");
    }

    data.push_str("]}}");
    include_str!("viewer.html").replace("__TRACE_DATA__", &data)
}
