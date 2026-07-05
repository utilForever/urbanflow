# UrbanFlow

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://github.com/utilForever/urbanflow/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/utilForever/urbanflow/actions/workflows/rust.yml)
[![Typos](https://github.com/utilForever/urbanflow/actions/workflows/typos.yml/badge.svg?branch=main)](https://github.com/utilForever/urbanflow/actions/workflows/typos.yml)
[![Quality Gate Status](https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow)
[![Lines of Code](https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=ncloc)](https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow)
[![Coverage](https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=coverage)](https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow)

[![Maintainability Rating](https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=sqale_rating)](https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow)
[![Reliability Rating](https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=reliability_rating)](https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow)
[![Security Rating](https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=security_rating)](https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow)
[![Bugs](https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=bugs)](https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow)
[![Vulnerabilities](https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=vulnerabilities)](https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow)
[![Technical Debt](https://sonarcloud.io/api/project_badges/measure?project=utilForever_urbanflow&metric=sqale_index)](https://sonarcloud.io/summary/new_code?id=utilForever_urbanflow)

UrbanFlow is a Rust library for a reinforcement learning environment for building and optimizing multimodal urban transit networks with roads, rails, traffic, and passenger demand.

## What This Library Does

`urbanflow` provides the foundation for:

- Modeling multimodal urban transit networks with roads, rails, traffic, and passenger demand.
- Keeping simulation and environment logic in a reusable Rust library crate.
- Supporting future training, evaluation, and integration workflows around reinforcement learning agents.
- Measuring network behavior through accessibility, congestion, throughput, and service quality metrics.

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
