# GPUI Kit Rail playback trial

Issue [#208](https://github.com/utilForever/urbanflow/issues/208). This is an isolated feasibility consumer, not an application-stack commitment. The supported browser fallback remains the existing offline HTML/SVG viewer.

## Pinned inputs

- `gpui-kit = "=0.8.0"`, `component` enabled, default features disabled. Kit owns the compatible GPUI/platform dependencies; there is no direct GPUI dependency.
- Kit release source: [`1676826d9d49b90f4d7b6808b69924f58796f6f1`](https://github.com/longbridge/gpui-kit/tree/1676826d9d49b90f4d7b6808b69924f58796f6f1), also recorded in the published crate's `.cargo_vcs_info.json`.
- Kit pins `gpui-pre-* = 0.3.9`. The published GPUI manifest identifies Zed revision [`93d745668805029476e6d7fbb19652116fed51de`](https://github.com/zed-industries/zed/tree/93d745668805029476e6d7fbb19652116fed51de). Its [`gpui_web`](https://github.com/zed-industries/zed/blob/93d745668805029476e6d7fbb19652116fed51de/crates/gpui_web/Cargo.toml) and [`gpui_windows`](https://github.com/zed-industries/zed/blob/93d745668805029476e6d7fbb19652116fed51de/crates/gpui_windows/Cargo.toml) sources were checked. These are published snapshots, not an assertion that an unmodified Zed checkout is identical.
- Toolchain: `nightly-2026-10-21`, rustc `1.101.0-nightly (19833af72 2026-10-20)`. The web backend needs nightly; the toolchain file pins the date. `wasm-bindgen` crate and CLI both use `0.2.127`.
- All transitive versions and registry checksums are in this directory's `Cargo.lock`. Use `--locked`.
- Both targets embed the same existing [`rail_service.json`](../../tests/fixtures/rail_service.json), parsed and validated by the [result adapter](../../examples/results/mod.rs). No input file picker or new result schema is introduced.
- [Pretendard 1.3.9](https://github.com/orioncactus/pretendard/releases/tag/v1.3.9) Regular, Medium, SemiBold, and Bold are bundled with their SIL Open Font License in `assets/`. These are the unchanged `public/static/*.otf` files from the official `Pretendard-1.3.9.zip` release. The four weights used by this screen total 6,318,784 bytes (6.03 MiB); no font installation or external font request is needed.

## Behavior and parity

Play/Pause runs at one recorded tick per second. Previous, Next, Reset, and the individual tick buttons seek and pause. Playback stops at the final snapshot. To replay, Reset then Play. Space toggles playback; Left/Right seek; Home/End jump; Tab and Enter operate the Kit buttons. Only the cursor and elapsed wall time change: the consumer never constructs a simulation clock or calls a simulation step.

The vehicle marker moves at recorded ticks on a schematic line. Nodes retain stored order; this fixed fixture's two Rail edges join adjacent nodes. The Road edge is listed as context. This is not a general network layout or geographic map, and marker movement is not interpolated between snapshots. The summary shows selected values from all five analysis groups; the HTML viewer retains the full tables.

The layout follows [Fluent 2](https://fluent2.microsoft.design/): neutral layered surfaces, a blue brand accent, 4-pixel spacing increments, 4-pixel control corners, 8-pixel cards, and the 12/14/16/20/28/32-pixel type ramp. Play is the primary action; seeking and appearance use quieter controls. The route, playback controls, and current passenger counts take the main area; completed-run analysis stays separate. Below 1,000 logical pixels analysis stacks beneath playback. The desktop window initially opens at 1,280 × 820 logical pixels.

The header's **Dark mode** toggle changes the whole viewer immediately. Startup follows the platform's current appearance, including the browser's color-scheme preference. The toggle retains the selected snapshot and running/paused state, and works with Tab/Enter. Its native accessibility state exposes whether dark mode is on. The choice lasts for this window/session; restarting reads the system preference again. Later OS preference changes are not tracked automatically.

[`src/fluent.rs`](src/fluent.rs) maps Microsoft's [light/dark color aliases](https://github.com/microsoft/fluentui/tree/master/packages/tokens/src/alias) and [brandWeb ramp](https://github.com/microsoft/fluentui/blob/master/packages/tokens/src/global/brandColors.ts) onto the existing Kit theme. Layout, typography, and corner sizes follow the official [layout](https://fluent2.microsoft.design/layout), [typography](https://fluent2.microsoft.design/typography), [shapes](https://fluent2.microsoft.design/shapes), and [button](https://fluent2.microsoft.design/components/web/react/core/button/usage/) guidance. This is a GPUI Kit adaptation, not Microsoft's Fluent UI component implementation. All platforms use bundled Pretendard, including after light/dark switching. No UI dependency or external font request is added.

`cargo test --locked` in this directory checks both palettes for body-text contrast of at least 4.5:1, primary button states and selected-tick labels at least 4.5:1, and route/focus marks at least 3:1. These palette checks do not certify the canvas backend's accessibility.

| Observation                      | Expected value in both consumers     |
| -------------------------------- | ------------------------------------ |
| Nodes, in order                  | 8, 3, 21                             |
| Snapshots                        | 7, ticks 0 through 6                 |
| Tick 0                           | occupancy 0/2, waiting 6             |
| Tick 3                           | occupancy 2/2, waiting 2, arrived 2  |
| Tick 6                           | occupancy 0/2, arrived 4, unserved 2 |
| Mean waiting / onboard / journey | 2.00 / 2.50 / 4.50 ticks             |
| Maximum / mean occupancy         | 2 / 1.67                             |
| Elapsed / traveling / dwelling   | 6 / 4 / 2 ticks                      |

`cargo test --test gpui_playback` from the repository root checks the actual shared playback module against the existing viewer payload and hand-checked values. It covers pause/resume timing, seek bounds, completion, empty/single/partial recordings, exact `u64` tick labels, and unchanged serialized results after playback. Those broader model tests do not make the fixed UI a general result-file browser.

## Reproduce: Windows first

Use Windows 10 or later with MSVC, Visual Studio's **Desktop development with C++** workload, a Windows SDK, and CMake on PATH. Run in the developer shell. See Kit's [installation guide](https://gpui-kit.com/docs/installation/); a working GPU/driver is also needed to assess rendering.

From the repository root:

```powershell
cd prototypes/gpui
rustc -Vv
cargo -V
cmake --version
cargo check --locked --target x86_64-pc-windows-msvc
cargo build --locked --target x86_64-pc-windows-msvc
cargo run --locked --target x86_64-pc-windows-msvc
```

If your host is not Windows x64, add that Rust target explicitly. Installing a Rust target alone does not supply the Windows SDK or linker and does not run the application. Record OS, GPU/driver, SDK/MSVC versions, build duration, and the interaction checklist below on a real Windows host.

## Reproduce: macOS desktop

Kit documents macOS 15+ and Xcode Command Line Tools. On the evaluation host: Apple Silicon (`aarch64-apple-darwin`), Darwin 27.0.0, Xcode 26.6 (`17F113`). Kit enables the platform's `font-kit` and runtime shader features.

```sh
cd prototypes/gpui
cargo build --locked
cargo run --locked
```

## Reproduce: web

This follows Kit's [web entry pattern](https://github.com/longbridge/gpui-kit/blob/v0.8.0/crates/story-web/src/lib.rs): initialize the web platform and Kit, bundle the font, open a Root-wrapped window, and retain the `ApplicationHandle` from `run_embedded`. The single-threaded upstream backend avoids shared-memory/COOP/COEP requirements. It still requires a functioning browser graphics backend and a secure context such as localhost. A successful build or removal of the loading label is not evidence that the canvas painted.

From the prototype directory:

```sh
cargo install wasm-bindgen-cli --version 0.2.127 --locked
cargo build --locked --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/debug/urbanflow-gpui.wasm --target web --out-dir pkg --out-name urbanflow_gpui
python3 -m http.server 8122 --bind 127.0.0.1
```

Open <http://127.0.0.1:8122/>. `pkg/` is generated and ignored. Serve over HTTP; this is not the offline `file://` path of the HTML viewer. No Bun, Vite, web server framework, or hosted deployment is needed. For optimized artifact-size comparisons, rebuild with `--release` and use the `release` WASM path; debug sizes must not be presented as production bundle sizes.

## Interaction and accessibility checklist

On each tested platform, record actual results for:

1. Initial frame paints nodes, marker, controls, and the tick-zero counts.
2. Play reaches tick 6 and stops; Pause holds its frame; Reset pauses at zero.
3. Seek to ticks 2 and 3, then back to 0; compare positions and counts with the table above.
4. Analysis stays unchanged through every operation.
5. Toggle Dark mode while paused and while playing; verify the cursor, playback state, and analysis are preserved. Check the toggle's native on/off accessibility state.
6. Keyboard navigation has visible focus and Enter activates controls. Check Space, arrows, Home/End, and window resizing separately.
7. Measure startup and input response on the tested device. Do not infer performance under load from seven snapshots.
8. Inspect the accessibility tree and test a screen reader. Kit button semantics alone do not establish platform accessibility.

The selected web backend has no implementation of the platform accessibility adapter callbacks; the core defaults are no-ops. The rendered canvas therefore must not be treated as equivalent to semantic HTML controls/tables. Native assistive technology and reduced-motion preferences are not certified by the model tests. The prototype has no file dialogs, multiple windows, clipboard integration, or application packaging.

## HTML/SVG fallback

The existing `cargo run --example rail_viewer` produces the supported offline viewer (its normal, larger demo). To compare this exact seven-snapshot fixture, generate an HTML file from the existing template and tested fixture, from the repository root:

```sh
python3 - <<'PY'
from pathlib import Path
template = Path('examples/rail_viewer/viewer.html').read_text()
data = Path('tests/fixtures/rail_service.json').read_text()
for source, escaped in [('&', r'\u0026'), ('<', r'\u003c'), ('>', r'\u003e'), ('\u2028', r'\u2028'), ('\u2029', r'\u2029')]:
    data = data.replace(source, escaped)
Path('prototypes/gpui/fallback.html').write_text(template.replace('__TRACE_DATA__', data))
PY
```

Open `fallback.html` directly, offline. It is generated and ignored. This command deliberately uses the checked-in fixture; use the Rust result adapter to validate arbitrary external results.

Root validation remains the commands in [AGENTS.md](../../AGENTS.md), including the existing browser playback runner. Check the isolated UI separately with `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked` from this directory.
