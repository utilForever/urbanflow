#[allow(dead_code)]
#[path = "../../../examples/results/mod.rs"]
mod results;

use gpui_kit::component::Theme;
use gpui_kit::{
    App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};

const DEMO: &str = include_str!("../../../tests/fixtures/rail_service.json");

struct Desktop {
    result: results::RailResult,
}

impl Render for Desktop {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);

        div()
            .size_full()
            .p_8()
            .flex()
            .flex_col()
            .gap_4()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family("Segoe UI")
            .child(div().text_2xl().child("Urbanflow"))
            .child("Urban transit simulation")
            .child(format!(
                "Bundled Rail result: {} nodes, {} snapshots",
                self.result.world.nodes.len(),
                self.result.trace.snapshots.len(),
            ))
            .child("Desktop application ready. Scenario commands and playback are coming next.")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = results::parse("rail_service.json", DEMO)?;

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
            |_, cx| cx.new(|_| Desktop { result }),
        )
        .expect("open Urbanflow desktop window");
        cx.activate(true);
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_result_uses_the_existing_adapter_and_core_analysis() {
        let result = results::parse("rail_service.json", DEMO).unwrap();

        assert_eq!(result.world.nodes.len(), 3);
        assert_eq!(result.trace.snapshots.len(), 7);
        assert_eq!(result.trace.snapshots.last().unwrap().tick, 6);
        assert!(result.trace.completed);
        assert_eq!(
            result.analysis.unwrap(),
            urbanflow::analysis::analyze(&result.trace).unwrap()
        );
    }
}
