#[path = "../examples/rail_viewer/mod.rs"]
pub mod rail_viewer;

use std::path::PathBuf;

use rail_viewer::results::{MAX_BYTES, RailResult, encode, load, parse, save};
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

#[rstest]
#[case::version("/version", json!(2), "version")]
#[case::numeric_id("/nodes/0", json!(8), "nodes")]
#[case::leading_zero("/nodes/0", json!("08"), "decimal string")]
#[case::signed_id("/nodes/0", json!("+8"), "decimal string")]
#[case::id_overflow("/nodes/0", json!("18446744073709551616"), "number too large")]
#[case::duplicate_node("/nodes/1", json!("8"), "DuplicateNode")]
#[case::edge_id("/edges/1/id", json!("0"), "insertion index")]
#[case::edge_kind("/edges/0/kind", json!("Tram"), "expected Road or Rail")]
#[case::kind_object("/edges/0/kind", json!({"Road":null}), "string")]
#[case::unknown_endpoint("/edges/0/from", json!("999"), "UnknownEdgeEndpoint")]
#[case::self_connection("/edges/0/to", json!("8"), "SelfConnection")]
#[case::completion("/trace/completed", json!(false), "Complete must be")]
#[case::empty_complete("/trace/snapshots", json!([]), "empty")]
#[case::position("/trace/snapshots/0/position/kind", json!("Flying"), "unknown variant")]
#[case::node("/trace/snapshots/0/position/node", json!("999"), "unknown node")]
#[case::dwell("/trace/snapshots/0/position/dwell_ticks_remaining", json!("0"), "must be positive")]
#[case::road_position("/trace/snapshots/1/position/edge", json!("0"), "Rail edge")]
#[case::unknown_edge("/trace/snapshots/1/position/edge", json!("999"), "Rail edge")]
#[case::wrong_endpoint("/trace/snapshots/1/position/to", json!("8"), "Rail edge")]
#[case::travel_progress("/trace/snapshots/1/position/travel_ticks_elapsed", json!("2"), "less than")]
#[case::capacity("/trace/snapshots/1/capacity", json!(0), "capacity")]
#[case::occupancy("/trace/snapshots/1/occupancy", json!(0), "occupancy")]
#[case::tick_overflow("/trace/snapshots/0/tick", json!("18446744073709551616"), "number too large")]
#[case::fraction("/trace/snapshots/0/capacity", json!(2.5), "u32")]
#[case::count_overflow("/trace/snapshots/0/passengers/0/amount", json!(4294967296u64), "u32")]
#[case::bad_analysis("/analysis", json!([]), "analysis")]
#[case::edge_array("/edges/0", json!(["0", "8", "21", "Road"]), "expected object")]
#[case::trace_array("/trace", json!([false, []]), "expected object")]
#[case::snapshot_array("/trace/snapshots/0", json!([]), "expected object")]
#[case::passenger_array("/trace/snapshots/0/passengers/0", json!(["3", "21", 2, 2, 0, 0, 0]), "expected object")]
#[case::at_stop_position_array("/trace/snapshots/0/position", json!(["AtStop", "0", "8", "1"]), "expected object")]
#[case::traveling_position_array("/trace/snapshots/1/position", json!(["Traveling", "0", "1", "8", "3", "0", "2"]), "expected object")]
#[case::complete_position_array("/trace/snapshots/6/position", json!(["Complete", "2", "21"]), "expected object")]
fn malformed_fields_are_rejected(
    #[case] pointer: &str,
    #[case] replacement: Value,
    #[case] expected: &str,
) {
    let mut value: Value = serde_json::from_str(COMPLETE).unwrap();
    *value.pointer_mut(pointer).unwrap() = replacement;

    rejection(&value.to_string(), expected);
}

#[rstest]
#[case::root("")]
#[case::edge("/edges/0")]
#[case::trace("/trace")]
#[case::snapshot("/trace/snapshots/0")]
#[case::position("/trace/snapshots/0/position")]
#[case::passenger("/trace/snapshots/0/passengers/0")]
fn unknown_fields_are_rejected(#[case] pointer: &str) {
    let mut value: Value = serde_json::from_str(COMPLETE).unwrap();
    value
        .pointer_mut(pointer)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("unexpected".into(), json!(1));

    rejection(&value.to_string(), "unknown field");
}

