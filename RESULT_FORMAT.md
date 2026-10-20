# Rail Result Format, Version 1

The [result adapter](examples/results/mod.rs) implements [#206](https://github.com/utilForever/urbanflow/issues/206). It saves and reloads recorded Rail state for playback and operational analysis. Serialization and file I/O stay in the consumer layer; the core has no serialization dependency. This is not a scenario, an Env result, or a resumable vehicle checkpoint.

## Consumer API

Include `examples/results/mod.rs` as a module, as the viewer does. The adapter provides:

- `save(path, &world, &trace)`: validate, compute analysis, and write JSON.
- `load(path)`: read and validate JSON, returning a `RailResult` with `world`, `trace`, and freshly computed optional `analysis`.
- `encode(&world, &trace)` and `parse(file, json)`: the same conversion without file I/O. The `file` argument qualifies parse errors.

Pass the loaded world and trace to `rail_viewer::render` for offline playback. Loading and rendering do not run the simulation or mutate an existing run. The viewer uses this same encoder, so saved and embedded payloads have identical values. Command-line file selection and scenario execution remain tracked in [#207](https://github.com/utilForever/urbanflow/issues/207); the current viewer command still records its built-in demo.

## Document Shape

One UTF-8 JSON object contains these fields. All except `analysis` are required; names and enum labels are case-sensitive. Unknown, missing, null, duplicate, and wrongly typed fields are rejected, except for the optional analysis cache described below. Objects must be objects, not positional arrays. Whitespace and object member order do not affect meaning; array order does.

| Field      | Type                     | Meaning                                                           |
| ---------- | ------------------------ | ----------------------------------------------------------------- |
| `version`  | Integer number           | Exactly `1`; checked before interpreting version-specific fields. |
| `nodes`    | Array of decimal strings | Unique node IDs in layout order.                                  |
| `edges`    | Array of objects         | Directed network edges in insertion order.                        |
| `trace`    | Object                   | Exactly `completed` (boolean) and `snapshots` (ordered array).    |
| `analysis` | Object or null, optional | Untrusted cache, discarded and recomputed on import.              |

An edge has exactly `id`, `from`, `to` (decimal ID strings), and `kind` (`"Road"` or `"Rail"`). Its `id` must equal its zero-based insertion index. Endpoints must exist in `nodes`. Self-connections and duplicate `(from, to, kind)` edges are rejected through `Network::add_edge`; Road and Rail can share endpoints. The adapter neither sorts nor renumbers anything.

A snapshot has exactly:

| Field        | Type             | Meaning                                               |
| ------------ | ---------------- | ----------------------------------------------------- |
| `tick`       | Decimal string   | Absolute `u64` simulation tick.                       |
| `position`   | Object           | One of the position variants below.                   |
| `occupancy`  | Integer number   | `u32` onboard passenger total.                        |
| `capacity`   | Integer number   | Positive `u32` vehicle capacity.                      |
| `passengers` | Array of objects | Original ordered demand records and lifecycle counts. |

Each passenger object has exactly `from`, `to` (existing node ID strings), and `amount`, `waiting`, `onboard`, `arrived`, `unserved` (unsigned `u32` integer numbers). Duplicate and zero-amount demands retain separate records in their original order. Lifecycle counts must sum to `amount`; occupancy must equal the sum of onboard counts and cannot exceed capacity.

The `position.kind` string selects exactly these additional fields, all decimal strings:

| Kind        | Additional fields                                                                |
| ----------- | -------------------------------------------------------------------------------- |
| `AtStop`    | `stop_index`, `node`, `dwell_ticks_remaining`                                    |
| `Traveling` | `edge_index`, `edge`, `from`, `to`, `travel_ticks_elapsed`, `travel_ticks_total` |
| `Complete`  | `stop_index`, `node`                                                             |

Stop nodes must exist. Traveling positions must identify a Rail edge with matching endpoints and satisfy `0 <= travel_ticks_elapsed < travel_ticks_total`. Remaining dwell must be positive. Route indices identify visits, not network IDs; repeated node visits remain distinct. The file does not include a route definition or vehicle settings for restarting simulation.

## Numbers, Ordering, and Limits

- Every ID, route index, tick, duration, and aggregate count uses an ASCII decimal string matching `0|[1-9][0-9]*`, bounded by `u64::MAX`. IDs and route indices must also fit the target's `usize`. Reject overflow, leading zeroes, signs, fractions, exponents, whitespace inside these strings, and numeric JSON substitutes. IDs that fit a 64-bit target may not fit a 32-bit target.
- Per-record counts, capacity, and occupancy use unsigned integer JSON numbers in `0..=4294967295`; capacity excludes zero. These are exact in JavaScript. Analysis means and ratios use finite `f64` numbers; undefined values are `null`.
- Nodes, edges, snapshots, passengers, and analysis stop visits retain their stored order. Ticks are exactly one apart and can start at any `u64` tick, including a recording ending at `u64::MAX`.
- The maximum input or encoded output is **16 MiB (16,777,216 UTF-8 bytes)**, including whitespace. `load` reads at most the limit plus one byte before rejecting an oversized file. `parse` checks input size before decoding and canonical output size after recomputing analysis. Both must fit: a file below the input limit can still be rejected if adding the canonical analysis object or null would exceed the output limit. An unchanged loaded result therefore fits when encoded again. The encoder checks its completed JSON before any output file is opened. This bounds file size, not total process memory; decoded state and encoding buffers require additional memory.

## Completion and Analysis

`completed` describes vehicle completion, not whether everyone arrived. A `Complete` position is allowed only as the final snapshot of a completed recording. A completed recording must contain a final `Complete`; an empty partial recording is allowed.

Both complete and partial recordings require constant positive capacity, identical ordered demands, conserved counts, and matching occupancy. Within recorded intervals, waiting cannot increase, arrivals cannot decrease or exceed previously onboard counts, and unserved counts can increase only at completion. Partial recordings keep their waiting/onboard state without forcing completion.

The adapter calls `analysis::analyze` for every complete recording. A full recording receives all five core summaries. Partial recordings, resumed recordings missing initial history, and completion-only recordings remain usable for playback with `analysis: null`. Only `MissingInitialState` is treated as unavailable analysis; all other analysis errors, including overflow and inconsistent stop sequences, reject the result.

The optional imported `analysis` object is a cache with no trusted fields. Its contents, including unknown or duplicate member names, are ignored; malformed JSON is still rejected. Omitting it, setting it to null, or forging its metrics cannot bypass trace validation or change the recomputed result. Saving always emits the canonical core-produced cache or null. That cache follows the existing viewer convention:

- `passenger_outcomes`: `requested`, `arrived`, `unserved` as count strings; `served_share` as a number or null.
- `passenger_times`: `waiting_passenger_ticks`, `onboard_passenger_ticks`, `arrived_waiting_passenger_ticks`, `arrived_onboard_passenger_ticks` as strings; `mean_waiting_ticks`, `mean_onboard_ticks`, `mean_journey_ticks` as numbers or null.
- `vehicle_occupancy`: `capacity`, `max_occupancy` as `u32` numbers; `occupied_passenger_ticks` as a string; `mean_occupancy`, `max_load_factor`, `mean_load_factor` as numbers or null.
- `stops`: ordered visits containing string fields `stop_index`, `node`, `boarded`, `alighted`, `remaining_waiting`.
- `route_timing`: string fields `elapsed_ticks`, `traveling_ticks`, `dwelling_ticks`, `completion_tick`.

See [analysis rustdoc](src/analysis.rs) for accounting units, intervals, history requirements, and errors. Import checks consistency and references; it does **not** prove every movement duration, boarding decision, or agreement with an unavailable original route. It does not reconstruct missing history or replay simulation to validate it.

## Errors and Output Safety

Unsupported or missing versions are rejected; there is no automatic migration of unversioned viewer payloads. `ResultError` carries the file and diagnostic detail, including field or snapshot indices where available. In-memory encoding uses `<result>` as the file label. No partial loaded state is returned on error.

Saving validates and encodes before opening the destination, so invalid analysis, malformed state, or oversized encoded output cannot overwrite a valid result. Rendering likewise finishes before the viewer writes HTML. Successful writes replace an existing file. Ordinary write failures, such as a full disk during the write, are returned but are not a transactional filesystem guarantee.

Standalone output is JSON. The viewer escapes `<`, `>`, `&`, U+2028, and U+2029 before embedding JSON into a script element, so strings cannot terminate the element or inject HTML. Consumers embedding JSON elsewhere must apply escaping appropriate to that context.

## Fixtures and Validation

The [full service fixture](tests/fixtures/rail_service.json) is shared by the Rust round-trip tests and browser playback assertions. [Partial](tests/fixtures/results/partial.json) and [completion-only](tests/fixtures/results/completion-only.json) fixtures exercise recordings without full analysis. [Result tests](tests/result_loading.rs) also cover exact integer boundaries, duplicate demands, repeated visits, malformed documents, unsupported versions, forged analysis, and preservation of existing output files on validation failure.

Run `cargo test --test result_loading` for the adapter and the validation commands in [AGENTS.md](AGENTS.md) for the repository. No database, binary format, simulation checkpoint, or core serialization API is introduced.
