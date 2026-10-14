//! Arsenal - Arma Reforger Server Launcher.
//!
//! Run without arguments for the GUI. `arsenal --help` lists the small CLI.

// Release builds on Windows are GUI programs, so no terminal window opens behind the app.
// Debug builds keep the console, which is useful for seeing panics while developing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod build_info;
mod cli;
mod config;
mod launch;
mod logfiles;
mod metrics;
mod panels;
mod paths;
mod server;
mod settings;
mod state;
mod theme;
mod workshop;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = cli::run(&args) {
        std::process::exit(code);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Arsenal")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Arsenal",
        options,
        Box::new(|cc| Box::new(app::ArsenalApp::new(cc))),
    )
}
