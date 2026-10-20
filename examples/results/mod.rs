//! Consumer-owned Rail result persistence; see RESULT_FORMAT.md for version 1.
//! No serialization traits or file I/O are added to the simulation crate.

mod decode;
mod encode;

use std::collections::HashSet;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use urbanflow::analysis::{self, AnalysisError, OperationalAnalysis};
use urbanflow::rail::{RailPosition, RailTrace};
use urbanflow::world::{EdgeKind, World};

/// Maximum UTF-8 JSON size, including whitespace, on both read and write (16 MiB).
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

/// Recorded state and freshly computed analysis, never imported cached metrics.
#[derive(Debug)]
pub struct RailResult {
    pub world: World,
    pub trace: RailTrace,
    pub analysis: Option<OperationalAnalysis>,
}

/// File-qualified I/O, JSON, version, size, reference, or analysis failure.
/// Diagnostics include field/snapshot indices where available; wording may change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultError {
    pub file: PathBuf,
    pub detail: String,
}

impl fmt::Display for ResultError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.file.display(), self.detail)
    }
}

impl std::error::Error for ResultError {}

fn error(file: &Path, detail: impl fmt::Display) -> ResultError {
    ResultError {
        file: file.to_owned(),
        detail: detail.to_string(),
    }
}

fn check_size(size: usize) -> Result<(), String> {
    if size > MAX_BYTES {
        Err(format!("size: result exceeds {MAX_BYTES} bytes (16 MiB)"))
    } else {
        Ok(())
    }
}

/// Reads at most MAX_BYTES + 1 bytes, then parses and validates one UTF-8 file.
/// Oversized input is rejected before JSON decoding, including on non-seekable files.
pub fn load(path: impl AsRef<Path>) -> Result<RailResult, ResultError> {
    let path = path.as_ref();
    let mut bytes = Vec::new();

    std::fs::File::open(path)
        .and_then(|file| file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes))
        .map_err(|detail| error(path, detail))?;
    check_size(bytes.len()).map_err(|detail| error(path, detail))?;

    let json = std::str::from_utf8(&bytes).map_err(|detail| error(path, detail))?;

    parse(path, json)
}

/// Parses a versioned result without running simulation or mutating a live run.
/// Imported analysis may be absent, null, or an object; its contents are discarded.
/// Complete recordings are analyzed anew. Missing initial history yields no
/// analysis; every other core analysis error is returned. Partial recordings,
/// including empty ones, are retained without analysis.
/// Both the input and the canonical JSON with recomputed analysis must fit
/// MAX_BYTES, so an unchanged loaded result can be encoded again.
pub fn parse(file: impl AsRef<Path>, json: &str) -> Result<RailResult, ResultError> {
    let file = file.as_ref();

    check_size(json.len()).map_err(|detail| error(file, detail))?;
    let (world, trace) = decode::document(json).map_err(|detail| error(file, detail))?;
    let analysis = validate(&world, &trace).map_err(|detail| error(file, detail))?;

    check_size(encode::payload(&world, &trace, analysis.as_ref()).len())
        .map_err(|detail| error(file, format!("encoded {detail}")))?;

    Ok(RailResult {
        world,
        trace,
        analysis,
    })
}

/// Produces deterministic JSON in stored order after validation and analysis.
/// IDs/ticks/totals use decimal strings; per-record counts remain exact u32 numbers.
/// The result is JSON, not HTML; consumers must escape it for their embedding context.
/// In-memory failures use `<result>` as their diagnostic filename.
pub fn encode(world: &World, trace: &RailTrace) -> Result<String, ResultError> {
    let file = Path::new("<result>");
    let analysis = validate(world, trace).map_err(|detail| error(file, detail))?;
    let json = encode::payload(world, trace, analysis.as_ref());

    check_size(json.len()).map_err(|detail| error(file, detail))?;

    Ok(json)
}

/// Validates and encodes fully before opening the destination for writing.
/// Invalid state, analysis errors, or oversized results leave existing files intact.
/// Successful writes replace the file; an I/O failure during writing is not atomic.
pub fn save(path: impl AsRef<Path>, world: &World, trace: &RailTrace) -> Result<(), ResultError> {
    let path = path.as_ref();
    let json = encode(world, trace).map_err(|failure| error(path, failure.detail))?;

    std::fs::write(path, json).map_err(|detail| error(path, detail))
}

