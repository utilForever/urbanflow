## What

<!-- One-line summary of what this PR does -->

## Why

<!-- Why is this change needed? Link to issue if applicable -->

## Checklist

### Required

- [ ] `cargo check --all-targets` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test --all` passes
- [ ] I linked the related issue (for example: `Closes #6`)

### Functional Validation

- [ ] Reinforcement learning environment behavior was verified for impacted transit flows (if applicable)
- [ ] Multimodal transit network changes (roads, rails, routes, or transfers) were reviewed and validated (if applicable)
- [ ] Traffic signal timing, phase transitions, or intersection control rules were validated (if applicable)
- [ ] Vehicle priority, routing, lane capacity, or congestion behavior was checked against expected traffic flow (if applicable)
- [ ] Passenger demand, boarding/alighting, transfer, or mode-choice behavior was validated (if applicable)
- [ ] Reward/cost model changes reflect the intended control objective, such as delay, throughput, safety, or emissions (if applicable)
- [ ] Safety or operational constraints for control actions were considered, including invalid, conflicting, or out-of-range actions (if applicable)
- [ ] Simulation changes remain deterministic or document any intentional nondeterminism (if applicable)
- [ ] Public API changes are covered by examples, tests, or documentation (if applicable)
- [ ] I added or updated tests for changed behavior (if applicable)

### Configuration & Documentation

- [ ] Runtime configuration changes are documented (if applicable)
- [ ] User/developer docs were updated (`README.md` or relevant docs, if applicable)
- [ ] No sensitive values or credentials were introduced

### If Applicable

- [ ] Security impact was considered
- [ ] Breaking behavior changes are clearly described in this PR
- [ ] Dependency or toolchain changes are documented
