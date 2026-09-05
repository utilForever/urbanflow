use urbanflow::demand::Demand;
use urbanflow::rail::RailPassengers;
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
