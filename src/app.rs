//! The application shell: menu bar, toolbar, status bar and the dockable panel area.

use std::time::Duration;

use egui::{Button, Context, Key, Modifiers, RichText, ViewportCommand};
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer};

use crate::config::validate::{has_errors, Severity};
use crate::panels::{self, windows, Tab};
use crate::settings::ThemeChoice;
use crate::state::{AppState, PendingAction};
use crate::theme;

pub struct ArsenalApp {
    state: AppState,
    dock: DockState<Tab>,
    applied_dark: Option<bool>,
    last_title: String,
}

/// The layout used on first run and by View > Reset layout. It follows the reference tool:
/// metrics and mods on the left (Mods shown first), server settings on the right, and the
/// launch options, log and problems along the bottom. Every panel can be dragged, split or
/// tabbed elsewhere.
fn default_dock() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Metrics, Tab::Mods]);
    let surface = dock.main_surface_mut();
    let [top, _bottom] = surface.split_below(
        NodeIndex::root(),
        0.62,
        vec![Tab::Log, Tab::Launch, Tab::Problems],
    );
    let [_mods, _settings] = surface.split_right(
        top,
        0.42,
        vec![Tab::Server, Tab::Network, Tab::Rcon, Tab::Gameplay, Tab::Operating],
    );
    // Metrics sits leftmost, but the Mods list is what should be on screen at start.
    if let Some(path) = dock.find_tab(&Tab::Mods) {
        dock.set_active_tab(path);
    }
    dock
}

impl ArsenalApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let state = AppState::new();
        let dock = state
            .settings
            .dock
            .clone()
            .and_then(|value| serde_json::from_value::<DockState<Tab>>(value).ok())
            .unwrap_or_else(default_dock);
        Self {
            state,
            dock,
            applied_dark: None,
            last_title: String::new(),
        }
    }

    fn save_layout(&mut self) {
        self.state.settings.dock = serde_json::to_value(&self.dock).ok();
        self.state.persist_settings();
    }

    fn toggle_panel(&mut self, tab: Tab) {
        match self.dock.find_tab(&tab) {
            Some(path) => {
                self.dock.remove_tab(path);
            }
            None => self.dock.push_to_focused_leaf(tab),
        }
    }

    fn apply_theme(&mut self, ctx: &Context, frame: &eframe::Frame) {
        let dark = match self.state.settings.theme {
            ThemeChoice::Dark => true,
            // Fall back to dark when the platform cannot tell us.
            ThemeChoice::System => !matches!(
                frame.info().system_theme,
                Some(eframe::Theme::Light)
            ),
        };
        if self.applied_dark != Some(dark) {
            theme::apply(ctx, dark);
            self.applied_dark = Some(dark);
        }
    }

    fn shortcuts(&mut self, ctx: &Context) {
        let (save, open, new) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::O),
                i.consume_key(Modifiers::COMMAND, Key::N),
            )
        });
        if save {
            self.state.save();
        }
        if open {
            self.state.request_action(PendingAction::Open);
        }
        if new {
            self.state.request_action(PendingAction::New);
        }
    }

    fn menu_bar(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.add(Button::new("New").shortcut_text("Ctrl+N")).clicked() {
                        self.state.request_action(PendingAction::New);
                        ui.close_menu();
                    }
                    if ui.add(Button::new("Open...").shortcut_text("Ctrl+O")).clicked() {
                        self.state.request_action(PendingAction::Open);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add(Button::new("Save").shortcut_text("Ctrl+S")).clicked() {
                        self.state.save();
                        ui.close_menu();
                    }
                    if ui.button("Save As...").clicked() {
                        self.state.save_as();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        self.state.request_action(PendingAction::Quit);
                        ui.close_menu();
                    }
                });

                ui.menu_button("Server", |ui| {
                    let running = self.state.server.is_running();
                    if ui.add_enabled(!running, Button::new("Start")).clicked() {
                        self.state.start_server();
                        ui.close_menu();
                    }
                    if ui.add_enabled(running, Button::new("Stop")).clicked() {
                        self.state.stop_server();
                        ui.close_menu();
                    }
                    if ui.button("Restart").clicked() {
                        self.state.restart_server();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Set Server Folder...").clicked() {
                        self.state.pick_server_folder();
                        ui.close_menu();
                    }
                    if ui.button("Reload Scenarios").clicked() {
                        self.state.refresh_scenarios();
                        ui.close_menu();
                    }
                });

                ui.menu_button("View", |ui| {
                    ui.label("Theme");
                    let mut changed = false;
                    changed |= ui
                        .radio_value(&mut self.state.settings.theme, ThemeChoice::Dark, "Dark")
                        .changed();
                    changed |= ui
                        .radio_value(&mut self.state.settings.theme, ThemeChoice::System, "System")
                        .changed();
                    if changed {
                        self.state.persist_settings();
                    }
                    ui.separator();
                    ui.label("Panels");
                    for tab in Tab::ALL {
                        let mut shown = self.dock.find_tab(&tab).is_some();
                        if ui.checkbox(&mut shown, tab.title()).changed() {
                            self.toggle_panel(tab);
                        }
                    }
                    ui.separator();
                    if ui.button("Reset Layout").clicked() {
                        self.dock = default_dock();
                        ui.close_menu();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About Arsenal").clicked() {
                        self.state.ui.show_about = true;
                        ui.close_menu();
                    }
                    if ui.button("License").clicked() {
                        self.state.ui.show_license = true;
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn toolbar(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let running = self.state.server.is_running();
                if ui.add_enabled(!running, Button::new("Start Server")).clicked() {
                    self.state.start_server();
                }
                if ui.add_enabled(running, Button::new("Stop")).clicked() {
                    self.state.stop_server();
                }
                if ui.add_enabled(running, Button::new("Restart")).clicked() {
                    self.state.restart_server();
                }
                ui.separator();

                let dark = ui.visuals().dark_mode;
                if running {
                    let pid = self
                        .state
                        .server
                        .pid()
                        .map(|p| format!(" (PID {p})"))
                        .unwrap_or_default();
                    ui.label(RichText::new(format!("Running{pid}")).color(theme::ok_color(dark)));
                } else {
                    ui.weak("Stopped");
                }
                ui.separator();

                match &self.state.config_path {
                    Some(path) => {
                        ui.label(path.display().to_string());
                    }
                    None => {
                        ui.weak("Unsaved Config");
                    }
                }
                if self.state.server_exe().is_none() {
                    ui.separator();
                    let color = ui.visuals().warn_fg_color;
                    ui.colored_label(color, "Server Folder Not Set");
                }
            });
        });
    }

    fn status_bar(&mut self, ctx: &Context) {
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(status) = &self.state.status {
                    let color = if status.is_error {
                        ui.visuals().error_fg_color
                    } else {
                        theme::ok_color(ui.visuals().dark_mode)
                    };
                    ui.colored_label(color, status.text.as_str());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let errors = self
                        .state
                        .issues
                        .iter()
                        .filter(|i| i.severity == Severity::Error)
                        .count();
                    let warnings = self.state.issues.len() - errors;
                    if has_errors(&self.state.issues) {
                        let color = ui.visuals().error_fg_color;
                        ui.colored_label(color, format!("{errors} errors, {warnings} warnings"));
                    } else if warnings > 0 {
                        let color = ui.visuals().warn_fg_color;
                        ui.colored_label(color, format!("{warnings} warnings"));
                    } else {
                        ui.weak("No Problems");
                    }
                });
            });
        });
    }
}

