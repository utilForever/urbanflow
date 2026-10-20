#[path = "../examples/rail_viewer/mod.rs"]
pub mod rail_viewer;

use std::path::PathBuf;

use rail_viewer::results::{RailResult, encode, load, parse, save};
use rstest::rstest;
use serde_json::{Value, json};
use urbanflow::analysis::analyze;
use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailPosition, RailRoute, RailTrace, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, Node, NodeId, World};

const COMPLETE: &str = include_str!("fixtures/rail_service.json");
const EMPTY: &str =
    r#"{"version":1,"nodes":[],"edges":[],"trace":{"completed":false,"snapshots":[]}}"#;

fn temporary_file(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "urbanflow-result-{}-{name}.json",
        std::process::id()
    ))
}

fn round_trip(world: &World, trace: &RailTrace) -> RailResult {
    let json = encode(world, trace).unwrap();
    let restored = parse("round-trip.json", &json).unwrap();

    assert_eq!(restored.world.nodes, world.nodes);
    assert_eq!(restored.world.network.edges(), world.network.edges());
    assert_eq!(&restored.trace, trace);
    assert_eq!(restored.analysis, analyze(trace).ok());
    assert_eq!(encode(&restored.world, &restored.trace).unwrap(), json);
    assert_eq!(
        rail_viewer::render(&restored.world, &restored.trace).unwrap(),
        rail_viewer::render(world, trace).unwrap(),
    );

    restored
}

fn rejection(json: &str, expected: &str) {
    let error = parse("invalid.json", json).unwrap_err();

    assert_eq!(error.file, PathBuf::from("invalid.json"));
    assert!(
        error.to_string().contains(expected),
        "{error} lacks {expected:?}"
    );
}

#[rstest]
#[case::complete(COMPLETE, true, true)]
#[case::partial(include_str!("fixtures/results/partial.json"), false, false)]
#[case::completion_only(include_str!("fixtures/results/completion-only.json"), true, false)]
#[case::empty(EMPTY, false, false)]
fn fixtures_round_trip(#[case] input: &str, #[case] completed: bool, #[case] has_analysis: bool) {
    let restored = parse("fixture.json", input).unwrap();

    assert_eq!(restored.trace.completed, completed);
    assert_eq!(restored.analysis.is_some(), has_analysis);
    round_trip(&restored.world, &restored.trace);
}

#[test]
fn full_fixture_matches_the_shared_viewer_payload_exactly() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rail_service.json");
    let restored = load(path).unwrap();

    assert_eq!(
        encode(&restored.world, &restored.trace).unwrap(),
        COMPLETE.split_ascii_whitespace().collect::<String>(),
    );
}

#[test]
fn demo_and_all_recording_prefixes_and_resumed_histories_round_trip() {
    let (world, trace) = rail_viewer::scenario();

    for end in 0..=trace.snapshots.len() {
        for start in [0, end.saturating_sub(1)] {
            let recording = RailTrace {
                snapshots: trace.snapshots[start..end].to_vec(),
                completed: end == trace.snapshots.len(),
            };

            round_trip(&world, &recording);
        }
    }

    let resumed = RailTrace {
        snapshots: trace.snapshots[1..].to_vec(),
        completed: true,
    };

    assert!(round_trip(&world, &resumed).analysis.is_none());
}

