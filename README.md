<p align="center">
  <picture>
    <img src="https://raw.githubusercontent.com/utilForever/urbanflow/refs/heads/main/assets/logo.png" width="400"/>
  </picture>
</p>
<p align="center">
  <b>A Rust library for an RL environment for building and optimizing multimodal urban transit networks</b>
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT" /></a>
  <a href="https://github.com/utilForever/urbanflow/actions/workflows/rust.yml"><img src="https://github.com/utilForever/urbanflow/actions/workflows/rust.yml/badge.svg?branch=main" alt="Rust" /></a>
  <a href="https://github.com/utilForever/urbanflow/actions/workflows/typos.yml"><img src="https://github.com/utilForever/urbanflow/actions/workflows/typos.yml/badge.svg?branch=main" alt="Typos" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=alert_status" alt="Quality Gate Status" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=ncloc" alt="Lines of Code" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=coverage" alt="Coverage" /></a>
  <br />
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=sqale_rating" alt="Maintainability Rating" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=reliability_rating" alt="Reliability Rating" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=security_rating" alt="Security Rating" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=bugs" alt="Bugs" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=vulnerabilities" alt="Vulnerabilities" /></a>
  <a href="https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow"><img src="https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=sqale_index" alt="Technical Debt" /></a>
</p>

## What This Library Does

`urbanflow` is a deterministic Rust library for building and evaluating urban transit networks:

- `Env` evaluates directed Road/Rail networks with demand, capacity, construction costs, and reinforcement learning rewards.
- `rail` runs one fixed-route vehicle with timed movement, boarding, alighting, snapshots, and bounded traces.
- `analysis` summarizes passenger outcomes and times, vehicle occupancy, stop activity, and route timing from completed Rail traces.
- An offline HTML viewer replays traces and displays the analysis.

Env and Rail run independently. Multiple vehicles, transfers, timetables, Tram/DRT service, road traffic, and geographic maps remain planned; see [Architecture](ARCHITECTURE.md#planned-direction).

## Quick Start

Requires Rust stable with edition 2024 support and Git.

```bash
git clone https://github.com/utilForever/urbanflow.git
cd urbanflow
cargo test --all
```

## Usage

### Environment

Use [`Env::new`](src/env.rs) for a caller-defined scenario or `Env::toy_city` for the built-in four-node world. `available_actions()` lists valid affordable actions; `step()` returns an observation, metrics, reward, and completion flag. `reset()` restores the initial scenario.

```rust
use urbanflow::action::Action;
use urbanflow::demand::Demand;
use urbanflow::env::Env;
use urbanflow::world::{EdgeKind, Network, Node, NodeId, World};

let world = World {
    nodes: vec![Node { id: NodeId(10) }, Node { id: NodeId(20) }],
    network: Network::new(),
};
let demands = vec![Demand::new(NodeId(10), NodeId(20), 15)];
let mut env = Env::new(world, demands, 10.0, 10).unwrap();
let action = Action::AddEdge {
    from: NodeId(10),
    to: NodeId(20),
    kind: EdgeKind::Rail,
};

let first = env.step(action).unwrap();
env.reset();

assert_eq!(env.step(action).unwrap(), first);
```

Metrics report served and unserved demand, congestion, and construction cost. See [simulation contracts](ARCHITECTURE.md#simulation-contracts) for allocation and reward rules.

### Scenario File Contract

The [version 1 scenario format](SCENARIO_FORMAT.md) specifies JSON inputs for independent Env and Rail runs, with ordered topology and demand, exact integer IDs and ticks, and representative valid/invalid fixtures. The consumer adapter in [`examples/scenario/mod.rs`](examples/scenario/mod.rs) provides `load(path)` and `parse(file, json)`, returning either a validated `Env` or fresh Rail inputs without running any actions or ticks. Errors identify the file and offending field or array index. IDs and step limits must fit the target's `usize`; tick values retain their full `u64` range.

The loader's [integration tests](tests/scenario_loading.rs) compare file inputs with the Rust-built baseline and Rail demo, including traces and analysis. Run them with `cargo test --test scenario_loading`. Parsing dependencies are development-only; the core library has no serialization dependency. Existing example commands still use their built-in scenarios; file execution and output generation are tracked in [#207](https://github.com/utilForever/urbanflow/issues/207).

### Rail Trace

Keep one vehicle, clock, and passenger set together. [`RailVehicle::record_trace`](src/rail.rs) records the initial state and each tick up to a supplied limit; `completed` means the vehicle finished its route, not that every passenger arrived. Use `advance()` and `snapshot()` for individual ticks.

The [analysis example](examples/operational_analysis.rs) includes the complete route, vehicle, demand, and recording setup.

### Operational Analysis

[`analysis::analyze(&trace)`](src/analysis.rs) returns all five summaries from a full recording through completion. Run the example, which checks its results with assertions:

```bash
cargo run --example operational_analysis
```

Expected output:

```text
Arrived: 6, unserved: 4, elapsed ticks: 2
```

The [analysis rustdoc](src/analysis.rs) defines units, interval accounting, undefined values, partial-history support for individual summaries, and errors.

### Browser Viewer

```bash
cargo run --example rail_viewer -- rail-viewer.html
```

Open the generated file directly in a modern browser; it works offline without a server. The optional output path defaults to `rail-viewer.html` and overwrites an existing file.

Use Play/Pause, Speed, Reset, and the Snapshot slider to inspect the recording. Seeking and resetting pause playback; 1× displays one tick per second. Playback never changes the simulation or analysis. The demo ends at tick 18 with 6 arrived and 5 unserved passengers.

The status panel follows the selected snapshot; the operational summary stays fixed. Partial recordings and completed recordings missing initial history remain viewable without analysis. Undefined summary values display `N/A`.

To change the demo, adapt [`scenario()` and `render()`](examples/rail_viewer/mod.rs). Rendering and serialization stay in the example.

## Baseline RL examples

```bash
cargo run --example random_policy
cargo run --example tabular_q_learning
```

These fixed-seed baselines use one decision per episode. See [training consumers](ARCHITECTURE.md#training-consumers) for their scope.

## Development

See [AGENTS.md](AGENTS.md) for validation commands and [Architecture](ARCHITECTURE.md#module-map) for module responsibilities. Browse API documentation with:

```bash
cargo doc --no-deps --open
```

Tests use small [Rail helpers](tests/support/mod.rs) and focused [analysis modules](tests/operational_analysis.rs). Keep edge-order and invalid-construction fixtures local. Use named synchronous `rstest` cases for input matrices and ordinary tests for stateful scenarios; prefer readability over wider parameterization. Run a subset with `cargo test --test operational_analysis validation::`.

The [viewer integration test](tests/rail_viewer.rs) runs under `cargo test --all`. CI also runs the browser assertions and runner failure checks. With Python 3 and Chrome installed, run them locally:

```bash
python3 tests/run_rail_viewer_playback.py
python3 tests/test_rail_viewer_playback_runner.py
```

The default browser is `google-chrome`; set `CHROME_BIN` to another Chrome or Chromium executable path if needed. For manual inspection, run `python3 -m http.server 8121 --bind 127.0.0.1` from the repository root and open [the playback test page](http://127.0.0.1:8121/tests/rail_viewer_playback.html).

## License

[MIT License](LICENSE). Copyright &copy; 2026 [Chris Ohk](https://github.com/utilForever) and [Jungwoo Kim](https://github.com/jungwoo9454).
