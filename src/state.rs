//! Application state shared by every panel, and the actions that change it.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::config::validate::{self, Issue};
use crate::config::{io, Mod, ServerConfig};
use crate::launch;
use crate::logfiles::LogFiles;
use crate::metrics::Metrics;
use crate::paths;
use crate::server::{self, ServerEvent, ServerProcess, SERVER_EXE};
use crate::settings::AppSettings;

pub struct StatusMsg {
    pub text: String,
    pub is_error: bool,
}

/// Actions that would throw away unsaved changes and so need a confirmation first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PendingAction {
    New,
    Open,
    Quit,
}

/// The "Add mod" / "Edit mod" form.
pub struct ModEditor {
    /// Index in the mod library when editing, `None` when adding.
    pub index: Option<usize>,
    pub data: Mod,
    pub version_text: String,
    /// A pasted Reforger Workshop page address; the mod ID and name are read from it.
    pub workshop_link: String,
}

/// What the Log panel is showing.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum LogView {
    /// The server's `console.log`.
    #[default]
    Console,
    /// The server's `error.log`.
    Error,
    /// The server's `script.log`.
    Script,
    /// What Arsenal itself captured from the server process, plus its own messages.
    Arsenal,
}

impl LogView {
    pub const ALL: [LogView; 4] = [LogView::Console, LogView::Error, LogView::Script, LogView::Arsenal];

    pub fn label(self) -> &'static str {
        match self {
            LogView::Console => "Console",
            LogView::Error => "Error",
            LogView::Script => "Script",
            LogView::Arsenal => "Arsenal",
        }
    }

    /// Index into the followed files, or `None` for Arsenal's own output.
    pub fn file_index(self) -> Option<usize> {
        match self {
            LogView::Console => Some(0),
            LogView::Error => Some(1),
            LogView::Script => Some(2),
            LogView::Arsenal => None,
        }
    }
}

#[derive(Default)]
pub struct UiState {
    pub log_view: LogView,
    pub mod_search: String,
    pub sel_available: Option<usize>,
    /// Rows selected in the Mods Enabled list (Ctrl-click adds, Shift-click selects a range).
    pub sel_enabled: BTreeSet<usize>,
    /// Row a Shift-click range starts from.
    pub sel_anchor: Option<usize>,
    pub mod_editor: Option<ModEditor>,

    pub show_scenarios: bool,
    pub scenario_filter: String,
    pub scenario_manual: String,
    pub scenario_selected: Option<String>,
    pub scenarios_loading: bool,

    pub show_mission_header: bool,
    pub mission_header_text: String,
    pub mission_header_error: Option<String>,

    pub show_passwords: bool,
    pub show_about: bool,
    pub show_license: bool,

    pub log_filter: String,
    pub log_autoscroll: bool,
    pub log_problems_only: bool,

    /// Save points found in the `.save` folder, and which folder they were read from.
    pub saves: Vec<paths::SavePoint>,
    pub saves_scanned_for: Option<PathBuf>,

    /// The picture shown in the About window, loaded the first time it is needed.
    pub about_image: Option<egui::TextureHandle>,
}

pub struct AppState {
    pub settings: AppSettings,
    pub config: ServerConfig,
    /// The config as last loaded or saved; used to tell whether there are unsaved changes.
    saved_snapshot: ServerConfig,
    pub config_path: Option<PathBuf>,
    pub server: ServerProcess,
    pub metrics: Metrics,
    pub logfiles: LogFiles,
    pub issues: Vec<Issue>,
    pub status: Option<StatusMsg>,
    pub ui: UiState,

    pub confirm: Option<PendingAction>,
    pub quit_now: bool,

    scenario_rx: Option<Receiver<server::ScenarioReport>>,
    restart_after_stop: bool,
    restart_at: Option<Instant>,
    last_scheduled_restart: Option<chrono::NaiveDate>,
}

