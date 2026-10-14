//! Server Management: where the server is installed, restart behavior, and every
//! startup parameter. The same table (`launch::PARAMS`) drives all the parameter rows.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use egui::{Button, CollapsingHeader, ComboBox, DragValue, RichText, ScrollArea, TextEdit, Ui};

use crate::launch::{
    build_args, display_arg, Kind, LaunchOptions, ParamSpec, ParamState, GROUPS, PARAMS,
};
use crate::paths;
use crate::server::SERVER_EXE;
use crate::state::AppState;

pub fn ui(state: &mut AppState, ui: &mut Ui) {
    server_location(state, ui);
    ui.separator();
    saves_and_logs(state, ui);
    ui.separator();
    restart_options(state, ui);
    ui.separator();

    ui.strong("Startup Parameters");
    ui.label(
        RichText::new("Tick a parameter to pass it to the server. Hover or read the gray text for what it does.")
            .small()
            .weak(),
    );
    for group in GROUPS {
        CollapsingHeader::new(*group)
            .default_open(*group == "Performance")
            .show(ui, |ui| {
                for spec in PARAMS.iter().filter(|p| p.group == *group) {
                    param_row(ui, spec, &mut state.settings.launch);
                }
            });
    }

    ui.separator();
    ui.strong("Extra Arguments");
    ui.label(
        RichText::new("Anything not listed above, written as you would on the command line.")
            .small()
            .weak(),
    );
    ui.add(
        TextEdit::singleline(&mut state.settings.launch.extra_args)
            .hint_text("-someFlag value")
            .desired_width(f32::INFINITY),
    );

    ui.separator();
    command_preview(state, ui);
}

fn server_location(state: &mut AppState, ui: &mut Ui) {
    ui.strong("Arma Reforger Server");
    let mut path_text = state
        .settings
        .server_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let edited = ui
        .add(
            TextEdit::singleline(&mut path_text)
                .hint_text("Server folder, or the full path to the server executable")
                .desired_width(f32::INFINITY),
        )
        .changed();
    if edited {
        state.settings.server_path = if path_text.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(&path_text))
        };
    }

    let warn = ui.visuals().warn_fg_color;
    match state.server_exe() {
        Some(exe) => {
            ui.label(format!("Using {}", exe.display()));
        }
        None => {
            ui.colored_label(warn, format!("{SERVER_EXE} was not found at that location."));
        }
    }
    if ui.button("Browse for Server Folder...").clicked() {
        state.pick_server_folder();
    }
}

/// Shows where the server will keep its profile, logs and saved games, and lists the
/// save points found. The folders come from the Profile and Logs Directory parameters
/// below; "Use Standard Folders" fills those in from the server folder.
fn saves_and_logs(state: &mut AppState, ui: &mut Ui) {
    ui.strong("Saves and Logs");

    let (profile_value, profile, logs) = {
        let params = &state.settings.launch.params;
        let value = |flag: &str| {
            params
                .get(flag)
                .filter(|p| p.enabled)
                .map(|p| p.value.trim().to_string())
                .unwrap_or_default()
        };
        let profile_value = value("-profile");
        let profile = Some(PathBuf::from(&profile_value)).filter(|p| p.is_absolute());
        let logs = paths::resolve_logs(profile.as_deref(), &value("-logsDir"));
        (profile_value, profile, logs)
    };
    let save_dir = profile.as_deref().map(paths::save_dir);

    // Rescan only when the folder changes or Refresh is pressed.
    if state.ui.saves_scanned_for != save_dir {
        state.ui.saves = save_dir
            .as_deref()
            .map(paths::find_savepoints)
            .unwrap_or_default();
        state.ui.saves_scanned_for = save_dir.clone();
    }

    let profile_hint = if profile_value.is_empty() {
        "Not set. Press Use Standard Folders, or tick Profile below."
    } else {
        "Not a full path, so the save folder cannot be shown."
    };
    egui::Grid::new("saves_and_logs_paths")
        .num_columns(2)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            path_row(ui, "Profile", profile.as_deref(), profile_hint);
            path_row(ui, "Logs", logs.as_deref(), "Not set.");
            path_row(ui, "Saves (.save)", save_dir.as_deref(), "Inside the profile folder: <Profile>/profile/.save.");
        });

    ui.horizontal(|ui| {
        let has_root = state.server_root().is_some();
        if ui
            .add_enabled(has_root, Button::new("Use Standard Folders"))
            .on_hover_text(
                "Sets Profile to <server folder>/saves and Logs Directory to \
                 <server folder>/saves/logs, replacing the values below. The server keeps its \
                 data in a profile folder inside Profile.",
            )
            .clicked()
        {
            state.apply_standard_folders(true);
            state.set_status("Profile and logs folders set under the server folder", false);
        }
        if ui.button("Refresh Saved Games").clicked() {
            state.ui.saves_scanned_for = None;
        }
    });

    CollapsingHeader::new(format!("Saved Games ({})", state.ui.saves.len()))
        .id_source("saved_games")
        .show(ui, |ui| {
            if state.ui.saves.is_empty() {
                ui.weak("No save points found yet. The server writes them to the .save folder while it runs.");
                return;
            }
            ScrollArea::vertical()
                .id_source("saved_games_scroll")
                .max_height(200.0)
                .show(ui, |ui| {
                    for save in &state.ui.saves {
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "{} / {} / {}",
                                save.scenario, save.playthrough, save.name
                            ));
                            if let Some(time) = save.modified {
                                ui.weak(format_time(time));
                            }
                            if ui.small_button("Copy Path").clicked() {
                                let text = save.path.display().to_string();
                                ui.ctx().output_mut(|o| o.copied_text = text);
                            }
                        });
                    }
                });
        });
    ui.label(
        RichText::new("Tick Load Session Save under Performance to resume the latest save when the server starts.")
            .small()
            .weak(),
    );
}

