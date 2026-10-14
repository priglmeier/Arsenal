//! Mod management: the library of known mods on the left, the mods enabled on the server on the right.

use std::collections::{BTreeSet, HashSet};

use egui::{Button, Modifiers, RichText, ScrollArea, TextEdit, Ui};

use crate::config::Mod;
use crate::state::{AppState, ModEditor};
use crate::workshop;

pub fn ui(state: &mut AppState, ui: &mut Ui) {
    let list_height = (ui.ctx().screen_rect().height() * 0.45).clamp(200.0, 520.0);
    let (divider_x, top, bottom) = ui.columns(2, |cols| {
        available(state, &mut cols[0], list_height);
        enabled(state, &mut cols[1], list_height);
        // The divider sits in the middle of the gap between the two columns.
        let x = cols[0].max_rect().right() + cols[0].spacing().item_spacing.x / 2.0;
        let top = cols[0].min_rect().top();
        let bottom = cols[0].min_rect().bottom().max(cols[1].min_rect().bottom());
        (x, top, bottom)
    });
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    ui.painter()
        .vline(divider_x, egui::Rangef::new(top, bottom), stroke);
}

fn mod_label(m: &Mod) -> String {
    let name = if m.name.is_empty() { "(unnamed)" } else { m.name.as_str() };
    format!("{} | {}", name, m.version.as_deref().unwrap_or("latest"))
}

