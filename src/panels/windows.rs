//! Floating dialogs: unsaved-changes prompt, mod editor, scenario picker,
//! mission header editor and the About box.

use egui::{Align2, Context, Grid, ScrollArea, TextEdit, Window};
use serde_json::{Map, Value};

use crate::config::validate::{is_mod_id, looks_like_scenario_id};
use crate::state::AppState;
use crate::theme::ok_color;
use crate::workshop;

/// Draws every dialog that is currently open.
pub fn show_all(state: &mut AppState, ctx: &Context) {
    confirm(state, ctx);
    mod_editor(state, ctx);
    scenarios(state, ctx);
    mission_header(state, ctx);
    about(state, ctx);
    license(state, ctx);
}

// ----- unsaved changes -----

enum Choice {
    Save,
    Discard,
    Cancel,
}

fn confirm(state: &mut AppState, ctx: &Context) {
    let Some(action) = state.confirm else {
        return;
    };
    let mut choice: Option<Choice> = None;
    Window::new("Unsaved Changes")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("The server config has changes that have not been saved.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    choice = Some(Choice::Save);
                }
                if ui.button("Discard Changes").clicked() {
                    choice = Some(Choice::Discard);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(Choice::Cancel);
                }
            });
        });
    match choice {
        // If saving fails or the save dialog is canceled, the prompt stays open.
        Some(Choice::Save) => {
            if state.save() {
                state.run_action(action);
            }
        }
        Some(Choice::Discard) => state.run_action(action),
        Some(Choice::Cancel) => state.confirm = None,
        None => {}
    }
}

// ----- mod editor -----

fn mod_editor(state: &mut AppState, ctx: &Context) {
    let Some(mut editor) = state.ui.mod_editor.take() else {
        return;
    };
    let title = if editor.index.is_some() { "Edit Mod" } else { "Add Mod" };
    let mut open = true;
    let mut save = false;
    let mut cancel = false;
    let mut browse = false;

    Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("Workshop Link").on_hover_text(
                "The address of a mod's page on the Reforger Workshop. \
                 The mod ID and name are filled in from it.",
            );
            ui.horizontal(|ui| {
                let response = ui.add(
                    TextEdit::singleline(&mut editor.workshop_link)
                        .hint_text("Paste a Workshop page address...")
                        .desired_width(300.0),
                );
                if response.changed() {
                    if let Some(found) = workshop::parse_link(&editor.workshop_link) {
                        editor.data.mod_id = found.mod_id;
                        if !found.name.is_empty() {
                            editor.data.name = found.name;
                        }
                    }
                }
                if ui
                    .button("Browse Workshop...")
                    .on_hover_text("Open the Reforger Workshop in your web browser.")
                    .clicked()
                {
                    browse = true;
                }
            });
            let link = editor.workshop_link.trim();
            if !link.is_empty() {
                if workshop::parse_link(link).is_some() {
                    let color = ok_color(ui.visuals().dark_mode);
                    ui.colored_label(color, "Mod ID and name filled in from the link.");
                } else {
                    let color = ui.visuals().warn_fg_color;
                    ui.colored_label(
                        color,
                        "That does not look like a mod page address (.../workshop/<ID>-<Name>).",
                    );
                }
            }
            ui.add_space(6.0);

            Grid::new("mod_editor_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                ui.label("Mod ID").on_hover_text("16 hexadecimal characters, from the Workshop page.");
                ui.add(TextEdit::singleline(&mut editor.data.mod_id).desired_width(300.0));
                ui.end_row();

                ui.label("Name");
                ui.add(TextEdit::singleline(&mut editor.data.name).desired_width(300.0));
                ui.end_row();

                ui.label("Version")
                    .on_hover_text("Leave empty to always use the latest version.");
                ui.add(
                    TextEdit::singleline(&mut editor.version_text)
                        .hint_text("latest")
                        .desired_width(300.0),
                );
                ui.end_row();
            });

            let id_ok = is_mod_id(editor.data.mod_id.trim());
            if !id_ok {
                let color = ui.visuals().warn_fg_color;
                ui.colored_label(color, "The mod ID must be 16 hexadecimal characters.");
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(id_ok, egui::Button::new("OK")).clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });

    if browse {
        if let Err(err) = workshop::open_in_browser(workshop::WORKSHOP_URL) {
            state.set_status(format!("Could not open the web browser: {err}"), true);
        }
    }

    if save {
        let mut data = editor.data.clone();
        data.mod_id = data.mod_id.trim().to_ascii_uppercase();
        data.name = data.name.trim().to_string();
        let version = editor.version_text.trim();
        data.version = if version.is_empty() { None } else { Some(version.to_string()) };

        match editor.index {
            Some(i) if i < state.settings.available_mods.len() => {
                state.settings.available_mods[i] = data;
            }
            _ => {
                let known = state
                    .settings
                    .available_mods
                    .iter()
                    .any(|m| m.mod_id.eq_ignore_ascii_case(&data.mod_id));
                if known {
                    state.set_status(format!("{} is already in the mod library", data.mod_id), true);
                } else {
                    let label = if data.name.is_empty() { data.mod_id.clone() } else { data.name.clone() };
                    state.remember_mod(&data);
                    state.set_status(format!("Added {label} to the mod library"), false);
                }
            }
        }
        state.persist_settings();
    } else if open && !cancel {
        // Still open: keep the form for the next frame.
        state.ui.mod_editor = Some(editor);
    }
}