impl AppState {
    pub fn new() -> Self {
        let settings = AppSettings::load();
        let mut config = ServerConfig::default();
        let mut config_path = None;
        let mut status = None;

        if let Some(path) = settings.last_config.clone() {
            if path.is_file() {
                match io::load(&path) {
                    Ok(loaded) => {
                        config = loaded;
                        config_path = Some(path);
                    }
                    Err(err) => {
                        status = Some(StatusMsg {
                            text: format!("{err:#}"),
                            is_error: true,
                        })
                    }
                }
            }
        }

        let issues = validate::validate(&config);
        let ui = UiState {
            log_autoscroll: true,
            ..UiState::default()
        };
        let mut state = Self {
            settings,
            saved_snapshot: config.clone(),
            config,
            config_path,
            server: ServerProcess::new(),
            metrics: Metrics::new(),
            logfiles: LogFiles::new(),
            issues,
            status,
            ui,
            confirm: None,
            quit_now: false,
            scenario_rx: None,
            restart_after_stop: false,
            restart_at: None,
            last_scheduled_restart: None,
        };
        state.fix_old_profile_value();
        if state.merge_config_mods() > 0 {
            state.persist_settings();
        }
        state
    }

    // ----- status -----

    pub fn set_status(&mut self, text: impl Into<String>, is_error: bool) {
        self.status = Some(StatusMsg {
            text: text.into(),
            is_error,
        });
    }

    pub fn persist_settings(&mut self) {
        if let Err(err) = self.settings.save() {
            self.set_status(format!("Could not save settings: {err:#}"), true);
        }
    }

    // ----- config file handling -----

    pub fn is_dirty(&self) -> bool {
        self.config != self.saved_snapshot
    }

