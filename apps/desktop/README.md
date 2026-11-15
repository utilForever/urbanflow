# Urbanflow desktop

The application entry point for [#209](https://github.com/utilForever/urbanflow/issues/209), targeting `x86_64-pc-windows-msvc`. It opens one native window and loads the bundled seven-snapshot Rail result through the existing [result adapter](../../examples/results/mod.rs). Startup validates the result and obtains analysis through the core API; rendering reads owned data and never advances simulation.

This first package provides the application shell. Opening/running/saving scenarios belongs to [#210](https://github.com/utilForever/urbanflow/issues/210); network playback and analysis tables belong to [#211](https://github.com/utilForever/urbanflow/issues/211). There are no file dialogs or output writes yet. Close the window to exit the process.

## Dependency boundary

This directory is a separate Cargo workspace with its own `Cargo.lock` and `target/`. It depends on `urbanflow` by path; the core never depends on this package. The result adapter is compiled directly from its existing consumer module, using the same Serde dependencies as the examples. There is no new adapter API or serialization dependency in the core.

`gpui-kit = "=0.8.0"` is the pinned stack used by the [feasibility trial](../../prototypes/gpui/README.md), with default features disabled and `component` enabled. Kit selects the compatible GPUI/platform crates; the lockfile records all transitive versions and checksums. Its published source corresponds to Kit revision `1676826d9d49b90f4d7b6808b69924f58796f6f1` and GPUI snapshot `93d745668805029476e6d7fbb19652116fed51de`. Use `--locked` for builds and checks.

The application uses stable Rust and the Windows system font, Segoe UI. It does not inherit the prototype's nightly/WASM setup or web entry point. The prototype remains a separate experiment; this package does not establish a web or other-OS support decision. The [offline HTML viewer](../../README.md#browser-viewer) remains available.

## Windows setup and launch

Use Windows x64 with a working graphics driver, Rust stable 1.99 or newer, Visual Studio's **Desktop development with C++** workload (MSVC x64 tools and Windows SDK), and CMake on PATH. Run from a Visual Studio developer PowerShell. The Rust target alone does not install a linker or SDK.

From the repository root:

```powershell
rustup target add x86_64-pc-windows-msvc --toolchain stable
cd apps/desktop
$env:CARGO_BUILD_JOBS = '2'
rustc +stable -Vv
cmake --version
cargo +stable run --locked --target x86_64-pc-windows-msvc
```

The window is titled **Urbanflow** and initially measures 900 × 600 logical pixels. It displays **Bundled Rail result: 3 nodes, 7 snapshots**. It needs no server or runtime data files. The debug executable is `apps/desktop/target/x86_64-pc-windows-msvc/debug/urbanflow-desktop.exe` relative to the repository root. There is no installer or distribution packaging yet.

## Validation and CI scope

Run from `apps/desktop`. The Windows workflow limits compilation to two parallel jobs for this dependency graph:

```powershell
$env:CARGO_BUILD_JOBS = '2'
cargo +stable check --locked --all-targets --target x86_64-pc-windows-msvc
cargo +stable fmt --all -- --check
cargo +stable clippy --locked --all-targets --target x86_64-pc-windows-msvc -- -D warnings
cargo +stable test --locked --target x86_64-pc-windows-msvc
cargo +stable build --locked --target x86_64-pc-windows-msvc
```

The focused startup test validates the embedded fixture and checks that its analysis agrees with `urbanflow::analysis::analyze`. The [Windows workflow](../../.github/workflows/desktop.yml) runs these package checks with stable Rust and caches only this workspace's artifacts. CI compilation does not prove that a window painted: manually launch on Windows, inspect the initial text, resize/minimize/restore the window, and close it to confirm that the process exits.

Root commands in [AGENTS.md](../../AGENTS.md) retain their existing scope. `cargo check --all`, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all` from the repository root do not build GPUI. Existing root Rust, browser, unused-dependency, coverage, and audit workflows continue to check the core/examples; they do not cover this separate desktop lockfile. To audit desktop dependencies separately, run `cargo audit --file apps/desktop/Cargo.lock` from the root with cargo-audit installed. Prototype checks remain in `prototypes/gpui`.

## Local Windows validation

Validation host on 2026-10-25: Windows 11 Enterprise N, build 26300; NVIDIA GeForce RTX 2080 Ti, driver 32.0.16.1088; MSVC 14.51.36231; Windows SDK 10.0.26100.0; CMake 4.3.3; stable rustc 1.99.0 (`b940084d7`, 2026-09-28). No nightly toolchain is used by this package.

All five package checks above passed, including the startup test. The native window painted the expected three-node/seven-snapshot text; maximize/restore and minimize/restore preserved it. Closing the window with Alt+F4 removed the window and exited the process. Root check, format, Clippy, and all tests also passed; the file-replacement tests required execution outside the restricted sandbox. Cargo metadata confirmed separate workspace roots and target directories, and the core's normal dependency tree contained only `urbanflow`.

The desktop lockfile audit reported no vulnerabilities and eight unmaintained-crate warnings: `instant`, `paste`, `rustls-pemfile`, `rustybuzz`, `ttf-parser`, `zbus_names`, `zvariant`, and `zvariant_derive`. These are transitive dependencies of the pinned stack, including other-platform packages retained in Cargo's lockfile. They are not suppressed; reassess them when updating GPUI Kit.

Revalidated on 2026-11-09 after updating only `async-executor` from the yanked 1.14.0 to 1.14.1 for its [shutdown memory-safety fix](https://github.com/smol-rs/async-executor/issues/163): all five desktop checks and the four core checks passed. The updated app painted the expected fixture counts and exited when its window closed. The audit reported no vulnerabilities or yanked packages; the eight unmaintained-crate warnings above remain.

Other OS targets, screen readers, and distribution behavior are not certified by these package checks.
