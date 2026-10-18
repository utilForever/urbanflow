#[path = "../examples/baseline/mod.rs"]
mod baseline;
#[path = "../examples/rail_viewer/mod.rs"]
mod rail_viewer;
#[path = "../examples/scenario/mod.rs"]
mod scenario;

use std::path::PathBuf;

use rstest::rstest;
use scenario::{Scenario, load, parse};
use urbanflow::action::Action;
use urbanflow::analysis::analyze;
use urbanflow::env::Env;
use urbanflow::rail::RailVehicleState;
use urbanflow::world::{EdgeId, EdgeKind, NodeId};

const ENV: &str = include_str!("fixtures/scenarios/env-baseline.json");
const RAIL: &str = include_str!("fixtures/scenarios/rail-demo.json");

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/scenarios")
        .join(name)
}

fn rejection(json: &str, field: &str, reason: &str) {
    let error = parse("input.json", json).unwrap_err();

    assert_eq!(error.file, PathBuf::from("input.json"));

    let message = error.to_string();

    for part in ["input.json", field, reason] {
        assert!(message.contains(part), "missing {part:?} in {message}");
    }
}

#[test]
fn env_file_matches_every_baseline_action_and_reset() {
    let Scenario::Env(mut loaded) = load(fixture("env-baseline.json")).unwrap() else {
        panic!("expected Env");
    };
    let mut built = baseline::scenario();

    assert_eq!(loaded.world.nodes, built.world.nodes);
    assert_eq!(loaded.world.network.edges(), built.world.network.edges());
    assert_eq!(loaded.demands, built.demands);
    assert_eq!(loaded.available_actions(), built.available_actions());
    assert_eq!(loaded.step_count, 0);

    for action in built.available_actions() {
        assert_eq!(loaded.reset(), built.reset());
        assert_eq!(loaded.step(action), built.step(action));
    }
}

#[rstest]
#[case::below_road_cost("0.9999999999999999", 0.9999999999999999)]
#[case::below_rail_cost("1.9999999999999998", 1.9999999999999998)]
#[case::rounds_to_max_finite("1.7976931348623158e308", f64::MAX)]
fn decimal_budget_matches_rust_values_and_actions(#[case] token: &str, #[case] budget: f64) {
    let json = ENV.replace("\"budget\": 2", &format!("\"budget\": {token}"));
    let Scenario::Env(mut loaded) = parse("budget.json", &json).unwrap() else {
        panic!("expected Env");
    };
    let initial = baseline::scenario();
    let mut built = Env::new(initial.world, initial.demands, budget, initial.max_steps).unwrap();

    assert_eq!(loaded.budget.to_bits(), budget.to_bits());
    assert_eq!(loaded.available_actions(), built.available_actions());

    for kind in EdgeKind::ALL {
        assert_eq!(loaded.reset(), built.reset());

        let action = Action::AddEdge {
            from: NodeId(0),
            to: NodeId(1),
            kind,
        };

        assert_eq!(loaded.step(action), built.step(action));
    }
}

#[test]
fn rail_file_matches_demo_trace_analysis_and_viewer_on_repeated_runs() {
    let (world, trace) = rail_viewer::scenario();

    for _ in 0..2 {
        let Scenario::Rail(mut loaded) = load(fixture("rail-demo.json")).unwrap() else {
            panic!("expected Rail");
        };

        assert_eq!(loaded.world.nodes, world.nodes);
        assert_eq!(loaded.world.network.edges(), world.network.edges());
        assert_eq!(loaded.clock.tick(), 0);
        assert_eq!(
            loaded.vehicle.state(),
            RailVehicleState::AtStop {
                stop_index: 0,
                dwell_ticks_remaining: 2,
            }
        );
        assert_eq!(
            loaded
                .vehicle
                .snapshot(&loaded.clock, &loaded.passengers)
                .unwrap(),
            trace.snapshots[0]
        );

        let recorded = loaded
            .vehicle
            .record_trace(&mut loaded.clock, &mut loaded.passengers, loaded.max_ticks)
            .unwrap();

        assert_eq!(recorded, trace);
        assert_eq!(analyze(&recorded), analyze(&trace));
        assert_eq!(
            rail_viewer::render(&loaded.world, &recorded),
            rail_viewer::render(&world, &trace)
        );
    }
}

