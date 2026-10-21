//! Bounded Rail scenario execution and consumer-owned JSON/HTML output.

#[path = "../rail_viewer/mod.rs"]
pub mod rail_viewer;
#[path = "../scenario/mod.rs"]
pub mod scenario;

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: cargo run --example run_scenario -- <scenario.json> <result.json> [viewer.html]\nRuns kind=rail with the file's max_ticks; Env execution is unsupported.\nUse distinct input/output paths and existing output directories.";

/// Runs one Rail scenario, or returns usage for a lone `--help`/`-h` argument.
///
/// Loading, recording, analysis, encoding, and optional rendering all finish
/// before any destination is replaced. Zero/short limits succeed with partial
/// results. Env inputs fail because this consumer defines no action policy or
/// Env result format. Errors include the input or output path where applicable.
///
/// Files are written only after all computation succeeds, HTML first and JSON
/// last. Successful writes replace files; an I/O error during a write can leave
/// that output incomplete. Resolved input/output aliases and directories as
/// outputs are rejected. File writes are not a filesystem transaction.
pub fn run(args: impl IntoIterator<Item = OsString>) -> Result<String, String> {
    let args: Vec<_> = args.into_iter().collect();

    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        return Ok(USAGE.to_owned());
    }

    if !(2..=3).contains(&args.len()) {
        return Err(USAGE.to_owned());
    }

    let input = Path::new(&args[0]);
    let scenario::Scenario::Rail(mut rail) = scenario::load(input).map_err(|e| e.to_string())?
    else {
        return Err(format!(
            "{}: kind: run_scenario supports only rail; Env requires an action policy and result format",
            input.display()
        ));
    };
    let input_path = fs::canonicalize(input).map_err(|e| format!("{}: {e}", input.display()))?;
    let result = output_path(Path::new(&args[1]))?;
    let html = args
        .get(2)
        .map(|arg| output_path(Path::new(arg)))
        .transpose()?;

    if result == input_path
        || html
            .as_ref()
            .is_some_and(|path| *path == input_path || *path == result)
    {
        return Err("input, result, and HTML paths must be distinct".to_owned());
    }

    let trace = rail
        .vehicle
        .record_trace(&mut rail.clock, &mut rail.passengers, rail.max_ticks)
        .map_err(|e| format!("{}: execution: {e:?}", input.display()))?;
    let json = rail_viewer::results::encode(&rail.world, &trace)
        .map_err(|e| format!("{}: {}", result.display(), e.detail))?;
    let rendered = html
        .as_ref()
        .map(|path| {
            rail_viewer::render(&rail.world, &trace)
                .map_err(|e| format!("{}: {}", path.display(), e.detail))
        })
        .transpose()?;

    if let (Some(path), Some(contents)) = (&html, &rendered) {
        fs::write(path, contents).map_err(|e| format!("{}: write: {e}", path.display()))?;
    }

    fs::write(&result, json).map_err(|e| format!("{}: write: {e}", result.display()))?;

    Ok(format!(
        "{} at tick {}; {} snapshots; result: {}{}",
        if trace.completed {
            "Completed"
        } else {
            "Partial"
        },
        rail.clock.tick(),
        trace.snapshots.len(),
        result.display(),
        html.map(|path| format!("; HTML: {}", path.display()))
            .unwrap_or_default(),
    ))
}

fn output_path(path: &Path) -> Result<PathBuf, String> {
    let resolved = if path.exists() {
        fs::canonicalize(path)
    } else {
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::canonicalize(parent).and_then(|parent| {
            path.file_name()
                .map(|name| parent.join(name))
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "expected an output filename")
                })
        })
    }
    .map_err(|e| format!("{}: {e}", path.display()))?;

    if resolved.exists() && !resolved.is_file() {
        return Err(format!("{}: output must be a regular file", path.display()));
    }

    Ok(resolved)
}