#[test]
fn full_width_values_duplicate_demands_and_repeated_visits_survive() {
    let mut network = Network::new();
    network
        .add_edge(NodeId(usize::MAX), NodeId(3), EdgeKind::Road)
        .unwrap();

    let edges = [(usize::MAX, 3), (3, usize::MAX)].map(|(from, to)| {
        network
            .add_edge(NodeId(from), NodeId(to), EdgeKind::Rail)
            .unwrap()
    });
    let world = World {
        nodes: [usize::MAX, 3].map(|id| Node { id: NodeId(id) }).to_vec(),
        network,
    };
    let mut vehicle = RailVehicle::new(
        RailRoute::new(&world.network, edges.to_vec()).unwrap(),
        u32::MAX,
        1,
        1,
    )
    .unwrap();
    let demand = Demand::new(NodeId(usize::MAX), NodeId(3), u32::MAX);
    let mut passengers = RailPassengers::new(&[
        demand,
        demand,
        Demand::new(NodeId(3), NodeId(usize::MAX), 0),
    ]);
    let mut clock = SimulationClock::default();
    let mut trace = vehicle
        .record_trace(&mut clock, &mut passengers, 4)
        .unwrap();

    for snapshot in &mut trace.snapshots {
        snapshot.tick += u64::MAX - 4;
    }

    let restored = round_trip(&world, &trace);
    let analysis = restored.analysis.unwrap();

    assert_eq!(
        analysis.passenger_outcomes.requested,
        2 * u64::from(u32::MAX)
    );
    assert_eq!(analysis.route_timing.completion_tick, u64::MAX);
    assert_eq!(analysis.stops[0].node, analysis.stops[2].node);
    assert_eq!(analysis.stops[2].stop_index, 2);

    let mut partial = RailTrace {
        snapshots: vec![trace.snapshots[0].clone()],
        completed: false,
    };
    partial.snapshots[0].position = RailPosition::AtStop {
        stop_index: usize::MAX,
        node: NodeId(usize::MAX),
        dwell_ticks_remaining: u64::MAX,
    };

    round_trip(&world, &partial);

    partial.snapshots[0] = trace.snapshots[1].clone();
    partial.snapshots[0].tick = u64::MAX;
    partial.snapshots[0].position = RailPosition::Traveling {
        edge_index: usize::MAX,
        edge: edges[0],
        from: NodeId(usize::MAX),
        to: NodeId(3),
        travel_ticks_elapsed: u64::MAX - 1,
        travel_ticks_total: u64::MAX,
    };

    round_trip(&world, &partial);
}

#[rstest]
#[case::absent(None)]
#[case::null(Some(Value::Null))]
#[case::forged(Some(json!({"passenger_outcomes": {"arrived": "999"}, "text": "</script><script>boom()</script>"})))]
fn imported_analysis_is_always_recomputed(#[case] cache: Option<Value>) {
    let mut value: Value = serde_json::from_str(COMPLETE).unwrap();

    if let Some(cache) = cache {
        value["analysis"] = cache;
    } else {
        value.as_object_mut().unwrap().remove("analysis");
    }

    let restored = parse("cached.json", &value.to_string()).unwrap();

    assert_eq!(restored.analysis, Some(analyze(&restored.trace).unwrap()));
    assert!(
        !rail_viewer::render(&restored.world, &restored.trace)
            .unwrap()
            .contains("boom()")
    );
}

#[test]
fn malformed_documents_and_missing_fields_are_rejected() {
    for input in ["", "[]", "null", "{", &format!("{COMPLETE} trailing")] {
        rejection(input, "JSON");
    }

    for field in ["version", "nodes", "edges", "trace"] {
        let mut value: Value = serde_json::from_str(COMPLETE).unwrap();
        value.as_object_mut().unwrap().remove(field);

        rejection(&value.to_string(), "missing field");
    }

    rejection(
        r#"{"version":999,"trace":"future shape"}"#,
        "unsupported version 999",
    );
}

#[test]
fn invalid_analysis_never_overwrites_a_saved_result_or_html() {
    let (world, trace) = rail_viewer::scenario();
    let result_path = temporary_file("preserve");

    save(&result_path, &world, &trace).unwrap();
    assert_eq!(load(&result_path).unwrap().trace, trace);

    let original = std::fs::read(&result_path).unwrap();

    let mut invalid = trace.clone();
    invalid.snapshots[1].occupancy = 0;

    let failure = save(&result_path, &world, &invalid).unwrap_err();

    assert_eq!(failure.file, result_path);
    assert!(failure.detail.contains("occupancy"));
    assert_eq!(std::fs::read(&result_path).unwrap(), original);

    // Exercise the viewer's render-before-write call pattern against a real file.
    let html_path = temporary_file("html");
    std::fs::write(&html_path, rail_viewer::render(&world, &trace).unwrap()).unwrap();

    let original_html = std::fs::read(&html_path).unwrap();
    let write_html = || -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(&html_path, rail_viewer::render(&world, &invalid)?)?;
        Ok(())
    };

    assert!(write_html().is_err());
    assert_eq!(std::fs::read(&html_path).unwrap(), original_html);

    std::fs::remove_file(result_path).unwrap();
    std::fs::remove_file(html_path).unwrap();
}
