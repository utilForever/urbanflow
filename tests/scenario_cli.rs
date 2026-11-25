#[path = "../apps/desktop/src/workflow.rs"]
mod desktop;
#[path = "../examples/run_scenario/mod.rs"]
pub mod runner;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use rstest::rstest;
use runner::rail_viewer::{self, results};
use runner::{files, output, scenario};

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
fn cli_and_desktop_save_the_same_bounded_recording(#[case] limit: u64, #[case] completed: bool) {
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

    let input = directory.0.join("scenario.json");
    let scenario = desktop::OpenedScenario::open(&input).unwrap();
    let desktop_output = directory.0.join("desktop.json");

    fs::write(&input, "edited after opening").unwrap();

    for _ in 0..2 {
        let recording = scenario.run().unwrap();
        recording.save(&desktop_output, &scenario.source).unwrap();

        assert_eq!(fs::read_to_string(&desktop_output).unwrap(), json);
        assert_eq!(recording.data.trace, saved.trace);
        assert_eq!(recording.data.analysis, saved.analysis);
    }

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

#[test]
fn staging_one_new_file_needs_only_a_sibling_and_cleans_up_on_failure() {
    let directory = RunDirectory::new("single-sibling");
    let result = directory.0.join("result.json");
    let staged = output::StagedOutput::new(&result, "new result", None).unwrap();

    assert!(!result.exists());

    for entry in fs::read_dir(&directory.0).unwrap() {
        assert!(entry.unwrap().file_type().unwrap().is_file());
    }

    assert!(
        staged
            .replace(&result, |_| Err("cancelled".into()))
            .is_err()
    );
    assert!(!result.exists());
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn an_existing_results_permissions_apply_during_staging_and_after_replacement() {
    use std::os::unix::fs::PermissionsExt;

    let directory = RunDirectory::new("private-result");
    let result = directory.0.join("result.json");

    fs::write(&result, "previous result").unwrap();
    fs::set_permissions(&result, fs::Permissions::from_mode(0o600)).unwrap();

    let staged = output::StagedOutput::new(&result, "new result", None).unwrap();
    let scratch = fs::read_dir(&directory.0)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.is_dir())
        .unwrap();
    let mode = |path: &std::path::Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;

    assert_eq!(mode(&scratch.join("result.json")), 0o600);
    assert_eq!(fs::read_to_string(&result).unwrap(), "previous result");

    fs::set_permissions(&result, fs::Permissions::from_mode(0o400)).unwrap();
    staged.replace(&result, |_| Ok(())).unwrap();

    assert_eq!(mode(&result), 0o400);
    assert_eq!(fs::read_to_string(result).unwrap(), "new result");
}

#[cfg(target_vendor = "apple")]
#[test]
fn replacing_a_result_keeps_its_macos_read_denial() {
    use std::process::Command;

    let directory = RunDirectory::new("result-acl");
    let result = directory.0.join("result.json");

    fs::write(&result, "previous result").unwrap();

    assert!(
        Command::new("chmod")
            .args(["+a", "everyone deny read"])
            .arg(&result)
            .status()
            .unwrap()
            .success()
    );

    runner::run(directory.args()).unwrap();

    let acl = Command::new("ls").arg("-le").arg(result).output().unwrap();

    assert!(acl.status.success());
    assert!(String::from_utf8(acl.stdout).unwrap().contains("deny read"));
}

#[test]
fn a_removed_replacement_destination_can_be_retried_as_a_new_file() {
    let directory = RunDirectory::new("removed-result");
    let result = directory.0.join("result.json");

    fs::write(&result, "previous result").unwrap();

    let staged = output::StagedOutput::new(&result, "staged result", None).unwrap();

    fs::remove_file(&result).unwrap();

    assert!(staged.replace(&result, |_| Ok(())).is_err());
    assert!(!result.exists());
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);

    runner::run(directory.args()).unwrap();

    assert!(results::load(result).unwrap().trace.completed);
    directory.assert_no_staging_files();
}

