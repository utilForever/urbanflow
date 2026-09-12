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

## License

[MIT License](LICENSE). Copyright &copy; 2026 [Chris Ohk](https://github.com/utilForever) and [Jungwoo Kim](https://github.com/jungwoo9454).
