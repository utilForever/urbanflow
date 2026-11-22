use super::*;
use std::fs;
#[cfg(any(windows, target_vendor = "apple"))]
use std::io;
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

#[cfg(any(target_vendor = "apple", windows))]
#[test]
fn new_results_can_be_saved_without_permission_to_create_subdirectories() {
    use std::process::Command;

    let directory = Directory::new("files-only");
    let outputs = directory.0.join("outputs");

    fs::create_dir(&outputs).unwrap();

    #[cfg(target_vendor = "apple")]
    let status = Command::new("chmod")
        .args(["+a", "everyone deny add_subdirectory"])
        .arg(&outputs)
        .status();
    #[cfg(windows)]
    let status = Command::new("icacls")
        .arg(&outputs)
        .args(["/deny", "*S-1-1-0:(AD)"])
        .status();

    assert!(status.unwrap().success());
    assert_eq!(
        fs::create_dir(outputs.join("subdir")).unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );

    for (input, result) in [
        (directory.0.join("scenario.json"), "scenario.json"),
        (outputs.join("local.json"), "result.json"),
    ] {
        fs::write(&input, DEMO_SCENARIO).unwrap();

        let opened = OpenedScenario::open(&input).unwrap();
        let recording = opened.run().unwrap();
        let result = outputs.join(result);

        recording.save(&result, &opened.source).unwrap();

        assert_eq!(results::load(result).unwrap().trace, recording.data.trace);
    }

    assert_eq!(fs::read_dir(outputs).unwrap().count(), 3);
}

#[cfg(any(target_vendor = "apple", windows))]
#[test]
fn new_results_inherit_the_same_acl_as_a_directly_created_file() {
    use std::process::Command;

    let directory = Directory::new("direct-child-acl");
    let input = directory.0.join("scenario.json");
    let result = directory.0.join("result.json");

    fs::write(&input, DEMO_SCENARIO).unwrap();

    let selection = OpenedScenario::open(&input).unwrap();
    let recording = selection.run().unwrap();

    #[cfg(target_vendor = "apple")]
    let acl = || {
        let output = Command::new("ls").arg("-le").arg(&result).output().unwrap();

        assert!(output.status.success());

        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .skip(1)
            .collect::<Vec<_>>()
            .join("\n")
    };
    #[cfg(windows)]
    let acl = || {
        let output = Command::new("icacls").arg(&result).output().unwrap();
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).into_owned()
    };

    fs::write(&result, "before inheritance").unwrap();

    let baseline = acl();

    fs::remove_file(&result).unwrap();

    #[cfg(target_vendor = "apple")]
    let grant = Command::new("chmod")
        .args(["+a", "group:staff allow read,file_inherit,limit_inherit"])
        .arg(&directory.0)
        .output()
        .unwrap();
    #[cfg(windows)]
    let grant = Command::new("icacls")
        .arg(&directory.0)
        .args(["/grant", "*S-1-5-32-546:(OI)(NP)(IO)R"])
        .output()
        .unwrap();

    assert!(
        grant.status.success(),
        "{}",
        String::from_utf8_lossy(&grant.stderr)
    );

    fs::write(&result, "direct child").unwrap();

    let expected = acl();

    assert_ne!(
        expected, baseline,
        "the direct child must inherit the new grant"
    );

    fs::remove_file(&result).unwrap();
    recording.save(&result, &selection.source).unwrap();

    assert_eq!(acl(), expected);
    assert_eq!(results::load(result).unwrap().trace, recording.data.trace);
}

#[cfg(windows)]
#[test]
fn encrypted_results_are_rejected_before_writing_and_before_replacement() {
    use std::io::Write;
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use windows_sys::Win32::Foundation::{
        ERROR_EFS_DISABLED, ERROR_NO_EFS, ERROR_NO_USER_KEYS, ERROR_NOT_SUPPORTED,
        ERROR_VOLUME_NOT_SUPPORT_EFS,
    };
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_ENCRYPTED;

    let directory = Directory::new("encrypted-result");
    let result = directory.0.join("result.json");
    let pending = output::StagedOutput::new(&result, "replacement", None).unwrap();
    let mut protected = match fs::File::options()
        .write(true)
        .create_new(true)
        .attributes(FILE_ATTRIBUTE_ENCRYPTED)
        .open(&result)
    {
        Ok(file) => file,
        Err(e)
            if e.raw_os_error().is_some_and(|code| {
                matches!(
                    code as u32,
                    ERROR_EFS_DISABLED
                        | ERROR_NOT_SUPPORTED
                        | ERROR_NO_EFS
                        | ERROR_NO_USER_KEYS
                        | ERROR_VOLUME_NOT_SUPPORT_EFS
                )
            }) =>
        {
            eprintln!("EFS unavailable for this test directory: {e}");
            return;
        }
        Err(e) => panic!("creating the EFS test file: {e}"),
    };

    if protected.metadata().unwrap().file_attributes() & FILE_ATTRIBUTE_ENCRYPTED == 0 {
        eprintln!("The test volume did not create an encrypted file");
        return;
    }

    protected.write_all(b"previous result").unwrap();
    drop(protected);

    assert!(
        output::StagedOutput::new(&result, "replacement", None)
            .err()
            .unwrap()
            .contains("EFS")
    );
    assert!(
        pending
            .replace(&result, |_| Ok(()))
            .unwrap_err()
            .contains("EFS")
    );
    assert_eq!(fs::read(&result).unwrap(), b"previous result");
    assert_ne!(
        fs::metadata(&result).unwrap().file_attributes() & FILE_ATTRIBUTE_ENCRYPTED,
        0
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[cfg(any(unix, windows))]
#[test]
fn save_can_replace_a_result_without_reading_its_contents() {
    let directory = Directory::new("write-only");
    let input = directory.0.join("input.json");
    let result = directory.0.join("result.json");

    fs::write(&input, DEMO_SCENARIO).unwrap();
    fs::write(&result, "old result").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&result, fs::Permissions::from_mode(0o200)).unwrap();
    }
    #[cfg(windows)]
    {
        // Deny reading file data but retain metadata and replacement rights in our directory.
        let acl = std::process::Command::new("icacls")
            .arg(&result)
            .args(["/deny", "*S-1-1-0:(RD)"])
            .output()
            .unwrap();
        assert!(
            acl.status.success(),
            "{}",
            String::from_utf8_lossy(&acl.stderr)
        );
        assert!(fs::File::open(&result).is_err());
    }

    let selection = OpenedScenario::open(&input).unwrap();
    let recording = selection.run().unwrap();

    recording.save(&result, &selection.source).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_eq!(
            fs::metadata(&result).unwrap().permissions().mode() & 0o777,
            0o200
        );

        fs::set_permissions(&result, fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[cfg(windows)]
    {
        assert_eq!(
            fs::File::open(&result).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert!(
            std::process::Command::new("icacls")
                .arg(&result)
                .args(["/remove:d", "*S-1-1-0"])
                .status()
                .unwrap()
                .success()
        );
    }
    assert_eq!(results::load(result).unwrap().trace, recording.data.trace);
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