fn path_row(ui: &mut Ui, label: &str, path: Option<&Path>, hint: &str) {
    ui.label(label);
    match path {
        Some(path) => {
            ui.horizontal(|ui| {
                ui.monospace(path.display().to_string());
                if !path.is_dir() {
                    ui.weak("(not created yet)");
                }
            });
        }
        None => {
            ui.weak(hint);
        }
    }
    ui.end_row();
}

fn format_time(time: SystemTime) -> String {
    chrono::DateTime::<chrono::Local>::from(time)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

fn restart_options(state: &mut AppState, ui: &mut Ui) {
    let warn = ui.visuals().warn_fg_color;
    let opts = &mut state.settings.launch;

    ui.strong("Restart Behavior");
    ui.checkbox(&mut opts.restart_on_crash, "Restart if the Server Crashes")
        .on_hover_text("Starts the server again 5 seconds after it exits with an error.");
    ui.horizontal(|ui| {
        ui.checkbox(&mut opts.auto_restart, "Restart Every Day at")
            .on_hover_text("Restarts the server once a day at this local time (24 hour, HH:MM).");
        ui.add(TextEdit::singleline(&mut opts.auto_restart_time).desired_width(60.0));
    });
    if opts.auto_restart && !valid_time(&opts.auto_restart_time) {
        ui.colored_label(warn, "Use 24 hour time, like 04:30.");
    }
}

fn valid_time(text: &str) -> bool {
    let text = text.trim();
    let Some((h, m)) = text.split_once(':') else {
        return false;
    };
    text.len() == 5
        && matches!(h.parse::<u32>(), Ok(h) if h < 24)
        && matches!(m.parse::<u32>(), Ok(m) if m < 60)
}

fn param_row(ui: &mut Ui, spec: &ParamSpec, opts: &mut LaunchOptions) {
    let st = opts
        .params
        .entry(spec.flag.to_string())
        .or_insert_with(|| ParamState {
            enabled: false,
            value: spec.default.to_string(),
        });

    ui.horizontal(|ui| {
        ui.checkbox(&mut st.enabled, spec.label);
        ui.add_enabled_ui(st.enabled, |ui| match spec.kind {
            Kind::Flag => {}
            Kind::Int { min, max } => int_editor(ui, st, min, max),
            Kind::OptInt { min, max } => {
                if st.value.is_empty() {
                    if ui.small_button("set value").clicked() {
                        st.value = spec.default.to_string();
                    }
                } else {
                    int_editor(ui, st, min, max);
                    if ui.small_button("clear").clicked() {
                        st.value.clear();
                    }
                }
            }
            Kind::Text => {
                ui.add(TextEdit::singleline(&mut st.value).desired_width(260.0));
            }
            Kind::OptText => {
                ui.add(
                    TextEdit::singleline(&mut st.value)
                        .hint_text("(optional)")
                        .desired_width(260.0),
                );
            }
            Kind::Folder => {
                ui.add(TextEdit::singleline(&mut st.value).desired_width(260.0));
                if ui.button("Browse...").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        st.value = dir.display().to_string();
                    }
                }
            }
            Kind::Choice(choices) => {
                ComboBox::from_id_source(spec.flag)
                    .selected_text(st.value.clone())
                    .show_ui(ui, |ui| {
                        for choice in choices {
                            ui.selectable_value(&mut st.value, choice.to_string(), *choice);
                        }
                    });
            }
        });
        ui.label(RichText::new(spec.flag).monospace().weak());
    });
    ui.label(RichText::new(spec.help).small().weak());
    ui.add_space(4.0);
}

fn int_editor(ui: &mut Ui, st: &mut ParamState, min: i64, max: i64) {
    let mut value: i64 = st.value.trim().parse().unwrap_or(min);
    if ui
        .add(DragValue::new(&mut value).clamp_range(min..=max))
        .changed()
    {
        st.value = value.to_string();
    } else if st.value.trim().parse::<i64>().is_err() {
        // Shows the number that will actually be used when the stored text is not one.
        st.value = value.to_string();
    }
}

fn command_preview(state: &AppState, ui: &mut Ui) {
    ui.strong("Command Line Preview");
    let config = state
        .config_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("<unsaved config>"));
    let args = build_args(&config, &state.settings.launch);
    let exe = state
        .server_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| SERVER_EXE.to_string());
    let shown: Vec<String> = args.iter().map(|a| display_arg(a)).collect();
    ui.label(RichText::new(format!("{} {}", display_arg(&exe), shown.join(" "))).monospace());
}
