//! Bounded Rail scenario execution and consumer-owned JSON/HTML output.

pub(crate) mod files;
pub(crate) mod output;
#[path = "../rail_viewer/mod.rs"]
pub mod rail_viewer;
#[path = "../scenario/mod.rs"]
pub mod scenario;

use std::ffi::OsString;
use std::path::Path;

use files::{ScenarioFile, output_path, same_file};
use output::StagedOutput;

const USAGE: &str = "usage: cargo run --example run_scenario -- <scenario.json> <result.json> [viewer.html]\nRuns kind=rail with the file's max_ticks; Env execution is unsupported.\nUse distinct input/output paths and existing output directories.";

/// Runs one Rail scenario, or returns usage for a lone `--help`/`-h` argument.
///
/// Loading, recording, analysis, encoding, and optional rendering all finish
/// before any destination is replaced. Zero/short limits succeed with partial
/// results. Env inputs fail because this consumer defines no action policy or
/// Env result format. Errors include the input or output path where applicable.
/// Each path must end in a filename. Trailing separators and final `.` or `..`
/// components are rejected before recording or writing.
///
/// Outputs are staged in sibling directories and compared using native filename lookup.
/// Replacements retain ownership and access rules. An existing EFS-encrypted output is rejected.
/// Files are then renamed: HTML first, JSON last. Each replacement is atomic,
/// but the pair is not a filesystem transaction: a final JSON rename failure
/// can leave the new HTML installed.
/// Temporary files are removed on ordinary failures; crash durability is not
/// guaranteed. Paths to the same file or directory entry are rejected, as are
/// output directories and symbolic links whose targets cannot be resolved.
pub fn run(args: impl IntoIterator<Item = OsString>) -> Result<String, String> {
    let args: Vec<_> = args.into_iter().collect();

    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        return Ok(USAGE.to_owned());
    }

    if !(2..=3).contains(&args.len()) {
        return Err(USAGE.to_owned());
    }

    let input = Path::new(&args[0]);
    let (source, mut rail) = ScenarioFile::open(input)?;
    let inputs = [&source];
    let result_arg = Path::new(&args[1]);
    let result = output_path(result_arg, &inputs)?;
    let html = args
        .get(2)
        .map(|arg| output_path(Path::new(arg), &inputs))
        .transpose()?;
    let check_outputs = || {
        if let Some(path) = &html
            && same_file(&result, path)?
        {
            return Err("result and HTML paths must be distinct".to_owned());
        }

        Ok(())
    };

    check_outputs()?;

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

    if let Some((path, contents)) = html.as_deref().zip(rendered.as_deref()) {
        StagedOutput::new(path, contents, Some(&result))?.replace(path, |_| check_outputs())?;
    }

    staged_result.replace(&result, |_| check_outputs())?;

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
