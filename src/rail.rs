use crate::world::{EdgeId, EdgeKind, Network};

/// Errors found while validating Rail service inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RailInitError {
    EmptyRoute,
    UnknownEdge(EdgeId),
    NonRailEdge(EdgeId),
    DisconnectedEdges { previous: EdgeId, next: EdgeId },
}

/// An ordered sequence of Rail edges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RailRoute {
    edges: Vec<EdgeId>,
}

impl RailRoute {
    pub fn new(network: &Network, edges: Vec<EdgeId>) -> Result<Self, RailInitError> {
        if edges.is_empty() {
            return Err(RailInitError::EmptyRoute);
        }

        for &edge_id in &edges {
            let edge = network
                .edges()
                .get(edge_id.0)
                .ok_or(RailInitError::UnknownEdge(edge_id))?;

            if edge.kind != EdgeKind::Rail {
                return Err(RailInitError::NonRailEdge(edge_id));
            }
        }

        for pair in edges.windows(2) {
            let previous = network.edges()[pair[0].0];
            let next = network.edges()[pair[1].0];

            if previous.to != next.from {
                return Err(RailInitError::DisconnectedEdges {
                    previous: previous.id,
                    next: next.id,
                });
            }
        }

        Ok(Self { edges })
    }

    pub fn edges(&self) -> &[EdgeId] {
        &self.edges
    }
}

/// The tick-based location of one Rail vehicle on its route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RailVehicleState {
    AtStop {
        stop_index: usize,
        dwell_ticks_remaining: u64,
    },
    Traveling {
        edge_index: usize,
        travel_ticks_elapsed: u64,
    },
    Complete,
}

/// One fixed-route Rail vehicle and its current state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RailVehicle {
    pub capacity: u32,
    pub travel_ticks_per_edge: u64,
    pub dwell_ticks_per_stop: u64,
    pub state: RailVehicleState,
}
