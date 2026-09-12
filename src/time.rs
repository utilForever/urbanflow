//! Deterministic discrete simulation time.

/// A checked integer clock that starts at tick zero.
#[derive(Clone, Debug, Default)]
pub struct SimulationClock {
    tick: u64,
}

/// Errors produced while advancing simulation time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationTimeError {
    /// Advancing would exceed the clock's integer capacity.
    Overflow,
}

impl SimulationClock {
    /// Returns the current simulation tick.
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    /// Advances exactly one tick, leaving the clock unchanged on overflow.
    pub fn advance(&mut self) -> Result<u64, SimulationTimeError> {
        let next_tick = self
            .tick
            .checked_add(1)
            .ok_or(SimulationTimeError::Overflow)?;

        self.tick = next_tick;
        Ok(next_tick)
    }
}

#[cfg(test)]
mod tests {
    use super::{SimulationClock, SimulationTimeError};
    use crate::demand::Demand;
    use crate::rail::{RailPassengers, RailRoute, RailStepError, RailVehicle, RailVehicleState};
    use crate::world::{EdgeKind, Network, NodeId};

    #[test]
    fn overflow_does_not_advance_the_clock() {
        let mut clock = SimulationClock { tick: u64::MAX };

        assert_eq!(clock.advance(), Err(SimulationTimeError::Overflow));
        assert_eq!(clock.tick(), u64::MAX);
    }

    #[test]
    fn clock_overflow_preserves_rail_state_and_passengers_at_every_transition() {
        let mut network = Network::new();
        let edge = network
            .add_edge(NodeId(0), NodeId(1), EdgeKind::Rail)
            .unwrap();
        let route = RailRoute::new(&network, vec![edge]).unwrap();
        let mut vehicle = RailVehicle::new(route, 1, 2, 2).unwrap();
        let mut passengers = RailPassengers::new(&[Demand::new(NodeId(0), NodeId(1), 2)]);
        let mut clock = SimulationClock::default();

        for _ in 0..4 {
            let before = (vehicle.clone(), passengers.clone());
            let mut full_clock = SimulationClock { tick: u64::MAX };

            assert_eq!(
                vehicle.advance(&mut full_clock, &mut passengers),
                Err(RailStepError::Time(SimulationTimeError::Overflow))
            );
            assert_eq!((&vehicle, &passengers), (&before.0, &before.1));
            assert_eq!(full_clock.tick(), u64::MAX);

            vehicle.advance(&mut clock, &mut passengers).unwrap();
        }

        let mut full_clock = SimulationClock { tick: u64::MAX };

        assert_eq!(
            vehicle.advance(&mut full_clock, &mut passengers),
            Ok(RailVehicleState::Complete)
        );
        assert_eq!(full_clock.tick(), u64::MAX);
        assert_eq!(passengers.records()[0].arrived, 1);
        assert_eq!(passengers.records()[0].unserved, 1);
    }

    #[test]
    fn trace_clock_overflow_keeps_only_successful_tick_progress() {
        let mut network = Network::new();
        let edge = network
            .add_edge(NodeId(0), NodeId(1), EdgeKind::Rail)
            .unwrap();
        let route = RailRoute::new(&network, vec![edge]).unwrap();
        let mut vehicle = RailVehicle::new(route, 1, 2, 2).unwrap();
        let mut passengers = RailPassengers::new(&[Demand::new(NodeId(0), NodeId(1), 2)]);
        let mut clock = SimulationClock { tick: u64::MAX - 1 };

        assert_eq!(
            vehicle.record_trace(&mut clock, &mut passengers, 4),
            Err(RailStepError::Time(SimulationTimeError::Overflow))
        );
        assert_eq!(clock.tick(), u64::MAX);
        assert_eq!(
            vehicle.state(),
            RailVehicleState::AtStop {
                stop_index: 0,
                dwell_ticks_remaining: 1,
            }
        );

        let record = passengers.records()[0];

        assert_eq!(
            (
                record.waiting,
                record.onboard,
                record.arrived,
                record.unserved
            ),
            (1, 1, 0, 0)
        );

        // A zero tick limit does not try to advance even a full clock.
        let trace = vehicle
            .record_trace(&mut clock, &mut passengers, 0)
            .unwrap();

        assert!(!trace.completed);
        assert_eq!(trace.snapshots.len(), 1);
        assert_eq!(trace.snapshots[0].tick, u64::MAX);
    }
}