#[cfg(target_os = "linux")]
#[test]
fn replacing_a_result_keeps_its_linux_named_user_acl() {
    use std::os::fd::AsRawFd;

    let directory = RunDirectory::new("result-acl");
    let result = directory.0.join("result.json");
    let original = fs::File::create(&result).unwrap();
    // Linux POSIX ACL version 2: owner, named user 65534, group, mask, other.
    let mut acl = 2u32.to_le_bytes().to_vec();

    for (tag, permissions, id) in [
        (1u16, 2u16, u32::MAX),
        (2, 4, 65534),
        (4, 0, u32::MAX),
        (16, 4, u32::MAX),
        (32, 0, u32::MAX),
    ] {
        acl.extend(tag.to_le_bytes());
        acl.extend(permissions.to_le_bytes());
        acl.extend(id.to_le_bytes());
    }

    let name = c"system.posix_acl_access".as_ptr();

    if unsafe {
        libc::fsetxattr(
            original.as_raw_fd(),
            name,
            acl.as_ptr().cast(),
            acl.len(),
            0,
        )
    } != 0
    {
        let error = std::io::Error::last_os_error();

        if error.raw_os_error() == Some(libc::EOPNOTSUPP) {
            eprintln!("Skipping named-user ACL check on a filesystem without POSIX ACL support");
            return;
        }

        panic!("{error}");
    }

    drop(original);
    runner::run(directory.args()).unwrap();

    let saved = fs::File::options().write(true).open(result).unwrap();
    let mut actual = vec![0u8; acl.len()];
    let size = unsafe {
        libc::fgetxattr(
            saved.as_raw_fd(),
            name,
            actual.as_mut_ptr().cast(),
            actual.len(),
        )
    };

    assert_eq!(size, acl.len() as isize);
    assert_eq!(actual, acl);
}

#[rstest]
#[case::case("result.json", "RESULT.JSON")]
#[case::unicode("\u{e9}.json", "e\u{301}.json")]
fn output_names_follow_filesystem_alias_rules(#[case] first: &str, #[case] second: &str) {
    let directory = RunDirectory::new(first);
    let result = directory.0.join(first);
    let html = directory.0.join(second);

    // Detect the actual volume's rules, then exercise two absent destinations.
    fs::write(&result, "probe").unwrap();

    let aliases = html.try_exists().unwrap();

    fs::remove_file(&result).unwrap();

    let args = vec![
        directory.0.join("scenario.json").into_os_string(),
        result.clone().into_os_string(),
        html.clone().into_os_string(),
    ];
    let outcome = runner::run(args.clone());

    if aliases {
        assert!(outcome.unwrap_err().contains("distinct"));
        assert!(!result.exists() && !html.exists());
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);

        runner::run(args[..2].iter().cloned()).unwrap();

        let before = fs::read(&result).unwrap();

        assert!(runner::run(args).unwrap_err().contains("distinct"));
        assert_eq!(fs::read(&result).unwrap(), before);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    } else {
        outcome.unwrap();

        let saved = results::load(&result).unwrap();

        assert_eq!(
            fs::read_to_string(&html).unwrap(),
            rail_viewer::render(&saved.world, &saved.trace).unwrap(),
        );
        directory.assert_no_staging_files();
    }

    assert_eq!(
        fs::read_to_string(directory.0.join("scenario.json")).unwrap(),
        RAIL
    );
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

#[test]
fn recording_stops_at_the_first_frame_that_exceeds_the_result_limit() {
    let mut input: serde_json::Value = serde_json::from_str(RAIL).unwrap();
    input["demands"] = vec![input["demands"][0].clone(); 1200].into();
    input["vehicle"]["travel_ticks_per_edge"] = "1000".into();
    input["max_ticks"] = "300".into();

    let file = std::path::Path::new("oversized.json");
    let initial = scenario::parse(file, &input.to_string())
        .unwrap()
        .into_rail(file)
        .unwrap();
    let mut rail = initial.clone();
    let error = results::record(
        rail.world,
        &mut rail.vehicle,
        &mut rail.clock,
        &mut rail.passengers,
        rail.max_ticks,
    )
    .unwrap_err();

    assert!(error.contains("16 MiB"), "{error}");

    let rejected_tick = rail.clock.tick();

    assert!((1..initial.max_ticks).contains(&rejected_tick));

    let mut rail = initial;
    let mut trace = rail
        .vehicle
        .record_trace(&mut rail.clock, &mut rail.passengers, rejected_tick)
        .unwrap();
    let rejected = trace.snapshots.pop().unwrap();

    assert!(results::encode(&rail.world, &trace).is_ok());

    trace.snapshots.push(rejected);

    assert!(
        results::encode(&rail.world, &trace)
            .unwrap_err()
            .detail
            .contains("16 MiB")
    );
}

