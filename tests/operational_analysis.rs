use urbanflow::analysis::{PassengerOutcomes, PassengerTimes, VehicleOccupancy};

#[test]
fn empty_denominators_are_representable_without_nan_or_infinity() {
    let outcomes = PassengerOutcomes {
        requested: 0,
        arrived: 0,
        unserved: 0,
        served_share: None,
    };
    let times = PassengerTimes {
        waiting_passenger_ticks: 0,
        onboard_passenger_ticks: 0,
        arrived_waiting_passenger_ticks: 0,
        arrived_onboard_passenger_ticks: 0,
        mean_waiting_ticks: None,
        mean_onboard_ticks: None,
        mean_journey_ticks: None,
    };
    let occupancy = VehicleOccupancy {
        capacity: 4,
        max_occupancy: 0,
        occupied_passenger_ticks: 0,
        mean_occupancy: None,
        max_load_factor: None,
        mean_load_factor: None,
    };

    assert_eq!(outcomes.served_share, None);
    assert_eq!(times.mean_waiting_ticks, None);
    assert_eq!(times.mean_onboard_ticks, None);
    assert_eq!(times.mean_journey_ticks, None);
    assert_eq!(occupancy.mean_occupancy, None);
    assert_eq!(occupancy.max_load_factor, None);
    assert_eq!(occupancy.mean_load_factor, None);
}