    pub fn window_title(&self) -> String {
        let name = self
            .config_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string());
        let marker = if self.is_dirty() { "*" } else { "" };
        format!("{name}{marker} - Arsenal")
    }

    pub fn request_action(&mut self, action: PendingAction) {
        if self.is_dirty() {
            self.confirm = Some(action);
        } else {
            self.run_action(action);
        }
    }

    pub fn run_action(&mut self, action: PendingAction) {
        self.confirm = None;
        match action {
            PendingAction::New => self.new_config(),
            PendingAction::Open => self.open_dialog(),
            PendingAction::Quit => self.quit_now = true,
        }
    }

    fn new_config(&mut self) {
        self.config = ServerConfig::default();
        self.saved_snapshot = self.config.clone();
        self.config_path = None;
        self.settings.last_config = None;
        self.set_status("Started a new config", false);
    }

    fn open_dialog(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Open Server Config")
            .add_filter("Server Config (JSON)", &["json"]);
        if let Some(dir) = self.config_path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.load_path(path);
        }
    }

    pub fn load_path(&mut self, path: PathBuf) {
        match io::load(&path) {
            Ok(cfg) => {
                self.saved_snapshot = cfg.clone();
                self.config = cfg;
                self.merge_config_mods();
                self.settings.last_config = Some(path.clone());
                self.config_path = Some(path.clone());
                self.persist_settings();
                self.set_status(format!("Loaded {}", path.display()), false);
            }
            Err(err) => self.set_status(format!("{err:#}"), true),
        }
    }

    /// Saves to the current file, asking for a location the first time. Returns true on success.
    pub fn save(&mut self) -> bool {
        match self.config_path.clone() {
            Some(path) => self.save_to(path),
            None => self.save_as(),
        }
    }

    pub fn save_as(&mut self) -> bool {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Save Server Config")
            .add_filter("Server Config (JSON)", &["json"])
            .set_file_name("server.json");
        if let Some(dir) = self.config_path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        match dialog.save_file() {
            Some(path) => self.save_to(path),
            None => false,
        }
    }

    fn save_to(&mut self, path: PathBuf) -> bool {
        match io::save(&path, &self.config) {
            Ok(()) => {
                self.saved_snapshot = self.config.clone();
                self.settings.last_config = Some(path.clone());
                self.config_path = Some(path.clone());
                self.persist_settings();
                self.set_status(format!("Saved {}", path.display()), false);
                true
            }
            Err(err) => {
                self.set_status(format!("{err:#}"), true);
                false
            }
        }
    }

    // ----- server location -----

    /// The server executable, if the configured path points at it (directly or via its folder).
    pub fn server_exe(&self) -> Option<PathBuf> {
        let path = self.settings.server_path.as_ref()?;
        if path.is_file() {
            return Some(path.clone());
        }
        let candidate = path.join(SERVER_EXE);
        candidate.is_file().then_some(candidate)
    }

    pub fn pick_server_folder(&mut self) {
        let dialog = rfd::FileDialog::new().set_title("Select the Arma Reforger Server Folder");
        if let Some(dir) = dialog.pick_folder() {
            self.settings.server_path = Some(dir);
            self.persist_settings();
            if self.server_exe().is_none() {
                self.set_status(format!("{SERVER_EXE} was not found in that folder"), true);
            } else if self.apply_standard_folders(false) {
                self.set_status("Server folder set. Profile and logs folders filled in under it.", false);
            }
        }
    }

    /// The folder the server is installed in, if a server path is set.
    pub fn server_root(&self) -> Option<PathBuf> {
        self.settings.server_path.as_deref().map(paths::server_root)
    }

    /// Points `-profile` and `-logsDir` at `saves` and `saves/logs` under the server
    /// folder and turns them on. With `overwrite` false, a parameter that is already switched
    /// on with a value is left alone. Returns true when anything was changed.
    /// The folder the server writes its logs into, from the `-profile` and `-logsDir` settings.
    /// `None` until a full `-profile` path is set.
    pub fn logs_root(&self) -> Option<PathBuf> {
        let params = &self.settings.launch.params;
        let value = |flag: &str| {
            params
                .get(flag)
                .filter(|p| p.enabled)
                .map(|p| p.value.trim().to_string())
                .unwrap_or_default()
        };
        let profile = Some(PathBuf::from(value("-profile"))).filter(|p| p.is_absolute());
        paths::resolve_logs(profile.as_deref(), &value("-logsDir"))
    }

    /// Earlier versions set `-profile` to `saves/profile`, which makes the server look in
    /// `saves/profile/profile` and miss every save. Fixes that one value and nothing else.
    pub fn fix_old_profile_value(&mut self) {
        let Some(root) = self.server_root() else {
            return;
        };
        let old = paths::old_profile_value(&root);
        let fixed = paths::standard_folders(&root).profile;
        if let Some(param) = self.settings.launch.params.get_mut("-profile") {
            if PathBuf::from(param.value.trim()) == old {
                param.value = fixed.display().to_string();
                self.persist_settings();
            }
        }
    }

    pub fn apply_standard_folders(&mut self, overwrite: bool) -> bool {
        let Some(root) = self.server_root() else {
            return false;
        };
        let folders = paths::standard_folders(&root);
        let mut changed = false;
        for (flag, path) in [("-profile", folders.profile), ("-logsDir", folders.logs)] {
            let param = self.settings.launch.params.entry(flag.to_string()).or_default();
            let in_use = param.enabled && !param.value.trim().is_empty();
            if overwrite || !in_use {
                param.enabled = true;
                param.value = path.display().to_string();
                changed = true;
            }
        }
        if changed {
            self.ui.saves_scanned_for = None;
            self.persist_settings();
        }
        changed
    }

    // ----- running the server -----

    pub fn start_server(&mut self) {
        if self.server.is_running() {
            return;
        }
        self.issues = validate::validate(&self.config);
        if validate::has_errors(&self.issues) {
            self.set_status("Fix the errors in the Problems panel before starting the server", true);
            return;
        }
        let Some(exe) = self.server_exe() else {
            self.set_status(
                format!("Set the Arma Reforger Server folder first ({SERVER_EXE} was not found)"),
                true,
            );
            return;
        };

        // The server reads the file from disk, so make sure it matches what is on screen.
        let saved = if self.config_path.is_none() || self.is_dirty() {
            self.save()
        } else {
            true
        };
        if !saved {
            return;
        }
        let Some(config_path) = self.config_path.clone() else {
            return;
        };

        let args = launch::build_args(&config_path, &self.settings.launch);
        match self.server.start(&exe, &args) {
            Ok(()) => self.set_status("Server started", false),
            Err(err) => self.set_status(format!("{err:#}"), true),
        }
    }

    pub fn stop_server(&mut self) {
        self.restart_at = None;
        self.restart_after_stop = false;
        self.server.stop();
    }

    pub fn restart_server(&mut self) {
        if self.server.is_running() {
            self.restart_after_stop = true;
            self.server.stop();
        } else {
            self.start_server();
        }
    }

    // ----- scenarios -----

    pub fn refresh_scenarios(&mut self) {
        let Some(exe) = self.server_exe() else {
            self.set_status(
                format!("Set the Arma Reforger Server folder first ({SERVER_EXE} was not found)"),
                true,
            );
            return;
        };
        let addons = self
            .config
            .game
            .mods
            .iter()
            .map(|m| m.mod_id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let mut args = launch::path_args(&self.settings.launch);
        if !addons.is_empty() {
            args.push("-addons".into());
            args.push(addons);
        }
        self.server.push_system("Listing scenarios (server base game, then the Workshop page of each enabled mod)...");
        let mods: Vec<(String, String)> = self
            .config
            .game
            .mods
            .iter()
            .map(|m| (m.mod_id.clone(), m.name.clone()))
            .collect();
        self.scenario_rx = Some(server::list_scenarios(exe, args, mods));
        self.ui.scenarios_loading = true;
    }

    // ----- mods -----

    /// Adds a mod to the enabled list (and the library) unless it is already enabled.
    pub fn enable_mod(&mut self, m: Mod) {
        self.remember_mod(&m);
        let exists = self
            .config
            .game
            .mods
            .iter()
            .any(|x| x.mod_id.eq_ignore_ascii_case(&m.mod_id));
        if !exists {
            self.config.game.mods.push(m);
        }
    }

    /// Adds a mod to the library unless its ID is already there.
    pub fn remember_mod(&mut self, m: &Mod) {
        let known = self
            .settings
            .available_mods
            .iter()
            .any(|x| x.mod_id.eq_ignore_ascii_case(&m.mod_id));
        if !known {
            self.settings.available_mods.push(m.clone());
        }
    }

    /// Adds every mod of the loaded config to the library, so that disabling one later
    /// leaves it in the Mods Library. Returns how many were new. The caller saves the settings.
    pub fn merge_config_mods(&mut self) -> usize {
        let before = self.settings.available_mods.len();
        for m in self.config.game.mods.clone() {
            self.remember_mod(&m);
        }
        self.settings.available_mods.len() - before
    }

    /// Imports a JSON list of mods (or the mods of a full server config) into the enabled list.
    pub fn import_mods_dialog(&mut self) {
        let dialog = rfd::FileDialog::new()
            .set_title("Import Mod List")
            .add_filter("Mod List or Server Config (JSON)", &["json"]);
        let Some(path) = dialog.pick_file() else {
            return;
        };
        match read_mod_list(&path) {
            Ok(mods) => {
                let count = mods.len();
                for m in mods {
                    self.enable_mod(m);
                }
                self.persist_settings();
                self.set_status(format!("Imported {count} mods from {}", path.display()), false);
            }
            Err(err) => self.set_status(format!("{err:#}"), true),
        }
    }

    pub fn export_mods_dialog(&mut self) {
        let dialog = rfd::FileDialog::new()
            .set_title("Export Enabled Mods")
            .add_filter("Mod List (JSON)", &["json"])
            .set_file_name("mods.json");
        let Some(path) = dialog.save_file() else {
            return;
        };
        let result = serde_json::to_string_pretty(&self.config.game.mods)
            .map_err(anyhow::Error::from)
            .and_then(|text| {
                fs::write(&path, text).with_context(|| format!("Could not write {}", path.display()))
            });
        match result {
            Ok(()) => self.set_status(format!("Exported mod list to {}", path.display()), false),
            Err(err) => self.set_status(format!("{err:#}"), true),
        }
    }

    // ----- per frame -----

    /// Called once per frame before the UI is drawn.
    pub fn tick(&mut self) {
        self.issues = validate::validate(&self.config);
        self.metrics.update(self.server.pid());
        let logs_root = self.logs_root();
        self.logfiles.set_root(logs_root);
        self.logfiles.poll();

        match self.server.poll() {
            ServerEvent::Exited { success, requested } => {
                // The server may have written new save points; list them again.
                self.ui.saves_scanned_for = None;
                if self.restart_after_stop {
                    self.restart_after_stop = false;
                    self.restart_at = Some(Instant::now() + Duration::from_secs(2));
                    self.set_status("Restarting server...", false);
                } else if requested {
                    self.set_status("Server stopped", false);
                } else if success {
                    self.set_status("Server exited", false);
                } else {
                    self.set_status("Server exited unexpectedly", true);
                    if self.settings.launch.restart_on_crash {
                        self.restart_at = Some(Instant::now() + Duration::from_secs(5));
                        self.server.push_system("Restarting in 5 seconds (restart on crash is on)");
                    }
                }
            }
            ServerEvent::None => {}
        }

        if let Some(at) = self.restart_at {
            if Instant::now() >= at && !self.server.is_running() {
                self.restart_at = None;
                self.start_server();
            }
        }

        self.check_scheduled_restart();
        self.check_scenario_results();
    }

    fn check_scheduled_restart(&mut self) {
        let opts = &self.settings.launch;
        if !opts.auto_restart || !self.server.is_running() {
            return;
        }
        let now = chrono::Local::now();
        let today = now.date_naive();
        if now.format("%H:%M").to_string() == opts.auto_restart_time.trim()
            && self.last_scheduled_restart != Some(today)
        {
            self.last_scheduled_restart = Some(today);
            self.server.push_system("Scheduled daily restart");
            self.restart_server();
        }
    }

    fn check_scenario_results(&mut self) {
        let Some(rx) = self.scenario_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(report) => {
                self.scenario_rx = None;
                self.ui.scenarios_loading = false;
                self.server.push_system(format!("Scenario command: {}", report.command));
                if let Some(err) = &report.error {
                    self.server.push_system(format!("Scenario listing failed: {err}"));
                }
                for note in &report.workshop_notes {
                    self.server.push_system(format!("Workshop: {note}"));
                }
                let count = report.scenarios.len();
                self.server.push_system(format!("Scenario listing found {count} scenarios"));
                if count < 10 {
                    for line in report.tail.iter().rev().take(25).rev() {
                        self.server.push_system(format!("  {line}"));
                    }
                }
                if count == 0 {
                    self.set_status(
                        "No scenarios were reported by the server (see Log > Arsenal). You can still type a scenario ID by hand.",
                        true,
                    );
                } else {
                    self.settings.scenarios = report.scenarios;
                    self.persist_settings();
                    self.set_status(format!("Found {count} scenarios"), false);
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.scenario_rx = None;
                self.ui.scenarios_loading = false;
            }
        }
    }
}

