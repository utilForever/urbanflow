#![cfg_attr(target_family = "wasm", no_main)]

mod fluent;
mod playback;
#[allow(dead_code)]
#[path = "../../../examples/results/mod.rs"]
mod results;

use gpui_kit::component::{
    Selectable, Theme, ThemeColor, ThemeMode,
    button::{Button, ButtonCustomVariant, ButtonVariant, ButtonVariants},
};
use gpui_kit::{
    App, Bounds, Context, Div, ElementId, FocusHandle, FontWeight, KeyBinding, Role, SharedString,
    Task, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, relative, size,
};
use playback::{Playback, RECORDING};
use std::time::Duration;
use web_time::Instant;

actions!(
    rail_playback,
    [Toggle, Previous, Next, Reset, Last, Tab, TabBack]
);

#[derive(Clone, Copy)]
enum Command {
    Toggle,
    Seek(usize),
}

struct Viewer {
    playback: Playback,
    focus: FocusHandle,
    last_update: Instant,
    _timer: Task<()>,
}

impl Viewer {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();

        window.focus(&focus, cx);

        let timer = cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;

                if view
                    .update(cx, |this, cx| {
                        let before = (this.playback.cursor(), this.playback.playing());

                        this.sync();

                        if before != (this.playback.cursor(), this.playback.playing()) {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });

        Self {
            playback: Playback::new(
                results::parse("rail_service.json", RECORDING).expect("validated fixture"),
            ),
            focus,
            last_update: Instant::now(),
            _timer: timer,
        }
    }

    fn sync(&mut self) {
        let now = Instant::now();

        self.playback.elapse(now.duration_since(self.last_update));
        self.last_update = now;
    }

    fn command(&mut self, command: Command, cx: &mut Context<Self>) {
        self.sync();

        match command {
            Command::Toggle => self.playback.toggle(),
            Command::Seek(index) => self.playback.seek(index),
        }

        cx.notify();
    }

    fn button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        command: Command,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .ghost()
            .label(label)
            .on_click(cx.listener(move |this, _, _, cx| this.command(command, cx)))
    }
}

fn panel(colors: ThemeColor) -> Div {
    div()
        .bg(colors.popover)
        .border_1()
        .border_color(colors.border)
        .rounded(px(8.))
}

fn caption(text: impl Into<SharedString>, colors: ThemeColor) -> Div {
    div()
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(colors.muted_foreground)
        .child(text.into())
}

fn metric(label: &'static str, value: String, colors: ThemeColor) -> Div {
    panel(colors)
        .flex_1()
        .min_w(px(110.))
        .p_4()
        .child(caption(label, colors))
        .child(
            div()
                .mt_1()
                .text_size(px(28.))
                .line_height(px(36.))
                .font_weight(FontWeight::MEDIUM)
                .child(value),
        )
}

