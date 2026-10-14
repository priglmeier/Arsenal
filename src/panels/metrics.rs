//! The Metrics panel: CPU and RAM use of the Arma server and of the whole system.

use egui::{RichText, Ui};

use crate::metrics::{bytes_text, cpu_level, ram_level, Usage};
use crate::metrics::{CPU_CRITICAL, CPU_WARNING, RAM_CRITICAL, RAM_WARNING};
use crate::state::AppState;
use crate::theme::level_color;

pub fn ui(state: &mut AppState, ui: &mut Ui) {
    let dark = ui.visuals().dark_mode;
    let metrics = &state.metrics;

    egui::Grid::new("metrics_grid")
        .num_columns(6)
        .spacing([28.0, 10.0])
        .show(ui, |ui| {
            let server_memory = metrics.server.map(|u| u.ram_bytes);
            row(ui, "Arma Server", metrics.server, MemoryReading::server(server_memory), dark);
            let memory = MemoryReading::system(metrics.total_ram_bytes, metrics.system.ram_bytes);
            row(ui, "System", Some(metrics.system), memory, dark);
        });

    ui.add_space(8.0);
    let small = |text: String| RichText::new(text).small().weak();
    ui.label(small(
        "CPU is shown as a share of the whole machine, like Task Manager. Updated once a second."
            .to_string(),
    ));

    ui.add_space(8.0);
    ui.separator();
    ui.label(small(format!(
        "Colors: green up to {CPU_WARNING:.0}% CPU / {RAM_WARNING:.0}% RAM, \
         yellow up to {CPU_CRITICAL:.0}% CPU / {RAM_CRITICAL:.0}% RAM, red above that."
    )));
}

/// One line: `Name  CPU% ##.##  RAM% ##.##  RAM Free ##  Used ##  Total ##`. A label and its number
/// share one grid cell with a single space between them; the grid column gap is the "tab", and
/// because every pair is a column, the rows line up.
fn row(ui: &mut Ui, name: &str, usage: Option<Usage>, memory: MemoryReading, dark: bool) {
    let level = usage.map(|u| ram_level(u.ram));
    ui.strong(name);
    pair(ui, "CPU%", percent_text(usage.map(|u| u.cpu)), usage.map(|u| cpu_level(u.cpu)), dark);
    pair(ui, "RAM%", percent_text(usage.map(|u| u.ram)), level, dark);
    pair(ui, "RAM Free", memory_text(memory.free), level, dark);
    pair(ui, "Used", memory_text(memory.used), level, dark);
    pair(ui, "Total", memory_text(memory.total), level, dark);
    ui.end_row();
}

/// `label value` with one space between them. The number is bold, monospace and colored.
fn pair(ui: &mut Ui, label: &str, text: String, level: Option<crate::metrics::Level>, dark: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label(format!("{label} "));
        let number = RichText::new(text).monospace().strong();
        match level {
            Some(level) => ui.label(number.color(level_color(level, dark))),
            None => ui.label(number.weak()),
        };
    });
}

fn percent_text(percent: Option<f32>) -> String {
    match percent {
        Some(p) => format!("{p:05.2}"),
        None => "--.--".to_string(),
    }
}

fn memory_text(bytes: Option<u64>) -> String {
    match bytes {
        Some(b) => format!("{:>7}", bytes_text(b)),
        None => format!("{:>7}", "--.--"),
    }
}

/// Free, used and total memory for one row. `None` shows as `--.--`.
#[derive(Clone, Copy)]
struct MemoryReading {
    free: Option<u64>,
    used: Option<u64>,
    total: Option<u64>,
}

impl MemoryReading {
    /// The whole machine: free is what is left of the installed memory.
    fn system(total: u64, used: u64) -> Self {
        Self {
            free: Some(total.saturating_sub(used)),
            used: Some(used),
            total: Some(total),
        }
    }

    /// The Arma server only has a "used" figure.
    fn server(used: Option<u64>) -> Self {
        Self {
            free: None,
            used,
            total: None,
        }
    }
}
