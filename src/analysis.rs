//! Owned operational results for one completed Rail service.
//!
//! [`PassengerOutcomes::from_trace`] summarizes final passenger counts with
//! completion and terminal passenger validation. Other result types are data
//! contracts only; full trace validation and analysis are not yet implemented.
//! Public fields permit caller construction without validation.
//! Unlike [`crate::metrics::Metrics`], these summaries describe a recorded
//! service, not aggregate network allocation, construction cost, or reward.
//!
//! Counts and tick totals are exact integers; producers must use checked
//! arithmetic when accumulating them. Means and ratios are derived `f64` values
//! and may round large integers. An undefined value is `None`, never NaN or
//! infinity. Time is measured in simulation ticks, with no wall-clock unit.
//!
//! The interval convention is `[snapshot.tick, next_snapshot.tick)`: use the
//! first snapshot's state and counts for that interval. Lifecycle changes take
//! effect at the tick where they first appear in the recording. The terminal
//! snapshot contributes no interval. Completed-passenger time totals count only
//! passengers who arrived, excluding time spent waiting by unserved passengers.

use crate::rail::{RailPosition, RailTrace};
use crate::world::NodeId;
use std::fmt;

/// Errors while deriving operational results from an owned Rail trace.
///
/// Implements [`std::error::Error`] for propagation with `?` into
/// `Box<dyn std::error::Error>`. Display messages include the demand index when
/// one is available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalysisError {
    /// There is no snapshot to summarize.
    EmptyTrace,
    /// The trace is not marked completed or its final position is not complete.
    IncompleteTrace,
    /// A final passenger record still has waiting or onboard passengers.
    UnfinishedPassengers { demand_index: usize },
    /// A record's lifecycle counts do not sum to its demand amount.
    InvalidPassengerCounts { demand_index: usize },
    /// An aggregate passenger count would exceed `u64::MAX`.
    CountOverflow,
    /// This snapshot's tick is not strictly greater than the previous tick.
    InvalidTickOrder { snapshot_index: usize },
    /// This snapshot changes the number, identity, or order of demand records.
    InconsistentDemands { snapshot_index: usize },
    /// Passenger counts cannot follow the preceding recorded lifecycle state.
    InvalidPassengerTransition {
        snapshot_index: usize,
        demand_index: usize,
    },
    /// A passenger-tick product, total, or combined journey exceeds `u64::MAX`.
    TimeOverflow,
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTrace => formatter.write_str("the trace is empty"),
            Self::IncompleteTrace => formatter.write_str("the trace is incomplete"),
            Self::UnfinishedPassengers { demand_index } => write!(
                formatter,
                "passenger demand record {demand_index} still has waiting or onboard passengers"
            ),
            Self::InvalidPassengerCounts { demand_index } => write!(
                formatter,
                "passenger counts do not match demand record {demand_index}"
            ),
            Self::CountOverflow => formatter.write_str("passenger count overflow"),
            Self::InvalidTickOrder { snapshot_index } => write!(
                formatter,
                "snapshot {snapshot_index} does not advance the recorded tick"
            ),
            Self::InconsistentDemands { snapshot_index } => write!(
                formatter,
                "snapshot {snapshot_index} has inconsistent demand records"
            ),
            Self::InvalidPassengerTransition {
                snapshot_index,
                demand_index,
            } => write!(
                formatter,
                "snapshot {snapshot_index} has an invalid lifecycle transition for demand record {demand_index}"
            ),
            Self::TimeOverflow => formatter.write_str("passenger time overflow"),
        }
    }
}

impl std::error::Error for AnalysisError {}

/// Owned passenger, vehicle, stop, and timing summaries for a completed service.
#[derive(Clone, Debug, PartialEq)]
pub struct OperationalAnalysis {
    /// Final demand outcomes, separate from network-allocation metrics.
    pub passenger_outcomes: PassengerOutcomes,
    /// Overall passenger-tick totals and means for passengers who arrived.
    pub passenger_times: PassengerTimes,
    /// Capacity use across the recorded active service intervals.
    pub vehicle_occupancy: VehicleOccupancy,
    /// One entry per route stop visit, including the initial and final stops.
    ///
    /// Entries follow route order by `stop_index`, not node ID. Repeated visits
    /// to the same node remain separate entries; no sorting or grouping by node.
    pub stops: Vec<StopActivity>,
    /// Recorded duration breakdown and absolute completion tick.
    pub route_timing: RouteTiming,
}

/// Aggregate final passenger counts, including duplicate demand records.
///
/// In a completed service, `requested == arrived + unserved`, with no waiting
/// or onboard passengers remaining. Counts use `u64` because totals across
/// demands can exceed one demand's `u32` amount.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PassengerOutcomes {
    /// Total requested passengers across all demand records.
    pub requested: u64,
    /// Passengers who reached their destinations.
    pub arrived: u64,
    /// Passengers who had not arrived when service completed.
    pub unserved: u64,
    /// `arrived / requested`, in `[0, 1]`; `None` when `requested` is zero.
    pub served_share: Option<f64>,
}