#[rstest]
#[case::zero(0, false)]
#[case::partial(17, false)]
#[case::exact_completion(18, true)]
fn loaded_recording_preserves_limits(#[case] limit: u64, #[case] completed: bool) {
    let json = RAIL.replace(
        r#""max_ticks": "32""#,
        &format!(r#""max_ticks": "{limit}""#),
    );
    let Scenario::Rail(mut loaded) = parse("limit.json", &json).unwrap() else {
        panic!("expected Rail");
    };
    let trace = loaded
        .vehicle
        .record_trace(&mut loaded.clock, &mut loaded.passengers, loaded.max_ticks)
        .unwrap();

    assert_eq!(trace.completed, completed);
    assert_eq!(trace.snapshots.len() as u64, limit + 1);
    assert_eq!(loaded.clock.tick(), limit);

    if !completed {
        assert!(
            trace
                .snapshots
                .last()
                .unwrap()
                .passengers
                .iter()
                .all(|p| p.unserved == 0)
        );
    }
}

#[test]
fn empty_env_is_valid_and_loading_does_not_spend_initial_edge_costs() {
    let Scenario::Env(empty) = load(fixture("env-empty.json")).unwrap() else {
        panic!("expected Env");
    };

    assert!(empty.available_actions().is_empty());
    assert_eq!(empty.step_count, 0);

    let json = ENV.replace(
        r#""edges": []"#,
        r#""edges": [{"from":"0","to":"1","kind":"Rail"}]"#,
    );
    let Scenario::Env(mut env) = parse("initial.json", &json).unwrap() else {
        panic!("expected Env");
    };

    assert_eq!(env.budget, 2.0);
    assert_eq!(env.world.network.edges()[0].id, EdgeId(0));
    assert_eq!(env.reset().budget, 2.0);
}

#[test]
fn input_order_controls_actions_path_ties_and_demand_allocation() {
    let json = r#"{
        "version":1,"kind":"env","nodes":["8","3","21","5"],
        "edges":[
            {"from":"8","to":"21","kind":"Road"},
            {"from":"8","to":"3","kind":"Rail"},
            {"from":"21","to":"5","kind":"Road"},
            {"from":"3","to":"5","kind":"Rail"}
        ],
        "demands":[
            {"origin":"8","destination":"5","amount":8},
            {"origin":"8","destination":"21","amount":5},
            {"origin":"8","destination":"5","amount":8},
            {"origin":"3","destination":"3","amount":0}
        ],"budget":2,"max_steps":"1"
    }"#;
    let Scenario::Env(mut env) = parse("ordered.json", json).unwrap() else {
        panic!("expected Env");
    };

    assert_eq!(
        env.world.nodes.iter().map(|n| n.id.0).collect::<Vec<_>>(),
        [8, 3, 21, 5]
    );
    assert_eq!(
        env.demands.iter().map(|d| d.amount).collect::<Vec<_>>(),
        [8, 5, 8, 0]
    );
    assert_eq!(
        env.available_actions()[0],
        Action::AddEdge {
            from: NodeId(8),
            to: NodeId(3),
            kind: EdgeKind::Road
        }
    );

    let result = env
        .step(Action::AddEdge {
            from: NodeId(5),
            to: NodeId(8),
            kind: EdgeKind::Road,
        })
        .unwrap();

    assert_eq!(result.metrics.served_demand, 10);
    assert_eq!(result.metrics.unserved_demand, 11);
}

#[test]
fn boundary_fixture_is_target_checked_and_keeps_full_width_values() {
    let result = load(fixture("rail-boundaries.json"));

    if usize::BITS < 64 {
        let error = result.unwrap_err().to_string();
        assert!(error.contains("nodes[0]"));
        assert!(error.contains("32-bit usize"));
        return;
    }

    let Scenario::Rail(mut loaded) = result.unwrap() else {
        panic!("expected Rail");
    };

    assert_eq!(loaded.world.nodes[0].id.0, usize::MAX);
    assert_eq!(loaded.vehicle.capacity(), u32::MAX);
    assert_eq!(loaded.vehicle.travel_ticks_per_edge(), u64::MAX);
    assert_eq!(loaded.vehicle.dwell_ticks_per_stop(), u64::MAX);
    assert_eq!(
        loaded.vehicle.route().edges(),
        [EdgeId(0), EdgeId(1), EdgeId(0)]
    );

    let trace = loaded
        .vehicle
        .record_trace(&mut loaded.clock, &mut loaded.passengers, loaded.max_ticks)
        .unwrap();

    assert!(!trace.completed);
    assert_eq!(trace.snapshots.len(), 1);

    let records = &trace.snapshots[0].passengers;

    assert_eq!(records.len(), 3);
    assert_eq!(records[0], records[1]);
    assert_eq!(records[0].waiting, u32::MAX);
    assert_eq!(records[2].demand.amount, 0);
}

