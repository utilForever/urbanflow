//! Owned operational results for one completed Rail service.
//!
//! [`analyze`] derives all five summaries from a full service recording.
//! [`PassengerOutcomes::from_trace`] summarizes final passenger counts.
//! [`PassengerTimes::from_trace`] derives passenger-ticks and arrived-passenger
//! means over recorded intervals.
//! [`VehicleOccupancy::from_trace`] derives capacity use over active intervals.
//! [`StopActivity::from_trace`] summarizes each visit in a full service recording.
//! [`RouteTiming::from_trace`] breaks active duration into travel and dwell.
//! All operations share a read-only consistency preflight before calculation.
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
//!
//! # Trace validation
//!
//! Every operation first rejects an empty trace, then requires its completion
//! flag and a final [`RailPosition::Complete`]. Final passenger records must have
//! no waiting or onboard passengers and conserve each demand's amount.
//! Snapshots are then checked in recorded order for:
//!
//! - Strictly increasing ticks exactly one apart, with any initial tick.
//! - Positive capacity, constant throughout the recording.
//! - Identical ordered demands compared with the final snapshot, and conserved
//!   waiting + onboard + arrived + unserved counts for every record, including
//!   duplicates and zero amounts.
//! - Forward lifecycle transitions: waiting cannot increase, arrived and
//!   unserved cannot decrease, arrivals come from previously onboard passengers,
//!   and new unserved counts appear only in the final snapshot.
//! - Occupancy equal to the checked sum of onboard counts and within capacity.
//!   Completed positions cannot retain waiting or onboard passengers.
//!
//! Failures return [`AnalysisError`] without mutating the trace or returning
//! partial results. Indices are zero-based; interval errors identify the later
//! snapshot. Recordings starting during service or containing only completion
//! remain valid, except that [`analyze`] and [`StopActivity::from_trace`] require
//! full history.
//! Detailed movement timing, boarding eligibility, and agreement with an external
//! route or network are outside the shared preflight; stop-sequence checks remain
//! part of stop activity calculation, including in [`analyze`].

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
    /// A completed position still has waiting or onboard passengers.
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
    /// Occupancy exceeds capacity or differs from the sum of onboard passengers.
    InvalidOccupancy { snapshot_index: usize },
    /// Stop activity requires stop zero with all passengers still waiting.
    MissingInitialState,
    /// This snapshot skips one or more ticks.
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
                "snapshot {snapshot_index} has occupancy exceeding capacity or differing from onboard passengers"
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

    let capacity = trace.snapshots[0].capacity;

    for (snapshot_index, snapshot) in trace.snapshots.iter().enumerate() {
        if snapshot_index > 0 {
            let ticks = snapshot
                .tick
                .checked_sub(trace.snapshots[snapshot_index - 1].tick)
                .filter(|&ticks| ticks > 0)
                .ok_or(AnalysisError::InvalidTickOrder { snapshot_index })?;

            if ticks != 1 {
                return Err(AnalysisError::NonContiguousTicks { snapshot_index });
            }
        }

        if snapshot.capacity == 0 || snapshot.capacity != capacity {
            return Err(AnalysisError::InvalidCapacity { snapshot_index });
        }

        if snapshot.passengers.len() != final_snapshot.passengers.len() {
            return Err(AnalysisError::InconsistentDemands { snapshot_index });
        }

        let mut onboard = 0u64;

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

            if matches!(snapshot.position, RailPosition::Complete { .. })
                && (record.waiting != 0 || record.onboard != 0)
            {
                return Err(AnalysisError::UnfinishedPassengers { demand_index });
            }

            if snapshot_index > 0 {
                let previous = &trace.snapshots[snapshot_index - 1].passengers[demand_index];
                let invalid_transition = AnalysisError::InvalidPassengerTransition {
                    snapshot_index,
                    demand_index,
                };
                let arrivals = record
                    .arrived
                    .checked_sub(previous.arrived)
                    .ok_or(invalid_transition)?;

                if record.waiting > previous.waiting
                    || arrivals > previous.onboard
                    || record.unserved < previous.unserved
                    || (record.unserved != previous.unserved
                        && snapshot_index != trace.snapshots.len() - 1)
                {
                    return Err(invalid_transition);
                }
            }

            onboard = onboard
                .checked_add(u64::from(record.onboard))
                .ok_or(AnalysisError::CountOverflow)?;
        }

        if snapshot.occupancy > capacity || u64::from(snapshot.occupancy) != onboard {
            return Err(AnalysisError::InvalidOccupancy { snapshot_index });
        }
    }

    Ok(final_snapshot)
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

