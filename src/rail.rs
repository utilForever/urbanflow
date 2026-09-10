use crate::demand::Demand;
use crate::time::{SimulationClock, SimulationTimeError};
use crate::world::{EdgeId, EdgeKind, Network, NodeId};

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
    stops: Vec<NodeId>,
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

        let stops = std::iter::once(network.edges()[edges[0].0].from)
            .chain(edges.iter().map(|id| network.edges()[id.0].to))
            .collect();

        Ok(Self { edges, stops })
    }

    pub fn edges(&self) -> &[EdgeId] {
        &self.edges
    }
}

/// The tick-based location of one Rail vehicle on its route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RailVehicleState {
    AtStop {
        /// Zero is the first edge's origin; stop i > 0 is edge i - 1's destination.
        stop_index: usize,
        dwell_ticks_remaining: u64,
    },
    Traveling {
        /// Zero-based index into the vehicle's route edges.
        edge_index: usize,
        travel_ticks_elapsed: u64,
    },
    Complete,
}

/// A failed tick leaves the vehicle, clock, and passenger records unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RailStepError {
    Time(SimulationTimeError),
    Passengers(RailPassengerError),
}

/// One fixed-route Rail vehicle and its current state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RailVehicle {
    route: RailRoute,
    capacity: u32,
    travel_ticks_per_edge: u64,
    dwell_ticks_per_stop: u64,
    state: RailVehicleState,
}