// ----- scenario picker -----

fn scenarios(state: &mut AppState, ctx: &Context) {
    if !state.ui.show_scenarios {
        return;
    }
    let mut open = true;
    let mut chosen: Option<String> = None;
    let mut reload = false;
    let mut close = false;

    let filter = state.ui.scenario_filter.trim().to_lowercase();
    let list: Vec<String> = state
        .settings
        .scenarios
        .iter()
        .filter(|s| filter.is_empty() || s.to_lowercase().contains(&filter))
        .cloned()
        .collect();
    let total = state.settings.scenarios.len();
    let loading = state.ui.scenarios_loading;

    Window::new("Select Scenario")
        .open(&mut open)
        .collapsible(false)
        .default_width(580.0)
        .default_height(440.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Filter");
                ui.add(
                    TextEdit::singleline(&mut state.ui.scenario_filter)
                        .hint_text("Search Scenarios...")
                        .desired_width(260.0),
                );
                if loading {
                    ui.spinner();
                    ui.label("Looking up scenarios...");
                } else if ui
                    .button("Reload Scenarios")
                    .on_hover_text("Lists the base-game scenarios from the server, plus the scenarios the Workshop shows for each enabled mod (needs an internet connection).")
                    .clicked()
                {
                    reload = true;
                }
            });

            if total == 0 && !loading {
                ui.weak("No scenarios are known yet. Set the server folder and press Reload Scenarios, or type an ID below.");
            } else {
                ui.weak(format!("Showing {} of {}", list.len(), total));
            }

            ScrollArea::vertical()
                .id_source("scenario_list")
                .max_height(280.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for id in &list {
                        let selected = state.ui.scenario_selected.as_deref() == Some(id.as_str());
                        let response = ui.selectable_label(selected, id.as_str());
                        if response.clicked() {
                            state.ui.scenario_selected = Some(id.clone());
                        }
                        if response.double_clicked() {
                            chosen = Some(id.clone());
                        }
                    }
                });

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Or Type an ID");
                ui.add(
                    TextEdit::singleline(&mut state.ui.scenario_manual)
                        .hint_text("{ECC61978EDCC2B5A}Missions/23_Campaign.conf")
                        .desired_width(f32::INFINITY),
                );
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let has_selection = state.ui.scenario_selected.is_some();
                if ui
                    .add_enabled(has_selection, egui::Button::new("Use Selected"))
                    .clicked()
                {
                    chosen = state.ui.scenario_selected.clone();
                }
                let typed_ok = looks_like_scenario_id(&state.ui.scenario_manual);
                if ui
                    .add_enabled(typed_ok, egui::Button::new("Use Typed ID"))
                    .clicked()
                {
                    chosen = Some(state.ui.scenario_manual.trim().to_string());
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

    if reload {
        state.refresh_scenarios();
    }
    if let Some(id) = chosen {
        state.config.game.scenario_id = id;
        close = true;
    }
    if close || !open {
        state.ui.show_scenarios = false;
    }
}

// ----- mission header -----

/// Parses the mission header editor text. Empty text and `{}` both mean "no overrides".
fn parse_header(text: &str) -> Result<Option<Map<String, Value>>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) if map.is_empty() => Ok(None),
        Ok(Value::Object(map)) => Ok(Some(map)),
        Ok(_) => Err("The mission header must be a JSON object, like { \"m_iPlayerCount\": 40 }".to_string()),
        Err(err) => Err(format!("Invalid JSON: {err}")),
    }
}