impl Viewer {
    fn route_diagram(&self, colors: ThemeColor) -> Div {
        let playback = &self.playback;
        let mut diagram = div()
            .relative()
            .h(px(212.))
            .flex_shrink_0()
            .mx(px(52.))
            .child(
                div()
                    .absolute()
                    .top(px(114.))
                    .w_full()
                    .h(px(4.))
                    .bg(colors.chart_grid),
            );

        if let Some(x) = playback.vehicle_x() {
            let label = playback.snapshot().map_or_else(
                || "Rail".into(),
                |frame| format!("Rail · {}/{}", frame.occupancy, frame.capacity),
            );

            diagram = diagram
                .child(
                    div()
                        .absolute()
                        .top(px(114.))
                        .w(relative(x))
                        .h(px(4.))
                        .bg(colors.primary),
                )
                .child(
                    div()
                        .absolute()
                        .left(relative(x))
                        .top(px(68.))
                        .w(px(1.))
                        .h(px(40.))
                        .bg(colors.button_primary),
                )
                .child(
                    div()
                        .absolute()
                        .left(relative(x))
                        .top(px(36.))
                        .ml(px(-44.))
                        .w(px(88.))
                        .py_2()
                        .rounded(px(8.))
                        .bg(colors.button_primary)
                        .text_color(colors.button_primary_foreground)
                        .text_center()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(label),
                );
        }

        diagram.children(
            playback
                .result()
                .world
                .nodes
                .iter()
                .enumerate()
                .map(|(index, node)| {
                    div()
                        .absolute()
                        .left(relative(playback.node_x(node.id)))
                        .top(px(99.))
                        .w(px(80.))
                        .ml(px(-40.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .size(px(34.))
                                .rounded_full()
                                .border_3()
                                .border_color(colors.primary)
                                .bg(colors.secondary)
                                .text_color(colors.foreground)
                                .flex()
                                .items_center()
                                .justify_center()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(node.id.0.to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(14.))
                                .line_height(px(20.))
                                .text_color(colors.foreground)
                                .child(format!("Node {}", node.id.0)),
                        )
                        .child(
                            caption(format!("Stop {index}"), colors)
                                .text_color(colors.muted_foreground),
                        )
                }),
        )
    }

    fn transport(&self, cx: &mut Context<Self>) -> Div {
        let colors = Theme::global(cx).colors;
        let playback = &self.playback;

        div()
            .p_4()
            .border_t_1()
            .border_color(colors.border)
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        self.button(
                            "play",
                            if playback.playing() { "Pause" } else { "Play" },
                            Command::Toggle,
                            cx,
                        )
                        .primary()
                        .w(px(80.)),
                    )
                    .child(self.button(
                        "previous",
                        "Previous",
                        Command::Seek(playback.cursor().saturating_sub(1)),
                        cx,
                    ))
                    .child(self.button(
                        "next",
                        "Next",
                        Command::Seek(playback.cursor().saturating_add(1)),
                        cx,
                    ))
                    .child(self.button("reset", "Reset", Command::Seek(0), cx))
                    .child(div().flex_1())
                    .child(caption("1 tick / second", colors)),
            )
            .child(
                div().flex().flex_wrap().gap_2().children(
                    playback
                        .result()
                        .trace
                        .snapshots
                        .iter()
                        .enumerate()
                        .map(|(index, frame)| {
                            self.button(
                                ("snapshot", index),
                                format!("Tick {}", frame.tick),
                                Command::Seek(index),
                                cx,
                            )
                            .selected(index == playback.cursor())
                            .outline()
                            .when(
                                index == playback.cursor(),
                                |button| {
                                    button.with_variant(ButtonVariant::Custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(colors.accent)
                                            .foreground(colors.accent_foreground)
                                            .hover(colors.accent)
                                            .active(colors.accent),
                                    ))
                                },
                            )
                        }),
                ),
            )
    }

    fn analysis(&self, colors: ThemeColor) -> Div {
        panel(colors)
            .p_5()
            .flex()
            .flex_col()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(16.))
                    .line_height(px(22.))
                    .child("Completed run"),
            )
            .child(caption("Final results · fixed during playback", colors))
            .children(
                self.playback
                    .summary()
                    .into_iter()
                    .enumerate()
                    .map(|(index, row)| {
                        let title = match index {
                            0 => "Passenger outcomes",
                            1 => "Mean passenger time",
                            2 => "Vehicle occupancy",
                            3 => "Route timing",
                            _ => "Stop activity",
                        };
                        div()
                            .id(("analysis", index))
                            .role(Role::Group)
                            .aria_label(row.clone())
                            .pt_3()
                            .pb_3()
                            .when((1..=4).contains(&index), |row| {
                                row.border_t_1().border_color(colors.border)
                            })
                            .when(index <= 4, |row| row.child(caption(title, colors)))
                            .when(index > 4, |row| row.pt_1().pb_1())
                            .child(
                                div()
                                    .mt_1()
                                    .text_size(px(14.))
                                    .line_height(px(20.))
                                    .child(row.replace(" | ", " · ")),
                            )
                    }),
            )
    }
}

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::global(cx).colors;
        let font = Theme::global(cx).font_family.clone();
        let dark = Theme::global(cx).is_dark();
        let playback = &self.playback;
        let count = playback.result().trace.snapshots.len();
        let wide = window.viewport_size().width >= px(1000.);
        let frame = playback.snapshot();
        let tick = frame.map_or_else(|| "—".into(), |frame| frame.tick.to_string());
        let [waiting, _, arrived, unserved] = playback.passenger_totals();
        let occupancy = frame.map_or_else(
            || "—".into(),
            |frame| format!("{} / {}", frame.occupancy, frame.capacity),
        );
        let route = panel(colors)
            .child(
                div()
                    .m_2()
                    .rounded(px(4.))
                    .bg(colors.secondary)
                    .child(
                        div()
                            .px_5()
                            .pt_5()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_3()
                            .child(div().size(px(8.)).rounded_full().bg(colors.primary))
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(colors.foreground)
                                    .child("Route preview"),
                            )
                            .child(
                                caption(
                                    format!(
                                        "Snapshot {} / {} · Tick {tick}",
                                        playback.cursor() + 1,
                                        count
                                    ),
                                    colors,
                                )
                                .text_color(colors.muted_foreground),
                            ),
                    )
                    .child(self.route_diagram(colors))
                    .child(
                        div().px_5().pb_5().child(
                            caption(
                                "Rail edges 1, 2 · Road edge 0: 8 → 21 (context only)",
                                colors,
                            )
                            .text_color(colors.muted_foreground),
                        ),
                    ),
            )
            .child(self.transport(cx));
        let content = div()
            .w_full()
            .max_w(px(1400.))
            .mx_auto()
            .p_6()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .child(
                                div()
                                    .text_size(px(32.))
                                    .line_height(px(40.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("8 → 3 → 21"),
                            )
                            .child(caption(
                                "Rail service · 3 stops · 7 recorded snapshots",
                                colors,
                            )),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .px_3()
                            .py_1()
                            .rounded_full()
                            .bg(colors.accent)
                            .text_color(colors.accent_foreground)
                            .child(if playback.playing() {
                                "Playing"
                            } else {
                                "Paused"
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_5()
                    .when(!wide, |row| row.flex_col())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(route)
                            .child(
                                div()
                                    .id("snapshot-stats")
                                    .role(Role::Group)
                                    .aria_label(playback.status())
                                    .flex()
                                    .flex_wrap()
                                    .gap_3()
                                    .child(metric("Onboard / capacity", occupancy, colors))
                                    .child(metric("Waiting", waiting.to_string(), colors))
                                    .child(metric("Arrived", arrived.to_string(), colors))
                                    .child(metric("Unserved", unserved.to_string(), colors)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_3()
                                    .child(caption("Space  Play / pause", colors))
                                    .child(caption("← →  Seek", colors))
                                    .child(caption("Home / End  Jump", colors))
                                    .child(caption("Tab + Enter  Controls", colors)),
                            ),
                    )
                    .child(
                        div()
                            .when(wide, |column| column.w(px(320.)).flex_shrink_0())
                            .when(!wide, |column| column.w_full())
                            .child(self.analysis(colors)),
                    ),
            );
        div()
            .id("viewer")
            .role(Role::Application)
            .aria_label("Rail trace playback")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Toggle, _, cx| this.command(Command::Toggle, cx)))
            .on_action(cx.listener(|this, _: &Previous, _, cx| {
                this.command(Command::Seek(this.playback.cursor().saturating_sub(1)), cx)
            }))
            .on_action(cx.listener(|this, _: &Next, _, cx| {
                this.command(Command::Seek(this.playback.cursor().saturating_add(1)), cx)
            }))
            .on_action(cx.listener(|this, _: &Reset, _, cx| this.command(Command::Seek(0), cx)))
            .on_action(
                cx.listener(|this, _: &Last, _, cx| this.command(Command::Seek(usize::MAX), cx)),
            )
            .on_action(cx.listener(|_, _: &Tab, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &TabBack, window, cx| window.focus_prev(cx)))
            .size_full()
            .flex()
            .bg(colors.background)
            .text_color(colors.foreground)
            .text_size(px(14.))
            .line_height(px(20.))
            .font_family(font)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(64.))
                            .bg(colors.popover)
                            .flex_shrink_0()
                            .px_6()
                            .border_b_1()
                            .border_color(colors.border)
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .size(px(32.))
                                    .rounded(px(8.))
                                    .bg(colors.button_primary)
                                    .text_color(colors.button_primary_foreground)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .font_weight(FontWeight::BOLD)
                                    .child("u"),
                            )
                            .child(
                                div()
                                    .text_size(px(20.))
                                    .line_height(px(26.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("urbanflow"),
                            )
                            .child(caption("Rail playback", colors))
                            .child(div().flex_1())
                            .when(window.viewport_size().width >= px(640.), |header| {
                                header.child(caption("rail_service.json", colors))
                            })
                            .child(
                                Button::new("dark-mode")
                                    .ghost()
                                    .label("Dark mode")
                                    .selected(dark)
                                    .toggled(dark)
                                    .tooltip(if dark {
                                        "Switch to light mode"
                                    } else {
                                        "Switch to dark mode"
                                    })
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        let mode = if Theme::global(cx).is_dark() {
                                            ThemeMode::Light
                                        } else {
                                            ThemeMode::Dark
                                        };
                                        fluent::apply(mode, cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id("content")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(content),
                    ),
            )
    }
}