#[rstest]
#[case::version("\"version\": 1", "\"version\": 1, \"version\": 1")]
#[case::node("\"node\": \"8\"", "\"node\": \"8\", \"node\": \"8\"")]
#[case::count("\"amount\": 2", "\"amount\": 2, \"amount\": 2")]
fn duplicate_fields_are_rejected(#[case] original: &str, #[case] duplicate: &str) {
    let input = COMPLETE.replacen(original, duplicate, 1);

    assert_ne!(input, COMPLETE);
    rejection(&input, "duplicate field");
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

#[rstest]
#[case::ticks("/trace/snapshots/1/tick", json!("1"), "ticks")]
#[case::capacity("/trace/snapshots/1/capacity", json!(3), "capacity")]
#[case::counts("/trace/snapshots/0/passengers/0/waiting", json!(1), "counts")]
#[case::occupancy("/trace/snapshots/0/occupancy", json!(0), "occupancy")]
#[case::missing_demands("/trace/snapshots/1/passengers", json!([]), "demand")]
#[case::changed_demand("/trace/snapshots/1/passengers/0/to", json!("8"), "demand")]
#[case::unknown_demand("/trace/snapshots/0/passengers/0/to", json!("999"), "unknown node")]
fn partial_recordings_are_checked(
    #[case] pointer: &str,
    #[case] replacement: Value,
    #[case] expected: &str,
) {
    let mut value: Value =
        serde_json::from_str(include_str!("fixtures/results/partial.json")).unwrap();
    *value.pointer_mut(pointer).unwrap() = replacement;

    rejection(&value.to_string(), expected);
}

#[rstest]
#[case::arrival_without_boarding(json!({"waiting": 1, "arrived": 1}))]
#[case::unserved_before_completion(json!({"waiting": 1, "unserved": 1}))]
fn partial_lifecycle_transitions_are_checked(#[case] changes: Value) {
    let mut value: Value =
        serde_json::from_str(include_str!("fixtures/results/partial.json")).unwrap();
    value["trace"]["snapshots"][1]["passengers"][0]
        .as_object_mut()
        .unwrap()
        .extend(changes.as_object().unwrap().clone());

    rejection(&value.to_string(), "invalid lifecycle transition");
}

#[test]
fn byte_limit_applies_before_parsing_and_file_reads_are_bounded() {
    let mut input = EMPTY.to_owned();
    input.extend(std::iter::repeat_n(' ', MAX_BYTES - input.len()));

    parse("at-limit.json", &input).unwrap();
    input.push(' ');

    rejection(&input, "exceeds");

    let path = temporary_file("oversize");

    std::fs::write(&path, input).unwrap();
    assert!(load(&path).unwrap_err().detail.contains("exceeds"));

    std::fs::write(&path, [0xff, 0xfe]).unwrap();
    assert!(load(&path).unwrap_err().detail.contains("utf-8"));

    std::fs::remove_file(&path).unwrap();
    assert_eq!(load(&path).unwrap_err().file, path);

    let (world, trace) = rail_viewer::scenario();
    let missing_parent = path.join("result.json");

    assert_eq!(
        save(&missing_parent, &world, &trace).unwrap_err().file,
        missing_parent
    );
}

#[test]
fn loading_checks_the_canonical_output_limit() {
    let header = r#"{"version":1,"nodes":["0","1"],"edges":[],"trace":{"completed":false,"snapshots":[{"tick":"10000000000000000000","position":{"kind":"AtStop","stop_index":"0","node":"0","dwell_ticks_remaining":"1"},"occupancy":0,"capacity":1000,"passengers":["#;
    let record =
        r#"{"from":"0","to":"1","amount":0,"waiting":0,"onboard":0,"arrived":0,"unserved":0}"#;
    let tail = "]}]}}";
    let count = (MAX_BYTES - header.len() - tail.len() + 1) / (record.len() + 1);
    let mut input = header.to_owned();

    for index in 0..count {
        if index > 0 {
            input.push(',');
        }

        input.push_str(record);
    }

    input.push_str(tail);

    // Adding the omitted analysis:null field uses the remaining 16 bytes.
    assert_eq!(input.len(), MAX_BYTES - 16);

    let loaded = parse("at-limit.json", &input).unwrap();
    let encoded = encode(&loaded.world, &loaded.trace).unwrap();

    assert_eq!(encoded.len(), MAX_BYTES);
    assert!(rail_viewer::render(&loaded.world, &loaded.trace).is_ok());

    let input = input.replacen("\"capacity\":1000", "\"capacity\":10000", 1);

    assert_eq!(input.len(), MAX_BYTES - 15);

    let failure = match parse("over-limit.json", &input) {
        Ok(_) => panic!("accepted input whose encoded result exceeds the byte limit"),
        Err(error) => error,
    };

    assert_eq!(failure.file, PathBuf::from("over-limit.json"));
    assert!(failure.detail.contains("encoded size"));

    let path = temporary_file("canonical-limit");
    std::fs::write(&path, input).unwrap();

    let failure = match load(&path) {
        Ok(_) => panic!("loaded input whose encoded result exceeds the byte limit"),
        Err(error) => error,
    };

    assert_eq!(failure.file, path);
    assert!(failure.detail.contains("encoded size"));

    std::fs::remove_file(path).unwrap();
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

#[test]
fn oversized_output_is_rejected_before_replacing_a_file() {
    let path = temporary_file("oversize-output");
    let (world, trace) = rail_viewer::scenario();

    save(&path, &world, &trace).unwrap();

    let original = std::fs::read(&path).unwrap();

    let mut snapshot = trace.snapshots[0].clone();
    snapshot.passengers = vec![snapshot.passengers[0]; 300_000];

    let oversized = RailTrace {
        snapshots: vec![snapshot],
        completed: false,
    };

    assert!(
        save(&path, &world, &oversized)
            .unwrap_err()
            .detail
            .contains("exceeds")
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);

    std::fs::remove_file(path).unwrap();
}