/// Reads a mod list from a JSON file: either an array of mods or a full server config.
fn read_mod_list(path: &Path) -> Result<Vec<Mod>> {
    let text = fs::read_to_string(path).with_context(|| format!("Could not read {}", path.display()))?;
    let text = text.trim_start_matches('\u{feff}');
    if let Ok(list) = serde_json::from_str::<Vec<Mod>>(text) {
        return Ok(list);
    }
    let cfg: ServerConfig = serde_json::from_str(text)
        .with_context(|| format!("{} is neither a mod list nor a server config", path.display()))?;
    Ok(cfg.game.mods)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mod_with_id(id: &str) -> Mod {
        Mod {
            mod_id: id.into(),
            name: id.into(),
            ..Mod::default()
        }
    }

    #[test]
    fn mod_list_can_be_read_from_an_array_or_a_config() {
        let dir = std::env::temp_dir().join(format!("arsenal-state-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();

        let array = dir.join("mods.json");
        fs::write(&array, r#"[{ "modId": "59727DAE364DEADB", "name": "A" }]"#).unwrap();
        assert_eq!(read_mod_list(&array).unwrap().len(), 1);

        let config = dir.join("config.json");
        fs::write(
            &config,
            r#"{ "game": { "mods": [
                { "modId": "59727DAE364DEADB", "name": "A" },
                { "modId": "59727DAE32981C7D", "name": "B" } ] } }"#,
        )
        .unwrap();
        assert_eq!(read_mod_list(&config).unwrap().len(), 2);

        let bad = dir.join("bad.json");
        fs::write(&bad, "42").unwrap();
        assert!(read_mod_list(&bad).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn enabling_a_mod_twice_keeps_one_copy() {
        let mut state = AppState::new();
        state.config.game.mods.clear();
        state.settings.available_mods.clear();
        state.enable_mod(mod_with_id("59727DAE364DEADB"));
        state.enable_mod(mod_with_id("59727dae364deadb"));
        assert_eq!(state.config.game.mods.len(), 1);
        assert_eq!(state.settings.available_mods.len(), 1);
    }

    #[test]
    fn mods_of_a_loaded_config_join_the_library_once() {
        let mut state = AppState::new();
        state.settings.available_mods.clear();
        state.config.game.mods = vec![
            mod_with_id("59727DAE364DEADB"),
            mod_with_id("59727dae364deadb"),
            mod_with_id("5965550F24A0C152"),
        ];
        assert_eq!(state.merge_config_mods(), 2);
        assert_eq!(state.merge_config_mods(), 0);
        assert_eq!(state.settings.available_mods.len(), 2);
    }

    #[test]
    fn editing_marks_the_config_dirty() {
        let mut state = AppState::new();
        state.new_config();
        assert!(!state.is_dirty());
        state.config.game.max_players += 1;
        assert!(state.is_dirty());
    }
}