#[test]
fn full_width_ticks_work_on_every_target_without_precomputing_route_duration() {
    let json = RAIL
        .replace(
            r#""travel_ticks_per_edge": "4""#,
            r#""travel_ticks_per_edge": "18446744073709551615""#,
        )
        .replace(
            r#""dwell_ticks_per_stop": "2""#,
            r#""dwell_ticks_per_stop": "18446744073709551615""#,
        )
        .replace(r#""max_ticks": "32""#, r#""max_ticks": "1""#);
    let Scenario::Rail(mut loaded) = parse("long-route.json", &json).unwrap() else {
        panic!("expected Rail");
    };

    assert_eq!(loaded.vehicle.travel_ticks_per_edge(), u64::MAX);

    let trace = loaded
        .vehicle
        .record_trace(&mut loaded.clock, &mut loaded.passengers, loaded.max_ticks)
        .unwrap();

    assert!(!trace.completed);
    assert_eq!(
        loaded.vehicle.state(),
        RailVehicleState::AtStop {
            stop_index: 0,
            dwell_ticks_remaining: u64::MAX - 1
        }
    );

    let json = RAIL.replace(
        r#""max_ticks": "32""#,
        r#""max_ticks": "18446744073709551615""#,
    );
    let Scenario::Rail(mut loaded) = parse("large-limit.json", &json).unwrap() else {
        panic!("expected Rail");
    };

    assert_eq!(loaded.max_ticks, u64::MAX);

    let trace = loaded
        .vehicle
        .record_trace(&mut loaded.clock, &mut loaded.passengers, loaded.max_ticks)
        .unwrap();

    assert!(trace.completed);
    assert_eq!(trace.snapshots.len(), 19);
}

#[test]
fn rail_demands_need_known_endpoints_even_when_zero_but_need_not_be_on_route() {
    let mut document: serde_json::Value = serde_json::from_str(RAIL).unwrap();
    document["demands"][0]["amount"] = 0.into();
    document["demands"][0]["destination"] = "99".into();

    rejection(
        &document.to_string(),
        "demands[0].destination",
        "unknown node ID",
    );

    document["nodes"].as_array_mut().unwrap().push("99".into());
    document["demands"][1]["destination"] = "3".into();

    let Scenario::Rail(loaded) = parse("off-route.json", &document.to_string()).unwrap() else {
        panic!("expected Rail");
    };

    assert_eq!(loaded.passengers.records().len(), 2);
    assert_eq!(
        loaded.passengers.records()[0].demand.destination,
        NodeId(99)
    );
    assert_eq!(loaded.passengers.records()[1].waiting, 3);
}

#[rstest]
#[case::largest_32_bit("4294967295")]
#[case::first_64_bit("4294967296")]
#[case::beyond_float_precision("9007199254740993")]
#[case::largest_64_bit("18446744073709551615")]
fn node_ids_and_step_limits_use_target_usize(#[case] value: &str) {
    let json = format!(
        r#"{{"version":1,"kind":"env","nodes":["{value}"],"edges":[],"demands":[],"budget":0,"max_steps":"{value}"}}"#
    );
    let result = parse("target.json", &json);
    let number: u64 = value.parse().unwrap();

    if let Ok(expected) = usize::try_from(number) {
        let Scenario::Env(env) = result.unwrap() else {
            panic!("expected Env");
        };

        assert_eq!(env.world.nodes[0].id.0, expected);
        assert_eq!(env.max_steps, expected);
    } else {
        assert!(result.unwrap_err().detail.contains("32-bit usize"));
    }

    let json = RAIL.replace(
        r#""route": ["4", "5", "6"]"#,
        &format!(r#""route": ["{value}"]"#),
    );

    rejection(
        &json,
        "route[0]",
        if usize::try_from(number).is_ok() {
            "unknown edge ID"
        } else {
            "32-bit usize"
        },
    );
}

#[rstest]
#[case::unsupported_version("unsupported-version.json", "version", "unsupported version")]
#[case::duplicate_node("duplicate-node.json", "nodes[1]", "duplicate node")]
#[case::unknown_demand("unknown-demand-node.json", "demands[0].destination", "unknown node")]
#[case::unknown_route("unknown-route-edge.json", "route[0]", "unknown edge")]
#[case::numeric_id("numeric-node-id.json", "nodes[0]", "string")]
#[case::amount_overflow("amount-overflow.json", "demands[0].amount", "u32")]
#[case::tick_overflow("tick-overflow.json", "max_ticks", "u64::MAX")]
#[case::zero_travel("zero-travel-ticks.json", "vehicle.travel_ticks_per_edge", "positive")]
#[case::unknown_field("unknown-field.json", "max_ticks", "unknown field")]
fn invalid_fixtures_report_file_field_and_reason(
    #[case] name: &str,
    #[case] field: &str,
    #[case] reason: &str,
) {
    let error = load(fixture(name)).unwrap_err().to_string();

    for part in [name, field, reason] {
        assert!(error.contains(part), "missing {part:?} in {error}");
    }
}

#[rstest]
#[case::missing_version(r#"{"kind":"env"}"#, "version", "missing field")]
#[case::unsupported_before_fields(
    r#"{"version":2,"kind":null,"vehicle":123}"#,
    "version",
    "unsupported version"
)]
#[case::unknown_kind(r#"{"version":1,"kind":"tram"}"#, "kind", "unsupported kind")]
#[case::missing_kind(r#"{"version":1}"#, "kind", "missing field")]
#[case::duplicate_version(r#"{"version":1,"version":2}"#, "version", "duplicate field")]
#[case::duplicate_kind(
    r#"{"version":1,"kind":"env","kind":"rail"}"#,
    "kind",
    "duplicate field"
)]
#[case::root_array("[1,\"env\",[],[],[],0,\"0\"]", "", "object")]
#[case::root_null("null", "", "object")]
#[case::trailing_document(r#"{"version":1} {}"#, "", "trailing characters")]
#[case::trailing_comma(r#"{"version":1,}"#, "", "JSON")]
#[case::comment("{/*comment*/}", "", "JSON")]
fn rejects_invalid_documents(#[case] json: &str, #[case] field: &str, #[case] reason: &str) {
    rejection(json, field, reason);
}

