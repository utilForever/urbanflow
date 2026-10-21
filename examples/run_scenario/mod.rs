//! Bounded Rail scenario execution and consumer-owned JSON/HTML output.

#[path = "../rail_viewer/mod.rs"]
pub mod rail_viewer;
#[path = "../scenario/mod.rs"]
pub mod scenario;

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const USAGE: &str = "usage: cargo run --example run_scenario -- <scenario.json> <result.json> [viewer.html]\nRuns kind=rail with the file's max_ticks; Env execution is unsupported.\nUse distinct input/output paths and existing output directories.";

/// Runs one Rail scenario, or returns usage for a lone `--help`/`-h` argument.
///
/// Loading, recording, analysis, encoding, and optional rendering all finish
/// before any destination is replaced. Zero/short limits succeed with partial
/// results. Env inputs fail because this consumer defines no action policy or
/// Env result format. Errors include the input or output path where applicable.
///
/// Outputs are staged beside their destinations, then renamed: HTML first,
/// JSON last. Each replacement is atomic, but the pair is not a filesystem
/// transaction: a final JSON rename failure can leave the new HTML installed.
/// Temporary files are removed on ordinary failures; crash durability is not
/// guaranteed. Resolved input/output aliases and output directories are rejected.
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

    let staged_result = StagedOutput::new(&result, &json, html.as_deref())?;

    if let (Some(path), Some(contents)) = (&html, &rendered) {
        StagedOutput::new(path, contents, Some(&result))?.replace(path)?;
    }

    staged_result.replace(&result)?;

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

struct StagedOutput(PathBuf);

impl StagedOutput {
    fn new(
        destination: &Path,
        contents: &str,
        other_output: Option<&Path>,
    ) -> Result<Self, String> {
        static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

        loop {
            let path = destination.with_file_name(format!(
                ".urbanflow-{}-{}.tmp",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed),
            ));

            if path == destination || Some(path.as_path()) == other_output {
                continue;
            }

            let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => file,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("{}: staging: {e}", destination.display())),
            };
            let staged = Self(path);
            let written = file
                .write_all(contents.as_bytes())
                .and_then(|()| file.sync_all());

            drop(file);
            written.map_err(|e| format!("{}: staging: {e}", destination.display()))?;

            return Ok(staged);
        }
    }

    fn replace(self, destination: &Path) -> Result<(), String> {
        fs::rename(&self.0, destination)
            .map_err(|e| format!("{}: replacement: {e}", destination.display()))
    }
}

impl Drop for StagedOutput {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
