//! Live server output: the server's own log files, and what Arsenal captured from the process.

use egui::{RichText, TextEdit, Ui};

use crate::logfiles::FILES;
use crate::server::{LineKind, LogLine};
use crate::state::{AppState, LogView};

/// The server marks its lines like `SCRIPT (W):`, `RESOURCES (E):` and `ENGINE (F):`.
fn is_error(line: &LogLine) -> bool {
    line.kind == LineKind::Err
        || line.text.contains("ERROR")
        || line.text.contains("(E)")
        || line.text.contains("(F)")
}

fn is_warning(line: &LogLine) -> bool {
    line.text.contains("WARNING") || line.text.contains("(W)")
}

fn is_problem(line: &LogLine) -> bool {
    is_error(line) || is_warning(line)
}

pub fn ui(state: &mut AppState, ui: &mut Ui) {
    // Which log is shown.
    ui.horizontal(|ui| {
        for view in LogView::ALL {
            ui.selectable_value(&mut state.ui.log_view, view, view.label());
        }
    });

    let view = state.ui.log_view;
    let mut copy = false;
    let mut clear = false;
    ui.horizontal_wrapped(|ui| {
        ui.add(
            TextEdit::singleline(&mut state.ui.log_filter)
                .hint_text("Filter...")
                .desired_width(200.0),
        );
        ui.checkbox(&mut state.ui.log_problems_only, "Warnings and Errors Only");
        ui.checkbox(&mut state.ui.log_autoscroll, "Auto-scroll")
            .on_hover_text("Follow new lines. Scrolling up pauses it; scroll back to the end to follow again.");
        copy = ui.button("Copy").clicked();
        clear = ui.button("Clear Log").clicked();
    });

    // Which file is being followed, so a wrong logs folder is easy to spot.
    match view.file_index() {
        Some(index) => match state.logfiles.file_path(index) {
            Some(path) => {
                ui.weak(path.display().to_string());
            }
            None => match state.logfiles.root() {
                Some(root) => {
                    ui.weak(format!("Waiting for {} in {}", FILES[index].1, root.display()));
                }
                None => {
                    ui.weak("Logs folder not set. Set the server folder or the Profile in Launch Options.");
                }
            },
        },
        None => {
            ui.weak("Arsenal's own messages and anything the server prints to its console pipe.");
        }
    }
    ui.separator();

    let source = match view.file_index() {
        Some(index) => state.logfiles.lines(index),
        None => &state.server.log,
    };

    if copy {
        let text = source.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
        ui.output_mut(|o| o.copied_text = text);
    }

    let filter = state.ui.log_filter.trim().to_string();
    let problems_only = state.ui.log_problems_only;
    let lines: Vec<&LogLine> = source
        .iter()
        .filter(|l| !problems_only || is_problem(l))
        .filter(|l| contains_ignore_case(&l.text, &filter))
        .collect();

    let error_color = ui.visuals().error_fg_color;
    let warn_color = ui.visuals().warn_fg_color;
    let system_color = ui.visuals().hyperlink_color;
    let autoscroll = state.ui.log_autoscroll;

    if lines.is_empty() {
        ui.weak("Nothing to show.");
    } else {
        // The toolbar above stays put; only the lines below scroll. With Auto-scroll on, the
        // view follows new lines until you scroll up, and follows again when you scroll back to
        // the end. Only the rows on screen are built, so the whole history can be scrolled while
        // the server is running.
        let row_height =
            ui.text_style_height(&egui::TextStyle::Monospace) + ui.spacing().item_spacing.y;
        egui::ScrollArea::both()
            .id_source(("log_scroll", view.label()))
            .auto_shrink([false, false])
            .stick_to_bottom(autoscroll)
            .show_rows(ui, row_height, lines.len(), |ui, range| {
                for line in &lines[range] {
                    let mut text = RichText::new(&line.text).monospace();
                    if line.kind == LineKind::System {
                        text = text.color(system_color);
                    } else if is_error(line) {
                        text = text.color(error_color);
                    } else if is_warning(line) {
                        text = text.color(warn_color);
                    }
                    ui.add(egui::Label::new(text).wrap(false));
                }
            });
    }

    if clear {
        match view.file_index() {
            Some(index) => state.logfiles.clear(index),
            None => state.server.clear_log(),
        }
    }
}

/// Case-insensitive "contains" without building a lowercase copy of every line each frame.
fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    let needle = needle.as_bytes();
    if needle.is_empty() {
        return true;
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

#[cfg(test)]
mod tests {
    use super::contains_ignore_case;

    #[test]
    fn filter_ignores_case_and_allows_empty() {
        assert!(contains_ignore_case("Session Restored", "restored"));
        assert!(contains_ignore_case("Session Restored", ""));
        assert!(!contains_ignore_case("abc", "abcd"));
    }
}