#[test]
fn output_errors_and_aliases_preserve_existing_files() {
    let directory = RunDirectory::new("output-errors");
    let too_long = "h".repeat(300);

    let mut args = directory.args();
    args[2] = directory.0.join(&too_long).into_os_string();

    let error = runner::run(args).unwrap_err();

    assert!(
        error.contains(&too_long) && !error.contains("result.json"),
        "{error}"
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);

    let before = directory.seed_outputs();

    for (index, path, expected) in [
        (1, directory.0.join("./scenario.json"), "distinct"),
        (2, directory.0.join("./result.json"), "distinct"),
        (2, directory.0.join("./scenario.json"), "distinct"),
        (1, directory.0.clone(), "regular file"),
        (2, directory.0.join("missing/viewer.html"), "viewer.html"),
        (2, directory.0.join(&too_long), too_long.as_str()),
    ] {
        let mut args = directory.args();
        args[index] = path.into_os_string();

        let error = runner::run(args).unwrap_err();

        assert!(error.contains(expected), "{error}");
        assert_eq!(directory.outputs(), before);
        assert_eq!(
            fs::read_to_string(directory.0.join("scenario.json")).unwrap(),
            RAIL
        );
        directory.assert_no_staging_files();
    }
}

#[test]
fn directory_endings_on_file_arguments_preserve_previous_results() {
    let directory = RunDirectory::new("directory-endings");
    let input = directory.0.join("scenario.json");

    let mut partial: serde_json::Value = serde_json::from_str(RAIL).unwrap();
    partial["max_ticks"] = "3".into();

    fs::write(&input, partial.to_string()).unwrap();

    let before = directory.seed_outputs();

    fs::write(&input, RAIL).unwrap();

    let opened = desktop::OpenedScenario::open(&input).unwrap();
    let recording = opened.run().unwrap();
    // Joining a path component can normalize away the ending on verbatim Windows paths.
    let with_ending = |base: &Path, ending: &str| {
        let mut path = base.to_path_buf();
        path.as_mut_os_string().push(ending);
        path
    };
    let result = directory.0.join("result.json");

    assert!(
        desktop::OpenedScenario::open(&with_ending(&input, "/."))
            .is_err_and(|error| error.contains("filename"))
    );
    assert!(
        recording
            .save(&with_ending(&result, "/."), &opened.source)
            .is_err_and(|error| error.contains("filename"))
    );

    #[cfg(windows)]
    let endings = ["/.", "/", "/..", "\\.", "\\", "\\.."];
    #[cfg(not(windows))]
    let endings = ["/.", "/", "/.."];
    // Include verbatim paths for new outputs on Windows.
    let canonical = fs::canonicalize(&directory.0).unwrap();

    for (index, base) in [
        (0, input),
        (1, result),
        (2, directory.0.join("viewer.html")),
        (1, canonical.join("new.json")),
        (2, canonical.join("new.html")),
    ] {
        for ending in endings {
            let path = with_ending(&base, ending).into_os_string();

            let mut args = directory.args();
            args[index] = path.clone();

            let error = runner::run(args).unwrap_err();

            assert!(
                error.starts_with(path.to_string_lossy().as_ref()) && error.contains("filename"),
                "{error}"
            );

            assert_eq!(directory.outputs(), before);
            directory.assert_no_staging_files();
        }
    }
}

#[rstest]
#[case::input_and_json(0, 1)]
#[case::input_and_html(0, 2)]
#[case::both_outputs(1, 2)]
fn hard_linked_paths_are_rejected(#[case] first: usize, #[case] second: usize) {
    let directory = RunDirectory::new(&format!("hard-links-{first}-{second}"));
    let args = directory.args();

    fs::write(&args[1], "previous result").unwrap();
    fs::write(&args[2], "previous viewer").unwrap();
    fs::remove_file(&args[second]).unwrap();
    fs::hard_link(&args[first], &args[second]).unwrap();

    let before = directory.outputs();

    assert!(runner::run(args).unwrap_err().contains("distinct"));
    assert_eq!(directory.outputs(), before);
    assert_eq!(
        fs::read_to_string(directory.0.join("scenario.json")).unwrap(),
        RAIL
    );
    directory.assert_no_staging_files();
}

#[test]
fn absent_outputs_are_compared_in_the_physical_directory() {
    let directory = RunDirectory::new("directory-alias");

    fs::create_dir(directory.0.join("sub")).unwrap();

    let result = directory.0.join("result.json");
    let alias = directory.0.join("sub/../result.json");
    let staged = output::StagedOutput::new(&result, "result", Some(&alias));

    assert!(matches!(staged, Err(error) if error.contains("distinct")));
    assert!(!result.exists());
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
}

