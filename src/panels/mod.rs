//! The dockable panels. Each [`Tab`] is one panel the user can drag, split and re-arrange.

use egui::Ui;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

mod launch_panel;
mod log_panel;
mod metrics;
mod mods;
mod problems;
mod server_panels;
mod widgets;
pub mod windows;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Tab {
    Metrics,
    Mods,
    Server,
    Network,
    Rcon,
    Gameplay,
    Operating,
    Launch,
    Log,
    Problems,
}

impl Tab {
    pub const ALL: [Tab; 10] = [
        Tab::Metrics,
        Tab::Mods,
        Tab::Server,
        Tab::Network,
        Tab::Rcon,
        Tab::Gameplay,
        Tab::Operating,
        Tab::Launch,
        Tab::Log,
        Tab::Problems,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Metrics => "Metrics",
            Tab::Mods => "Mods",
            Tab::Server => "Server",
            Tab::Network => "Network",
            Tab::Rcon => "RCON",
            Tab::Gameplay => "Gameplay",
            Tab::Operating => "Operating",
            Tab::Launch => "Launch Options",
            Tab::Log => "Log",
            Tab::Problems => "Problems",
        }
    }
}

/// Draws one panel.
pub fn show(state: &mut AppState, ui: &mut Ui, tab: Tab) {
    match tab {
        Tab::Metrics => metrics::ui(state, ui),
        Tab::Mods => mods::ui(state, ui),
        Tab::Server => server_panels::server(state, ui),
        Tab::Network => server_panels::network(state, ui),
        Tab::Rcon => server_panels::rcon(state, ui),
        Tab::Gameplay => server_panels::gameplay(state, ui),
        Tab::Operating => server_panels::operating(state, ui),
        Tab::Launch => launch_panel::ui(state, ui),
        Tab::Log => log_panel::ui(state, ui),
        Tab::Problems => problems::ui(state, ui),
    }
}
