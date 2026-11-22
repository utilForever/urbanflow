# Architecture

`urbanflow` has two independent simulation paths: `Env` evaluates network-building actions through aggregate demand allocation; `rail` models one vehicle's timed service. `Env::step` neither advances Rail movement nor uses its results.

See [README](README.md) for usage, [AGENTS](AGENTS.md) for contributor rules, and each module's rustdoc for API and error contracts.

## Overview

```mermaid
flowchart LR
    subgraph Environment["Environment: network design"]
        Inputs["World + demands + action"] --> Env
        Env --> Allocation["simulation::tick"]
        Allocation --> Metrics
        Metrics --> Env
        Env --> Result["StepResult + Observation"]
    end
    subgraph Rail["Rail: fixed-route service"]
        State["Vehicle + passengers + clock"] --> Advance["advance"]
        Advance --> State
        State --> Record["snapshot / record_trace"]
        Record --> Trace["RailTrace"]
    end
    Trace --> Analyze["analysis::analyze"]
    Analyze --> Summaries["OperationalAnalysis"]
    Trace --> Viewer["Example: offline HTML viewer"]
    Summaries --> Viewer
```

Core outputs own their data. Consumers can retain observations and replay traces without borrowing live state or making simulation decisions.

The [scenario file contract](SCENARIO_FORMAT.md) defines versioned consumer inputs for either Env or Rail. The [`scenario` consumer adapter](examples/scenario/mod.rs) reads UTF-8 JSON, checks the version before interpreting its fields, preserves array order, and converts decimal strings through checked `u64`/`usize` conversions. Edge IDs remain insertion indices. Serde, serde_json, and serde_path_to_error are development dependencies used only at this boundary; the core has no configuration hierarchy or serialization dependency.

Loading returns a validated `Env` or a `RailScenario` containing the world, vehicle, passengers, tick-zero clock, and recording limit. `Network::add_edge`, `Env::validate_inputs` (including `World::validate_topology` and demand endpoints for both kinds), `Env::new`, `RailRoute::new`, and `RailVehicle::new` own simulation validation. The adapter adds strict file-shape/numeric checks and file/field diagnostics. It exposes no partial state on failure and performs no actions, ticks, analysis, or output writes.

The [result adapter](examples/results/mod.rs) owns versioned Rail result persistence independently of scenario loading. It reuses the viewer payload, enforces the [result format](RESULT_FORMAT.md)'s byte limit, numeric bounds, topology references, and recording consistency, and reconstructs owned `World`/`RailTrace` values without simulation. Complete-trace analysis uses the core API; partial traces receive consumer-side consistency checks because core analysis requires completion. Imported analysis is discarded and recomputed, with missing initial history represented by no analysis. Loading checks canonical output size with that recomputed analysis before returning state. Validation does not certify every movement or boarding rule. Validation and encoding finish before opening output files; there is no core serialization hierarchy or new dependency.

## Module Map

All modules are public except `simulation`.

| Modules                                                                                               | Responsibility                                                   |
| ----------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| [`world`](src/world.rs), [`demand`](src/demand.rs)                                                    | Topology, mode capacities and costs, passenger demand            |
| [`network`](src/network.rs)                                                                           | Derived adjacency and directed shortest paths                    |
| [`simulation`](src/simulation.rs)                                                                     | Aggregate capacity allocation and metrics                        |
| [`env`](src/env.rs), [`action`](src/action.rs)                                                        | Scenario lifecycle, actions, episodes, rewards                   |
| [`time`](src/time.rs)                                                                                 | Checked integer simulation clock                                 |
| [`rail`](src/rail.rs)                                                                                 | Routes, vehicle movement, passenger lifecycle, snapshots, traces |
| [`analysis`](src/analysis.rs)                                                                         | Validation and operational summaries of completed Rail traces    |
| [`metrics`](src/metrics.rs), [`observation`](src/observation.rs), [`step_result`](src/step_result.rs) | Owned environment outputs                                        |