impl PassengerOutcomes {
    /// Summarizes the final snapshot without modifying the trace.
    ///
    /// Counts every demand record separately, including duplicates and zero
    /// amounts, using checked `u64` totals. A completion-only recording is valid;
    /// earlier snapshots are not inspected. Full tick, occupancy, and lifecycle
    /// consistency validation across the recording is outside this operation.
    /// `served_share` is `None` for zero requested passengers.
    ///
    /// # Errors
    ///
    /// Checks for [`AnalysisError::EmptyTrace`] first, then requires both the
    /// trace's completion flag and a final [`RailPosition::Complete`] position
    /// or returns [`AnalysisError::IncompleteTrace`]. In final demand order,
    /// returns [`AnalysisError::UnfinishedPassengers`] for waiting or onboard
    /// passengers, then [`AnalysisError::InvalidPassengerCounts`] unless arrived
    /// plus unserved equals the original demand amount. Each error's
    /// `demand_index` is the zero-based index in the final snapshot. Aggregate
    /// overflow returns [`AnalysisError::CountOverflow`]; no partial result is
    /// returned on any error.
    pub fn from_trace(trace: &RailTrace) -> Result<Self, AnalysisError> {
        let final_snapshot = trace.snapshots.last().ok_or(AnalysisError::EmptyTrace)?;

        if !trace.completed || !matches!(final_snapshot.position, RailPosition::Complete { .. }) {
            return Err(AnalysisError::IncompleteTrace);
        }

        let mut requested = 0u64;
        let mut arrived = 0u64;
        let mut unserved = 0u64;

        for (demand_index, record) in final_snapshot.passengers.iter().enumerate() {
            if record.waiting != 0 || record.onboard != 0 {
                return Err(AnalysisError::UnfinishedPassengers { demand_index });
            }

            if record.arrived.checked_add(record.unserved) != Some(record.demand.amount) {
                return Err(AnalysisError::InvalidPassengerCounts { demand_index });
            }

            requested = requested
                .checked_add(u64::from(record.demand.amount))
                .ok_or(AnalysisError::CountOverflow)?;
            arrived = arrived
                .checked_add(u64::from(record.arrived))
                .ok_or(AnalysisError::CountOverflow)?;
            unserved = unserved
                .checked_add(u64::from(record.unserved))
                .ok_or(AnalysisError::CountOverflow)?;
        }

        Ok(Self {
            requested,
            arrived,
            unserved,
            served_share: (requested != 0).then(|| arrived as f64 / requested as f64),
        })
    }
}

/// Integer passenger-ticks and passenger-weighted means over recorded intervals.
///
/// One passenger in a lifecycle state for one tick contributes one
/// passenger-tick. Overall totals include all demands, even unserved passengers.
/// The `arrived_*` totals retain only the time attributable to passengers who
/// reached their destinations, so their means do not include unserved waiting.
/// All three means are `None` when [`PassengerOutcomes::arrived`] is zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PassengerTimes {
    /// Waiting passenger-ticks across all passengers, including the unserved.
    pub waiting_passenger_ticks: u64,
    /// Onboard passenger-ticks across all passengers, including onboard dwell.
    pub onboard_passenger_ticks: u64,
    /// Waiting passenger-ticks attributable to passengers who arrived.
    pub arrived_waiting_passenger_ticks: u64,
    /// Onboard passenger-ticks attributable to passengers who arrived.
    pub arrived_onboard_passenger_ticks: u64,
    /// `arrived_waiting_passenger_ticks / arrived`, in ticks per arrived passenger.
    pub mean_waiting_ticks: Option<f64>,
    /// `arrived_onboard_passenger_ticks / arrived`, in ticks per arrived passenger.
    pub mean_onboard_ticks: Option<f64>,
    /// Sum of the two `arrived_*` totals divided by `arrived`, in ticks.
    pub mean_journey_ticks: Option<f64>,
}

/// Occupancy of one vehicle across active intervals, including travel and dwell.
///
/// Interval weighting follows the module convention. The denominator for mean
/// occupancy is [`RouteTiming::elapsed_ticks`]. For zero active ticks, both
/// integer measurements are zero and all three derived values are `None`.
/// With active ticks but no passengers, all derived values are `Some(0.0)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VehicleOccupancy {
    /// Configured positive passenger capacity, constant throughout the service.
    pub capacity: u32,
    /// Highest active-interval occupancy; zero if there are no active intervals.
    pub max_occupancy: u32,
    /// Sum of occupancy times interval duration, in passenger-ticks.
    pub occupied_passenger_ticks: u64,
    /// `occupied_passenger_ticks / elapsed_ticks`, in passengers.
    pub mean_occupancy: Option<f64>,
    /// `max_occupancy / capacity`, in `[0, 1]`; `None` without active intervals.
    pub max_load_factor: Option<f64>,
    /// `mean_occupancy / capacity`, in `[0, 1]`; `None` without active intervals.
    pub mean_load_factor: Option<f64>,
}

/// Passenger activity at one route stop visit, not aggregated by node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StopActivity {
    /// Route position: zero is the initial stop; the edge count is the final stop.
    pub stop_index: usize,
    /// Network node visited, which can occur at multiple route positions.
    pub node: NodeId,
    /// Total passengers boarding during this visit.
    pub boarded: u64,
    /// Total passengers alighting at their destinations during this visit.
    pub alighted: u64,
    /// Passengers originating here still waiting after this visit's boarding.
    ///
    /// Includes ineligible demand. At the final stop, count before remaining
    /// passengers become unserved. Later visits do not revise earlier entries.
    pub remaining_waiting: u64,
}

/// Duration of the recorded service, excluding any final-stop dwell.
///
/// Classify each interval by its starting position: `AtStop` is dwelling and
/// `Traveling` is traveling. Thus `elapsed_ticks == traveling_ticks +
/// dwelling_ticks`, also equal to the final tick minus the initial tick. A
/// completion-only recording has zero durations but retains its absolute tick.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteTiming {
    /// Total active recorded duration, in ticks.
    pub elapsed_ticks: u64,
    /// Ticks spent traveling along route edges.
    pub traveling_ticks: u64,
    /// Ticks spent dwelling before departure, including the initial stop.
    pub dwelling_ticks: u64,
    /// Absolute tick of the final, completed snapshot; need not equal duration.
    pub completion_tick: u64,
}
