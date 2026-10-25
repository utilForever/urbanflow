#[path = "../prototypes/gpui/src/playback.rs"]
mod playback;
#[allow(dead_code)]
#[path = "../examples/rail_viewer/mod.rs"]
mod rail_viewer;

use playback::{Playback, RECORDING};
use rail_viewer::results;
use std::time::Duration;

fn demo() -> Playback {
    Playback::new(results::parse("rail_service.json", RECORDING).unwrap())
}

#[test]
fn recorded_values_match_html_and_playback_never_changes_the_recording() {
    let mut playback = demo();
    let before = results::encode(&playback.result().world, &playback.result().trace).unwrap();
    let html = rail_viewer::render(&playback.result().world, &playback.result().trace).unwrap();
    assert!(html.contains(&before));
    let summary = playback.summary();
    assert_eq!(
        summary,
        [
            "Requested 6 | Arrived 4 | Unserved 2",
            "Mean waiting 2.00 | Onboard 2.50 | Journey 4.50 ticks",
            "Capacity 2 | Maximum occupancy 2 | Mean occupancy 1.67",
            "Elapsed 6 | Traveling 4 | Dwelling 2 | Completion tick 6",
            "Stop 0 (node 8): boarded 2 | alighted 0 | waiting 2",
            "Stop 1 (node 3): boarded 2 | alighted 2 | waiting 0",
            "Stop 2 (node 21): boarded 0 | alighted 2 | waiting 0",
        ]
    );
    let positions = [0.0, 0.0, 0.25, 0.5, 0.5, 0.75, 1.0];
    for (index, expected_x) in positions.into_iter().enumerate() {
        playback.seek(index);
        assert_eq!(playback.snapshot().unwrap().tick, index as u64);
        assert_eq!(playback.vehicle_x(), Some(expected_x));
        assert_eq!(playback.summary(), summary);
    }
    assert_eq!(
        playback.status(),
        "Tick 6 | Occupancy 0/2 | Waiting 0 | Onboard 0 | Arrived 4 | Unserved 2"
    );
    assert_eq!(playback.passenger_totals(), [0, 0, 4, 2]);
    playback.seek(3);
    assert_eq!(playback.passenger_totals(), [2, 2, 2, 0]);
    playback.seek(0);
    playback.toggle();
    playback.elapse(Duration::from_secs(100));
    assert_eq!(playback.cursor(), 6);
    assert!(!playback.playing());
    playback.toggle();
    assert!(!playback.playing());
    assert_eq!(
        results::encode(&playback.result().world, &playback.result().trace).unwrap(),
        before
    );
}

#[test]
fn pause_resume_and_seek_account_for_fractional_time() {
    let mut playback = demo();
    assert!(!playback.playing());
    playback.elapse(Duration::from_secs(50));
    assert_eq!(playback.cursor(), 0);
    playback.toggle();
    playback.elapse(Duration::from_millis(750));
    playback.toggle();
    playback.elapse(Duration::from_secs(50));
    assert_eq!(playback.cursor(), 0);
    playback.toggle();
    playback.elapse(Duration::from_millis(250));
    assert_eq!(playback.cursor(), 1);
    playback.elapse(Duration::from_millis(900));
    playback.seek(2);
    assert!(!playback.playing());
    playback.toggle();
    playback.elapse(Duration::from_millis(100));
    assert_eq!(playback.cursor(), 2);
    playback.seek(usize::MAX);
    assert_eq!(playback.cursor(), 6);
    playback.seek(0);
    assert_eq!(playback.cursor(), 0);
}

#[test]
fn empty_single_and_partial_recordings_remain_bounded_without_analysis() {
    for json in [
        include_str!("fixtures/results/partial.json"),
        include_str!("fixtures/results/completion-only.json"),
    ] {
        let mut playback = Playback::new(results::parse("partial.json", json).unwrap());
        assert!(playback.result().analysis.is_none());
        assert!(playback.summary()[0].contains("N/A"));
        playback.toggle();
        playback.elapse(Duration::MAX);
        assert!(!playback.playing());
        assert_eq!(
            playback.cursor() + 1,
            playback.result().trace.snapshots.len()
        );
    }
    let mut json: serde_json::Value = serde_json::from_str(RECORDING).unwrap();
    json["trace"]["completed"] = false.into();
    json["trace"]["snapshots"] = serde_json::json!([]);
    let mut playback = Playback::new(results::parse("empty.json", &json.to_string()).unwrap());
    playback.seek(usize::MAX);
    playback.toggle();
    playback.elapse(Duration::MAX);
    assert!(!playback.playing());
    assert_eq!(playback.snapshot(), None);
    assert_eq!(playback.vehicle_x(), None);
    assert_eq!(playback.passenger_totals(), [0; 4]);
    assert_eq!(playback.status(), "No recorded snapshots");
}

#[test]
fn absolute_ticks_keep_u64_precision() {
    let mut json: serde_json::Value = serde_json::from_str(RECORDING).unwrap();
    for (index, frame) in json["trace"]["snapshots"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        frame["tick"] = (u64::MAX - 6 + index as u64).to_string().into();
    }
    let mut playback =
        Playback::new(results::parse("large-ticks.json", &json.to_string()).unwrap());
    playback.toggle();
    playback.elapse(Duration::from_secs(3));
    assert_eq!(playback.cursor(), 3);
    assert_eq!(playback.snapshot().unwrap().tick, u64::MAX - 3);
    playback.elapse(Duration::from_secs(3));
    assert!(playback.status().starts_with("Tick 18446744073709551615 |"));
}
