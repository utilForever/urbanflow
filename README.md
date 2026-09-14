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

`urbanflow` provides the foundation for:

- Modeling caller-defined node sets and directed road and rail networks with capacity and passenger demand.
- Returning owned observations with node identifiers in caller-supplied world order.
- Advancing one Rail vehicle along an ordered fixed route with deterministic dwell and travel ticks.
- Tracking ordered demand batches as waiting, onboard, arrived, or unserved Rail passengers.
- Processing Rail stops with deterministic alighting, boarding priority, and vehicle capacity limits.
- Retaining owned movement snapshots with tick, vehicle position, occupancy, and passenger counts.
- Keeping simulation and environment logic in a reusable Rust library crate.
- Supporting future training, evaluation, and integration workflows around reinforcement learning agents.
- Measuring served and unserved demand, congestion, and network construction cost.

## Project Objectives

The current library is a compact road-and-rail simulation core. As it evolves, `urbanflow` aims to:

- Expand multimodal networks with transit modes such as trams and demand-responsive transit (DRT).
- Support both macro-level network analysis and micro-level movement and service analysis.
- Make simulation capabilities available to applications, websites, and other platforms through reusable interfaces.
- Run large-scale simulations with low latency for reinforcement learning and interactive analysis.

These are project directions, not claims of current feature support. See [Architecture](ARCHITECTURE.md) for the implemented system and planned boundaries, and [AGENTS.md](AGENTS.md) for repository development guidance.

## Quick Start

### Prerequisites

- Rust stable toolchain with edition 2024 support
- Git

### 1. Clone

```bash
git clone https://github.com/utilForever/urbanflow.git
cd urbanflow
```

### 2. Check the Library

```bash
cargo check --all
cargo test --all
```

## Usage

```rust
use urbanflow::action::Action;
use urbanflow::demand::Demand;
use urbanflow::env::Env;
use urbanflow::world::{EdgeKind, Network, Node, NodeId, World};

fn main() {
    let nodes = vec![
        Node { id: NodeId(10) },
        Node { id: NodeId(20) },
    ];
    let world = World {
        nodes,
        network: Network::new(),
    };
    let demands = vec![Demand::new(NodeId(10), NodeId(20), 15)];
    let mut env = Env::new(world, demands, 10.0, 10).expect("scenario is valid");

    let action = Action::AddEdge {
        from: NodeId(10),
        to: NodeId(20),
        kind: EdgeKind::Rail,
    };
    let first_episode = env.step(action).expect("step is valid");

    env.reset();
    let repeated_episode = env.step(action).expect("step is valid after reset");

    assert_eq!(repeated_episode, first_episode);
}
```

`Env::new` validates the initial topology, demand endpoints, and budget before constructing an environment. It preserves caller-supplied node, edge, and demand order so repeated episodes remain deterministic.

Calling `reset()` restores the world, demands, and budget passed to `Env::new`, clears the episode metrics and step counter, and returns the restored observation. It leaves `max_steps` unchanged.

Calling `available_actions()` returns an owned list of the valid affordable `AddEdge` actions in stored node `from`/`to` order and supported edge-kind order. It returns an empty list after the step limit is reached.

`SimulationClock` provides deterministic discrete time that starts at tick zero and advances one checked integer tick per call:

```rust
use urbanflow::time::SimulationClock;

let mut clock = SimulationClock::default();
assert_eq!(clock.tick(), 0);
assert_eq!(clock.advance(), Ok(1));
```

`RailVehicle::advance` uses this clock for vehicle movement. The clock is not yet integrated with `Env` or observations.

`RailRoute::new` preserves caller-supplied edge order and stores the corresponding stop nodes while rejecting empty routes, missing or non-Rail edges, and disconnected consecutive edges. `RailVehicle::new(route, capacity, travel_ticks_per_edge, dwell_ticks_per_stop)` takes ownership of a validated route, rejects zero capacity or durations, and starts the vehicle at the first stop. Both constructors return typed `RailInitError` values for invalid inputs.

