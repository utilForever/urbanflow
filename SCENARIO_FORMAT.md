# Scenario File Format, Version 1

This is the consumer-side input contract for [#204](https://github.com/utilForever/urbanflow/issues/204), implemented by the [scenario loader](examples/scenario/mod.rs) for [#205](https://github.com/utilForever/urbanflow/issues/205). The [`run_scenario` CLI](README.md#run-a-scenario-file) executes Rail files and writes result JSON and optional offline HTML. It rejects Env files because no action policy or Env result format is defined. The other examples still construct inputs in Rust. The normative fixtures below describe loader acceptance; CLI execution additionally requires a Rail kind and valid output paths.

One UTF-8 JSON file contains one object describing a fresh `env` or `rail` scenario. These kinds run independently: Env actions do not move a Rail vehicle, and Rail movement does not evaluate Env rewards. The file is not a saved episode, trace, analysis result, or viewer payload.

## Document Shape

All listed fields are required for their kind. Names and enum values are case-sensitive. Reject missing fields, `null`, unknown fields at any level, and duplicate object member names. Object member order and JSON whitespace have no meaning; array order is significant. Comments, trailing commas, and non-JSON values such as `NaN` and `Infinity` are invalid.

| Field     | JSON type           | Meaning                                                    |
| --------- | ------------------- | ---------------------------------------------------------- |
| `version` | Integer number      | Exactly `1`.                                               |
| `kind`    | String              | Exactly `"env"` or `"rail"`.                               |
| `nodes`   | Array of ID strings | Unique node IDs in caller-supplied order.                  |
| `edges`   | Array of objects    | Directed edges in insertion order; fields described below. |
| `demands` | Array of objects    | Passenger demand records in allocation/boarding order.     |

Each edge has exactly `from` (node ID string), `to` (node ID string), and `kind` (`"Road"` or `"Rail"`). Its ID is its zero-based position in `edges`, matching `Network::add_edge`. There is no explicit edge `id` field or ID remapping. Node IDs are values, not positions in `nodes`; node and edge IDs are separate namespaces.

Each demand has exactly `origin` (node ID string), `destination` (node ID string), and `amount` (unsigned 32-bit integer number). Demands have no separate IDs; their array positions identify their records.

An `env` object additionally has exactly:

| Field       | JSON type      | Meaning                                                                                      |
| ----------- | -------------- | -------------------------------------------------------------------------------------------- |
| `budget`    | Number         | Finite, nonnegative `f64`, in the same abstract cost units as `EdgeKind::construction_cost`. |
| `max_steps` | Decimal string | Nonnegative episode step limit, checked for the target's `usize` range. Zero is allowed.     |

A `rail` object additionally has exactly:

| Field       | JSON type                | Meaning                                                         |
| ----------- | ------------------------ | --------------------------------------------------------------- |
| `route`     | Array of edge ID strings | Nonempty ordered sequence of connected Rail edges from `edges`. |
| `vehicle`   | Object                   | Exactly the three fields below, for one vehicle.                |
| `max_ticks` | Decimal string           | Nonnegative `u64` recording limit. Zero is allowed.             |

| `vehicle` field         | JSON type      | Meaning                                                                  |
| ----------------------- | -------------- | ------------------------------------------------------------------------ |
| `capacity`              | Integer number | Positive `u32` passenger capacity, independent of network edge capacity. |
| `travel_ticks_per_edge` | Decimal string | Positive `u64` travel duration used for every route edge.                |
| `dwell_ticks_per_stop`  | Decimal string | Positive `u64` dwell duration before each departure.                     |

Do not combine the kinds: `route`, `vehicle`, and `max_ticks` are invalid for `env`; `budget` and `max_steps` are invalid for `rail`. No field has an implicit default. Empty node, edge, and demand arrays are allowed when all remaining references and kind-specific constraints are satisfied; Rail still requires a nonempty route.

## Numbers, IDs, and Units

Every ID, `max_steps`, and tick value is a JSON string of ASCII decimal digits matching `0|[1-9][0-9]*`, with a value at most `18446744073709551615` (`u64::MAX`). Reject signs, leading zeroes except `"0"`, spaces, fractions, exponents, and JSON numbers in these fields. For example, `"9007199254740993"` retains its exact value; converting it through a floating-point number is forbidden.

Node IDs, edge references, and `max_steps` also must fit `usize` on the execution target. Use checked conversion, never truncation, wrapping, clamping, or renumbering. On a 32-bit target their maximum is `4294967295`; on a 64-bit target it is `18446744073709551615`. Files using larger IDs are not portable to smaller targets. Edge references must additionally identify an existing array entry. Durations and `max_ticks` remain `u64` on either target.

`version`, `amount`, and `capacity` use unsigned JSON integer tokens (digits only, no minus sign, fraction, or exponent). `amount` is in `0..=4294967295`; `capacity` is in `1..=4294967295`. Booleans and quoted numbers are not substitutes. These values fit exactly in browser numbers. Budget alone uses binary64 floating-point semantics, matching `Env`; reject values that convert to a negative or non-finite `f64`. Decimal budget values round to the nearest representable `f64`, matching Rust's `str::parse::<f64>` so file loading preserves action affordability at construction-cost boundaries.

Passenger amounts and capacities count people. Ticks are dimensionless simulation ticks, not seconds; playback speed does not change them. `max_steps` counts Env decisions, not Rail ticks. Network capacities and construction costs come from `EdgeKind` and are not configurable in this format.

## Ordering and References

- Preserve `nodes` order, which controls Env action enumeration and the viewer's layout.
- Preserve `edges` order, which defines edge IDs and breaks equal-hop shortest-path ties. Do not sort edges or synthesize reverse connections.
- Preserve `demands` order, including duplicates and zero amounts. This order determines which demand receives limited capacity. Do not merge records.
- Preserve `route` order. Repeated edge references and repeated node visits are valid when each consecutive edge connects in the directed order; they represent separate visits, not duplicate definitions.
- Reject duplicate node IDs. Reject self-connected edges and repeated `(from, to, kind)` edges. Road and Rail may share an ordered node pair; a reverse edge is a separate edge.
- Every edge endpoint and demand endpoint must name a node in `nodes`, even for zero demand. Every route reference must name an existing Rail edge, and consecutive edges must connect. No node or edge is created to repair a missing reference.

Demand between existing nodes may be unreachable, outside the Rail route, or have the same origin and destination. These are valid inputs; existing simulation rules determine service. In particular, Rail boarding requires a destination at a strictly later route visit, so a repeated visit may serve same-node demand. Rejecting unknown demand endpoints at the file boundary applies to both kinds, even though `RailPassengers::new` itself does not validate a world.

## Construction and Recording

A consumer must parse and validate the whole input before starting simulation or replacing an existing run. Reuse the existing constructors and validation rather than introducing a second simulation model:

1. Decode the JSON object, require version `1`, select the kind, and check field shapes, exact integers, and target ranges.
2. Build nodes in order and insert edges in order through `Network::add_edge`. Validate topology and demand endpoints; retain the original demand records.
3. For `env`, call `Env::new(world, demands, budget, max_steps)`. Initial edges are already present and do not deduct from the supplied budget. Actions, policies, training seeds, and reward weights are not file fields; a consumer supplies actions through the existing API.
4. For `rail`, validate topology with `World::validate_topology`, check demand endpoints, then call `RailRoute::new` and `RailVehicle::new`. Initialize `RailPassengers::new` and a default `SimulationClock`, and record with `max_ticks` only after validation succeeds.

Rail starts at tick zero, at route stop zero with its full dwell duration; all demand is waiting. The first advance processes the initial stop. `record_trace` includes the initial snapshot without consuming the limit and then at most `max_ticks` advances. A zero limit gives one initial snapshot and `completed = false`. Finishing on the last allowed tick gives `completed = true`. A smaller limit preserves waiting/onboard passengers in a partial trace; it does not force completion or mark them unserved. Final arrival adds no dwell, and completion describes the vehicle, not universal passenger arrival.

Limits bound attempted work; they do not promise that service will finish or that a recording will fit in memory. A route's full duration need not fit in `u64` if only a prefix is recorded. Preserve the core's checked runtime errors and the analysis module's overflow/history rules. Do not precompute the full duration with unchecked addition/multiplication or silently shorten a requested recording.

## Versions and Errors

Version `1` is the only supported version. Reject an absent, wrongly typed, or unsupported version; never infer a version, fall back to version `1`, or partially interpret a newer document. After JSON decoding, reject an unsupported version before interpreting version-specific fields. Changing field names, accepted types, units, or behavior requires a new version; there is no generic schema registry or automatic migration.

A rejection must identify the file and offending field or array index and explain the problem (for example, `route[1]: unknown edge ID "9"` or `nodes[0]: ID does not fit 32-bit usize`). Distinguish malformed JSON/shape, unsupported version/kind, numeric range/conversion, and invalid topology/reference/vehicle settings. Exact diagnostic wording and precedence between independent errors are not standardized. Reject the whole scenario without exposing partially built state or starting any ticks/actions. Do not silently ignore, correct, reorder, or coerce invalid values.

## Fixtures and Expected Results

Fixtures live in [`tests/fixtures/scenarios`](tests/fixtures/scenarios). They are complete documents, with no include/override mechanism. The invalid files are intentionally syntactically valid JSON; each demonstrates one contract violation. [Loader tests](tests/scenario_loading.rs) enforce these outcomes and compare loaded scenarios against their Rust-built equivalents. They also cover malformed JSON, strict field shapes, duplicate members, numeric tokens, target-dependent boundaries, deterministic outputs, and partial recordings. Result round trips belong to the separate result adapter.

| Valid fixture                                                         | Expected interpretation                                                                                                                                                                                                                                               |
| --------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [env-baseline.json](tests/fixtures/scenarios/env-baseline.json)       | Exactly [`baseline::scenario`](examples/baseline/mod.rs): nodes `0, 1`, no edges, demand `0 -> 1` of 15, budget 2, one step. Adding Rail `0 -> 1` serves 15, leaves budget 0, gives reward 12.25, and completes the episode.                                          |
| [rail-demo.json](tests/fixtures/scenarios/rail-demo.json)             | Exactly [`rail_viewer::scenario`](examples/rail_viewer/mod.rs): nodes `8, 3, 21, 5`; Road edges `0..3`; Rail route `4, 5, 6`. Capacity 6, travel 4, dwell 2, limit 32: 19 snapshots at ticks `0..18`, completed, 6 arrived and 5 unserved.                            |
| [rail-boundaries.json](tests/fixtures/scenarios/rail-boundaries.json) | Valid on 64-bit targets; reject `nodes[0]` on 32-bit targets. Uses `u64::MAX` as a node ID and duration, `u32::MAX` capacity/amount, repeated route edges, duplicate demands, and a zero demand. Zero tick limit yields one initial snapshot with all demand waiting. |
| [env-empty.json](tests/fixtures/scenarios/env-empty.json)             | Empty topology/demand, zero budget and zero step limit are valid. There are no available actions.                                                                                                                                                                     |

| Invalid fixture                                                               | Required rejection                                                          |
| ----------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| [unsupported-version.json](tests/fixtures/scenarios/unsupported-version.json) | `version` is `2`; unsupported version.                                      |
| [duplicate-node.json](tests/fixtures/scenarios/duplicate-node.json)           | `nodes[1]` repeats ID `"0"`.                                                |
| [unknown-demand-node.json](tests/fixtures/scenarios/unknown-demand-node.json) | `demands[0].destination` names absent node `"9"`.                           |
| [unknown-route-edge.json](tests/fixtures/scenarios/unknown-route-edge.json)   | `route[0]` names edge `"1"`, but only edge `"0"` exists.                    |
| [numeric-node-id.json](tests/fixtures/scenarios/numeric-node-id.json)         | `nodes[0]` is a number instead of an ID string.                             |
| [amount-overflow.json](tests/fixtures/scenarios/amount-overflow.json)         | `demands[0].amount` is `4294967296`, above `u32::MAX`.                      |
| [tick-overflow.json](tests/fixtures/scenarios/tick-overflow.json)             | `max_ticks` is `"18446744073709551616"`, above `u64::MAX`.                  |
| [zero-travel-ticks.json](tests/fixtures/scenarios/zero-travel-ticks.json)     | `vehicle.travel_ticks_per_edge` is `"0"`; travel duration must be positive. |
| [unknown-field.json](tests/fixtures/scenarios/unknown-field.json)             | `max_ticks` is not an Env field.                                            |

Tram/DRT, GIS coordinates, multiple routes/vehicles, timetables, transfers, checkpoint state, and result serialization are outside version 1. Consumer I/O and parsing dependencies remain outside the core simulation crate.
