use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengerError, RailPassengers};
use urbanflow::world::NodeId;

#[test]
fn passenger_records_preserve_every_demand_in_stored_order() {
    let demands = [
        Demand::new(NodeId(8), NodeId(3), 7),
        Demand::new(NodeId(3), NodeId(8), 0),
        Demand::new(NodeId(8), NodeId(3), 7),
        Demand::new(NodeId(3), NodeId(3), u32::MAX),
    ];
    let passengers = RailPassengers::new(&demands);

    assert_eq!(passengers.records().len(), demands.len());

    for (record, demand) in passengers.records().iter().zip(demands) {
        assert_eq!(record.demand, demand);
        assert_eq!(record.waiting, demand.amount);
        assert_eq!((record.onboard, record.arrived, record.unserved), (0, 0, 0));
    }

    assert!(RailPassengers::new(&[]).records().is_empty());
}

fn assert_counts(passengers: &RailPassengers, expected: &[(u32, u32, u32, u32)]) {
    assert_eq!(passengers.records().len(), expected.len());

    for (record, &counts) in passengers.records().iter().zip(expected) {
        assert_eq!(
            (
                record.waiting,
                record.onboard,
                record.arrived,
                record.unserved
            ),
            counts
        );
        assert_eq!(
            u64::from(record.waiting)
                + u64::from(record.onboard)
                + u64::from(record.arrived)
                + u64::from(record.unserved),
            u64::from(record.demand.amount)
        );
    }
}

#[test]
fn lifecycle_transfers_conserve_passengers_without_combining_duplicate_demands() {
    let demand = Demand::new(NodeId(0), NodeId(2), 10);
    let mut passengers = RailPassengers::new(&[demand, demand]);

    passengers.board(0, 6).unwrap();
    assert_counts(&passengers, &[(4, 6, 0, 0), (10, 0, 0, 0)]);

    passengers.alight(0, 4).unwrap();
    assert_counts(&passengers, &[(4, 2, 4, 0), (10, 0, 0, 0)]);

    passengers.board(1, 3).unwrap();
    assert_counts(&passengers, &[(4, 2, 4, 0), (7, 3, 0, 0)]);

    passengers.alight(0, 2).unwrap();
    assert_counts(&passengers, &[(4, 0, 6, 0), (7, 3, 0, 0)]);
}

#[test]
fn invalid_lifecycle_transfers_leave_all_records_unchanged() {
    let mut passengers = RailPassengers::new(&[Demand::new(NodeId(0), NodeId(1), 10)]);
    passengers.board(0, 6).unwrap();

    let before = passengers.clone();

    assert_eq!(
        passengers.board(0, 5),
        Err(RailPassengerError::InsufficientWaiting)
    );
    assert_eq!(passengers, before);
    assert_eq!(
        passengers.alight(0, 7),
        Err(RailPassengerError::InsufficientOnboard)
    );
    assert_eq!(passengers, before);

    for index in [1, usize::MAX] {
        assert_eq!(
            passengers.board(index, 0),
            Err(RailPassengerError::UnknownDemand(index))
        );
        assert_eq!(passengers, before);
        assert_eq!(
            passengers.alight(index, 0),
            Err(RailPassengerError::UnknownDemand(index))
        );
        assert_eq!(passengers, before);
    }
}

#[test]
fn lifecycle_transfers_handle_zero_and_maximum_demand() {
    let mut passengers = RailPassengers::new(&[Demand::new(NodeId(0), NodeId(1), u32::MAX)]);

    passengers.board(0, 0).unwrap();
    passengers.alight(0, 0).unwrap();
    assert_counts(&passengers, &[(u32::MAX, 0, 0, 0)]);

    passengers.board(0, u32::MAX - 1).unwrap();
    passengers.board(0, 1).unwrap();
    assert_counts(&passengers, &[(0, u32::MAX, 0, 0)]);

    passengers.alight(0, u32::MAX - 1).unwrap();
    passengers.alight(0, 1).unwrap();
    assert_counts(&passengers, &[(0, 0, u32::MAX, 0)]);
}

#[test]
fn service_completion_marks_every_unfinished_passenger_unserved() {
    let mut passengers = RailPassengers::new(&[
        Demand::new(NodeId(0), NodeId(2), 10),
        Demand::new(NodeId(9), NodeId(8), u32::MAX),
        Demand::new(NodeId(2), NodeId(0), 0),
        Demand::new(NodeId(0), NodeId(1), 3),
    ]);
    passengers.board(0, 6).unwrap();
    passengers.alight(0, 4).unwrap();
    passengers.board(3, 3).unwrap();
    passengers.alight(3, 3).unwrap();

    passengers.complete();
    assert_counts(
        &passengers,
        &[
            (0, 0, 4, 6),
            (0, 0, 0, u32::MAX),
            (0, 0, 0, 0),
            (0, 0, 3, 0),
        ],
    );

    let completed = passengers.clone();

    passengers.complete();
    assert_eq!(passengers, completed);
    assert_eq!(
        passengers.board(0, 1),
        Err(RailPassengerError::InsufficientWaiting)
    );
    assert_eq!(
        passengers.alight(0, 1),
        Err(RailPassengerError::InsufficientOnboard)
    );
    assert_eq!(passengers, completed);

    let mut empty = RailPassengers::new(&[]);

    empty.complete();
    assert!(empty.records().is_empty());
}
