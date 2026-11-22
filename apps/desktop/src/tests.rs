use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use workflow::OpenedScenario;

const DEMO_SCENARIO: &str = include_str!("../../../tests/fixtures/scenarios/rail-demo.json");

struct Directory(PathBuf);

impl Directory {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("urbanflow-desktop-{}-{name}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn startup_result_uses_the_existing_adapter_and_core_analysis() {
    let result = results::parse("rail_service.json", DEMO).unwrap();

    assert_eq!(result.world.nodes.len(), 3);
    assert_eq!(result.trace.snapshots.len(), 7);
    assert_eq!(result.trace.snapshots.last().unwrap().tick, 6);
    assert!(result.trace.completed);
    assert_eq!(
        result.analysis.unwrap(),
        urbanflow::analysis::analyze(&result.trace).unwrap()
    );
}

#[cfg(any(unix, windows))]
#[test]
fn save_protects_replaced_and_retargeted_input_links() {
    #[cfg(unix)]
    use std::os::unix::fs::symlink as symlink_file;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_file;

    let directory = Directory::new("input-links");
    let original = directory.0.join("original.json");
    let first = directory.0.join("first.json");
    let selected = directory.0.join("selected.json");
    let edited = directory.0.join("edited.json");

    fs::write(&original, DEMO_SCENARIO).unwrap();

    let link = symlink_file(&original, &first);

    #[cfg(windows)]
    if link.as_ref().is_err_and(|e| e.raw_os_error() == Some(1314)) {
        eprintln!("Windows did not grant permission to create a symbolic link");
        return;
    }

    link.unwrap();
    symlink_file(&original, &selected).unwrap();

    let recording = OpenedScenario::open(&first).unwrap().run().unwrap();
    let selection = OpenedScenario::open(&selected).unwrap();

    // The selected entry survives when just its target is moved.
    let moved = directory.0.join("moved.json");
    fs::rename(&original, &moved).unwrap();

    #[cfg(unix)]
    use std::os::unix::fs::symlink as link_directory;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_dir as link_directory;

    let via = directory.0.join("via");

    link_directory(&directory.0, &via).unwrap();

    for path in [&first, &selected] {
        let alias = via.join(path.file_name().unwrap());

        assert!(
            recording
                .save(&alias, &selection.source)
                .unwrap_err()
                .contains("symbolic link")
        );
        assert_eq!(fs::read_link(path).unwrap(), original);
    }

    fs::rename(&moved, &original).unwrap();

    let contents = DEMO_SCENARIO.replace("\"max_ticks\": \"32\"", "\"max_ticks\": \"3\"");

    fs::write(&edited, &contents).unwrap();
    fs::rename(&edited, &first).unwrap();
    fs::write(&edited, &contents).unwrap();
    fs::remove_file(&selected).unwrap();

    symlink_file(&edited, &selected).unwrap();

    let alias = directory.0.join("edited-link.json");
    fs::hard_link(&edited, &alias).unwrap();

    let case_alias = directory.0.join("FIRST.JSON");
    let mut paths = vec![&first, &selected, &edited, &alias];

    if case_alias.try_exists().unwrap() {
        paths.push(&case_alias);
    }

    for path in paths {
        assert!(
            recording
                .save(path, &selection.source)
                .unwrap_err()
                .contains("distinct")
        );
        assert_eq!(fs::read_to_string(path).unwrap(), contents);
    }

    assert!(
        recording
            .save(&original, &selection.source)
            .unwrap_err()
            .contains("distinct")
    );
    assert_eq!(fs::read_to_string(&original).unwrap(), DEMO_SCENARIO);

    let revised = directory.0.join("revision.json");

    fs::write(&revised, &contents).unwrap();
    fs::remove_file(&first).unwrap();
    fs::rename(&original, &moved).unwrap();

    symlink_file(&revised, &original).unwrap();

    for path in [&original, &revised] {
        assert!(
            recording
                .save(path, &selection.source)
                .unwrap_err()
                .contains("distinct")
        );
        assert_eq!(fs::read_to_string(&revised).unwrap(), contents);
    }

    let exports = directory.0.join("exports");
    fs::create_dir(&exports).unwrap();

    let result = exports.join("result.json");
    recording.save(&result, &selection.source).unwrap();

    fs::remove_file(&selected).unwrap();

    let circular = Path::new("selected.json");
    symlink_file(circular, &selected).unwrap();

    fs::write(&result, "previous result").unwrap();
    recording.save(&result, &selection.source).unwrap();

    assert_eq!(results::load(result).unwrap().trace, recording.data.trace);
    assert!(
        recording
            .save(&via.join("selected.json"), &selection.source)
            .is_err()
    );
    assert_eq!(fs::read_link(&selected).unwrap(), circular);
    assert!(
        recording
            .save(&moved, &selection.source)
            .unwrap_err()
            .contains("distinct")
    );
    assert_eq!(fs::read_to_string(moved).unwrap(), DEMO_SCENARIO);
}

#[test]
fn save_survives_an_input_directory_becoming_a_file() {
    let directory = Directory::new("moved-parent");
    let parent = directory.0.join("inputs");
    let input = parent.join("scenario.json");
    let saved = directory.0.join("scenario.json");

    fs::create_dir(&parent).unwrap();
    fs::write(&input, DEMO_SCENARIO).unwrap();

    let selection = OpenedScenario::open(&input).unwrap();
    let recording = selection.run().unwrap();

    fs::rename(&parent, directory.0.join("moved")).unwrap();
    fs::write(&parent, "a regular file").unwrap();

    for _ in 0..2 {
        recording.save(&saved, &selection.source).unwrap();
        assert_eq!(results::load(&saved).unwrap().trace, recording.data.trace);
    }
}

#[cfg(windows)]
#[test]
fn opened_scenario_allows_exclusive_edits_and_keeps_the_recorded_input() {
    use std::io::Write;
    use std::os::windows::fs::OpenOptionsExt;

    let directory = Directory::new("exclusive-editor");
    let input = directory.0.join("scenario.json");

    fs::write(&input, DEMO_SCENARIO).unwrap();

    let selection = OpenedScenario::open(&input).unwrap();
    let recording = selection.run().unwrap();

    let mut editor = fs::File::options()
        .write(true)
        .truncate(true)
        .share_mode(0)
        .open(&input)
        .unwrap();
    editor.write_all(b"edited").unwrap();
    drop(editor);

    let moved = directory.0.join("moved.json");
    fs::rename(input, &moved).unwrap();

    assert!(
        recording
            .save(&moved, &selection.source)
            .unwrap_err()
            .contains("distinct")
    );
    assert_eq!(fs::read_to_string(moved).unwrap(), "edited");
}