/// Connects the dock area to the panel functions.
struct Viewer<'a> {
    state: &'a mut AppState,
}

impl TabViewer for Viewer<'_> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        match tab {
            // The log has its own scroll area below a fixed toolbar, so it can follow new lines.
            Tab::Log => panels::show(self.state, ui, *tab),
            _ => {
                egui::ScrollArea::vertical()
                    .id_source(("panel_scroll", tab.title()))
                    .auto_shrink([false, false])
                    .show(ui, |ui| panels::show(self.state, ui, *tab));
            }
        }
    }
}

impl eframe::App for ArsenalApp {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        self.apply_theme(ctx, frame);
        self.state.tick();
        self.shortcuts(ctx);

        // Closing the window with unsaved changes asks first.
        if ctx.input(|i| i.viewport().close_requested())
            && !self.state.quit_now
            && self.state.is_dirty()
        {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.state.confirm = Some(PendingAction::Quit);
        }
        if self.state.quit_now {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }

        let title = self.state.window_title();
        if title != self.last_title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }

        self.menu_bar(ctx);
        self.toolbar(ctx);
        self.status_bar(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let style = Style::from_egui(ui.style().as_ref());
                DockArea::new(&mut self.dock)
                    .style(style)
                    .show_inside(ui, &mut Viewer { state: &mut self.state });
            });

        windows::show_all(&mut self.state, ctx);

        // Keep the log and process status fresh even when the user is idle.
        let interval = if self.state.server.is_running() { 100 } else { 500 };
        ctx.request_repaint_after(Duration::from_millis(interval));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_layout();
        self.state.persist_settings();
        // A running server is killed when the process is dropped (see `ServerProcess::drop`).
    }
}
