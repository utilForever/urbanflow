# AGENTS.md

`urbanflow` is a Rust library for urban transit simulation and reinforcement learning. Read the issue and [README](README.md) before editing; use [Architecture](ARCHITECTURE.md) for module responsibilities and simulation contracts.

## Working Rules

- Keep each step atomic: reject invalid actions and detect overflow before mutation. Preserve deterministic demand order and equal-hop path tie-breaking by edge insertion order.
- Put behavior in its owning module: `world` for topology and modes, `network` for graph queries, `simulation` for aggregate allocation and metrics, `rail` for Rail service state and movement, `time` for checked ticks, and `env` for episode orchestration and rewards.
- Before changing shared behavior or public types, use `rg` to trace every caller and change the lowest layer that owns the rule.
- Prefer existing code and the standard library. Add dependencies only when their maintenance and build cost are justified.
- Keep planned directions separate from implemented capabilities. Do not add speculative abstractions or claim support without code and tests.
- Keep documentation with the behavior it describes: summarize user-visible behavior in the README, put detailed API and error contracts in rustdoc, and record module or system changes in Architecture. Include required tests and documentation with public changes.

## Validation

Add or update the smallest focused test for behavior changes, then run:

```sh
cargo check --all
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

For documentation-only changes, review Markdown rendering and run `typos` when installed; no Rust build is required unless code or build configuration changed.

Treat `.github/workflows/` as the compatibility contract. When build settings, dependencies, lint policy, tests, or CI change, reproduce every affected workflow command locally.

## Commits and Protected Settings

- Keep commits focused, use a concise conventional prefix when it fits (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`, or `chore:`), and include associated tests and documentation in the same logical commit.
- Do not edit or commit generated `target/` files. Leave unrelated workflow versions and CI permissions alone.
- Do not change reward weights, capacities, construction costs, or deterministic ordering rules unless the task requires that behavior change.
- Do not implement speculative planned features unless asked.