fn validate(world: &World, trace: &RailTrace) -> Result<Option<OperationalAnalysis>, String> {
    world
        .validate_topology()
        .map_err(|detail| format!("topology: {detail:?}"))?;

    let nodes: HashSet<_> = world.nodes.iter().map(|node| node.id).collect();

    for (index, snapshot) in trace.snapshots.iter().enumerate() {
        let field = format!("trace.snapshots[{index}]");
        let check_node = |node| {
            if nodes.contains(&node) {
                Ok(())
            } else {
                Err(format!("{field}: unknown node {node:?}"))
            }
        };

        match snapshot.position {
            RailPosition::AtStop {
                node,
                dwell_ticks_remaining,
                ..
            } => {
                check_node(node)?;

                if dwell_ticks_remaining == 0 {
                    return Err(format!(
                        "{field}.position.dwell_ticks_remaining: must be positive"
                    ));
                }
            }
            RailPosition::Complete { node, .. } => {
                check_node(node)?;

                if !trace.completed || index + 1 != trace.snapshots.len() {
                    return Err(format!(
                        "{field}.position: Complete must be the last snapshot of a completed trace"
                    ));
                }
            }
            RailPosition::Traveling {
                edge,
                from,
                to,
                travel_ticks_elapsed,
                travel_ticks_total,
                ..
            } => {
                let valid_edge = world.network.edges().get(edge.0).is_some_and(|edge| {
                    edge.kind == EdgeKind::Rail && edge.from == from && edge.to == to
                });

                if !valid_edge {
                    return Err(format!(
                        "{field}.position.edge: must reference a Rail edge with matching endpoints"
                    ));
                }

                if travel_ticks_elapsed >= travel_ticks_total {
                    return Err(format!(
                        "{field}.position: travel_ticks_elapsed must be less than travel_ticks_total"
                    ));
                }
            }
        }

        for (demand_index, passenger) in snapshot.passengers.iter().enumerate() {
            for node in [passenger.demand.origin, passenger.demand.destination] {
                if !nodes.contains(&node) {
                    return Err(format!(
                        "{field}.passengers[{demand_index}]: unknown node {node:?}"
                    ));
                }
            }
        }
    }

    if trace.completed {
        match analysis::analyze(trace) {
            Ok(value) => Ok(Some(value)),
            Err(AnalysisError::MissingInitialState) => Ok(None),
            Err(detail) => Err(detail.to_string()),
        }
    } else {
        validate_partial(trace).map_err(|detail| detail.to_string())?;
        Ok(None)
    }
}

// Complete traces use core preflight; partial traces cannot use that API.
fn validate_partial(trace: &RailTrace) -> Result<(), AnalysisError> {
    for (snapshot_index, snapshot) in trace.snapshots.iter().enumerate() {
        if snapshot.capacity == 0 || snapshot.capacity != trace.snapshots[0].capacity {
            return Err(AnalysisError::InvalidCapacity { snapshot_index });
        }

        let previous = snapshot_index.checked_sub(1).map(|i| &trace.snapshots[i]);

        if let Some(previous) = previous {
            if previous.tick.checked_add(1) != Some(snapshot.tick) {
                return Err(AnalysisError::NonContiguousTicks { snapshot_index });
            }

            if snapshot.passengers.len() != previous.passengers.len() {
                return Err(AnalysisError::InconsistentDemands { snapshot_index });
            }
        }

        let mut onboard = 0u64;

        for (demand_index, passenger) in snapshot.passengers.iter().enumerate() {
            if passenger
                .waiting
                .checked_add(passenger.onboard)
                .and_then(|count| count.checked_add(passenger.arrived))
                .and_then(|count| count.checked_add(passenger.unserved))
                != Some(passenger.demand.amount)
            {
                return Err(AnalysisError::InvalidPassengerCounts { demand_index });
            }

            if let Some(previous) = previous {
                let previous = &previous.passengers[demand_index];

                if passenger.demand != previous.demand {
                    return Err(AnalysisError::InconsistentDemands { snapshot_index });
                }

                let arrivals = passenger.arrived.checked_sub(previous.arrived);

                if passenger.waiting > previous.waiting
                    || !arrivals.is_some_and(|count| count <= previous.onboard)
                    || passenger.unserved != previous.unserved
                {
                    return Err(AnalysisError::InvalidPassengerTransition {
                        snapshot_index,
                        demand_index,
                    });
                }
            }

            onboard = onboard
                .checked_add(u64::from(passenger.onboard))
                .ok_or(AnalysisError::CountOverflow)?;
        }

        if snapshot.occupancy > snapshot.capacity || u64::from(snapshot.occupancy) != onboard {
            return Err(AnalysisError::InvalidOccupancy { snapshot_index });
        }
    }

    Ok(())
}
