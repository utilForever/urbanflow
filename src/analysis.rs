//! Owned operational results for one completed Rail service.
//!
//! [`PassengerOutcomes::from_trace`] summarizes final passenger counts with
//! completion and terminal passenger validation. [`PassengerTimes::from_trace`]
//! derives passenger-ticks and arrived-passenger means over recorded intervals.
//! [`VehicleOccupancy::from_trace`] derives capacity use over active intervals.
//! [`StopActivity::from_trace`] summarizes each visit in a full service recording.
//! [`RouteTiming::from_trace`] breaks active duration into travel and dwell.
//! Full trace validation and combined analysis are not yet implemented.
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

use crate::rail::{RailPosition, RailSnapshot, RailTrace};
use crate::world::NodeId;
use std::fmt;

/// Errors while deriving operational results from an owned Rail trace.
///
/// Implements [`std::error::Error`] for propagation with `?` into
/// `Box<dyn std::error::Error>`. Display messages include the demand or snapshot
/// index when one is available.
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
    /// A duration, passenger-tick product, or time total exceeds `u64::MAX`.
    TimeOverflow,
    /// Capacity is zero or differs from the first snapshot's capacity.
    InvalidCapacity { snapshot_index: usize },
    /// Recorded occupancy exceeds the configured capacity.
    InvalidOccupancy { snapshot_index: usize },
    /// Stop activity requires stop zero with all passengers still waiting.
    MissingInitialState,
    /// Stop activity requires every tick; this snapshot skips one or more ticks.
    NonContiguousTicks { snapshot_index: usize },
    /// Positions cannot identify consecutive route visits and their transitions.
    InvalidStopSequence { snapshot_index: usize },
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
            Self::TimeOverflow => formatter.write_str("time overflow"),
            Self::InvalidCapacity { snapshot_index } => write!(
                formatter,
                "snapshot {snapshot_index} has zero or inconsistent vehicle capacity"
            ),
            Self::InvalidOccupancy { snapshot_index } => write!(
                formatter,
                "snapshot {snapshot_index} has occupancy exceeding vehicle capacity"
            ),
            Self::MissingInitialState => {
                formatter.write_str("the trace lacks the initial stop with all passengers waiting")
            }
            Self::NonContiguousTicks { snapshot_index } => {
                write!(formatter, "snapshot {snapshot_index} skips recorded ticks")
            }
            Self::InvalidStopSequence { snapshot_index } => write!(
                formatter,
                "snapshot {snapshot_index} has an inconsistent route stop sequence"
            ),
        }
    }
}

impl std::error::Error for AnalysisError {}

fn validate_trace(trace: &RailTrace) -> Result<&RailSnapshot, AnalysisError> {
    let final_snapshot = trace.snapshots.last().ok_or(AnalysisError::EmptyTrace)?;

    if !trace.completed || !matches!(final_snapshot.position, RailPosition::Complete { .. }) {
        return Err(AnalysisError::IncompleteTrace);
    }

    for (demand_index, record) in final_snapshot.passengers.iter().enumerate() {
        if record.waiting != 0 || record.onboard != 0 {
            return Err(AnalysisError::UnfinishedPassengers { demand_index });
        }

        if record.arrived.checked_add(record.unserved) != Some(record.demand.amount) {
            return Err(AnalysisError::InvalidPassengerCounts { demand_index });
        }
    }

    Ok(final_snapshot)
}

