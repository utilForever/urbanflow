//! Consumer adapter for SCENARIO_FORMAT.md version 1; no simulation is run here.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::value::RawValue;
use urbanflow::demand::Demand;
use urbanflow::env::{Env, InitError};
use urbanflow::rail::{RailInitError, RailPassengers, RailRoute, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{AddEdgeError, EdgeId, EdgeKind, Network, Node, NodeId, World};

/// Fully validated inputs, with no actions or ticks performed.
#[derive(Debug)]
pub enum Scenario {
    Env(Env),
    Rail(RailScenario),
}

/// A fresh Rail service. Pass these fields to the existing recording API.
#[derive(Debug)]
pub struct RailScenario {
    pub world: World,
    pub vehicle: RailVehicle,
    pub passengers: RailPassengers,
    pub clock: SimulationClock,
    pub max_ticks: u64,
}

/// A file-qualified I/O, JSON/shape, version/kind, range, or validation error.
/// Diagnostics identify the offending field/index; wording is not a stable API.
#[derive(Debug)]
pub struct LoadError {
    pub file: PathBuf,
    pub detail: String,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.detail)
    }
}

impl std::error::Error for LoadError {}

/// Reads one UTF-8 JSON file and validates it before returning any state.
/// No existing run is mutated, no input is repaired, and no work limit is consumed.
pub fn load(path: impl AsRef<Path>) -> Result<Scenario, LoadError> {
    let path = path.as_ref();
    let json = std::fs::read_to_string(path).map_err(|error| LoadError {
        file: path.to_owned(),
        detail: format!("I/O: {error}"),
    })?;

    parse(path, &json)
}

/// Parses already-read UTF-8 JSON, using `file` to qualify diagnostics.
///
/// Checks the version before interpreting version-specific fields. Missing,
/// unknown, null, duplicate, or wrongly typed fields are rejected. Decimal
/// strings are checked as u64, then IDs/step limits are checked against usize.
/// Ordered arrays, duplicate demands, zero demands, and repeated route visits
/// are retained. Core constructors own topology, budget, route, and vehicle
/// validation. Rail demand endpoints use the same validation as Env.
///
/// Success does not guarantee a full run fits in time or memory. Consumers must
/// retain the core's checked execution errors and partial-recording semantics.
pub fn parse(file: impl AsRef<Path>, json: &str) -> Result<Scenario, LoadError> {
    parse_document(json).map_err(|detail| LoadError {
        file: file.as_ref().to_owned(),
        detail,
    })
}

#[derive(Deserialize)]
struct Version {
    version: Box<RawValue>,
}

#[derive(Deserialize)]
struct Kind {
    kind: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvFile {
    #[serde(rename = "version")]
    _version: u32,
    #[serde(rename = "kind")]
    _kind: String,
    nodes: Vec<String>,
    edges: Vec<Box<RawValue>>,
    demands: Vec<Box<RawValue>>,
    budget: f64,
    max_steps: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RailFile {
    #[serde(rename = "version")]
    _version: u32,
    #[serde(rename = "kind")]
    _kind: String,
    nodes: Vec<String>,
    edges: Vec<Box<RawValue>>,
    demands: Vec<Box<RawValue>>,
    route: Vec<String>,
    vehicle: Box<RawValue>,
    max_ticks: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeFile {
    from: String,
    to: String,
    kind: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DemandFile {
    origin: String,
    destination: String,
    amount: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VehicleFile {
    capacity: u32,
    travel_ticks_per_edge: String,
    dwell_ticks_per_stop: String,
}

fn decode<T: DeserializeOwned>(json: &str, field: &str) -> Result<T, String> {
    if !json.trim_start().starts_with('{') {
        return Err(format!("{field}: JSON/shape: expected object"));
    }

    let mut deserializer = serde_json::Deserializer::from_str(json);
    let value = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        let path = error.path().to_string();
        let path = if field.is_empty() {
            path
        } else if path == "." {
            field.to_owned()
        } else {
            format!("{field}.{path}")
        };
        format!("{path}: JSON/shape: {}", error.inner())
    })?;

    deserializer
        .end()
        .map_err(|error| format!("JSON: {error}"))?;
    Ok(value)
}

fn decimal(value: &str, field: &str) -> Result<u64, String> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(format!(
            "{field}: expected decimal string matching 0|[1-9][0-9]*"
        ));
    }

    value
        .parse()
        .map_err(|_| format!("{field}: numeric range exceeds u64::MAX"))
}

fn index(value: &str, field: &str) -> Result<usize, String> {
    usize::try_from(decimal(value, field)?).map_err(|_| {
        format!(
            "{field}: numeric range does not fit {}-bit usize",
            usize::BITS
        )
    })
}

fn parse_document(json: &str) -> Result<Scenario, String> {
    let version = decode::<Version>(json, "")?;
    let version = version.version.get();

    if !version.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("version: expected unsigned JSON integer token".into());
    }

    if version != "1" {
        return Err(format!(
            "version: unsupported version {version}; expected 1"
        ));
    }

