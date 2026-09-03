use urbanflow::time::SimulationClock;

#[test]
fn simulation_clock_advances_one_tick_at_a_time() {
    let mut clock = SimulationClock::default();

    assert_eq!(clock.tick(), 0);
    assert_eq!(clock.advance(), Ok(1));
    assert_eq!(clock.advance(), Ok(2));
    assert_eq!(clock.tick(), 2);
}