fn validate_passenger_records(trace: &RailTrace) -> Result<(), AnalysisError> {
    let final_snapshot = trace.snapshots.last().ok_or(AnalysisError::EmptyTrace)?;

    for (snapshot_index, snapshot) in trace.snapshots.iter().enumerate() {
        if snapshot.passengers.len() != final_snapshot.passengers.len() {
            return Err(AnalysisError::InconsistentDemands { snapshot_index });
        }

        for (demand_index, (record, final_record)) in snapshot
            .passengers
            .iter()
            .zip(&final_snapshot.passengers)
            .enumerate()
        {
            if record.demand != final_record.demand {
                return Err(AnalysisError::InconsistentDemands { snapshot_index });
            }

            if record
                .waiting
                .checked_add(record.onboard)
                .and_then(|count| count.checked_add(record.arrived))
                .and_then(|count| count.checked_add(record.unserved))
                != Some(record.demand.amount)
            {
                return Err(AnalysisError::InvalidPassengerCounts { demand_index });
            }
        }
    }

    Ok(())
}

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
        let final_snapshot = validate_trace(trace)?;

        let mut requested = 0u64;
        let mut arrived = 0u64;
        let mut unserved = 0u64;

        for record in final_snapshot.passengers.iter() {
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

impl PassengerTimes {
    /// Derives passenger time from a completed trace without modifying it.
    ///
    /// Each interval uses its starting counts times the positive tick delta,
    /// including waiting before the first recorded boarding and onboard dwell.
    /// The terminal snapshot adds no time. A recording that starts during or
    /// after service measures only its recorded intervals, never missing history.
    /// A completion-only recording therefore has zero totals.
    ///
    /// Within each demand, arrivals are attributed to earlier boardings first.
    /// At each snapshot, passengers who will arrive are the final arrived count
    /// minus those already arrived. They are onboard up to the recorded onboard
    /// count, with the remainder still waiting. This counts separate boarding
    /// batches, including repeated stops, without individual passenger timelines.
    /// Duplicate demands remain separate. All means divide by the final arrived
    /// count, including arrivals before recording, or are `None` if it is zero.
    ///
    /// # Errors
    ///
    /// Applies [`PassengerOutcomes::from_trace`] validation first. Then requires
    /// identical ordered demands and conserved counts in every snapshot, returning
    /// [`AnalysisError::InconsistentDemands`] or [`AnalysisError::InvalidPassengerCounts`].
    /// Intervals require strictly increasing ticks ([`AnalysisError::InvalidTickOrder`])
    /// and forward lifecycle transitions ([`AnalysisError::InvalidPassengerTransition`]):
    /// waiting cannot increase, arrived and unserved cannot decrease, arrivals
    /// must come from previously onboard passengers, and new unserved counts
    /// may appear only at completion. Error indices are zero-based; transition
    /// and tick errors identify the later snapshot.
    ///
    /// Checked products, totals, and the combined arrived journey total return
    /// [`AnalysisError::TimeOverflow`] on overflow, without partial results.
    /// Tick contiguity, occupancy, capacity, and route-position consistency are
    /// outside this operation; a gap uses the starting counts for its duration.
    pub fn from_trace(trace: &RailTrace) -> Result<Self, AnalysisError> {
        let outcomes = PassengerOutcomes::from_trace(trace)?;

        validate_passenger_records(trace)?;

        let final_snapshot = trace.snapshots.last().ok_or(AnalysisError::EmptyTrace)?;
        let mut totals = [0u64; 4];

        for (index, pair) in trace.snapshots.windows(2).enumerate() {
            let current = &pair[0];
            let next = &pair[1];
            let snapshot_index = index + 1;
            let ticks = next
                .tick
                .checked_sub(current.tick)
                .filter(|&ticks| ticks > 0)
                .ok_or(AnalysisError::InvalidTickOrder { snapshot_index })?;

            for (demand_index, record) in current.passengers.iter().enumerate() {
                let next_record = &next.passengers[demand_index];
                let invalid_transition = AnalysisError::InvalidPassengerTransition {
                    snapshot_index,
                    demand_index,
                };
                let arrivals = next_record
                    .arrived
                    .checked_sub(record.arrived)
                    .ok_or(invalid_transition)?;

                if next_record.waiting > record.waiting
                    || arrivals > record.onboard
                    || next_record.unserved < record.unserved
                    || (next_record.unserved != record.unserved
                        && snapshot_index != trace.snapshots.len() - 1)
                {
                    return Err(invalid_transition);
                }

                let future_arrivals = final_snapshot.passengers[demand_index]
                    .arrived
                    .checked_sub(record.arrived)
                    .ok_or(invalid_transition)?;
                let arrived_onboard = record.onboard.min(future_arrivals);
                let arrived_waiting = future_arrivals - arrived_onboard;

                if arrived_waiting > record.waiting {
                    return Err(invalid_transition);
                }

                for (total, count) in totals.iter_mut().zip([
                    record.waiting,
                    record.onboard,
                    arrived_waiting,
                    arrived_onboard,
                ]) {
                    *total = u64::from(count)
                        .checked_mul(ticks)
                        .and_then(|time| total.checked_add(time))
                        .ok_or(AnalysisError::TimeOverflow)?;
                }
            }
        }

        let [
            waiting_passenger_ticks,
            onboard_passenger_ticks,
            arrived_waiting_passenger_ticks,
            arrived_onboard_passenger_ticks,
        ] = totals;
        let journey = arrived_waiting_passenger_ticks
            .checked_add(arrived_onboard_passenger_ticks)
            .ok_or(AnalysisError::TimeOverflow)?;
        let mean = |total| (outcomes.arrived != 0).then(|| total as f64 / outcomes.arrived as f64);

        Ok(Self {
            waiting_passenger_ticks,
            onboard_passenger_ticks,
            arrived_waiting_passenger_ticks,
            arrived_onboard_passenger_ticks,
            mean_waiting_ticks: mean(arrived_waiting_passenger_ticks),
            mean_onboard_ticks: mean(arrived_onboard_passenger_ticks),
            mean_journey_ticks: mean(journey),
        })
    }
}

/// Occupancy of one vehicle across active intervals, including travel and dwell.
///
/// Interval weighting follows the module convention. The denominator for mean
/// occupancy is the summed active interval duration. For zero active ticks, both
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
    /// `occupied_passenger_ticks / active_ticks`, in passengers.
    pub mean_occupancy: Option<f64>,
    /// `max_occupancy / capacity`, in `[0, 1]`; `None` without active intervals.
    pub max_load_factor: Option<f64>,
    /// `mean_occupancy / capacity`, in `[0, 1]`; `None` without active intervals.
    pub mean_load_factor: Option<f64>,
}

