//! Owned operational result types for one completed Rail service.
//!
//! These are data contracts only: trace analysis and validation are not yet
//! implemented. Public fields permit caller construction without validation.
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

use crate::world::NodeId;

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
