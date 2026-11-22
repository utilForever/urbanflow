#[path = "../../../examples/run_scenario/files.rs"]
mod files;
#[path = "../../../examples/run_scenario/output.rs"]
mod output;
#[allow(dead_code)]
#[path = "../../../examples/results/mod.rs"]
mod results;
#[allow(dead_code)]
#[path = "../../../examples/scenario/mod.rs"]
mod scenario;
mod workflow;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::component::{
    Disableable, Theme,
    button::{Button, ButtonVariants},
};
use gpui_kit::{
    App, AsyncApp, Bounds, Context, Entity, PathPromptOptions, Role, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, size,
};
use workflow::{OpenedScenario, Recording};

const DEMO: &str = include_str!("../../../tests/fixtures/rail_service.json");

struct Desktop {
    scenario: Option<Arc<OpenedScenario>>,
    recording: Option<Arc<Recording>>,
    preview: results::RailResult,
    activity: Activity,
    status: String,
}

enum Activity {
    Idle,
    Working,
    Saving,
}

enum Change {
    Opened(Arc<OpenedScenario>),
    Ran(Arc<Recording>),
    Saved(PathBuf),
    Cancelled,
}

impl Desktop {
    fn new() -> Result<Self, results::ResultError> {
        Ok(Self {
            scenario: None,
            recording: None,
            preview: results::parse("rail_service.json", DEMO)?,
            activity: Activity::Idle,
            status: String::new(),
        })
    }

    fn perform(
        &mut self,
        activity: Activity,
        status: &str,
        cx: &mut Context<Self>,
        operation: impl AsyncFnOnce(&mut AsyncApp) -> gpui_kit::Result<Change> + 'static,
    ) {
        if !matches!(self.activity, Activity::Idle) {
            return;
        }

        self.activity = activity;
        self.status = status.to_owned();

        cx.notify();
        cx.spawn(async move |view, cx| {
            let outcome = operation(cx).await;

            let _ = view.update(cx, |this, cx| {
                this.finish(outcome);
                cx.notify();
            });
        })
        .detach();
    }

    fn attach(self, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let view = cx.new(|_| self);
        let weak = view.downgrade();

        window.on_window_should_close(cx, move |_, cx| {
            weak.update(cx, |this, _| !matches!(this.activity, Activity::Saving))
                .unwrap_or(true)
        });

        // The first draw registers Open as the first enabled tab stop.
        window.defer(cx, Window::focus_next);
        view
    }

    fn finish(&mut self, outcome: gpui_kit::Result<Change>) {
        self.activity = Activity::Idle;
        self.status = match outcome {
            Ok(Change::Opened(scenario)) => {
                self.scenario = Some(scenario);
                String::new()
            }
            Ok(Change::Ran(recording)) => {
                self.recording = Some(recording);
                String::new()
            }
            Ok(Change::Saved(path)) => format!("Saved to {}", path.display()),
            Ok(Change::Cancelled) => String::new(),
            Err(error) => format!("Error: {error}"),
        };
    }

    fn open(&mut self, cx: &mut Context<Self>) {
        self.perform(Activity::Working, "Opening scenario…", cx, async |cx| {
            let paths = cx
                .update(|cx| {
                    cx.prompt_for_paths(PathPromptOptions {
                        files: true,
                        directories: false,
                        multiple: false,
                        prompt: Some("Open Rail scenario".into()),
                    })
                })
                .await??;
            let Some(path) = paths.and_then(|mut paths| paths.pop()) else {
                return Ok(Change::Cancelled);
            };
            let scenario = cx
                .background_spawn(async move { OpenedScenario::open(&path) })
                .await
                .map_err(io::Error::other)?;

            Ok(Change::Opened(Arc::new(scenario)))
        });
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let Some(scenario) = self.scenario.clone() else {
            return;
        };

        self.perform(
            Activity::Working,
            "Running Rail scenario…",
            cx,
            async move |cx| {
                let recording = cx
                    .background_spawn(async move { scenario.run() })
                    .await
                    .map_err(io::Error::other)?;
                Ok(Change::Ran(Arc::new(recording)))
            },
        );
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let (Some(selected), Some(recording)) = (&self.scenario, self.recording.clone()) else {
            return;
        };
        let selected = selected.source.clone();

        self.perform(Activity::Saving, "Saving result…", cx, async move |cx| {
            let directory = recording.source.path.parent().unwrap_or(Path::new("."));
            let path = cx
                .update(|cx| cx.prompt_for_new_path(directory, Some("result.json")))
                .await??;
            let Some(path) = path else {
                return Ok(Change::Cancelled);
            };

            cx.background_spawn(async move {
                recording.save(&path, &selected).map_err(io::Error::other)?;
                Ok(Change::Saved(path))
            })
            .await
        });
    }

    fn displayed_result(&self) -> (&results::RailResult, String) {
        let Some(recording) = &self.recording else {
            return (&self.preview, "Bundled Rail result".to_owned());
        };
        (
            &recording.data,
            format!("Result from {}", recording.source.path.display()),
        )
    }
}

impl Render for Desktop {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::global(cx).colors;
        let busy = !matches!(self.activity, Activity::Idle);
        let (result, source) = self.displayed_result();
        let commands = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                Button::new("open")
                    .label("Open scenario…")
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, _, cx| this.open(cx))),
            )
            .child(
                Button::new("run")
                    .label("Run")
                    .primary()
                    .disabled(busy || self.scenario.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.run(cx))),
            )
            .child(
                Button::new("save")
                    .label("Save result as…")
                    .disabled(busy || self.recording.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            );
        let selection = self.scenario.as_ref().map_or_else(
            || "Open a Rail scenario JSON file to run it.".to_owned(),
            |scenario| {
                format!(
                    "Scenario: {} · Tick limit: {}",
                    scenario.source.path.display(),
                    scenario.rail.max_ticks
                )
            },
        );

        div()
            .id("desktop")
            .size_full()
            .overflow_y_scroll()
            .p_8()
            .flex()
            .flex_col()
            .gap_4()
            .bg(colors.background)
            .text_color(colors.foreground)
            .font_family("Segoe UI")
            .child(div().text_2xl().child("Urbanflow"))
            .child(commands)
            .child(selection)
            .child(source)
            .child(format!(
                "{} at tick {} · {} nodes · {} snapshots",
                if result.trace.completed {
                    "Completed"
                } else {
                    "Partial"
                },
                result
                    .trace
                    .snapshots
                    .last()
                    .expect("recordings include an initial snapshot")
                    .tick,
                result.world.nodes.len(),
                result.trace.snapshots.len(),
            ))
            .when(self.recording.is_some(), |view| {
                view.child("Saving replaces an existing result file.")
            })
            .when(!self.status.is_empty(), |view| {
                view.child(
                    div()
                        .id("status")
                        .role(Role::Status)
                        .child(self.status.clone()),
                )
            })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let desktop = Desktop::new()?;

    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(900.), px(600.)), cx);

        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui_kit::TitlebarOptions {
                    title: Some("Urbanflow".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            cx,
            |window, cx| desktop.attach(window, cx),
        )
        .expect("open Urbanflow desktop window");
        cx.activate(true);
    });

    Ok(())
}

#[cfg(test)]
mod tests;
