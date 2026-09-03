use crate::world::EdgeId;

/// An ordered sequence of Rail edges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RailRoute {
    pub edges: Vec<EdgeId>,
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