#[rstest]
#[case::negative("-1")]
#[case::negative_zero("-0")]
#[case::fraction("1.0")]
#[case::exponent("1e0")]
#[case::quoted("\"1\"")]
#[case::boolean("true")]
#[case::null("null")]
fn unsigned_numeric_tokens_reject_coercion(#[case] token: &str) {
    rejection(
        &ENV.replace("\"version\": 1", &format!("\"version\": {token}")),
        "version",
        "integer",
    );
    rejection(
        &ENV.replace("\"amount\": 15", &format!("\"amount\": {token}")),
        "demands[0].amount",
        "u32",
    );
    rejection(
        &RAIL.replace("\"capacity\": 6", &format!("\"capacity\": {token}")),
        "vehicle.capacity",
        "u32",
    );
}

#[rstest]
#[case::empty("")]
#[case::sign("+1")]
#[case::negative("-1")]
#[case::leading_zero("01")]
#[case::space(" 1")]
#[case::fraction("1.0")]
#[case::exponent("1e0")]
#[case::unicode("١")]
#[case::overflow("18446744073709551616")]
fn decimal_strings_reject_noncanonical_or_out_of_range_values(#[case] value: &str) {
    rejection(
        &ENV.replace(
            "\"max_steps\": \"1\"",
            &format!("\"max_steps\": \"{value}\""),
        ),
        "max_steps",
        "",
    );
    rejection(
        &RAIL.replace(
            "\"max_ticks\": \"32\"",
            &format!("\"max_ticks\": \"{value}\""),
        ),
        "max_ticks",
        "",
    );
}