`RailVehicle::route()` exposes the vehicle's route through a read-only reference. A traveling vehicle's `edge_index` selects `vehicle.route().edges()[edge_index]`. Stop zero is the first edge's origin; stop `i > 0` is the destination of route edge `i - 1`, including the final stop at `vehicle.route().edges().len()`.

`RailPassengers::new(&demands)` creates one lifecycle record per demand in supplied order, including duplicate and zero-amount demands. Every passenger starts waiting. The read-only `records()` slice exposes each original demand and its `waiting`, `onboard`, `arrived`, and `unserved` counts, whose sum always equals the requested amount.

```rust
use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailPosition, RailRoute, RailVehicle, RailVehicleState};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, NodeId};

let mut network = Network::new();
let edge = network.add_edge(NodeId(0), NodeId(2), EdgeKind::Rail).unwrap();
let route = RailRoute::new(&network, vec![edge]).unwrap();

let mut vehicle = RailVehicle::new(route, 6, 1, 1).unwrap();
let mut clock = SimulationClock::default();
let mut passengers = RailPassengers::new(&[Demand::new(NodeId(0), NodeId(2), 10)]);

let initial = vehicle.snapshot(&clock, &passengers).unwrap();

assert_eq!(vehicle.advance(&mut clock, &mut passengers).unwrap(),
    RailVehicleState::Traveling { edge_index: 0, travel_ticks_elapsed: 0 });

let traveling = vehicle.snapshot(&clock, &passengers).unwrap();

assert_eq!((traveling.tick, traveling.occupancy, traveling.capacity), (1, 6, 6));
assert_eq!(traveling.position, RailPosition::Traveling {
    edge_index: 0, edge, from: NodeId(0), to: NodeId(2),
    travel_ticks_elapsed: 0, travel_ticks_total: 1,
});

assert_eq!(vehicle.advance(&mut clock, &mut passengers).unwrap(), RailVehicleState::Complete);
assert_eq!(clock.tick(), 2);
assert_eq!((initial.tick, initial.occupancy, initial.passengers[0].waiting), (0, 0, 10));
assert_eq!(traveling.passengers[0].onboard, 6);

let record = passengers.records()[0];

assert_eq!((record.waiting, record.onboard, record.arrived, record.unserved), (0, 0, 6, 4));
```

`advance(&mut clock, &mut passengers)` returns the resulting `RailVehicleState` and advances time by one tick while service is active. The first tick processes the initial stop, then consumes one dwell tick. Consuming the last dwell tick starts travel with zero elapsed ticks; each subsequent tick advances edge progress. Reaching the configured travel duration processes the next stop immediately and starts its full dwell. At the final stop, it alights passengers, marks outstanding demand unserved, and enters `Complete` without another dwell. Further advances leave the vehicle, clock, and passengers unchanged.

Keep the same clock and passenger records for the service, and let `advance` handle time and stop processing. `RailStepError` distinguishes clock overflow from passenger errors; either leaves all three inputs unchanged. Time overflow is checked first. Movement uses fixed integer durations for one vehicle; reverse service, multiple vehicles, and traffic interactions remain future work.

`snapshot(&clock, &passengers)` reads that same service into an owned `RailSnapshot` without advancing time or processing stops. It contains the supplied tick, a `RailPosition`, occupancy, capacity, and a `Vec<PassengerState>` in original demand order, including duplicates and zero amounts. Later ticks or changes to the returned snapshot do not affect each other.

`RailPosition::AtStop` identifies the route stop index, actual `NodeId`, and remaining dwell ticks. `Traveling` identifies the route edge index, actual `EdgeId`, endpoint nodes, and integer elapsed and total travel ticks. Elapsed ticks start at zero and stay below the total; arrival changes the position to the next stop or `Complete`. `Complete` retains the final stop index and node, including on routes that revisit a node. Consumers own coordinates, animation timing, and serialization.

Snapshot occupancy sums every onboard record using the same checked arithmetic as stop processing. Explicit accounting transfers can exceed vehicle capacity; the snapshot reports that value without clamping. If the total exceeds `u32::MAX`, it returns `RailPassengerError::CountOverflow` without changing the service. Passenger lifecycle counts remain per demand and are separate from `Env` metrics and observations.