impl RailVehicle {
    pub fn new(
        route: RailRoute,
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
            route,
            capacity,
            travel_ticks_per_edge,
            dwell_ticks_per_stop,
            state: RailVehicleState::AtStop {
                stop_index: 0,
                dwell_ticks_remaining: dwell_ticks_per_stop,
            },
        })
    }

    pub const fn route(&self) -> &RailRoute {
        &self.route
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

    /// Advances one service tick and returns the resulting vehicle state.
    ///
    /// The first tick processes stop zero before consuming dwell time. Each
    /// later stop is processed on arrival, once per visit. Consuming the last
    /// dwell tick starts travel at zero elapsed ticks; subsequent ticks advance
    /// edge progress. Final arrival alights passengers, marks those remaining
    /// unserved, and completes immediately without a final dwell.
    ///
    /// Pass the same clock and passenger records throughout this service; do not
    /// separately advance the clock or process stops. Time overflow is checked
    /// before passenger processing. Any error leaves all three inputs unchanged.
    /// Advancing a completed vehicle is a no-op, including for the clock.
    pub fn advance(
        &mut self,
        clock: &mut SimulationClock,
        passengers: &mut RailPassengers,
    ) -> Result<RailVehicleState, RailStepError> {
        if self.state == RailVehicleState::Complete {
            return Ok(self.state);
        }

        let mut next_clock = clock.clone();
        next_clock.advance().map_err(RailStepError::Time)?;

        let next_state = match self.state {
            RailVehicleState::AtStop {
                stop_index,
                dwell_ticks_remaining,
            } => {
                if stop_index == 0 && dwell_ticks_remaining == self.dwell_ticks_per_stop {
                    passengers
                        .process_stop(self, 0)
                        .map_err(RailStepError::Passengers)?;
                }

                if dwell_ticks_remaining > 1 {
                    RailVehicleState::AtStop {
                        stop_index,
                        dwell_ticks_remaining: dwell_ticks_remaining - 1,
                    }
                } else {
                    RailVehicleState::Traveling {
                        edge_index: stop_index,
                        travel_ticks_elapsed: 0,
                    }
                }
            }
            RailVehicleState::Traveling {
                edge_index,
                travel_ticks_elapsed,
            } => {
                // Elapsed ticks stay below the positive configured duration.
                let elapsed = travel_ticks_elapsed + 1;

                if elapsed < self.travel_ticks_per_edge {
                    RailVehicleState::Traveling {
                        edge_index,
                        travel_ticks_elapsed: elapsed,
                    }
                } else {
                    let stop_index = edge_index + 1;

                    passengers
                        .process_stop(self, stop_index)
                        .map_err(RailStepError::Passengers)?;

                    if stop_index == self.route.edges.len() {
                        passengers.complete();
                        RailVehicleState::Complete
                    } else {
                        RailVehicleState::AtStop {
                            stop_index,
                            dwell_ticks_remaining: self.dwell_ticks_per_stop,
                        }
                    }
                }
            }
            RailVehicleState::Complete => unreachable!(),
        };

        self.state = next_state;
        *clock = next_clock;
        Ok(self.state)
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

/// Invalid passenger transfers leave all lifecycle records unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RailPassengerError {
    UnknownDemand(usize),
    InsufficientWaiting,
    InsufficientOnboard,
    UnknownStop(usize),
    CapacityExceeded,
    CountOverflow,
}

/// One lifecycle record per demand, kept in caller-supplied order.
///
/// The four counts always sum to the original demand amount. Explicit transfers
/// only account for passengers; `process_stop` also enforces route eligibility
/// and vehicle capacity. `RailVehicle::advance` controls automatic stop order.
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

    /// Moves waiting passengers onboard by their original demand index.
    pub fn board(&mut self, demand_index: usize, amount: u32) -> Result<(), RailPassengerError> {
        let record = self
            .records
            .get_mut(demand_index)
            .ok_or(RailPassengerError::UnknownDemand(demand_index))?;
        let waiting = record
            .waiting
            .checked_sub(amount)
            .ok_or(RailPassengerError::InsufficientWaiting)?;
        let onboard = record
            .onboard
            .checked_add(amount)
            .ok_or(RailPassengerError::CountOverflow)?;

        record.onboard = onboard;
        record.waiting = waiting;
        Ok(())
    }

    /// Records destination arrivals from passengers currently onboard.
    pub fn alight(&mut self, demand_index: usize, amount: u32) -> Result<(), RailPassengerError> {
        let record = self
            .records
            .get_mut(demand_index)
            .ok_or(RailPassengerError::UnknownDemand(demand_index))?;
        let onboard = record
            .onboard
            .checked_sub(amount)
            .ok_or(RailPassengerError::InsufficientOnboard)?;
        let arrived = record
            .arrived
            .checked_add(amount)
            .ok_or(RailPassengerError::CountOverflow)?;

        record.arrived = arrived;
        record.onboard = onboard;
        Ok(())
    }

    /// Alights destination passengers, then boards waiting demand in stored order.
    ///
    /// Only demands originating at this stop with a destination strictly later
    /// in the route may board. All onboard records count toward vehicle capacity.
    /// Invalid stop indices, excess initial occupancy, or arithmetic overflow
    /// leave every record unchanged, even when passengers could alight here.
    ///
    /// `RailVehicle::advance` invokes this once per stop visit in route order,
    /// starting at zero. Manual callers must follow the same order; this operation
    /// does not advance the vehicle or complete service.
    pub fn process_stop(
        &mut self,
        vehicle: &RailVehicle,
        stop_index: usize,
    ) -> Result<(), RailPassengerError> {
        let (&stop, later_stops) = vehicle
            .route
            .stops
            .get(stop_index..)
            .and_then(|stops| stops.split_first())
            .ok_or(RailPassengerError::UnknownStop(stop_index))?;
        let onboard = self.records.iter().try_fold(0u32, |total, record| {
            total
                .checked_add(record.onboard)
                .ok_or(RailPassengerError::CountOverflow)
        })?;

        let mut remaining = vehicle
            .capacity
            .checked_sub(onboard)
            .ok_or(RailPassengerError::CapacityExceeded)?;
        let mut next = self.clone();

        for index in 0..next.records.len() {
            let record = next.records[index];

            if record.demand.destination == stop {
                next.alight(index, record.onboard)?;
                remaining = remaining
                    .checked_add(record.onboard)
                    .ok_or(RailPassengerError::CountOverflow)?;
            }
        }

        for index in 0..next.records.len() {
            let record = next.records[index];

            if record.demand.origin == stop && later_stops.contains(&record.demand.destination) {
                let amount = record.waiting.min(remaining);
                next.board(index, amount)?;
                remaining = remaining
                    .checked_sub(amount)
                    .ok_or(RailPassengerError::CapacityExceeded)?;
            }
        }

        *self = next;
        Ok(())
    }

    /// Ends service after final-stop arrivals have been recorded.
    ///
    /// All passengers still waiting or onboard become unserved, including
    /// demands the fixed route could not carry. Repeated completion is a no-op.
    pub fn complete(&mut self) {
        for record in &mut self.records {
            record.unserved = record.demand.amount - record.arrived;
            record.waiting = 0;
            record.onboard = 0;
        }
    }
}