    match decode::<Kind>(json, "")?.kind.as_str() {
        "env" => {
            let input: EnvFile = decode(json, "")?;
            let max_steps = index(&input.max_steps, "max_steps")?;
            let (world, demands) = build_world(input.nodes, input.edges, input.demands)?;

            Env::new(world, demands, input.budget, max_steps)
                .map(Scenario::Env)
                .map_err(|error| format!("budget: must be finite and nonnegative ({error:?})"))
        }
        "rail" => {
            let input: RailFile = decode(json, "")?;
            let settings: VehicleFile = decode(input.vehicle.get(), "vehicle")?;
            let max_ticks = decimal(&input.max_ticks, "max_ticks")?;
            let travel = decimal(
                &settings.travel_ticks_per_edge,
                "vehicle.travel_ticks_per_edge",
            )?;
            let dwell = decimal(
                &settings.dwell_ticks_per_stop,
                "vehicle.dwell_ticks_per_stop",
            )?;
            let edges = input
                .route
                .iter()
                .enumerate()
                .map(|(i, value)| index(value, &format!("route[{i}]")).map(EdgeId))
                .collect::<Result<Vec<_>, _>>()?;
            let (world, demands) = build_world(input.nodes, input.edges, input.demands)?;
            let route = RailRoute::new(&world.network, edges.clone())
                .map_err(|error| route_error(&edges, error))?;
            let vehicle =
                RailVehicle::new(route, settings.capacity, travel, dwell).map_err(|error| {
                    let field = match error {
                        RailInitError::InvalidCapacity => "capacity",
                        RailInitError::InvalidTravelTicks => "travel_ticks_per_edge",
                        RailInitError::InvalidDwellTicks => "dwell_ticks_per_stop",
                        _ => unreachable!("route already validated"),
                    };
                    format!("vehicle.{field}: invalid vehicle setting; must be positive")
                })?;

            Ok(Scenario::Rail(RailScenario {
                world,
                vehicle,
                passengers: RailPassengers::new(&demands),
                clock: SimulationClock::default(),
                max_ticks,
            }))
        }
        kind => Err(format!(
            "kind: unsupported kind {kind:?}; expected env or rail"
        )),
    }
}

fn build_world(
    nodes: Vec<String>,
    edges: Vec<Box<RawValue>>,
    demands: Vec<Box<RawValue>>,
) -> Result<(World, Vec<Demand>), String> {
    let nodes = nodes
        .iter()
        .enumerate()
        .map(|(i, value)| index(value, &format!("nodes[{i}]")).map(|id| Node { id: NodeId(id) }))
        .collect::<Result<Vec<_>, _>>()?;
    let mut network = Network::new();

    for (i, edge) in edges.iter().enumerate() {
        let edge: EdgeFile = decode(edge.get(), &format!("edges[{i}]"))?;
        let from = NodeId(index(&edge.from, &format!("edges[{i}].from"))?);
        let to = NodeId(index(&edge.to, &format!("edges[{i}].to"))?);
        let kind = match edge.kind.as_str() {
            "Road" => EdgeKind::Road,
            "Rail" => EdgeKind::Rail,
            _ => return Err(format!("edges[{i}].kind: expected Road or Rail")),
        };

        network.add_edge(from, to, kind).map_err(|error| {
            let reason = match error {
                AddEdgeError::DuplicateEdge => "duplicate (from, to, kind) edge",
                AddEdgeError::SelfConnection => "self-connected edge",
            };
            format!("edges[{i}]: invalid topology: {reason}")
        })?;
    }

    let world = World { nodes, network };
    let demands = demands
        .iter()
        .enumerate()
        .map(|(i, demand)| {
            let demand: DemandFile = decode(demand.get(), &format!("demands[{i}]"))?;
            Ok(Demand::new(
                NodeId(index(&demand.origin, &format!("demands[{i}].origin"))?),
                NodeId(index(
                    &demand.destination,
                    &format!("demands[{i}].destination"),
                )?),
                demand.amount,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;

    // Reuse topology and endpoint rules for both kinds, without constructing an Env.
    Env::validate_inputs(&world, &demands, 0.0)
        .map_err(|error| input_error(&world, &demands, error))?;

    Ok((world, demands))
}

fn input_error(world: &World, demands: &[Demand], error: InitError) -> String {
    match error {
        InitError::DuplicateNode(id) => {
            let i = world
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| node.id == id)
                .nth(1)
                .unwrap()
                .0;
            format!("nodes[{i}]: invalid topology: duplicate node ID {}", id.0)
        }
        InitError::UnknownEdgeEndpoint(id) => {
            let edge = world
                .network
                .edges()
                .iter()
                .find(|edge| edge.from == id || edge.to == id)
                .unwrap();
            let field = if edge.from == id { "from" } else { "to" };
            format!("edges[{}].{field}: unknown node ID {}", edge.id.0, id.0)
        }
        InitError::UnknownDemandEndpoint(id) => {
            let (i, demand) = demands
                .iter()
                .enumerate()
                .find(|(_, demand)| demand.origin == id || demand.destination == id)
                .unwrap();
            let field = if demand.origin == id {
                "origin"
            } else {
                "destination"
            };
            format!("demands[{i}].{field}: unknown node ID {}", id.0)
        }
        InitError::InvalidBudget => unreachable!("validation budget is zero"),
    }
}

fn route_error(edges: &[EdgeId], error: RailInitError) -> String {
    match error {
        RailInitError::EmptyRoute => {
            "route: invalid route: must contain at least one Rail edge".into()
        }
        RailInitError::UnknownEdge(id) | RailInitError::NonRailEdge(id) => {
            let i = edges.iter().position(|edge| *edge == id).unwrap();
            let reason = if matches!(error, RailInitError::UnknownEdge(_)) {
                "unknown edge ID"
            } else {
                "non-Rail edge ID"
            };
            format!("route[{i}]: {reason} {}", id.0)
        }
        RailInitError::DisconnectedEdges { previous, next } => {
            let i = edges
                .windows(2)
                .position(|pair| pair == [previous, next])
                .unwrap()
                + 1;
            format!(
                "route[{i}]: disconnected edge {}; previous edge is {}",
                next.0, previous.0
            )
        }
        _ => unreachable!("route validation does not check vehicle settings"),
    }
}
