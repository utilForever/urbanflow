#[path = "../examples/rail_viewer/mod.rs"]
mod rail_viewer;

#[test]
fn demo_service_records_repeatable_complete_trace() {
    let (_, trace) = rail_viewer::scenario();
    let (_, repeated_trace) = rail_viewer::scenario();

    assert!(trace.completed);
    assert_eq!(trace, repeated_trace);
}
