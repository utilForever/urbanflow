use std::fmt;

use serde::{Deserialize, Deserializer, de::DeserializeOwned, de::Error};
use serde_json::value::RawValue;
use urbanflow::demand::Demand;
use urbanflow::rail::{PassengerState, RailPosition, RailSnapshot, RailTrace};
use urbanflow::world::{EdgeId, EdgeKind, Network, Node, NodeId, World};

#[derive(Deserialize)]
struct Version {
    version: Box<RawValue>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    nodes: Vec<NodeFile>,
    #[serde(deserialize_with = "objects")]
    edges: Vec<EdgeFile>,
    #[serde(deserialize_with = "object")]
    trace: TraceFile,
    // Imported summaries are only a cache; the core recomputes every value.
    analysis: Option<Box<RawValue>>,
}

#[derive(Deserialize)]
struct NodeFile(#[serde(deserialize_with = "node_id")] NodeId);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeFile {
    #[serde(deserialize_with = "edge_id")]
    id: EdgeId,
    #[serde(deserialize_with = "node_id")]
    from: NodeId,
    #[serde(deserialize_with = "node_id")]
    to: NodeId,
    #[serde(deserialize_with = "edge_kind")]
    kind: EdgeKind,
}

fn edge_kind<'de, D: Deserializer<'de>>(deserializer: D) -> Result<EdgeKind, D::Error> {
    match String::deserialize(deserializer)?.as_str() {
        "Road" => Ok(EdgeKind::Road),
        "Rail" => Ok(EdgeKind::Rail),
        _ => Err(D::Error::custom("expected Road or Rail")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TraceFile {
    completed: bool,
    #[serde(deserialize_with = "objects")]
    snapshots: Vec<Snapshot>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    #[serde(deserialize_with = "decimal")]
    tick: u64,
    #[serde(deserialize_with = "object")]
    position: PositionFile,
    occupancy: u32,
    capacity: u32,
    #[serde(deserialize_with = "objects")]
    passengers: Vec<Passenger>,
}

#[derive(Deserialize)]
struct PositionFile(#[serde(with = "Position")] RailPosition);

#[derive(Deserialize)]
#[serde(remote = "RailPosition", tag = "kind", deny_unknown_fields)]
enum Position {
    AtStop {
        #[serde(deserialize_with = "decimal")]
        stop_index: usize,
        #[serde(deserialize_with = "node_id")]
        node: NodeId,
        #[serde(deserialize_with = "decimal")]
        dwell_ticks_remaining: u64,
    },
    Traveling {
        #[serde(deserialize_with = "decimal")]
        edge_index: usize,
        #[serde(deserialize_with = "edge_id")]
        edge: EdgeId,
        #[serde(deserialize_with = "node_id")]
        from: NodeId,
        #[serde(deserialize_with = "node_id")]
        to: NodeId,
        #[serde(deserialize_with = "decimal")]
        travel_ticks_elapsed: u64,
        #[serde(deserialize_with = "decimal")]
        travel_ticks_total: u64,
    },
    Complete {
        #[serde(deserialize_with = "decimal")]
        stop_index: usize,
        #[serde(deserialize_with = "node_id")]
        node: NodeId,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Passenger {
    #[serde(deserialize_with = "node_id")]
    from: NodeId,
    #[serde(deserialize_with = "node_id")]
    to: NodeId,
    amount: u32,
    waiting: u32,
    onboard: u32,
    arrived: u32,
    unserved: u32,
}

fn decimal<'de, D: Deserializer<'de>, T: TryFrom<u64>>(deserializer: D) -> Result<T, D::Error>
where
    T::Error: fmt::Display,
{
    let text = String::deserialize(deserializer)?;

    if text.is_empty()
        || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(D::Error::custom(
            "expected decimal string matching 0|[1-9][0-9]*",
        ));
    }

    let value = text.parse::<u64>().map_err(D::Error::custom)?;
    T::try_from(value).map_err(D::Error::custom)
}

fn node_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<NodeId, D::Error> {
    decimal(deserializer).map(NodeId)
}

fn edge_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<EdgeId, D::Error> {
    decimal(deserializer).map(EdgeId)
}

// Derived structs also accept JSON arrays; this file format requires objects.
fn object<'de, D: Deserializer<'de>, T: DeserializeOwned>(deserializer: D) -> Result<T, D::Error> {
    let raw = Box::<RawValue>::deserialize(deserializer)?;
    decode(raw.get()).map_err(D::Error::custom)
}

fn objects<'de, D: Deserializer<'de>, T: DeserializeOwned>(
    deserializer: D,
) -> Result<Vec<T>, D::Error> {
    Vec::<Box<RawValue>>::deserialize(deserializer)?
        .iter()
        .enumerate()
        .map(|(index, raw)| {
            decode(raw.get()).map_err(|error| D::Error::custom(format!("[{index}]: {error}")))
        })
        .collect()
}

fn decode<T: DeserializeOwned>(json: &str) -> Result<T, String> {
    if !json.trim_start().starts_with('{') {
        return Err("JSON/shape: expected object".into());
    }

    let mut deserializer = serde_json::Deserializer::from_str(json);
    let result = serde_path_to_error::deserialize(&mut deserializer)
        .map_err(|error| format!("JSON/shape: {error}"))?;

    deserializer
        .end()
        .map_err(|error| format!("JSON: {error}"))?;

    Ok(result)
}

pub(super) fn document(json: &str) -> Result<(World, RailTrace), String> {
    let version: Version = decode(json)?;

    if version.version.get() != "1" {
        return Err(format!(
            "version: unsupported version {}; expected 1",
            version.version
        ));
    }

    let document: Document = decode(json)?;

    debug_assert_eq!(document.version, 1);

    if document
        .analysis
        .is_some_and(|value| !value.get().trim_start().starts_with('{'))
    {
        return Err("analysis: expected an object or null".into());
    }

    let mut network = Network::new();

    for (index, edge) in document.edges.into_iter().enumerate() {
        if edge.id != EdgeId(index) {
            return Err(format!(
                "edges[{index}].id: expected insertion index {index}"
            ));
        }

        network
            .add_edge(edge.from, edge.to, edge.kind)
            .map_err(|error| format!("edges[{index}]: {error:?}"))?;
    }

    let world = World {
        nodes: document
            .nodes
            .into_iter()
            .map(|node| Node { id: node.0 })
            .collect(),
        network,
    };
    let trace = RailTrace {
        completed: document.trace.completed,
        snapshots: document
            .trace
            .snapshots
            .into_iter()
            .map(|snapshot| RailSnapshot {
                tick: snapshot.tick,
                position: snapshot.position.0,
                occupancy: snapshot.occupancy,
                capacity: snapshot.capacity,
                passengers: snapshot
                    .passengers
                    .into_iter()
                    .map(|passenger| PassengerState {
                        demand: Demand::new(passenger.from, passenger.to, passenger.amount),
                        waiting: passenger.waiting,
                        onboard: passenger.onboard,
                        arrived: passenger.arrived,
                        unserved: passenger.unserved,
                    })
                    .collect(),
            })
            .collect(),
    };

    Ok((world, trace))
}
