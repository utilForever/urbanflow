# Urbanflow desktop

The Windows app opens a Rail scenario, runs it up to its tick limit, and saves version 1 result JSON. The target is `x86_64-pc-windows-msvc`.

## Run and save a scenario

1. **Open scenario…** has keyboard focus when the window opens. Press **Enter** or click the button, then select [rail-demo.json](../../tests/fixtures/scenarios/rail-demo.json). The window shows the validated file and its tick limit. Opening a file keeps the last result on screen with its source path. You can save it until a new run succeeds.
2. Choose **Run**. The demo shows **Completed at tick 18** and **19 snapshots**. Each run starts from the inputs captured when the file was opened. You can edit the file in an external editor and reopen it to pick up the changes.
3. Choose **Save result as…** and select a JSON filename. The app saves the displayed result, including its core analysis, with the same bytes as the [scenario CLI](../../README.md#run-a-scenario-file). A new file inherits the chosen folder's permissions. Replacing a file preserves its owner and access permissions, including write-only access. If the app cannot copy those permissions, it reports an error and leaves the file intact. The app refuses to replace a Windows EFS-encrypted file. To save an encrypted copy, choose a new name in an EFS-encrypted folder.

Save protects both the selected scenario and the recording's source. For each input, the app checks the selected name, the path it resolved to at open time, and the files both paths currently reach. Their names stay reserved if either file moves or a symbolic link loses its target. The check follows the directory's case and Unicode naming rules and protects hard links. Saving through a symbolic link updates its target. If the target is missing or the link changes targets during Save, the app reports the chosen path and preserves the link. You can still export the recording elsewhere if the input folder disappears or an input link becomes circular.

A folder that permits file creation and renaming can accept a new result. If an input name in that folder no longer resolves, the app may also need permission to create a temporary subdirectory to check for equivalent spellings. The window stays open while a save is active.

A zero tick limit saves just the initial snapshot. A short run saves all recorded snapshots with `completed: false` and `analysis: null`. Env files require an action policy and are rejected. The window displays validation, execution, encoding, and file errors. A failed operation or cancelled picker keeps the last successful run available to save. Each snapshot is encoded as it is recorded. The run stops once the output exceeds 16 MiB, even if many ticks remain.

## Dependency boundary

This directory is a separate Cargo workspace with its own `Cargo.lock` and `target/`. It depends on `urbanflow` by path. The app compiles the same scenario loader, result encoder, and file replacement code as the CLI. GPUI schedules loading, recording, encoding, and saving on its background executor. Only one command runs at a time.

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

The **Urbanflow** window initially measures 900 × 600 logical pixels. It displays a bundled Rail preview with 3 nodes and 7 snapshots. **Run** becomes available after opening a scenario and **Save result as…** after a successful run. The debug executable is `apps/desktop/target/x86_64-pc-windows-msvc/debug/urbanflow-desktop.exe` relative to the repository root.

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

The [desktop tests](src/tests.rs) cover the startup preview, repeated runs, exclusive Windows edits, renamed inputs, dangling links, missing input folders, and permission-preserving replacement of a write-only result. The GPUI test dispatcher delivers a foreground event during a bounded run. The [Windows workflow](../../.github/workflows/desktop.yml) runs the desktop package checks with stable Rust.

To check native interaction, open the demo, run it, and save it through the two file pickers. Repeat with a copy that sets `max_ticks` to `"3"` and check for **Partial at tick 3** and four snapshots. Try opening an invalid file and saving the still-displayed result under another name. For a longer bounded run, set `vehicle.travel_ticks_per_edge` to `"30000"` and `max_ticks` to `"25000"` in the copy, reopen it, and resize the window during execution. The window should repaint and then report **Partial at tick 25000**.

Root commands in [AGENTS.md](../../AGENTS.md) retain their existing scope. `cargo check --all`, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all` from the repository root do not build GPUI. Existing root Rust, browser, unused-dependency, coverage, and audit workflows continue to check the core/examples; they do not cover this separate desktop lockfile. To audit desktop dependencies separately, run `cargo audit --file apps/desktop/Cargo.lock` from the root with cargo-audit installed. Prototype checks remain in `prototypes/gpui`.

## Windows validation of the original shell (#209)

Validation host on 2026-10-25: Windows 11 Enterprise N, build 26300; NVIDIA GeForce RTX 2080 Ti, driver 32.0.16.1088; MSVC 14.51.36231; Windows SDK 10.0.26100.0; CMake 4.3.3; stable rustc 1.99.0 (`b940084d7`, 2026-09-28). No nightly toolchain is used by this package.

All five package checks above passed, including the startup test. The native window painted the expected three-node/seven-snapshot text; maximize/restore and minimize/restore preserved it. Closing the window with Alt+F4 removed the window and exited the process. Root check, format, Clippy, and all tests also passed; the file-replacement tests required execution outside the restricted sandbox. Cargo metadata confirmed separate workspace roots and target directories, and the core's normal dependency tree contained only `urbanflow`.

The desktop lockfile audit reported no vulnerabilities and eight unmaintained-crate warnings: `instant`, `paste`, `rustls-pemfile`, `rustybuzz`, `ttf-parser`, `zbus_names`, `zvariant`, and `zvariant_derive`. These are transitive dependencies of the pinned stack, including other-platform packages retained in Cargo's lockfile. They are not suppressed; reassess them when updating GPUI Kit.

Revalidated on 2026-11-09 after updating only `async-executor` from the yanked 1.14.0 to 1.14.1 for its [shutdown memory-safety fix](https://github.com/smol-rs/async-executor/issues/163): all five desktop checks and the four core checks passed. The updated app painted the expected fixture counts and exited when its window closed. The audit reported no vulnerabilities or yanked packages; the eight unmaintained-crate warnings above remain.

Other OS targets, screen readers, and distribution behavior are not certified by these package checks.
