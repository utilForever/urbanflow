#[path = "../examples/run_scenario/mod.rs"]
pub mod runner;

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use rstest::rstest;
use runner::rail_viewer::{self, results};

const RAIL: &str = include_str!("fixtures/scenarios/rail-demo.json");

struct RunDirectory(PathBuf);

impl RunDirectory {
    fn new(name: &str) -> Self {
        let directory =
            Self(std::env::temp_dir().join(format!("urbanflow-cli-{}-{name}", std::process::id())));

        fs::create_dir(&directory.0).unwrap();
        fs::write(directory.0.join("scenario.json"), RAIL).unwrap();

        directory
    }

    fn args(&self) -> Vec<OsString> {
        ["scenario.json", "result.json", "viewer.html"]
            .map(|name| self.0.join(name).into_os_string())
            .to_vec()
    }

    fn seed_outputs(&self) -> (String, String) {
        runner::run(self.args()).unwrap();
        self.outputs()
    }

    fn outputs(&self) -> (String, String) {
        (
            fs::read_to_string(self.0.join("result.json")).unwrap(),
            fs::read_to_string(self.0.join("viewer.html")).unwrap(),
        )
    }

    fn assert_no_staging_files(&self) {
        assert_eq!(fs::read_dir(&self.0).unwrap().count(), 3);
    }
}

impl Drop for RunDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn repeated_runs_match_demo_trace_analysis_json_and_html() {
    let directory = RunDirectory::new("repeated");
    let (world, trace) = rail_viewer::scenario();
    let expected = (
        results::encode(&world, &trace).unwrap(),
        rail_viewer::render(&world, &trace).unwrap(),
    );

    for _ in 0..2 {
        let message = runner::run(directory.args()).unwrap();

        assert!(message.starts_with("Completed at tick 18; 19 snapshots;"));
        assert_eq!(directory.outputs(), expected);

        let saved = results::load(directory.0.join("result.json")).unwrap();

        assert_eq!(saved.trace, trace);
        assert_eq!(
            saved.analysis,
            Some(urbanflow::analysis::analyze(&trace).unwrap())
        );

        directory.assert_no_staging_files();
    }
}

#[rstest]
#[case::zero(0, false)]
#[case::partial(3, false)]
#[case::exact_completion(18, true)]
#[case::full_width_limit(u64::MAX, true)]
fn file_limit_controls_recording(#[case] limit: u64, #[case] completed: bool) {
    let directory = RunDirectory::new(&format!("limit-{limit}"));

    fs::write(
        directory.0.join("scenario.json"),
        RAIL.replace(
            "\"max_ticks\": \"32\"",
            &format!("\"max_ticks\": \"{limit}\""),
        ),
    )
    .unwrap();

    let message = runner::run(directory.args()).unwrap();
    let saved = results::load(directory.0.join("result.json")).unwrap();

    assert_eq!(saved.trace.completed, completed);
    assert_eq!(saved.analysis.is_some(), completed);
    assert_eq!(saved.trace.snapshots.len() as u64, limit.min(18) + 1);
    assert_eq!(saved.trace.snapshots.last().unwrap().tick, limit.min(18));
    assert_eq!(message.starts_with("Completed"), completed);

    let (json, html) = directory.outputs();
    let embedded = html
        .split_once("<script id=\"trace-data\" type=\"application/json\">")
        .unwrap()
        .1
        .split_once("</script>")
        .unwrap()
        .0
        .trim();

    assert_eq!(embedded, json);

    if !completed {
        assert!(message.starts_with("Partial"));
        assert!(
            saved
                .trace
                .snapshots
                .last()
                .unwrap()
                .passengers
                .iter()
                .all(|record| record.unserved == 0)
        );
    }
}

#[test]
fn html_is_optional() {
    let directory = RunDirectory::new("json-only");

    runner::run(directory.args().into_iter().take(2)).unwrap();

    assert!(
        results::load(directory.0.join("result.json"))
            .unwrap()
            .trace
            .completed
    );
    assert!(!directory.0.join("viewer.html").exists());
}

#[rstest]
#[case::invalid("{", "JSON")]
#[case::version(include_str!("fixtures/scenarios/unsupported-version.json"), "version")]
#[case::env(include_str!("fixtures/scenarios/env-baseline.json"), "supports only rail")]
fn rejected_input_preserves_existing_outputs(#[case] json: &str, #[case] expected: &str) {
    let directory = RunDirectory::new(expected);
    let before = directory.seed_outputs();

    fs::write(directory.0.join("scenario.json"), json).unwrap();

    let error = runner::run(directory.args()).unwrap_err();

    assert!(
        error.contains("scenario.json") && error.contains(expected),
        "{error}"
    );
    assert_eq!(directory.outputs(), before);

    directory.assert_no_staging_files();
}

#[test]
fn encoding_failure_after_recording_preserves_existing_outputs() {
    let directory = RunDirectory::new("oversized");
    let before = directory.seed_outputs();

    let mut json: serde_json::Value = serde_json::from_str(RAIL).unwrap();
    json["demands"] = vec![json["demands"][0].clone(); 1200].into();
    json["vehicle"]["travel_ticks_per_edge"] = "100".into();
    json["max_ticks"] = "200".into();

    fs::write(directory.0.join("scenario.json"), json.to_string()).unwrap();

    let error = runner::run(directory.args()).unwrap_err();

    assert!(
        error.contains("result.json") && error.contains("16 MiB"),
        "{error}"
    );
    assert_eq!(directory.outputs(), before);

    directory.assert_no_staging_files();
}

#[rstest]
#[case::no_args(vec![], false)]
#[case::missing_output(vec!["scenario.json"], false)]
#[case::extra_arg(vec!["a", "b", "c", "d"], false)]
#[case::help(vec!["--help"], true)]
#[case::short_help(vec!["-h"], true)]
fn command_usage(#[case] args: Vec<&str>, #[case] success: bool) {
    let result = runner::run(args.into_iter().map(OsString::from));

    assert_eq!(result.is_ok(), success);
    assert!(result.unwrap_or_else(|error| error).starts_with("usage:"));
}
