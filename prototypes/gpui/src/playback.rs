//! Recorded-only playback, shared by the desktop/web UI and ordinary Cargo tests.

use crate::results;

use std::time::Duration;
use urbanflow::rail::{RailPosition, RailSnapshot};
use urbanflow::world::NodeId;

pub const RECORDING: &str = include_str!("../../../tests/fixtures/rail_service.json");

pub struct Playback {
    result: results::RailResult,
    cursor: usize,
    playing: bool,
    elapsed: Duration,
}

impl Playback {
    pub fn new(result: results::RailResult) -> Self {
        Self {
            result,
            cursor: 0,
            playing: false,
            elapsed: Duration::ZERO,
        }
    }

    pub fn result(&self) -> &results::RailResult {
        &self.result
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn playing(&self) -> bool {
        self.playing
    }

    pub fn snapshot(&self) -> Option<&RailSnapshot> {
        self.result.trace.snapshots.get(self.cursor)
    }

    pub fn toggle(&mut self) {
        self.playing = !self.playing && self.cursor + 1 < self.result.trace.snapshots.len();
    }

    /// Seek clamps to the recording, pauses, and discards fractional elapsed time.
    pub fn seek(&mut self, index: usize) {
        self.cursor = index.min(self.result.trace.snapshots.len().saturating_sub(1));
        self.playing = false;
        self.elapsed = Duration::ZERO;
    }

    /// One recorded tick per wall-clock second.
    /// Large absolute ticks never pass through floating point.
    pub fn elapse(&mut self, elapsed: Duration) {
        if !self.playing {
            return;
        }

        self.elapsed = self.elapsed.saturating_add(elapsed);

        let frames = &self.result.trace.snapshots;

        while let Some(next) = frames.get(self.cursor + 1) {
            let interval = Duration::from_secs(next.tick - frames[self.cursor].tick);

            if self.elapsed < interval {
                break;
            }

            self.elapsed -= interval;
            self.cursor += 1;
        }

        if self.cursor + 1 >= frames.len() {
            self.playing = false;
            self.elapsed = Duration::ZERO;
        }
    }

    /// Schematic coordinates only; identifiers and displayed values remain exact.
    pub fn node_x(&self, node: NodeId) -> f32 {
        let nodes = &self.result.world.nodes;
        let index = nodes
            .iter()
            .position(|entry| entry.id == node)
            .expect("validated node");

        if nodes.len() < 2 {
            0.5
        } else {
            index as f32 / (nodes.len() - 1) as f32
        }
    }

    pub fn vehicle_x(&self) -> Option<f32> {
        self.snapshot().map(|frame| match frame.position {
            RailPosition::AtStop { node, .. } | RailPosition::Complete { node, .. } => {
                self.node_x(node)
            }
            RailPosition::Traveling {
                from,
                to,
                travel_ticks_elapsed,
                travel_ticks_total,
                ..
            } => {
                let progress = travel_ticks_elapsed as f64 / travel_ticks_total as f64;
                self.node_x(from) + (self.node_x(to) - self.node_x(from)) * progress as f32
            }
        })
    }

    /// Waiting, onboard, arrived, and unserved totals at the selected snapshot.
    pub fn passenger_totals(&self) -> [u64; 4] {
        let mut totals = [0_u64; 4];
        let Some(frame) = self.snapshot() else {
            return totals;
        };

        for passenger in &frame.passengers {
            for (total, count) in totals.iter_mut().zip([
                passenger.waiting,
                passenger.onboard,
                passenger.arrived,
                passenger.unserved,
            ]) {
                *total += u64::from(count);
            }
        }

        totals
    }

    pub fn status(&self) -> String {
        let Some(frame) = self.snapshot() else {
            return "No recorded snapshots".into();
        };

        let totals = self.passenger_totals();

        format!(
            "Tick {} | Occupancy {}/{} | Waiting {} | Onboard {} | Arrived {} | Unserved {}",
            frame.tick, frame.occupancy, frame.capacity, totals[0], totals[1], totals[2], totals[3]
        )
    }

    pub fn summary(&self) -> Vec<String> {
        let Some(a) = &self.result.analysis else {
            return vec!["Analysis: N/A (partial or missing initial history)".into()];
        };
        let mean = |value: Option<f64>| value.map_or("N/A".into(), |v| format!("{v:.2}"));
        let p = &a.passenger_outcomes;
        let t = &a.passenger_times;
        let v = &a.vehicle_occupancy;
        let r = &a.route_timing;

        let mut rows = vec![
            format!(
                "Requested {} | Arrived {} | Unserved {}",
                p.requested, p.arrived, p.unserved
            ),
            format!(
                "Mean waiting {} | Onboard {} | Journey {} ticks",
                mean(t.mean_waiting_ticks),
                mean(t.mean_onboard_ticks),
                mean(t.mean_journey_ticks)
            ),
            format!(
                "Capacity {} | Maximum occupancy {} | Mean occupancy {}",
                v.capacity,
                v.max_occupancy,
                mean(v.mean_occupancy)
            ),
            format!(
                "Elapsed {} | Traveling {} | Dwelling {} | Completion tick {}",
                r.elapsed_ticks, r.traveling_ticks, r.dwelling_ticks, r.completion_tick
            ),
        ];
        rows.extend(a.stops.iter().map(|stop| {
            format!(
                "Stop {} (node {}): boarded {} | alighted {} | waiting {}",
                stop.stop_index, stop.node.0, stop.boarded, stop.alighted, stop.remaining_waiting
            )
        }));

        rows
    }
}