## Execution Flows

### Environment

`Env::new` validates and retains the initial scenario. Each `step` validates an action before adding an edge, deducting its cost, allocating demand, and returning metrics, reward, and an observation. `reset` restores the scenario for repeatable episodes. Input order, error precedence, and reset semantics are specified in the [Env API](src/env.rs).

### Rail Service

The caller keeps a vehicle, clock, and passenger set together. `advance` coordinates one atomic tick of movement and stop processing; `snapshot` copies the state, and `record_trace` records bounded advances. A partial trace retains its last state without forcing unfinished journeys to complete.

Rail uses fixed travel/dwell durations and aggregate passenger counts. Vehicle capacity is independent of network edge capacity; movement uses neither shortest-path routing nor congestion. Simulation ticks have no wall-clock unit or pacing policy. See the [Rail API](src/rail.rs) for timing and passenger contracts.

### Scenario CLI

The [`run_scenario` consumer](examples/run_scenario/mod.rs) connects scenario loading to `RailVehicle::record_trace`, the result encoder, and optional offline viewer rendering. It passes the file's `max_ticks` directly to recording, retains partial state, and uses the existing result adapter for validation and analysis. Env files are rejected because the CLI has no action policy or Env result contract.

All computation finishes before output staging. The CLI and desktop share [file identity checks](examples/run_scenario/files.rs) for existing inputs and outputs. The CLI compares output directory identities across mount paths and uses native filename lookup for case and Unicode aliases even when the outputs do not exist. Both consumers resolve live output links and verify the chosen destination again before rename. An unresolved link or a changed target stops that replacement. The CLI stages both outputs and renames HTML before JSON. Each replacement is atomic. A final JSON rename failure can therefore leave updated HTML. Ordinary failures clean up temporary files and directories. CLI I/O and exit behavior stay outside the core. The adapter's standalone `save` API retains its documented ordinary-write semantics. See [command usage](README.md#run-a-scenario-file).

### Operational Result Contracts

Every analysis requires a completed trace; `analyze` and `StopActivity::from_trace` additionally require every tick from stop zero with all passengers initially waiting, while other summaries accept partial history.

`analysis::analyze` validates once and combines the same calculations used by individual `from_trace` summaries. It returns owned results without advancing service or reconstructing missing history. Units, history requirements, undefined denominators, and errors belong in [analysis rustdoc](src/analysis.rs); the [executable example](examples/operational_analysis.rs) demonstrates the workflow.

## Simulation Contracts

- Edges are directed. Return connections have their own construction cost; self-connections and same-mode duplicates are rejected. Road and Rail may share an ordered node pair.
- Demands consume capacity in stored order along the first shortest path by hop count, with ties resolved by edge insertion order. The path's smallest remaining capacity limits service; excess and unreachable demand are unserved.
- Congestion is the maximum edge load/capacity ratio, or zero without edges. Cost sums construction costs. Reward is `served demand - unserved demand - congestion - cost`.
- Invalid actions and overflow are detected before mutation. A failed Rail tick preserves vehicle, clock, and passengers together. Rail lifecycle counts always conserve each original demand, including duplicates and zero amounts.

## Training Consumers

[`random_policy`](examples/random_policy.rs) and [`tabular_q_learning`](examples/tabular_q_learning.rs) use the public Env API with fixed seeds and one step per episode. Policy state stays outside the core; the learner stores one value per action, with no general state table or multi-step training loop.

## Browser Consumer

The [`rail_viewer` example](examples/rail_viewer.rs) records service before generating one offline HTML file. Its [Rust module](examples/rail_viewer/mod.rs) calls the shared result encoder and escapes the JSON for an HTML script element; its [template](examples/rail_viewer/viewer.html) owns layout, playback, and formatting. Identical inputs produce identical traces and HTML.

