# AGENTS.md

Guidance for AI coding agents working in this repository. Humans should start with [README.md](README.md); this file puts the project rules in the order an agent usually needs them.

## What This Repository Is

`urbanflow` is a Rust library for a reinforcement learning environment that builds and evaluates multimodal urban transit networks. The implemented system is currently a deterministic four-node toy city with road and rail edges, passenger demand allocation, network metrics, and a scalar reward.

Trams, demand-responsive transit (DRT), macro- and micro-level analysis, external application interfaces, and large-scale low-latency simulation are planned directions. Do not describe them as implemented without code and tests that prove it.

## Golden Rules

1. **Keep steps deterministic and atomic.** Reject invalid actions before mutating the environment. Preserve the documented demand order and path tie-breaking rules.
2. **Put behavior in the module that owns it.** Topology and mode properties belong in `world`; graph queries in `network`; demand allocation in `simulation`; episode orchestration and reward calculation in `env`.
3. **Check every caller before changing shared behavior.** Use `rg` to trace public types and shared functions, then fix the lowest layer that owns the rule.
4. **Keep current capabilities separate from planned direction.** Update [README.md](README.md) and [ARCHITECTURE.md](ARCHITECTURE.md) when a change moves a goal into implemented behavior.
5. **Prefer the standard library and existing code.** Add a dependency only when the task justifies its maintenance and build cost.
6. **Run the smallest relevant check, then the full required checks.** Documentation-only changes need review and spelling checks; behavior changes need focused tests plus the Rust checks below.

## Common Task Flow

1. Read the issue, [README.md](README.md), and [ARCHITECTURE.md](ARCHITECTURE.md).
2. Trace the affected path through `src/` and its callers with `rg`.
3. Add or update the smallest focused test for a behavior change.
4. Make the minimum change at the owning module boundary.
5. Update user and architecture documentation when public behavior or project scope changes.
6. Run the relevant validation commands.

## Repository Map

| Area                   | Paths                                                                     | Responsibility                                                         |
| ---------------------- | ------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| Environment API        | `src/env.rs`, `src/action.rs`, `src/observation.rs`, `src/step_result.rs` | Episode state, actions, validation, observations, rewards, and results |
| Transit model          | `src/world.rs`, `src/demand.rs`                                           | Nodes, directed edges, transit modes, capacities, costs, and demand    |
| Routing and simulation | `src/network.rs`, `src/simulation.rs`                                     | Directed shortest paths, capacity allocation, and metric calculation   |
| Metrics                | `src/metrics.rs`                                                          | Served demand, unserved demand, congestion, and cost snapshots         |
| Public crate surface   | `src/lib.rs`                                                              | Public modules and intentionally private implementation modules        |
| Integration tests      | `tests/`                                                                  | Public behavior and state-transition coverage                          |
| Automation             | `.github/workflows/`                                                      | Formatting, linting, tests, dependency checks, audit, and coverage     |
| Project documentation  | `README.md`, `ARCHITECTURE.md`, `AGENTS.md`                               | User goals, implemented architecture, and contributor guidance         |

## Change Impact Guide

| Change                         | Check                                                                                      |
| ------------------------------ | ------------------------------------------------------------------------------------------ |
| Transit mode or edge property  | `world`, `env::is_done`, action handling, simulation assumptions, tests, and documentation |
| Routing or reachability        | `network`, capacity allocation in `simulation`, and reachability tests                     |
| Demand allocation or metrics   | `simulation`, `metrics`, reward calculation, and environment integration tests             |
| Episode lifecycle or errors    | `env`, observation/result snapshots, and state-atomicity tests                             |
| Public type or module          | `lib`, all callers, integration tests, usage example, and architecture documentation       |
| Dependency or CI configuration | `Cargo.toml`, `Cargo.lock`, workflows, and the affected full checks                        |

## Build and Validation

Run the core checks used by CI:

```sh
cargo check --all
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

For documentation-only changes, review Markdown rendering and run `typos` when it is installed. A Rust build is not required unless code or build configuration changed.

## CI Expectations

Pull requests and pushes to `main` run the Rust checks above on Ubuntu with the stable toolchain. CI also checks unused dependencies with nightly `cargo-udeps`, audits dependencies, checks spelling with `typos`, and produces SonarCloud coverage when its token is available.

Treat `.github/workflows/` as the compatibility contract. Reproduce the affected workflow locally when changing build settings, dependencies, lint policy, test behavior, or CI configuration; a local-only result is not complete.

## Commit Conventions

- Keep commits focused on one logical change.
- Use concise conventional prefixes where they fit: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, or `chore:`.
- Include tests and documentation in the same logical commit as the behavior that requires them.
- Do not mix unrelated behavior, dependency, CI, and documentation changes.
- Never commit generated build output from `target/`.

## Leave Alone Unless Asked

- Generated build output under `target/`.
- Workflow versions and CI permissions when the task is unrelated to CI.
- Reward weights, mode capacities, construction costs, and deterministic ordering rules when the task does not explicitly change simulation behavior.
- Speculative abstractions for planned applications, websites, transit modes, or simulation scales.
