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

#[test]
fn file_io_errors_identify_the_file() {
    let path = fixture("does-not-exist.json");
    let error = load(&path).unwrap_err();

    assert_eq!(error.file, path);
    assert!(error.detail.starts_with("I/O:"));
}
