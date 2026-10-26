#[path = "../results/mod.rs"]
pub mod results;

use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailRoute, RailTrace, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, Node, NodeId, World};

/// A small recorded service; all simulation decisions stay in the core API.
///
/// Route `8 -> 3 -> 21 -> 5`, capacity 6, four travel ticks per edge, and two
/// dwell ticks before departure. Records ticks 0 through 18: six of the eight
/// passengers at node 8 arrive at node 21; the other two and all three at node 3
/// become unserved. Final arrival adds no dwell.
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
/// Uses the versioned result adapter and escapes JSON for an HTML script element.
/// Invalid topology, snapshots, or complete-trace analysis fail before rendering.
/// IDs and u64 values are strings to preserve precision in JavaScript; per-demand
/// counts fit exactly in JavaScript numbers. Undefined analysis values use null.
/// Partial traces and completed traces missing their initial service state have
/// no analysis, but retain their snapshots for playback and inspection. Other
/// analysis errors are returned before producing HTML.
/// All metrics come from the core.
///
/// The template lays out nodes clockwise in stored order and separates parallel
/// and reverse edges. Road edges provide context only. Playback starts paused;
/// at 1x each tick takes one second. Pause and speed changes account for elapsed
/// time first; seeking and resetting pause. Complete and partial traces stop at
/// their final frame, and fewer than two frames cannot play.
///
/// Only the marker interpolates. Tick labels and lifecycle counts use the latest
/// reached snapshot, with `BigInt` totals across all demand records. Empty traces
/// show unavailable status; narrow screens place status below the network.
/// Analysis tables render once in route-visit order, retaining repeated nodes.
/// Means use two decimal places, shares/load factors use percentages, undefined
/// values use `N/A`, and integers keep their full precision.
pub fn render(world: &World, trace: &RailTrace) -> Result<String, results::ResultError> {
    let data = results::encode(world, trace)?;
    Ok(include_str!("viewer.html").replace("__TRACE_DATA__", &escape_json(&data)))
}

// Script elements parse HTML even when their type is application/json.
fn escape_json(json: &str) -> String {
    json.replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedded_json_cannot_close_its_script_element() {
        let value =
            serde_json::json!({"text": "</script><script>alert('&')</script>\u{2028}\u{2029}"});
        let escaped = super::escape_json(&value.to_string());

        assert!(!escaped.contains(['<', '>', '&', '\u{2028}', '\u{2029}']));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&escaped).unwrap(),
            value
        );
    }
}