/// Derives all operational summaries without modifying the source trace.
///
/// Requires a full recording from stop zero with all passengers waiting through
/// service completion, as for [`StopActivity::from_trace`]. The initial tick may
/// be nonzero. Results are owned and deterministic for the same trace, retaining
/// duplicate demands and repeated stop visits under each summary's conventions.
///
/// # Errors
///
/// Runs the shared [trace validation](self#trace-validation) once before any
/// calculation, then computes passenger outcomes, passenger times, vehicle
/// occupancy, stop activity, and route timing in that order. Returns the first
/// [`AnalysisError`], including missing initial history, inconsistent stop
/// sequences, or checked arithmetic overflow, without returning partial results.
pub fn analyze(trace: &RailTrace) -> Result<OperationalAnalysis, AnalysisError> {
    let final_snapshot = validate_trace(trace)?;
    let passenger_outcomes = PassengerOutcomes::from_final_snapshot(final_snapshot)?;

    Ok(OperationalAnalysis {
        passenger_outcomes,
        passenger_times: PassengerTimes::from_validated_trace(
            trace,
            final_snapshot,
            passenger_outcomes.arrived,
        )?,
        vehicle_occupancy: VehicleOccupancy::from_validated_trace(trace)?,
        stops: StopActivity::from_validated_trace(trace)?,
        route_timing: RouteTiming::from_validated_trace(trace, final_snapshot.tick)?,
    })
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
    /// amounts, using checked `u64` totals. A completion-only recording is valid.
    /// `served_share` is `None` for zero requested passengers.
    ///
    /// # Errors
    ///
    /// Applies the shared [trace validation](self#trace-validation) first.
    /// Aggregate overflow returns [`AnalysisError::CountOverflow`]; no partial
    /// result is returned on any error.
    pub fn from_trace(trace: &RailTrace) -> Result<Self, AnalysisError> {
        Self::from_final_snapshot(validate_trace(trace)?)
    }

    fn from_final_snapshot(final_snapshot: &RailSnapshot) -> Result<Self, AnalysisError> {
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
    /// Applies the shared [trace validation](self#trace-validation) first.
    /// Checked products, totals, and the combined arrived journey total return
    /// [`AnalysisError::TimeOverflow`] on overflow, without partial results.
    pub fn from_trace(trace: &RailTrace) -> Result<Self, AnalysisError> {
        let final_snapshot = validate_trace(trace)?;
        let outcomes = PassengerOutcomes::from_final_snapshot(final_snapshot)?;

        Self::from_validated_trace(trace, final_snapshot, outcomes.arrived)
    }

    fn from_validated_trace(
        trace: &RailTrace,
        final_snapshot: &RailSnapshot,
        arrived: u64,
    ) -> Result<Self, AnalysisError> {
        let mut totals = [0u64; 4];

        for pair in trace.snapshots.windows(2) {
            let current = &pair[0];
            let ticks = pair[1].tick - current.tick;

            for (demand_index, record) in current.passengers.iter().enumerate() {
                // Preflight guarantees forward, conserved lifecycle counts.
                let future_arrivals =
                    final_snapshot.passengers[demand_index].arrived - record.arrived;
                let arrived_onboard = record.onboard.min(future_arrivals);
                let arrived_waiting = future_arrivals - arrived_onboard;

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
        let mean = |total| (arrived != 0).then(|| total as f64 / arrived as f64);

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
    /// Applies the shared [trace validation](self#trace-validation) first.
    /// Checked passenger-tick products and totals return
    /// [`AnalysisError::TimeOverflow`] on overflow, without partial results.
    pub fn from_trace(trace: &RailTrace) -> Result<Self, AnalysisError> {
        validate_trace(trace)?;
        Self::from_validated_trace(trace)
    }

    fn from_validated_trace(trace: &RailTrace) -> Result<Self, AnalysisError> {
        let capacity = trace.snapshots[0].capacity;
        let mut active_ticks = 0u64;
        let mut max_occupancy = 0;
        let mut occupied_passenger_ticks = 0u64;

        for pair in trace.snapshots.windows(2) {
            let current = &pair[0];
            let ticks = pair[1].tick - current.tick;

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
    /// Applies the shared [trace validation](self#trace-validation) first.
    /// A missing initial stop or non-waiting initial passenger state returns
    /// [`AnalysisError::MissingInitialState`]. Each interval requires consistent
    /// stop/edge indices and nodes ([`AnalysisError::InvalidStopSequence`]).
    ///
    /// Counts may change only at stop processing: waiting decreases at its
    /// origin, arrivals increase at their destination from previously onboard
    /// passengers, and unserved counts change only at completion. Violations
    /// return [`AnalysisError::InvalidPassengerTransition`]. Error indices are
    /// zero-based; interval errors identify the later snapshot. Checked totals
    /// return [`AnalysisError::CountOverflow`], never a partial result.
    /// Travel/dwell durations, boarding eligibility, and agreement with an
    /// external network are outside this calculation.
    pub fn from_trace(trace: &RailTrace) -> Result<Vec<Self>, AnalysisError> {
        validate_trace(trace)?;
        Self::from_validated_trace(trace)
    }

    fn from_validated_trace(trace: &RailTrace) -> Result<Vec<Self>, AnalysisError> {
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
                let alighted = next_record.arrived - record.arrived;
                let boarded = if complete {
                    0
                } else {
                    record.waiting - next_record.waiting
                };

                if (boarded != 0 && record.demand.origin != node)
                    || (alighted != 0 && record.demand.destination != node)
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
    /// reconstruct earlier durations. A completion-only recording has zero
    /// durations.
    ///
    /// # Errors
    ///
    /// Applies the shared [trace validation](self#trace-validation) first.
    /// Checked duration totals return
    /// [`AnalysisError::TimeOverflow`] on overflow, without partial results.
    pub fn from_trace(trace: &RailTrace) -> Result<Self, AnalysisError> {
        let final_snapshot = validate_trace(trace)?;
        Self::from_validated_trace(trace, final_snapshot.tick)
    }

    fn from_validated_trace(
        trace: &RailTrace,
        completion_tick: u64,
    ) -> Result<Self, AnalysisError> {
        let mut timing = Self {
            elapsed_ticks: 0,
            traveling_ticks: 0,
            dwelling_ticks: 0,
            completion_tick,
        };

        for pair in trace.snapshots.windows(2) {
            let ticks = pair[1].tick - pair[0].tick;
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
