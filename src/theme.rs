//! Visual themes. Dark is the default; "System" picks light or dark from the operating system.

use egui::{Color32, Context, Visuals};

use crate::metrics::Level;

const ACCENT: Color32 = Color32::from_rgb(201, 160, 84);
const ACCENT_DIM: Color32 = Color32::from_rgb(125, 96, 48);

pub fn apply(ctx: &Context, dark: bool) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.interact_size.y = 24.0;
    // egui's "small" text is tiny next to the normal labels. Every hint, help line and note in
    // the app uses it, so keep it just under the body size instead.
    let body_size = style
        .text_styles
        .get(&egui::TextStyle::Body)
        .map_or(14.0, |font| font.size);
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::proportional((body_size - 1.0).max(10.0)),
    );
    ctx.set_style(style);

    let mut visuals = if dark { Visuals::dark() } else { Visuals::light() };
    if dark {
        visuals.panel_fill = Color32::from_rgb(27, 29, 33);
        visuals.window_fill = Color32::from_rgb(33, 35, 40);
        visuals.extreme_bg_color = Color32::from_rgb(18, 19, 22);
        visuals.faint_bg_color = Color32::from_rgb(34, 36, 41);
        visuals.selection.bg_fill = ACCENT_DIM;
        visuals.hyperlink_color = ACCENT;
    } else {
        visuals.selection.bg_fill = ACCENT;
        visuals.hyperlink_color = ACCENT_DIM;
    }
    ctx.set_visuals(visuals);
}

/// Green, yellow or red for a load level, readable on both the dark and the light theme.
pub fn level_color(level: Level, dark: bool) -> Color32 {
    match (level, dark) {
        (Level::Normal, dark) => ok_color(dark),
        (Level::Warning, true) => Color32::from_rgb(235, 190, 70),
        (Level::Warning, false) => Color32::from_rgb(150, 100, 0),
        (Level::Critical, true) => Color32::from_rgb(240, 90, 90),
        (Level::Critical, false) => Color32::from_rgb(190, 30, 30),
    }
}

pub fn ok_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(110, 200, 130)
    } else {
        Color32::from_rgb(30, 130, 60)
    }
}