`process_stop(&vehicle, stop_index)` first alights all onboard passengers whose destination is the current stop, then boards waiting demand in stored order up to the remaining vehicle capacity. Only demand originating at the current stop with a destination appearing strictly later in the route may board. Repeated route nodes are evaluated relative to this visit; a same-origin/destination demand can board only when that node occurs again later. Excess or ineligible demand remains waiting. The final stop only alights passengers.

Stop processing uses checked count arithmetic and returns `RailPassengerError` without changing any record for an unknown stop, initial occupancy above capacity, or count overflow. Occupancy includes every onboard record, including passengers boarded through the explicit accounting API. `advance` calls this operation once per visit in route order, starting at zero. Manual callers must follow the same order; `process_stop` itself does not check or advance vehicle state.

`board` and `alight` remain explicit accounting operations by original demand index, rejecting unknown indices or insufficient source counts without mutation. For manual stop processing, call `complete()` after recording final-stop arrivals to mark every remaining waiting or onboard passenger unserved, including demand the route could not carry. `advance` handles completion automatically. Repeated completion leaves the records unchanged. This lifecycle is separate from `Env`'s existing aggregate demand metrics.

For the built-in four-node world, demand `0 -> 3` with amount `10`, and budget `100.0`, use the convenience constructor:

```rust
let mut env = urbanflow::env::Env::toy_city(100);
```

## Baseline RL examples

Run the deterministic random-policy and tabular Q-learning baselines with:

```bash
cargo run --example random_policy
cargo run --example tabular_q_learning
```

The programs print:

```text
Random policy total reward over 100 episodes: -530.00
Tabular Q-learning action after 100 episodes: AddEdge { from: NodeId(0), to: NodeId(1), kind: Rail }
```

Both examples use the same fixed seed and two-node scenario. The random policy averages `-5.30` reward per episode. Replaying the learned Rail action produces `12.25` reward per episode, so the learned policy outperforms that baseline. The results are reproducible because the scenario, seed, and action order are deterministic.

These examples demonstrate the public environment API, not a production RL stack. The scenario has one decision state and one step per episode, and the learner stores only tabular action values. It does not provide deep RL, Python bindings, visualization tooling, or general multi-step training.

## Simulation and rewards

Each edge kind has fixed capacity and construction cost:

| Edge kind | Capacity | Construction cost |
| --------- | -------: | ----------------: |
| Road      |       10 |               1.0 |
| Rail      |       20 |               2.0 |

At each tick, demands are processed in stored order, so earlier demands consume shared capacity first. Each reachable demand follows the first shortest directed path; equal-hop paths follow edge insertion order. The served amount is limited by both the demand amount and the smallest remaining capacity along that path. Unreachable and excess demand is unserved.

`congestion` is the maximum `edge load / edge capacity` across the network, or `0` when the network has no edges. `cost` is the sum of every edge's construction cost. The reward is:

```text
served demand - unserved demand - congestion - cost
```

For example, start with `toy_city()`, a budget of `100.0`, and demand `1 -> 2` with amount `15`. Adding a direct Road or Rail produces:

| Added edge | Served | Unserved | Congestion | Cost | Budget | Reward |
| ---------- | -----: | -------: | ---------: | ---: | -----: | -----: |
| Road       |     10 |        5 |        1.0 |  4.0 |   99.0 |    0.0 |
| Rail       |     15 |        0 |       0.75 |  5.0 |   98.0 |   9.25 |

Rail costs more, but its higher capacity serves all demand with lower congestion and a higher reward.

## Development

Run the same core checks used in CI before submitting changes:

```bash
cargo check --all
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Optional local parity with CI:

```bash
cargo install cargo-udeps
cargo +nightly udeps --all-targets

cargo install typos-cli
typos
```

## License

<img align="right" src="https://149753425.v2.pressablecdn.com/wp-content/uploads/2009/06/OSIApproved_100X125.png" alt="Open Source Initiative approved license logo">

This project is licensed under the [MIT License](LICENSE):

Copyright &copy; 2026 [Chris Ohk](https://github.com/utilForever) and [Jungwoo Kim](https://github.com/jungwoo9454).

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