impl VehicleOccupancy {
    /// Derives vehicle capacity use from a completed trace without modifying it.
    ///
    /// Each interval uses its starting occupancy times the positive tick delta.
    /// Active intervals start at [`RailPosition::AtStop`] or
    /// [`RailPosition::Traveling`], including initial and onboard dwell. Intervals
    /// starting at [`RailPosition::Complete`] and the terminal snapshot contribute
    /// neither time nor maximum occupancy. Only recorded intervals contribute;
    /// recording during service does not reconstruct earlier occupancy.
    ///
    /// Means divide by the summed active duration, and load factors divide by
    /// the fixed capacity. With no active intervals, both integer measurements
    /// are zero and all three derived values are `None`. An active but empty
    /// vehicle instead has zero measurements and `Some(0.0)` derived values.
    /// Floating-point rounding cannot raise the mean above recorded maximum
    /// occupancy or raise a load factor above one.
    ///
    /// # Errors
    ///
    /// Applies [`PassengerOutcomes::from_trace`] validation first. Then checks
    /// every snapshot in order for positive capacity matching the first snapshot
    /// ([`AnalysisError::InvalidCapacity`]) and occupancy within capacity
    /// ([`AnalysisError::InvalidOccupancy`]). Intervals require strictly increasing
    /// ticks ([`AnalysisError::InvalidTickOrder`], identifying the later snapshot).
    /// Indices are zero-based. Checked passenger-tick products and totals return
    /// [`AnalysisError::TimeOverflow`] on overflow, without partial results.
    ///
    /// Tick contiguity, occupancy agreement with passenger records, lifecycle
    /// transitions, and route-position consistency are outside this operation.
    /// Gaps use the starting occupancy for their whole duration.
    pub fn from_trace(trace: &RailTrace) -> Result<Self, AnalysisError> {
        PassengerOutcomes::from_trace(trace)?;

        let capacity = trace.snapshots[0].capacity;

        for (snapshot_index, snapshot) in trace.snapshots.iter().enumerate() {
            if snapshot.capacity == 0 || snapshot.capacity != capacity {
                return Err(AnalysisError::InvalidCapacity { snapshot_index });
            }

            if snapshot.occupancy > capacity {
                return Err(AnalysisError::InvalidOccupancy { snapshot_index });
            }
        }

        let mut active_ticks = 0u64;
        let mut max_occupancy = 0;
        let mut occupied_passenger_ticks = 0u64;

        for (index, pair) in trace.snapshots.windows(2).enumerate() {
            let current = &pair[0];
            let ticks = pair[1]
                .tick
                .checked_sub(current.tick)
                .filter(|&ticks| ticks > 0)
                .ok_or(AnalysisError::InvalidTickOrder {
                    snapshot_index: index + 1,
                })?;

            if matches!(current.position, RailPosition::Complete { .. }) {
                continue;
            }

            active_ticks = active_ticks
                .checked_add(ticks)
                .ok_or(AnalysisError::TimeOverflow)?;
            max_occupancy = max_occupancy.max(current.occupancy);
            occupied_passenger_ticks = u64::from(current.occupancy)
                .checked_mul(ticks)
                .and_then(|time| occupied_passenger_ticks.checked_add(time))
                .ok_or(AnalysisError::TimeOverflow)?;
        }

        let mean_occupancy = (active_ticks != 0).then(|| {
            (occupied_passenger_ticks as f64 / active_ticks as f64).min(f64::from(max_occupancy))
        });

        Ok(Self {
            capacity,
            max_occupancy,
            occupied_passenger_ticks,
            mean_occupancy,
            max_load_factor: (active_ticks != 0)
                .then(|| f64::from(max_occupancy) / f64::from(capacity)),
            mean_load_factor: mean_occupancy.map(|mean| mean / f64::from(capacity)),
        })
    }
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

impl StopActivity {
    /// Summarizes every stop visit in route order without modifying the trace.
    ///
    /// Requires a full recording: stop zero with all passengers waiting, then
    /// every tick through completion. The initial clock tick may be nonzero.
    /// Unlike the other summaries, missing visits cannot be reconstructed from
    /// a recording started during service or containing only completion.
    ///
    /// Stop zero is processed on the first advance; other stops on arrival.
    /// Boarding is the decrease in waiting and alighting is the increase in
    /// arrived counts, so simultaneous boarding and alighting are both retained.
    /// Duplicate demands contribute separately. Empty visits and repeated nodes
    /// remain separate entries in route order, with no sorting by node ID.
    /// Remaining waiting includes ineligible demand originating at that node.
    /// At completion, boarding is zero and waiting uses the preceding snapshot,
    /// before waiting and leftover onboard passengers become unserved.
    ///
    /// # Errors
    ///
    /// Applies [`PassengerOutcomes::from_trace`] validation first, then checks
    /// ordered demands and count conservation as for [`PassengerTimes::from_trace`].
    /// A missing initial stop or non-waiting initial passenger state returns
    /// [`AnalysisError::MissingInitialState`]. Each interval requires increasing
    /// ticks ([`AnalysisError::InvalidTickOrder`]) exactly one apart
    /// ([`AnalysisError::NonContiguousTicks`]), followed by consistent stop/edge
    /// indices and nodes ([`AnalysisError::InvalidStopSequence`]).
    ///
    /// Counts may change only at stop processing: waiting decreases at its
    /// origin, arrivals increase at their destination from previously onboard
    /// passengers, and unserved counts change only at completion. Violations
    /// return [`AnalysisError::InvalidPassengerTransition`]. Error indices are
    /// zero-based; interval errors identify the later snapshot. Checked totals
    /// return [`AnalysisError::CountOverflow`], never a partial result.
    /// Capacity, occupancy, travel/dwell durations, boarding eligibility, and
    /// agreement with an external network are outside this calculation.
    pub fn from_trace(trace: &RailTrace) -> Result<Vec<Self>, AnalysisError> {
        PassengerOutcomes::from_trace(trace)?;
        validate_passenger_records(trace)?;

        let initial = &trace.snapshots[0];

        if !matches!(initial.position, RailPosition::AtStop { stop_index: 0, .. })
            || initial
                .passengers
                .iter()
                .any(|record| record.waiting != record.demand.amount)
        {
            return Err(AnalysisError::MissingInitialState);
        }

        let mut stops = Vec::new();

        for (index, pair) in trace.snapshots.windows(2).enumerate() {
            let current = &pair[0];
            let next = &pair[1];
            let snapshot_index = index + 1;
            let ticks = next
                .tick
                .checked_sub(current.tick)
                .filter(|&ticks| ticks > 0)
                .ok_or(AnalysisError::InvalidTickOrder { snapshot_index })?;

            if ticks != 1 {
                return Err(AnalysisError::NonContiguousTicks { snapshot_index });
            }

            let invalid_stop = AnalysisError::InvalidStopSequence { snapshot_index };
            let visit = match (current.position, next.position) {
                (
                    RailPosition::AtStop {
                        stop_index, node, ..
                    },
                    RailPosition::AtStop {
                        stop_index: next_index,
                        node: next_node,
                        ..
                    }
                    | RailPosition::Traveling {
                        edge_index: next_index,
                        from: next_node,
                        ..
                    },
                ) if (stop_index, node) == (next_index, next_node) => {
                    (index == 0).then_some((stop_index, node))
                }
                (
                    RailPosition::Traveling { edge_index, to, .. },
                    RailPosition::AtStop {
                        stop_index, node, ..
                    }
                    | RailPosition::Complete { stop_index, node },
                ) if edge_index.checked_add(1) == Some(stop_index) && to == node => {
                    Some((stop_index, node))
                }
                (
                    RailPosition::Traveling {
                        edge_index,
                        edge,
                        from,
                        to,
                        ..
                    },
                    RailPosition::Traveling {
                        edge_index: next_index,
                        edge: next_edge,
                        from: next_from,
                        to: next_to,
                        ..
                    },
                ) if (edge_index, edge, from, to)
                    == (next_index, next_edge, next_from, next_to) =>
                {
                    None
                }
                _ => return Err(invalid_stop),
            };

            let Some((stop_index, node)) = visit else {
                for (demand_index, (record, next_record)) in
                    current.passengers.iter().zip(&next.passengers).enumerate()
                {
                    if record != next_record {
                        return Err(AnalysisError::InvalidPassengerTransition {
                            snapshot_index,
                            demand_index,
                        });
                    }
                }
                continue;
            };

            if stop_index != stops.len() {
                return Err(invalid_stop);
            }

            let complete = matches!(next.position, RailPosition::Complete { .. });
            let mut activity = Self {
                stop_index,
                node,
                boarded: 0,
                alighted: 0,
                remaining_waiting: 0,
            };

            for (demand_index, (record, next_record)) in
                current.passengers.iter().zip(&next.passengers).enumerate()
            {
                let invalid_transition = AnalysisError::InvalidPassengerTransition {
                    snapshot_index,
                    demand_index,
                };
                let alighted = next_record
                    .arrived
                    .checked_sub(record.arrived)
                    .filter(|&count| count <= record.onboard)
                    .ok_or(invalid_transition)?;
                let boarded = if complete {
                    0
                } else {
                    record
                        .waiting
                        .checked_sub(next_record.waiting)
                        .ok_or(invalid_transition)?
                };

                if (boarded != 0 && record.demand.origin != node)
                    || (alighted != 0 && record.demand.destination != node)
                    || (!complete && next_record.unserved != record.unserved)
                {
                    return Err(invalid_transition);
                }

                activity.boarded = activity
                    .boarded
                    .checked_add(u64::from(boarded))
                    .ok_or(AnalysisError::CountOverflow)?;
                activity.alighted = activity
                    .alighted
                    .checked_add(u64::from(alighted))
                    .ok_or(AnalysisError::CountOverflow)?;

                if record.demand.origin == node {
                    let waiting = if complete {
                        record.waiting
                    } else {
                        next_record.waiting
                    };
                    activity.remaining_waiting = activity
                        .remaining_waiting
                        .checked_add(u64::from(waiting))
                        .ok_or(AnalysisError::CountOverflow)?;
                }
            }

            stops.push(activity);
        }

        Ok(stops)
    }
}

/// Duration of the recorded service, excluding any final-stop dwell.
///
/// Classify each interval by its starting position: `AtStop` is dwelling and
/// `Traveling` is traveling. Thus `elapsed_ticks == traveling_ticks +
/// dwelling_ticks`. Intervals starting at `Complete` add no active time. For
/// recordings produced by [`crate::rail::RailVehicle::record_trace`], elapsed
/// time also equals the final tick minus the initial tick. A completion-only
/// recording has zero durations but retains its absolute tick.
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

impl RouteTiming {
    /// Derives route durations from a completed trace without modifying it.
    ///
    /// Each interval contributes its positive tick delta according to its
    /// starting position: [`RailPosition::AtStop`] adds dwell time, including
    /// the initial stop, and [`RailPosition::Traveling`] adds travel time.
    /// Intervals starting at [`RailPosition::Complete`] and the terminal snapshot
    /// add no time. Elapsed ticks are the sum of travel and dwell ticks, with no
    /// final-stop dwell. The completion tick is the final snapshot's absolute tick.
    ///
    /// Only recorded intervals contribute; recording during service does not
    /// reconstruct earlier durations. Gaps use the starting position for their
    /// entire duration. A completion-only recording has zero durations.
    ///
    /// # Errors
    ///
    /// Applies [`PassengerOutcomes::from_trace`] validation first. Every interval,
    /// including completed intervals, then requires strictly increasing ticks
    /// ([`AnalysisError::InvalidTickOrder`], identifying the later snapshot by its
    /// zero-based index). Checked duration totals return
    /// [`AnalysisError::TimeOverflow`] on overflow, without partial results.
    /// Tick contiguity, capacity, occupancy, passenger lifecycle transitions,
    /// and route-position consistency are outside this operation.
    pub fn from_trace(trace: &RailTrace) -> Result<Self, AnalysisError> {
        PassengerOutcomes::from_trace(trace)?;

        let mut timing = Self {
            elapsed_ticks: 0,
            traveling_ticks: 0,
            dwelling_ticks: 0,
            completion_tick: trace
                .snapshots
                .last()
                .ok_or(AnalysisError::EmptyTrace)?
                .tick,
        };

        for (index, pair) in trace.snapshots.windows(2).enumerate() {
            let ticks = pair[1]
                .tick
                .checked_sub(pair[0].tick)
                .filter(|&ticks| ticks > 0)
                .ok_or(AnalysisError::InvalidTickOrder {
                    snapshot_index: index + 1,
                })?;
            let duration = match pair[0].position {
                RailPosition::AtStop { .. } => &mut timing.dwelling_ticks,
                RailPosition::Traveling { .. } => &mut timing.traveling_ticks,
                RailPosition::Complete { .. } => continue,
            };

            *duration = duration
                .checked_add(ticks)
                .ok_or(AnalysisError::TimeOverflow)?;
        }

        timing.elapsed_ticks = timing
            .traveling_ticks
            .checked_add(timing.dwelling_ticks)
            .ok_or(AnalysisError::TimeOverflow)?;

        Ok(timing)
    }
}