fn run() {
    #[cfg(target_family = "wasm")]
    let app = gpui_kit::platform::single_threaded_web();
    #[cfg(not(target_family = "wasm"))]
    let app = gpui_kit::application();

    let launch = |cx: &mut App| {
        gpui_kit::init(cx);

        cx.text_system()
            .add_fonts(
                [
                    include_bytes!("../assets/Pretendard-Regular.otf").as_slice(),
                    include_bytes!("../assets/Pretendard-Medium.otf").as_slice(),
                    include_bytes!("../assets/Pretendard-SemiBold.otf").as_slice(),
                    include_bytes!("../assets/Pretendard-Bold.otf").as_slice(),
                ]
                .into_iter()
                .map(std::borrow::Cow::Borrowed)
                .collect(),
            )
            .expect("load bundled Pretendard fonts");

        fluent::apply(ThemeMode::from(cx.window_appearance()), cx);

        cx.bind_keys([
            KeyBinding::new("space", Toggle, None),
            KeyBinding::new("left", Previous, None),
            KeyBinding::new("right", Next, None),
            KeyBinding::new("home", Reset, None),
            KeyBinding::new("end", Last, None),
            KeyBinding::new("tab", Tab, None),
            KeyBinding::new("shift-tab", TabBack, None),
        ]);

        let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);

        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui_kit::TitlebarOptions {
                    title: Some("Urbanflow GPUI trial".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Viewer::new(window, cx)),
        )
        .expect("open viewer");

        cx.activate(true);
    };

    #[cfg(not(target_family = "wasm"))]
    app.run(launch);
    #[cfg(target_family = "wasm")]
    APPLICATION.with(|handle| *handle.borrow_mut() = Some(app.run_embedded(launch)));
}

#[cfg(target_family = "wasm")]
thread_local! {
    static APPLICATION: std::cell::RefCell<Option<gpui_kit::ApplicationHandle>> = const { std::cell::RefCell::new(None) };
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    gpui_kit::platform::web_init();
    run();
}