#[cfg(target_os = "macos")]
#[test]
fn cli_rejects_the_same_entry_across_apfs_volume_paths() {
    let directory = RunDirectory::new("apfs-volume-paths");
    let root = fs::canonicalize(&directory.0).unwrap();
    let alias = std::path::Path::new("/System/Volumes/Data").join(root.strip_prefix("/").unwrap());

    if !alias.try_exists().unwrap() || !same_file::is_same_file(&root, &alias).unwrap() {
        return; // This volume has no alternate Data mount path.
    }

    let input = root.join("scenario.json");
    let result = root.join("result.json");

    for (json, html) in [
        (alias.join("scenario.json"), root.join("viewer.html")),
        (result.clone(), alias.join("result.json")),
    ] {
        let args = [input.clone(), json, html].map(PathBuf::into_os_string);

        assert!(runner::run(args).unwrap_err().contains("distinct"));
        assert_eq!(fs::read_to_string(&input).unwrap(), RAIL);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }
}

#[cfg(any(unix, windows))]
fn create_output_link(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    use std::os::unix::fs::symlink as symlink_file;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_file;

    let created = symlink_file(target, link);

    #[cfg(windows)]
    if created
        .as_ref()
        .is_err_and(|e| e.raw_os_error() == Some(1314))
    {
        return false; // Windows can require a privilege for symbolic links.
    }

    created.unwrap();
    true
}

#[cfg(any(unix, windows))]
#[rstest]
#[case::json(1, 2)]
#[case::html(2, 1)]
fn unresolved_output_links_preserve_both_outputs(#[case] link: usize, #[case] other: usize) {
    let directory = RunDirectory::new(&format!("output-link-{link}"));
    let args = directory.args();
    let target_name = Path::new("archive");
    let target = directory.0.join(target_name);
    let chosen = Path::new(&args[link]);

    fs::write(&target, "old contents").unwrap();

    if !create_output_link(target_name, chosen) {
        return;
    }

    runner::run(args.clone()).unwrap();

    let contents = fs::read(&target).unwrap();

    assert_ne!(contents, b"old contents");
    assert_eq!(fs::read_link(chosen).unwrap(), target_name);

    let previous = directory.0.join("previous");

    fs::rename(&target, &previous).unwrap();
    fs::write(&args[other], "keep this output").unwrap();

    let error = runner::run(args.clone()).unwrap_err();

    assert!(
        error.contains("symbolic link") && error.contains(chosen.to_str().unwrap()),
        "{error}"
    );

    if link == 1 {
        let opened = desktop::OpenedScenario::open(Path::new(&args[0])).unwrap();
        let error = opened
            .run()
            .unwrap()
            .save(chosen, &opened.source)
            .unwrap_err();

        assert!(error.contains("symbolic link"), "{error}");
    }

    assert_eq!(fs::read_link(chosen).unwrap(), target_name);
    assert_eq!(
        fs::read_to_string(&args[other]).unwrap(),
        "keep this output"
    );
    assert_eq!(fs::read(previous).unwrap(), contents);
    assert!(!target.exists());
}

#[cfg(any(unix, windows))]
#[test]
fn a_link_appearing_after_staging_is_preserved() {
    let directory = RunDirectory::new("late-output-link");
    let result = directory.0.join("result.json");
    let missing = Path::new("missing.json");
    let staged = output::StagedOutput::new(&result, "new result", None).unwrap();

    if !create_output_link(missing, &result) {
        return;
    }

    assert!(
        staged
            .replace(&result, |_| Ok(()))
            .unwrap_err()
            .contains("symbolic link")
    );
    assert_eq!(fs::read_link(result).unwrap(), missing);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
}

#[cfg(any(unix, windows))]
#[test]
fn retargeting_a_chosen_link_keeps_both_files_intact() {
    let directory = RunDirectory::new("retarget-output");
    let (source, _) = files::ScenarioFile::open(&directory.0.join("scenario.json")).unwrap();
    let link = directory.0.join("latest.json");
    let first = directory.0.join("first.json");
    let next = directory.0.join("next.json");

    fs::write(&first, "first").unwrap();
    fs::write(&next, "next").unwrap();

    if !create_output_link(&first, &link) {
        return;
    }

    let inputs = [&source];
    let destination = files::output_path(&link, &inputs).unwrap();
    let staged = output::StagedOutput::new(&destination, "result", None).unwrap();

    fs::remove_file(&link).unwrap();

    assert!(create_output_link(&next, &link));

    let error = staged
        .replace(&destination, |staged| {
            files::check_destination(staged, &link, &destination, &inputs)
        })
        .unwrap_err();

    assert!(error.contains("target changed"), "{error}");
    assert_eq!(fs::read_to_string(first).unwrap(), "first");
    assert_eq!(fs::read_to_string(link).unwrap(), "next");
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
