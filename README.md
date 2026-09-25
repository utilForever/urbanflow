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

`urbanflow` is a deterministic Rust library for building and evaluating urban transit networks. It currently supports:

- Caller-defined directed Road and Rail networks with capacity, passenger demand, metrics, and rewards.
- Repeatable reinforcement learning episodes with valid actions and owned observations.
- One fixed-route Rail vehicle with timed movement, passenger boarding and alighting, and owned snapshots and bounded traces.
- A self-contained HTML example for inspecting recorded Rail positions in a browser.

Trams, demand-responsive transit (DRT), broader analysis tools, application integrations, and large-scale simulation are planned. See [Architecture](ARCHITECTURE.md) for current capabilities and future direction.

## Quick Start

Requires Rust stable with edition 2024 support and Git.

```bash
git clone https://github.com/utilForever/urbanflow.git
cd urbanflow
cargo test --all
```

## Usage

### Environment

Use `Env::new` for a caller-defined scenario or `Env::toy_city` for the built-in four-node world. `available_actions()` lists valid affordable actions; `step()` returns an observation, metrics, reward, and completion flag. `reset()` restores the initial scenario.

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

Metrics track served and unserved demand, congestion, and construction cost. Reward is `served demand - unserved demand - congestion - cost`. See the [simulation contracts](ARCHITECTURE.md#simulation-contracts) for allocation and ordering rules.

### Rail Trace

Rail movement runs separately from `Env`. Keep the same vehicle, clock, and passenger records together throughout the service.

```rust
use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailRoute, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, NodeId};

let mut network = Network::new();
let edge = network.add_edge(NodeId(0), NodeId(2), EdgeKind::Rail).unwrap();
let route = RailRoute::new(&network, vec![edge]).unwrap();

// Capacity 6, one travel tick per edge, one dwell tick per stop.
let mut vehicle = RailVehicle::new(route, 6, 1, 1).unwrap();
let mut clock = SimulationClock::default();
let mut passengers = RailPassengers::new(&[Demand::new(NodeId(0), NodeId(2), 10)]);

let trace = vehicle.record_trace(&mut clock, &mut passengers, 2).unwrap();

assert!(trace.completed);
assert_eq!(trace.snapshots.len(), 3); // Initial state, travel, completion.
assert_eq!(trace.snapshots[2].passengers[0].arrived, 6);
```

A trace records the initial snapshot and each subsequent tick, up to the supplied number of advances. Check `completed` to distinguish a full service replay from a run stopped by the limit. For individual ticks, use `advance()` and `snapshot()`. Timing, passenger, and error contracts are documented in the [Rail API](src/rail.rs).

### Browser Viewer

From the repository root, generate the fixed-route demo:

```bash
cargo run --example rail_viewer -- rail-viewer.html
```

Open `rail-viewer.html` directly in a modern browser. The file embeds the demo network, recorded trace, CSS, and JavaScript, so it works offline without a server or frontend installation. The optional output path defaults to `rail-viewer.html`; an existing file at that path is overwritten.

Double-click the generated file, or open it from the same terminal with `open rail-viewer.html` on macOS, `xdg-open rail-viewer.html` on a Linux desktop, or `Start-Process .\rail-viewer.html` in Windows PowerShell.

The demo runs one vehicle along `8 → 3 → 21 → 5`, with capacity 6, four travel ticks per edge, and two dwell ticks at each stop before departure. It records ticks 0 through 18, including the initial state before boarding and the final arrival without a final dwell. Running the same scenario again produces the same trace and HTML.

The SVG places nodes clockwise in stored order and distinguishes directed Road and Rail edges. Use Play and Pause to watch the recorded service, Reset to return to the first snapshot and pause, and Speed to select 0.25×, 0.5×, 1×, 2×, or 4×. Playback starts paused; 1× displays one recorded tick per second and is a viewing pace, not a simulation time unit. Changing speed preserves playback progress.

Use the Snapshot slider (or arrow keys while focused) to select a recorded tick and pause playback. During playback, the marker moves smoothly along the Rail edge while the tick label and slider identify the latest reached snapshot. Playback stops at the last recorded snapshot, including for partial recordings; use Reset to watch again. The badge identifies complete versus partial recordings. Interpolation changes only the displayed marker, never recorded simulation state.

The Service status panel shows the selected snapshot's exact tick, vehicle state, stop index and node or directed edge, and load/capacity. Passenger totals sum waiting, onboard, arrived, and unserved counts across all demand records. These values update at recorded tick boundaries and follow playback, seeking, and reset; they are never interpolated. Empty recordings show unavailable values as dashes. On narrow screens, the panel appears below the network.

To view a different scenario, adapt [`scenario()`](examples/rail_viewer/mod.rs) and pass its world and core-produced trace to `render()`. Layout and display stay in the example; the viewer makes no simulation decisions.

## Baseline RL examples

```bash
cargo run --example random_policy
cargo run --example tabular_q_learning
```

These examples use a fixed seed and one decision per episode to demonstrate the public environment API. See [training consumers](ARCHITECTURE.md#training-consumers) for their scope.

## Development

Read [AGENTS.md](AGENTS.md) for repository rules and validation commands, and [Architecture](ARCHITECTURE.md#module-map) for module responsibilities. Generate and browse API documentation with:

```bash
cargo doc --no-deps --open
```

`cargo test --test rail_viewer fixed_route_service_reaches_the_viewer_end_to_end` checks one public-API Rail service from configuration through completion and HTML generation. It asserts every recorded position and passenger count, trace termination, the full embedded data against a fixed fixture, and playback control presence. It also runs as part of `cargo test --all`.

For the viewer's browser interaction tests, run `python3 -m http.server 8121 --bind 127.0.0.1` from the repository root and open [the playback test page](http://127.0.0.1:8121/tests/rail_viewer_playback.html). It checks playback and status-panel synchronization using the real viewer template, DOM controls, and SVG geometry with a controlled animation clock. This separate browser check requires no frontend dependencies and is not run by `cargo test`.

## License

[MIT License](LICENSE). Copyright &copy; 2026 [Chris Ohk](https://github.com/utilForever) and [Jungwoo Kim](https://github.com/jungwoo9454).
