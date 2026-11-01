use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, px, rgb};

pub fn apply(mode: ThemeMode, cx: &mut App) {
    Theme::change(mode, None, cx);
    Theme::update(cx, configure);
}

fn configure(theme: &mut Theme) {
    // Fluent 2 neutral aliases and brandWeb ramp; source links are in README.md.
    let color = |light, dark| rgb(if theme.is_dark() { dark } else { light }).into();
    let mut colors = theme.colors;

    colors.background = color(0xf5f5f5, 0x141414);
    colors.popover = color(0xffffff, 0x292929);
    colors.foreground = color(0x242424, 0xffffff);
    colors.popover_foreground = colors.foreground;
    colors.muted_foreground = color(0x616161, 0xadadad);
    colors.border = color(0xe0e0e0, 0x424242);
    colors.secondary = color(0xfafafa, 0x1f1f1f);
    colors.secondary_foreground = colors.foreground;
    colors.secondary_hover = color(0xf0f0f0, 0x333333);
    colors.secondary_active = color(0xdbdbdb, 0x141414);
    colors.primary = color(0x0f6cbd, 0x479ef5);
    colors.primary_hover = color(0x115ea3, 0x62abf5);
    colors.primary_active = color(0x0f548c, 0x2886de);
    colors.primary_foreground = rgb(0xffffff).into();
    colors.accent = color(0xebf3fc, 0x0a2e4a);
    colors.accent_foreground = color(0x115ea3, 0x96c6fa);
    colors.button = colors.popover;
    colors.button_foreground = colors.foreground;
    colors.button_hover = color(0xf5f5f5, 0x3d3d3d);
    colors.button_active = color(0xe0e0e0, 0x1f1f1f);
    colors.button_primary = color(0x0f6cbd, 0x115ea3);
    colors.button_primary_hover = color(0x115ea3, 0x0f6cbd);
    colors.button_primary_active = rgb(0x0c3b5e).into();
    colors.button_primary_foreground = colors.primary_foreground;
    colors.ring = colors.foreground;
    colors.chart_grid = color(0x707070, 0x999999);
    theme.colors = colors;
    theme.radius = px(4.);
    theme.radius_lg = px(8.);
    theme.font_size = px(14.);
    theme.font_family = "Pretendard".into();
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::Hsla;

    fn contrast(a: Hsla, b: Hsla) -> f32 {
        let luminance = |color: Hsla| {
            let rgb = color.to_rgb();
            let linear = |v: f32| {
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };

            0.2126 * linear(rgb.r) + 0.7152 * linear(rgb.g) + 0.0722 * linear(rgb.b)
        };

        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn both_modes_keep_text_and_route_marks_legible() {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let mut theme = Theme {
                mode,
                ..Theme::default()
            };

            configure(&mut theme);

            let c = theme.colors;

            for surface in [c.background, c.popover, c.secondary] {
                for text in [c.foreground, c.muted_foreground] {
                    assert!(contrast(text, surface) >= 4.5, "{mode:?} body text");
                }
            }

            assert!(contrast(c.accent_foreground, c.accent) >= 4.5);

            for background in [
                c.button_primary,
                c.button_primary_hover,
                c.button_primary_active,
            ] {
                assert!(contrast(c.button_primary_foreground, background) >= 4.5);
            }

            for mark in [c.primary, c.chart_grid, c.ring] {
                assert!(contrast(mark, c.secondary) >= 3., "{mode:?} route/focus");
            }
        }
    }
}
