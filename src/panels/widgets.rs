//! Small building blocks shared by the settings panels. The `*_row` helpers draw one
//! row of a two column `egui::Grid` (label on the left, editor on the right).

use egui::{DragValue, Id, Key, TextEdit, Ui};

fn label(ui: &mut Ui, text: &str, help: &str) {
    let response = ui.label(text);
    if !help.is_empty() {
        response.on_hover_text(help);
    }
}

pub fn text_row(ui: &mut Ui, text: &str, help: &str, value: &mut String) {
    label(ui, text, help);
    ui.add(TextEdit::singleline(value).desired_width(f32::INFINITY));
    ui.end_row();
}

pub fn secret_row(ui: &mut Ui, text: &str, help: &str, value: &mut String, show: bool) {
    label(ui, text, help);
    ui.add(
        TextEdit::singleline(value)
            .password(!show)
            .desired_width(f32::INFINITY),
    );
    ui.end_row();
}

pub fn port_row(ui: &mut Ui, text: &str, help: &str, value: &mut u16) {
    label(ui, text, help);
    ui.add(DragValue::new(value).clamp_range(0..=65535));
    ui.end_row();
}

pub fn int_row<T: egui::emath::Numeric>(
    ui: &mut Ui,
    text: &str,
    help: &str,
    value: &mut T,
    min: T,
    max: T,
) {
    label(ui, text, help);
    ui.add(DragValue::new(value).clamp_range(min..=max));
    ui.end_row();
}

pub fn bool_row(ui: &mut Ui, text: &str, help: &str, value: &mut bool) {
    label(ui, text, help);
    ui.checkbox(value, "");
    ui.end_row();
}

/// A setting that may be absent from the config. The checkbox adds or removes it;
/// while it is present `edit` draws its editor.
pub fn opt_row<T>(
    ui: &mut Ui,
    text: &str,
    help: &str,
    value: &mut Option<T>,
    default: T,
    edit: impl FnOnce(&mut Ui, &mut T),
) {
    label(ui, text, help);
    ui.horizontal(|ui| {
        let mut on = value.is_some();
        if ui.checkbox(&mut on, "").changed() {
            *value = if on { Some(default) } else { None };
        }
        match value.as_mut() {
            Some(v) => edit(ui, v),
            None => {
                ui.weak("not set (server default)");
            }
        }
    });
    ui.end_row();
}

/// An editable list of strings with a remove button per entry and an "add" box at the end.
pub fn string_list(ui: &mut Ui, id: &str, list: &mut Vec<String>, add_hint: &str) {
    let mut remove = None;
    for (i, item) in list.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(TextEdit::singleline(item).desired_width(280.0));
            if ui.small_button("Remove").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        list.remove(i);
    }

    let key = Id::new(("arsenal_list_new_entry", id));
    let mut text: String = ui.data_mut(|d| d.get_temp::<String>(key).unwrap_or_default());
    ui.horizontal(|ui| {
        let response = ui.add(
            TextEdit::singleline(&mut text)
                .hint_text(add_hint)
                .desired_width(280.0),
        );
        let enter = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if (ui.button("Add").clicked() || enter) && !text.trim().is_empty() {
            list.push(text.trim().to_string());
            text.clear();
        }
    });
    ui.data_mut(|d| d.insert_temp(key, text));
}

/// A checkbox that puts `name` in, or takes it out of, `list`.
pub fn toggle_in_list(ui: &mut Ui, list: &mut Vec<String>, name: &str) {
    let mut on = list.iter().any(|x| x == name);
    if ui.checkbox(&mut on, name).changed() {
        if on {
            list.push(name.to_string());
        } else {
            list.retain(|x| x != name);
        }
    }
}
