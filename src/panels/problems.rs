//! The list of problems found by validating the current config.

use egui::{RichText, Ui};

use crate::config::validate::Severity;
use crate::state::AppState;

pub fn ui(state: &mut AppState, ui: &mut Ui) {
    if state.issues.is_empty() {
        ui.colored_label(crate::theme::ok_color(ui.visuals().dark_mode), "No problems found.");
        return;
    }

    let error_color = ui.visuals().error_fg_color;
    let warn_color = ui.visuals().warn_fg_color;
    for issue in &state.issues {
        ui.horizontal_wrapped(|ui| {
            let (label, color) = match issue.severity {
                Severity::Error => ("Error", error_color),
                Severity::Warning => ("Warning", warn_color),
            };
            ui.label(RichText::new(label).color(color).strong());
            ui.label(RichText::new(&issue.field).monospace());
            ui.label(&issue.message);
        });
    }
    ui.add_space(6.0);
    ui.label(
        RichText::new("Errors stop the server from starting; warnings do not.")
            .small()
            .weak(),
    );
}
