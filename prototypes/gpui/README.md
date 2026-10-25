# GPUI Kit Rail playback trial

Issue [#208](https://github.com/utilForever/urbanflow/issues/208). This first stage isolates recorded playback from simulation; desktop and web rendering are separate consumer work.

## Recorded playback model

[`src/playback.rs`](src/playback.rs) reads the existing seven-snapshot [`rail_service.json`](../../tests/fixtures/rail_service.json) through the [result adapter](../../examples/results/mod.rs). It changes only the cursor and elapsed wall time, never the recording or simulation. Playback advances one recorded tick per second, stops at the final snapshot, and preserves fractional elapsed time when paused. Seeking clamps to the recording, pauses, and clears fractional time.

| Observation                      | Expected value in both consumers     |
| -------------------------------- | ------------------------------------ |
| Nodes, in order                  | 8, 3, 21                             |
| Snapshots                        | 7, ticks 0 through 6                 |
| Tick 0                           | occupancy 0/2, waiting 6             |
| Tick 3                           | occupancy 2/2, waiting 2, arrived 2  |
| Tick 6                           | occupancy 0/2, arrived 4, unserved 2 |
| Mean waiting / onboard / journey | 2.00 / 2.50 / 4.50 ticks             |
| Maximum / mean occupancy         | 2 / 1.67                             |
| Elapsed / traveling / dwelling   | 6 / 4 / 2 ticks                      |

`cargo test --test gpui_playback` from the repository root checks the actual shared playback module against the existing viewer payload and hand-checked values. It covers pause/resume timing, seek bounds, completion, empty/single/partial recordings, exact `u64` tick labels, and unchanged serialized results after playback. Those broader model tests do not make the fixed UI a general result-file browser.

## Bundled typography

[Pretendard 1.3.9](https://github.com/orioncactus/pretendard/releases/tag/v1.3.9) Regular, Medium, SemiBold, and Bold are bundled with their SIL Open Font License in `assets/`. These are the unchanged `public/static/*.otf` files from the official `Pretendard-1.3.9.zip` release. The four weights used by this screen total 6,318,784 bytes (6.03 MiB); no font installation or external font request is needed.
