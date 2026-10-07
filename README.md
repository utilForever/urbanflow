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
- Checked passenger outcome totals, served share, and waiting, onboard, and journey time metrics from completed Rail traces.
- Maximum and time-weighted mean vehicle occupancy and load factors over recorded active Rail service.
- Boarding, alighting, and remaining waiting passengers per ordered Rail stop visit.
- Recorded Rail travel, dwell, and total active duration with the absolute completion tick.
- Shared validation of tick continuity, passenger conservation, and vehicle occupancy before operational analysis.
- One `analysis::analyze` operation returning all five summaries from a full Rail trace.
- A self-contained HTML example for inspecting recorded Rail positions and completed-run operational analysis in a browser.

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

### Operational Analysis

Use [`analysis::analyze(&trace)`](src/analysis.rs) to derive all five summaries in one call, returning `Result<OperationalAnalysis, AnalysisError>`. It validates once, leaves the trace unchanged, and returns owned, deterministic results for the same trace. Record from stop zero with all passengers waiting and include every tick through completion; the initial tick may be nonzero.

This complete example records one Rail edge with capacity 6, one travel tick, one dwell tick, and 10 requested passengers:

```rust
use urbanflow::analysis::analyze;
use urbanflow::demand::Demand;
use urbanflow::rail::{RailPassengers, RailRoute, RailVehicle};
use urbanflow::time::SimulationClock;
use urbanflow::world::{EdgeKind, Network, NodeId};

fn main() {
    let mut network = Network::new();
    let edge = network.add_edge(NodeId(0), NodeId(2), EdgeKind::Rail).unwrap();
    let route = RailRoute::new(&network, vec![edge]).unwrap();
    let mut vehicle = RailVehicle::new(route, 6, 1, 1).unwrap();
    let mut clock = SimulationClock::default();
    let mut passengers = RailPassengers::new(&[Demand::new(NodeId(0), NodeId(2), 10)]);

    let trace = vehicle.record_trace(&mut clock, &mut passengers, 2).unwrap();
    assert!(trace.completed);

    let analysis = analyze(&trace).unwrap();
    let outcomes = analysis.passenger_outcomes;
    assert_eq!((outcomes.requested, outcomes.arrived, outcomes.unserved), (10, 6, 4));
    assert_eq!(outcomes.served_share, Some(0.6));

    let times = analysis.passenger_times;
    assert_eq!(times.waiting_passenger_ticks, 14); // Includes unserved waiting.
    assert_eq!(times.onboard_passenger_ticks, 6);
    assert_eq!(times.arrived_waiting_passenger_ticks, 6);
    assert_eq!(times.arrived_onboard_passenger_ticks, 6);
    assert_eq!(times.mean_waiting_ticks, Some(1.0));
    assert_eq!(times.mean_onboard_ticks, Some(1.0));
    assert_eq!(times.mean_journey_ticks, Some(2.0));

    let occupancy = analysis.vehicle_occupancy;
    assert_eq!(occupancy.capacity, 6);
    assert_eq!(occupancy.max_occupancy, 6);
    assert_eq!(occupancy.occupied_passenger_ticks, 6);
    assert_eq!(occupancy.mean_occupancy, Some(3.0));
    assert_eq!(occupancy.max_load_factor, Some(1.0));
    assert_eq!(occupancy.mean_load_factor, Some(0.5));

    assert_eq!(analysis.stops.len(), 2);
    assert_eq!((analysis.stops[0].boarded, analysis.stops[0].remaining_waiting), (6, 4));
    assert_eq!(analysis.stops[1].alighted, 6);

    let timing = analysis.route_timing;
    assert_eq!(timing.elapsed_ticks, 2);
    assert_eq!((timing.traveling_ticks, timing.dwelling_ticks), (1, 1));
    assert_eq!(timing.completion_tick, 2);
}
```