#[rstest]
#[case::null_nodes("env", "/nodes", "null", "nodes")]
#[case::duplicate_node("env", "/nodes", r#"["0","0"]"#, "nodes[1]")]
#[case::edge_array("rail", "/edges/0", r#"["8","3","Road"]"#, "edges[0]")]
#[case::demand_array("env", "/demands/0", r#"["0","1",15]"#, "demands[0]")]
#[case::vehicle_array("rail", "/vehicle", r#"[6,"4","2"]"#, "vehicle")]
#[case::self_edge("rail", "/edges/0/to", r#""8""#, "edges[0]")]
#[case::duplicate_edge(
    "rail",
    "/edges/1",
    r#"{"from":"8","to":"3","kind":"Road"}"#,
    "edges[1]"
)]
#[case::unknown_edge_node("rail", "/edges/0/from", r#""99""#, "edges[0].from")]
#[case::unknown_rail_demand("rail", "/demands/0/destination", r#""99""#, "demands[0].destination")]
#[case::road_route("rail", "/route", r#"["0"]"#, "route[0]")]
#[case::disconnected("rail", "/route", r#"["4","6"]"#, "route[1]")]
#[case::empty_route("rail", "/route", "[]", "route")]
#[case::zero_capacity("rail", "/vehicle/capacity", "0", "vehicle.capacity")]
#[case::capacity_overflow("rail", "/vehicle/capacity", "4294967296", "vehicle.capacity")]
#[case::zero_dwell(
    "rail",
    "/vehicle/dwell_ticks_per_stop",
    r#""0""#,
    "vehicle.dwell_ticks_per_stop"
)]
#[case::negative_budget("env", "/budget", "-1", "budget")]
#[case::numeric_steps("env", "/max_steps", "1", "max_steps")]
#[case::numeric_ticks("rail", "/max_ticks", "32", "max_ticks")]
fn rejects_invalid_shapes_and_core_settings(
    #[case] kind: &str,
    #[case] pointer: &str,
    #[case] replacement: &str,
    #[case] field: &str,
) {
    let mut document: serde_json::Value =
        serde_json::from_str(if kind == "env" { ENV } else { RAIL }).unwrap();
    *document.pointer_mut(pointer).unwrap() = serde_json::from_str(replacement).unwrap();

    rejection(&document.to_string(), field, "");
}

#[rstest]
#[case::duplicate_root(
    "env",
    "\"budget\": 2",
    "\"budget\": 2, \"budget\": 2",
    "budget",
    "duplicate field"
)]
#[case::duplicate_edge(
    "rail",
    "\"from\": \"8\"",
    "\"from\": \"8\", \"from\": \"8\"",
    "edges[0]",
    "duplicate field"
)]
#[case::duplicate_demand(
    "env",
    "\"amount\": 15",
    "\"amount\": 15, \"amount\": 15",
    "demands[0]",
    "duplicate field"
)]
#[case::duplicate_vehicle(
    "rail",
    "\"capacity\": 6",
    "\"capacity\": 6, \"capacity\": 6",
    "vehicle",
    "duplicate field"
)]
#[case::unknown_edge_field(
    "rail",
    "\"from\": \"8\"",
    "\"id\": \"0\", \"from\": \"8\"",
    "edges[0]",
    "unknown field"
)]
#[case::unknown_demand_field(
    "env",
    "\"amount\": 15",
    "\"extra\": null, \"amount\": 15",
    "demands[0]",
    "unknown field"
)]
#[case::unknown_vehicle_field(
    "rail",
    "\"capacity\": 6",
    "\"extra\": null, \"capacity\": 6",
    "vehicle",
    "unknown field"
)]
#[case::cross_kind_field(
    "rail",
    "\"version\": 1",
    "\"version\": 1, \"budget\": null",
    "budget",
    "unknown field"
)]
#[case::budget_overflow("env", "\"budget\": 2", "\"budget\": 1e400", "budget", "out of range")]
fn rejects_duplicate_unknown_and_unrepresentable_fields(
    #[case] kind: &str,
    #[case] from: &str,
    #[case] to: &str,
    #[case] field: &str,
    #[case] reason: &str,
) {
    let original = if kind == "env" { ENV } else { RAIL };

    assert!(original.contains(from));

    rejection(&original.replacen(from, to, 1), field, reason);
}

#[test]
fn file_io_errors_identify_the_file() {
    let path = fixture("does-not-exist.json");
    let error = load(&path).unwrap_err();

    assert_eq!(error.file, path);
    assert!(error.detail.starts_with("I/O:"));
}
