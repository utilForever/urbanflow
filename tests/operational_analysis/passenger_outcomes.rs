use super::support::completed_trace;
use rstest::rstest;
use urbanflow::analysis::PassengerOutcomes;
use urbanflow::demand::Demand;
use urbanflow::world::NodeId;

#[test]
fn passenger_outcomes_use_final_counts_including_duplicate_and_ineligible_demands() {
    let mut trace = completed_trace(
        &[
            Demand::new(NodeId(8), NodeId(3), 4),
            Demand::new(NodeId(8), NodeId(3), 4),
            Demand::new(NodeId(3), NodeId(8), 3),
            Demand::new(NodeId(21), NodeId(3), 2),
            Demand::new(NodeId(8), NodeId(3), 0),
        ],
        6,
    );
    let before = trace.clone();
    let expected = PassengerOutcomes {
        requested: 13,
        arrived: 6,
        unserved: 7,
        served_share: Some(6.0 / 13.0),
    };

    assert_eq!(PassengerOutcomes::from_trace(&trace), Ok(expected));
    assert_eq!(trace, before);

    // Recording an already completed service produces just its final frame.
    trace.snapshots.drain(..2);

    assert_eq!(PassengerOutcomes::from_trace(&trace), Ok(expected));
}

#[rstest]
#[case::empty(vec![], 0, 0, 0, None)]
#[case::zero_demand(vec![Demand::new(NodeId(8), NodeId(3), 0)], 0, 0, 0, None)]
#[case::all_served(vec![Demand::new(NodeId(8), NodeId(3), 2)], 2, 2, 0, Some(1.0))]
#[case::all_unserved(vec![Demand::new(NodeId(3), NodeId(8), 2)], 2, 0, 2, Some(0.0))]
fn passenger_outcomes_handle_empty_zero_all_served_and_all_unserved_demand(
    #[case] demands: Vec<Demand>,
    #[case] requested: u64,
    #[case] arrived: u64,
    #[case] unserved: u64,
    #[case] served_share: Option<f64>,
) {
    assert_eq!(
        PassengerOutcomes::from_trace(&completed_trace(&demands, 6)),
        Ok(PassengerOutcomes {
            requested,
            arrived,
            unserved,
            served_share,
        })
    );
}

#[test]
fn passenger_outcomes_preserve_totals_larger_than_one_demand() {
    let trace = completed_trace(&[Demand::new(NodeId(8), NodeId(3), u32::MAX); 2], 1);

    assert_eq!(
        PassengerOutcomes::from_trace(&trace),
        Ok(PassengerOutcomes {
            requested: 8_589_934_590,
            arrived: 1,
            unserved: 8_589_934_589,
            served_share: Some(1.0 / 8_589_934_590.0),
        })
    );
}