fn mission_header(state: &mut AppState, ctx: &Context) {
    if !state.ui.show_mission_header {
        return;
    }
    let mut open = true;
    let mut apply = false;
    let mut cancel = false;

    Window::new("Mission Header Overrides")
        .open(&mut open)
        .collapsible(false)
        .default_width(540.0)
        .show(ctx, |ui| {
            ui.label(
                "A JSON object whose values override the scenario's mission header, \
                 for example { \"m_iPlayerCount\": 40 }. Leave empty for no overrides.",
            );
            ui.add_space(4.0);
            ScrollArea::vertical()
                .id_source("mission_header_scroll")
                .max_height(300.0)
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut state.ui.mission_header_text)
                            .code_editor()
                            .desired_rows(12)
                            .desired_width(f32::INFINITY),
                    );
                });
            if let Some(err) = &state.ui.mission_header_error {
                let color = ui.visuals().error_fg_color;
                ui.colored_label(color, err.as_str());
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    apply = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });

    if apply {
        match parse_header(&state.ui.mission_header_text) {
            Ok(header) => {
                state.config.game.game_properties.mission_header = header;
                state.ui.mission_header_error = None;
                state.ui.show_mission_header = false;
            }
            Err(message) => state.ui.mission_header_error = Some(message),
        }
    } else if cancel || !open {
        state.ui.show_mission_header = false;
    }
}

// ----- about -----

/// The picture in the About window: 200 x 200 pixels of raw RGB data (see `assets/about.rgb`).
const ABOUT_IMAGE: &[u8] = include_bytes!("../../assets/about.rgb");
const ABOUT_IMAGE_SIZE: usize = 200;
/// How large the picture is drawn, in points.
const ABOUT_IMAGE_SHOWN: f32 = 96.0;

/// The text of the LICENSE file, built into the program.
const LICENSE_TEXT: &str = include_str!("../../LICENSE");

fn license(state: &mut AppState, ctx: &Context) {
    if !state.ui.show_license {
        return;
    }
    let mut open = true;
    Window::new("License")
        .open(&mut open)
        .collapsible(false)
        .default_width(520.0)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(
                "Arsenal is an unofficial tool and is not affiliated with Bohemia Interactive. \
                 Use it at your own risk.",
            );
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.monospace(LICENSE_TEXT);
                });
            ui.add_space(6.0);
            ui.weak(
                "Arsenal uses open-source libraries (egui, eframe, serde, sysinfo and others) \
                 under their own licenses, mostly MIT or Apache-2.0.",
            );
        });
    if !open {
        state.ui.show_license = false;
    }
}

fn about(state: &mut AppState, ctx: &Context) {
    if !state.ui.show_about {
        return;
    }
    // Loaded once, the first time the window is opened.
    let picture = state
        .ui
        .about_image
        .get_or_insert_with(|| {
            let image =
                egui::ColorImage::from_rgb([ABOUT_IMAGE_SIZE, ABOUT_IMAGE_SIZE], ABOUT_IMAGE);
            ctx.load_texture("about_image", image, egui::TextureOptions::LINEAR)
        })
        .clone();

    let mut open = true;
    Window::new("About Arsenal")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(260.0);
                    ui.heading("Arsenal");
                    ui.label("ARma Server ENvironment App Launcher");
                    ui.label("Created by Priglmeier 2026");
                    ui.horizontal(|ui| {
                        ui.weak(format!("Version {}", env!("CARGO_PKG_VERSION")));
                        ui.weak(crate::build_info::BUILD_EPOCH.to_string())
                            .on_hover_text("Build timestamp (Unix epoch, in seconds)");
                    });
                });
                ui.add_space(12.0);
                ui.image((picture.id(), egui::vec2(ABOUT_IMAGE_SHOWN, ABOUT_IMAGE_SHOWN)));
            });
            ui.add_space(6.0);
            if let Some(path) = crate::settings::settings_path() {
                ui.label("Launcher settings are stored in:");
                ui.monospace(path.display().to_string());
            }
        });
    if !open {
        state.ui.show_about = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_header_means_none() {
        assert_eq!(parse_header("").unwrap(), None);
        assert_eq!(parse_header("   \n").unwrap(), None);
        assert_eq!(parse_header("{}").unwrap(), None);
    }

    #[test]
    fn object_header_is_parsed() {
        let map = parse_header(r#"{ "m_iPlayerCount": 40, "other": "values" }"#)
            .unwrap()
            .unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map["m_iPlayerCount"], 40);
    }

    #[test]
    fn non_objects_and_bad_json_are_rejected() {
        assert!(parse_header("[1, 2]").is_err());
        assert!(parse_header("{ nope").is_err());
    }
}