Time is measured in simulation ticks, with no wall-clock unit. One passenger waiting or onboard for one tick contributes one passenger-tick. Each interval `[snapshot.tick, next_snapshot.tick)` uses its starting state and counts; changes take effect where they first appear in the recording, and the terminal snapshot adds no interval. Here all 10 passengers wait during `[0, 1)`, then 6 ride and 4 wait during `[1, 2)`. This gives 14 waiting passenger-ticks, but only 6 belong to passengers who arrive. Their mean waiting time is therefore `6 / 6 = 1` tick, not `14 / 6`.

| Result                   | Meaning and units                                                                                                                                                                                                                                                                                                                                                |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `passenger_outcomes`     | `requested`, `arrived`, and `unserved` count passengers across all demand records, including duplicates. At completion, `requested == arrived + unserved`. `served_share` is `arrived / requested`, a fraction from 0 to 1.                                                                                                                                      |
| `passenger_times` totals | `waiting_passenger_ticks` and `onboard_passenger_ticks` include all passengers, even those unserved. Onboard time includes dwell. The two `arrived_*` totals include only passengers who arrive, attributing arrivals to earlier boardings within each demand.                                                                                                   |
| `passenger_times` means  | `mean_waiting_ticks` and `mean_onboard_ticks` divide their respective `arrived_*` totals by final `arrived`; `mean_journey_ticks` divides their sum by `arrived`. All are ticks per arrived passenger.                                                                                                                                                           |
| `vehicle_occupancy`      | `capacity` is the fixed vehicle capacity; `max_occupancy` is the highest occupancy over recorded active intervals. `occupied_passenger_ticks` sums occupancy times duration, including dwell. `mean_occupancy` divides that total by active ticks. Maximum and mean load factors divide the corresponding occupancy by capacity, yielding fractions from 0 to 1. |
| `stops`                  | One entry per visit in route order: `stop_index`, `node`, `boarded`, `alighted`, and `remaining_waiting`. Repeated nodes retain separate entries, including empty visits. Boarding and alighting count separately. Remaining waiting includes ineligible demand originating there; at the final stop it is counted before conversion to unserved.                |
| `route_timing`           | Every `AtStop` interval adds `dwelling_ticks`, including initial and any recorded final-stop dwell. `Traveling` intervals add `traveling_ticks`; their sum is `elapsed_ticks`. `RailVehicle` completes immediately on final arrival, so its traces have no final-stop dwell. Intervals starting at `Complete` are excluded. `completion_tick` is the final snapshot's absolute tick, not a duration. |

Counts and time totals use checked integers; means and ratios use `f64` and may round large values. Undefined results use `None`, never NaN or infinity:

- No requested passengers: `served_share` is `None`. Positive demand with no arrivals gives `Some(0.0)`.
- No arrived passengers: all three passenger-time means are `None`, even if unserved waiting contributes to totals.
- No active intervals: maximum occupancy and occupied passenger-ticks are zero; mean occupancy and both load factors are `None`. Capacity stays positive. An active service with no onboard passengers instead has zero occupancy measurements and `Some(0.0)` mean occupancy and load factors.

Individual `PassengerOutcomes`, `PassengerTimes`, `VehicleOccupancy`, `StopActivity`, and `RouteTiming` summaries remain available through `from_trace(&trace)`. All require a completed recording. Except for stop activity, they also accept recordings starting during service or containing only completion; they never reconstruct missing history. Passenger-time means still divide by final arrivals, including those before recording. A completion-only recording has zero time totals and durations, zero passenger-time means if anyone arrived, and undefined occupancy ratios. `analyze` and stop activity reject missing initial history with `AnalysisError::MissingInitialState`.

Shared validation rejects empty or incomplete traces, noncontiguous ticks, changing ordered demands, invalid passenger counts or lifecycle transitions, zero or changing capacity, and inconsistent occupancy. Completed positions must have no waiting or onboard passengers. Stop activity also checks visit order and passenger changes at stops. Invalid data and arithmetic overflow return a typed `AnalysisError` without mutation or partial results. `AnalysisError` implements `Display` and `std::error::Error` for propagation with `?`. See the [analysis API](src/analysis.rs) for detailed error contracts.

