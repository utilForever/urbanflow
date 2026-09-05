use crate::demand::Demand;
use crate::world::{EdgeId, EdgeKind, Network};

/// Errors found while validating Rail service inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RailInitError {
    EmptyRoute,
    UnknownEdge(EdgeId),
    NonRailEdge(EdgeId),
    DisconnectedEdges { previous: EdgeId, next: EdgeId },
    InvalidCapacity,
    InvalidTravelTicks,
    InvalidDwellTicks,
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
    capacity: u32,
    travel_ticks_per_edge: u64,
    dwell_ticks_per_stop: u64,
    state: RailVehicleState,
}

impl RailVehicle {
    pub fn new(
        capacity: u32,
        travel_ticks_per_edge: u64,
        dwell_ticks_per_stop: u64,
    ) -> Result<Self, RailInitError> {
        if capacity == 0 {
            return Err(RailInitError::InvalidCapacity);
        }

        if travel_ticks_per_edge == 0 {
            return Err(RailInitError::InvalidTravelTicks);
        }

        if dwell_ticks_per_stop == 0 {
            return Err(RailInitError::InvalidDwellTicks);
        }

        Ok(Self {
            capacity,
            travel_ticks_per_edge,
            dwell_ticks_per_stop,
            state: RailVehicleState::AtStop {
                stop_index: 0,
                dwell_ticks_remaining: dwell_ticks_per_stop,
            },
        })
    }

    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    pub const fn travel_ticks_per_edge(&self) -> u64 {
        self.travel_ticks_per_edge
    }

    pub const fn dwell_ticks_per_stop(&self) -> u64 {
        self.dwell_ticks_per_stop
    }

    pub const fn state(&self) -> RailVehicleState {
        self.state
    }
}

/// Aggregated passenger counts for one demand during Rail service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PassengerState {
    pub demand: Demand,
    pub waiting: u32,
    pub onboard: u32,
    pub arrived: u32,
    pub unserved: u32,
}

/// One lifecycle record per demand, kept in caller-supplied order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RailPassengers {
    records: Vec<PassengerState>,
}

impl RailPassengers {
    /// Starts every passenger waiting, including demands outside the route.
    pub fn new(demands: &[Demand]) -> Self {
        Self {
            records: demands
                .iter()
                .map(|&demand| PassengerState {
                    demand,
                    waiting: demand.amount,
                    onboard: 0,
                    arrived: 0,
                    unserved: 0,
                })
                .collect(),
        }
    }

    /// Returns read-only records; duplicate and zero-amount demands are retained.
    pub fn records(&self) -> &[PassengerState] {
        &self.records
    }
}