fn available(state: &mut AppState, ui: &mut Ui, height: f32) {
    ui.strong(format!("Mods Library ({})", state.settings.available_mods.len()));
    ui.add(
        TextEdit::singleline(&mut state.ui.mod_search)
            .hint_text("Search Mods...")
            .desired_width(f32::INFINITY),
    );

    let search = state.ui.mod_search.trim().to_lowercase();
    let enabled_ids: HashSet<String> = state
        .config
        .game
        .mods
        .iter()
        .map(|m| m.mod_id.to_ascii_uppercase())
        .collect();

    let mut clicked: Option<usize> = None;
    let mut double_clicked: Option<usize> = None;
    ScrollArea::vertical()
        .id_source("available_mods")
        .max_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, m) in state.settings.available_mods.iter().enumerate() {
                if !search.is_empty()
                    && !m.name.to_lowercase().contains(&search)
                    && !m.mod_id.to_lowercase().contains(&search)
                {
                    continue;
                }
                let mut text = RichText::new(mod_label(m));
                if enabled_ids.contains(&m.mod_id.to_ascii_uppercase()) {
                    text = text.weak();
                }
                let response = ui
                    .selectable_label(state.ui.sel_available == Some(i), text)
                    .on_hover_text(m.mod_id.as_str());
                if response.clicked() {
                    clicked = Some(i);
                }
                if response.double_clicked() {
                    double_clicked = Some(i);
                }
            }
        });
    if let Some(i) = clicked {
        state.ui.sel_available = Some(i);
    }
    if let Some(i) = double_clicked {
        if let Some(m) = state.settings.available_mods.get(i).cloned() {
            state.enable_mod(m);
        }
    }

    let selected = state
        .ui
        .sel_available
        .filter(|i| *i < state.settings.available_mods.len());

    ui.horizontal_wrapped(|ui| {
        if ui.button("Add...").clicked() {
            state.ui.mod_editor = Some(ModEditor {
                index: None,
                data: Mod::default(),
                version_text: String::new(),
                workshop_link: String::new(),
            });
        }
        if ui
            .button("Browse...")
            .on_hover_text(
                "Open the Reforger Workshop in your web browser. Pick a mod, copy its page \
                 address and paste it into the Add Mod window.",
            )
            .clicked()
        {
            if let Err(err) = workshop::open_in_browser(workshop::WORKSHOP_URL) {
                state.set_status(format!("Could not open the web browser: {err}"), true);
            }
            state.ui.mod_editor = Some(ModEditor {
                index: None,
                data: Mod::default(),
                version_text: String::new(),
                workshop_link: String::new(),
            });
        }
        if ui.add_enabled(selected.is_some(), Button::new("Edit...")).clicked() {
            if let Some(i) = selected {
                let data = state.settings.available_mods[i].clone();
                state.ui.mod_editor = Some(ModEditor {
                    index: Some(i),
                    version_text: data.version.clone().unwrap_or_default(),
                    data,
                    workshop_link: String::new(),
                });
            }
        }
        if ui.add_enabled(selected.is_some(), Button::new("Remove")).clicked() {
            if let Some(i) = selected {
                state.settings.available_mods.remove(i);
                state.ui.sel_available = None;
                state.persist_settings();
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(selected.is_some(), Button::new("Enable  >")).clicked() {
            if let Some(m) = selected.and_then(|i| state.settings.available_mods.get(i).cloned()) {
                state.enable_mod(m);
            }
        }
        if ui.button("Enable All  >>").clicked() {
            let all = state.settings.available_mods.clone();
            for m in all {
                state.enable_mod(m);
            }
        }
    });
}

/// Updates a selection after a click on row `index`:
/// plain click selects just that row, Ctrl (Cmd on a Mac) adds or removes it,
/// Shift selects everything from the last clicked row, and Ctrl+Shift adds that range.
fn apply_click(
    selection: &mut BTreeSet<usize>,
    anchor: &mut Option<usize>,
    index: usize,
    modifiers: Modifiers,
) {
    if modifiers.shift {
        let from = anchor.unwrap_or(index);
        let (low, high) = if from <= index { (from, index) } else { (index, from) };
        if !modifiers.command {
            selection.clear();
        }
        selection.extend(low..=high);
        // The anchor stays, so the next Shift-click measures from the same row.
        anchor.get_or_insert(index);
    } else if modifiers.command {
        if !selection.remove(&index) {
            selection.insert(index);
        }
        *anchor = Some(index);
    } else {
        selection.clear();
        selection.insert(index);
        *anchor = Some(index);
    }
}

/// Removes the rows of `selection` from `list`, highest first so the indexes stay valid.
fn remove_selected<T>(list: &mut Vec<T>, selection: &BTreeSet<usize>) {
    for &index in selection.iter().rev() {
        if index < list.len() {
            list.remove(index);
        }
    }
}

fn enabled(state: &mut AppState, ui: &mut Ui, height: f32) {
    ui.strong(format!("Mods Enabled ({})", state.config.game.mods.len()));
    // Keeps this column's list level with the search box on the left.
    let spacer = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
    ui.add_space(spacer);

    let count = state.config.game.mods.len();
    state.ui.sel_enabled.retain(|i| *i < count);

    let modifiers = ui.input(|i| i.modifiers);
    let mut clicked: Option<usize> = None;
    let mut double_clicked: Option<usize> = None;
    ScrollArea::vertical()
        .id_source("enabled_mods")
        .max_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, m) in state.config.game.mods.iter().enumerate() {
                let response = ui
                    .selectable_label(state.ui.sel_enabled.contains(&i), mod_label(m))
                    .on_hover_text(m.mod_id.as_str());
                if response.clicked() {
                    clicked = Some(i);
                }
                if response.double_clicked() {
                    double_clicked = Some(i);
                }
            }
        });
    if let Some(i) = clicked {
        apply_click(&mut state.ui.sel_enabled, &mut state.ui.sel_anchor, i, modifiers);
    }
    if let Some(i) = double_clicked {
        if i < state.config.game.mods.len() {
            state.config.game.mods.remove(i);
            state.ui.sel_enabled.clear();
            state.ui.sel_anchor = None;
        }
    }

    let count = state.config.game.mods.len();
    let selected_count = state.ui.sel_enabled.len();
    // Moving works on one mod at a time.
    let single = if selected_count == 1 {
        state.ui.sel_enabled.iter().next().copied()
    } else {
        None
    };

    ui.horizontal_wrapped(|ui| {
        let label = if selected_count > 1 {
            format!("<  Disable ({selected_count})")
        } else {
            "<  Disable".to_string()
        };
        if ui
            .add_enabled(selected_count > 0, Button::new(label))
            .on_hover_text("Click selects one mod. Ctrl+click adds or removes a mod. Shift+click selects a range.")
            .clicked()
        {
            let selection = std::mem::take(&mut state.ui.sel_enabled);
            remove_selected(&mut state.config.game.mods, &selection);
            state.ui.sel_anchor = None;
        }
        if ui.add_enabled(count > 0, Button::new("<<  Disable All")).clicked() {
            state.config.game.mods.clear();
            state.ui.sel_enabled.clear();
            state.ui.sel_anchor = None;
        }
    });
    ui.horizontal_wrapped(|ui| {
        let can_up = single.map_or(false, |i| i > 0);
        let can_down = single.map_or(false, |i| i + 1 < count);
        if ui.add_enabled(can_up, Button::new("Move Up")).clicked() {
            if let Some(i) = single {
                state.config.game.mods.swap(i, i - 1);
                state.ui.sel_enabled = BTreeSet::from([i - 1]);
                state.ui.sel_anchor = Some(i - 1);
            }
        }
        if ui.add_enabled(can_down, Button::new("Move Down")).clicked() {
            if let Some(i) = single {
                state.config.game.mods.swap(i, i + 1);
                state.ui.sel_enabled = BTreeSet::from([i + 1]);
                state.ui.sel_anchor = Some(i + 1);
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui.button("Import...").clicked() {
            state.import_mods_dialog();
        }
        if ui.add_enabled(count > 0, Button::new("Export...")).clicked() {
            state.export_mods_dialog();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain() -> Modifiers {
        Modifiers::NONE
    }
    fn ctrl() -> Modifiers {
        Modifiers::COMMAND
    }
    fn shift() -> Modifiers {
        Modifiers::SHIFT
    }

    #[test]
    fn click_ctrl_and_shift_select_like_a_file_list() {
        let mut sel = BTreeSet::new();
        let mut anchor = None;

        apply_click(&mut sel, &mut anchor, 2, plain());
        assert_eq!(sel, BTreeSet::from([2]));

        // Shift-click 5 selects 2 to 5.
        apply_click(&mut sel, &mut anchor, 5, shift());
        assert_eq!(sel, BTreeSet::from([2, 3, 4, 5]));

        // Another Shift-click measures from the same anchor and replaces the range.
        apply_click(&mut sel, &mut anchor, 0, shift());
        assert_eq!(sel, BTreeSet::from([0, 1, 2]));

        // Ctrl-click adds a separate mod, and again removes it.
        apply_click(&mut sel, &mut anchor, 8, ctrl());
        assert_eq!(sel, BTreeSet::from([0, 1, 2, 8]));
        apply_click(&mut sel, &mut anchor, 1, ctrl());
        assert_eq!(sel, BTreeSet::from([0, 2, 8]));

        // A plain click starts over.
        apply_click(&mut sel, &mut anchor, 4, plain());
        assert_eq!(sel, BTreeSet::from([4]));
    }

    #[test]
    fn shift_click_without_an_anchor_selects_one_row() {
        let mut sel = BTreeSet::new();
        let mut anchor = None;
        apply_click(&mut sel, &mut anchor, 3, shift());
        assert_eq!(sel, BTreeSet::from([3]));
    }

    #[test]
    fn removing_a_selection_keeps_the_other_rows_in_order() {
        let mut list = vec!['a', 'b', 'c', 'd', 'e', 'f'];
        remove_selected(&mut list, &BTreeSet::from([1, 2, 4]));
        assert_eq!(list, ['a', 'd', 'f']);
    }
}