Playback reads fixed snapshots and never advances simulation. Only the vehicle marker is interpolated; status follows recorded ticks, and analysis stays fixed. IDs and `u64` values remain exact through string serialization and `BigInt` passenger totals. Partial traces and completed traces missing initial history remain viewable without analysis; other analysis errors prevent file writing. See [README](README.md#browser-viewer) for execution and the consumer's rustdoc for display contracts.

## Desktop Consumer

[`apps/desktop`](apps/desktop/README.md) is the Windows application entry point and a separate Cargo workspace. GPUI Kit and Serde are consumer dependencies. The window starts with a bundled Rail preview. It owns the selected scenario and the last successful recording separately so another selection, a cancelled dialog, or a failed operation preserves the displayed result and its source. Each successful Open creates a new selection. The window labels the recording as a previous run until a run of that selection succeeds.

GPUI serializes Open, Run, and Save and shows native file pickers on the foreground executor. Its background executor loads validated Rail inputs and runs a fresh copy through the core's `snapshot` and `advance` APIs. The result adapter encodes each frame before retaining it and stops on the 16 MiB output limit. The [desktop workflow](apps/desktop/src/workflow.rs) publishes the world, trace, validated analysis, and JSON together. Each input retains its chosen path, resolved path, and file identity. On Windows a metadata-only handle pins the opened file after decoding, so editors can request exclusive data access.

The [shared output writer](examples/run_scenario/output.rs) stages a new file directly in its destination folder so it inherits even ACL entries limited to immediate children. For an existing file it stages the replacement in a temporary directory, copies the owner's and group's identities and access rules before writing any bytes, and refreshes them before rename. This includes Unix access bits, Linux and macOS access ACLs, and the Windows discretionary ACL. An existing Windows EFS-encrypted destination stops the write because copying its ACL cannot preserve its encryption recipients. If a replacement destination disappears during staging, a retry creates a new file with the folder's permissions.

Before rename, both consumers check each opened input's identity and the current targets of its chosen and resolved paths. Desktop Save protects the recorded and selected inputs. The name of a circular input link stays reserved. For a single new export, the writer creates only a sibling file unless it needs a separate filename check. Directory identities exclude reserved names in other folders. A live input in the same folder can supply the filename check. For an unresolved name, a temporary directory lets the filesystem compare spellings without occupying the destination.

Closing the last window quits the process. Save defers a close request from the moment the command starts until the picker is cancelled or replacement succeeds. A save error clears the request and keeps the recording available. The dedicated Windows workflow checks the desktop package and runs the root integration test that compares desktop and CLI exports. Root CI checks the core and examples, and the GPUI prototype has its own workspace.

## GPUI Kit Feasibility Consumer

The isolated [`prototypes/gpui`](prototypes/gpui/README.md) consumer evaluates a pinned GPUI Kit desktop/web stack using `tests/fixtures/rail_service.json`. Both targets parse the same embedded result through the existing result adapter. Its playback cursor and wall-clock pacing remain consumer state; no `RailVehicle`, simulation clock, or simulation step is used. The appearance module maps Fluent 2 light/dark colors onto the Kit theme; changing appearance does not mutate playback. Root integration tests exercise the same playback module without GUI dependencies, while the isolated workspace tests palette contrast. The prototype has its own Cargo workspace, lockfile, and toolchain. The Windows application reuses the selected Kit version; browser and other-platform feasibility remain separate decisions.

## Planned Direction

Current Rail service has one fixed-route vehicle. Multiple vehicles/routes, automatic reverse service, timetables, headways, transfers, Tram/DRT service, road traffic (signals, lanes, congestion-driven movement), and GIS maps remain deferred.

Broader analysis, statistical experiments, confidence intervals, optimization, large-scale low-latency simulation, and application integrations are also planned. Dataframes, databases, query engines, charting dependencies, hosted dashboards, and frontend frameworks remain outside the supported workflow; WebAssembly is limited to the isolated feasibility trial above.

Future interfaces should use the public API and keep rendering, coordinates, serialization, storage, and transport in consumer layers. Add modules only for concrete requirements.
