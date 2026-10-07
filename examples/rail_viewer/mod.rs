use std::fmt::Write;

use urbanflow::analysis::{self, AnalysisError, OperationalAnalysis};
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

/// Embeds a world, its recorded trace, and completed-run analysis in offline HTML.
///
/// Inputs come from the same valid scenario. The fixed JSON schema contains
/// only numbers and enum labels, so no caller-supplied text needs escaping.
/// IDs and u64 values are strings to preserve precision in JavaScript; per-demand
/// counts fit exactly in JavaScript numbers. Undefined analysis values use null.
/// Partial traces and completed traces missing their initial service state have
/// no analysis, but retain their snapshots for playback and inspection. Other
/// `analysis::analyze` errors are returned before producing HTML.
/// All metrics come from the core.
pub fn render(world: &World, trace: &RailTrace) -> Result<String, AnalysisError> {
    let analysis = match trace
        .completed
        .then(|| analysis::analyze(trace))
        .transpose()
    {
        Ok(analysis) => analysis,
        Err(AnalysisError::MissingInitialState) => None,
        Err(error) => return Err(error),
    };
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

    data.push_str("]},\"analysis\":");

    if let Some(analysis) = analysis {
        write_analysis(&mut data, &analysis);
    } else {
        data.push_str("null");
    }

    data.push('}');

    Ok(include_str!("viewer.html").replace("__TRACE_DATA__", &data))
}

fn write_analysis(data: &mut String, analysis: &OperationalAnalysis) {
    let optional = |value: Option<f64>| value.map_or_else(|| "null".into(), |v| v.to_string());
    let outcomes = analysis.passenger_outcomes;

    write!(
        data,
        r#"{{"passenger_outcomes":{{"requested":"{}","arrived":"{}","unserved":"{}","served_share":{}}}"#,
        outcomes.requested, outcomes.arrived, outcomes.unserved, optional(outcomes.served_share),
    ).unwrap();

    let times = analysis.passenger_times;

    write!(
        data,
        r#","passenger_times":{{"waiting_passenger_ticks":"{}","onboard_passenger_ticks":"{}","arrived_waiting_passenger_ticks":"{}","arrived_onboard_passenger_ticks":"{}","mean_waiting_ticks":{},"mean_onboard_ticks":{},"mean_journey_ticks":{}}}"#,
        times.waiting_passenger_ticks, times.onboard_passenger_ticks,
        times.arrived_waiting_passenger_ticks, times.arrived_onboard_passenger_ticks,
        optional(times.mean_waiting_ticks), optional(times.mean_onboard_ticks), optional(times.mean_journey_ticks),
    ).unwrap();

    let occupancy = analysis.vehicle_occupancy;

    write!(
        data,
        r#","vehicle_occupancy":{{"capacity":{},"max_occupancy":{},"occupied_passenger_ticks":"{}","mean_occupancy":{},"max_load_factor":{},"mean_load_factor":{}}}"#,
        occupancy.capacity, occupancy.max_occupancy, occupancy.occupied_passenger_ticks,
        optional(occupancy.mean_occupancy), optional(occupancy.max_load_factor), optional(occupancy.mean_load_factor),
    ).unwrap();

    data.push_str(",\"stops\":[");

    for (index, stop) in analysis.stops.iter().enumerate() {
        if index > 0 {
            data.push(',');
        }

        write!(
            data,
            r#"{{"stop_index":"{}","node":"{}","boarded":"{}","alighted":"{}","remaining_waiting":"{}"}}"#,
            stop.stop_index, stop.node.0, stop.boarded, stop.alighted, stop.remaining_waiting,
        ).unwrap();
    }

    let timing = analysis.route_timing;

    write!(
        data,
        r#"],"route_timing":{{"elapsed_ticks":"{}","traveling_ticks":"{}","dwelling_ticks":"{}","completion_tick":"{}"}}}}"#,
        timing.elapsed_ticks, timing.traveling_ticks, timing.dwelling_ticks, timing.completion_tick,
    ).unwrap();
}
