//! Deterministic discrete simulation time.

/// A checked integer clock that starts at tick zero.
#[derive(Debug, Default)]
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

    #[test]
    fn overflow_does_not_advance_the_clock() {
        let mut clock = SimulationClock { tick: u64::MAX };

        assert_eq!(clock.advance(), Err(SimulationTimeError::Overflow));
        assert_eq!(clock.tick(), u64::MAX);
    }
}