These summaries describe one fixed-route, single-vehicle Rail service, separately from `Env` allocation, construction costs, and rewards. Validation does not verify detailed movement timing, boarding eligibility, or agreement with an external route or network. Multiple-route or multiple-vehicle comparisons, statistical experiments, confidence intervals, optimization, hosted dashboards, and GIS analysis remain planned. The [browser viewer](#browser-viewer) displays core-produced summaries; playback speed and seeking do not affect them.

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

| Passenger state | Meaning during this service                              |
| --------------- | -------------------------------------------------------- |
| Waiting         | Has not boarded; includes demand the route cannot carry. |
| Onboard         | Is riding the vehicle and counts toward its capacity.    |
| Arrived         | Has alighted at the demand's destination.                |
| Unserved        | Was still waiting or onboard when service ended.         |

The demo starts with 11 waiting passengers. Six board at node 8 and arrive at node 21; the remaining two at node 8 and all three at node 3 become unserved at completion. Seek to tick 18 to see 6 arrived, 5 unserved, and zero waiting or onboard. A complete recording means the vehicle finished its route, not that every passenger arrived. A partial trace retains its last recorded counts without forcing waiting passengers to become unserved.

The Completed-run operational summary appears below the playback controls immediately on opening the file. Its tables show final passenger outcomes and served share, passenger time totals and means, vehicle occupancy and load factors, route timing, and boarding/alighting/remaining waiting per stop visit. These values come from Rust's `analysis::analyze` and remain fixed during playback, seeking, and reset. Repeated nodes retain separate rows in route order.

Time uses simulation ticks, passenger-time totals use passenger-ticks, and passenger-time means include only arrived passengers. Means display two decimal places and shares/load factors display percentages; undefined values display `N/A`, while integer totals and completion ticks retain their full precision. Partial and empty recordings show an unavailable-analysis message. Completed recordings that start during service or contain only completion remain inspectable, with playback available when there are multiple snapshots; the summary explains that the initial service state is missing. Other analysis errors are returned during HTML generation, before the output file is written.

This is one fixed-route Rail service with fixed travel and dwell durations and aggregated passenger counts, independent of `Env` rewards. Road edges provide network context only. Multiple vehicles, timetables, headways, transfers, Road traffic, congestion-driven movement, Tram/DRT vehicles, and geographic maps are not modeled by this demo.

To view a different scenario, adapt [`scenario()`](examples/rail_viewer/mod.rs) and pass its world and core-produced trace to `render()`, which returns `Result<String, AnalysisError>`. The operational summary requires a completed recording of the full service from its initial state. Layout and display stay in the example; the viewer makes no simulation decisions.

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

`cargo test --test rail_viewer fixed_route_service_reaches_the_viewer_end_to_end` checks one public-API Rail service from configuration through completion, operational analysis, and HTML generation. It asserts every recorded position and passenger count, trace termination, the exact `OperationalAnalysis` across all five summaries, the full embedded data against a fixed fixture, and playback control presence. The scenario serves four of six passengers over four travel and two dwell ticks, including boarding and alighting at the middle stop. It also checks that all analysis entry points and the viewer reject an inconsistent vehicle load with the same typed error, without changing the trace. It runs as part of `cargo test --all`.

For the viewer's browser interaction tests, run `python3 -m http.server 8121 --bind 127.0.0.1` from the repository root and open [the playback test page](http://127.0.0.1:8121/tests/rail_viewer_playback.html). It checks playback and status-panel synchronization, final analysis display against the Rust-verified fixture, and narrow-screen layout using the real viewer template, DOM controls, and SVG geometry with a controlled animation clock. This separate browser check requires no frontend dependencies and is not run by `cargo test`.

## License

[MIT License](LICENSE). Copyright &copy; 2026 [Chris Ohk](https://github.com/utilForever) and [Jungwoo Kim](https://github.com/jungwoo9454).
