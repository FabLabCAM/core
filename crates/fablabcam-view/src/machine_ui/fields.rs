mod axes;
mod codes;
mod general;
use super::Section;
use eframe::egui;
use fablabcam_cam::gcode::machine::Profile;
use fablabcam_studio::machine_profiles::{configuration, Draft};

pub(super) fn show(
    ui: &mut egui::Ui,
    draft: &mut Draft,
    section: Section,
) -> Option<Result<(), String>> {
    let existing = draft.baseline().is_some();
    let p = &mut draft.profile;
    match section {
        Section::General => general::show(ui, p, existing),
        Section::Axes => axes::show(ui, p),
        Section::Spindle => spindle(ui, p),
        Section::Codes => codes::show(ui, p),
        Section::Review => review(ui, p),
        Section::Json => return json(ui, draft),
    }
    None
}

fn text(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
    ui.add_space(8.);
}
fn optional_text(ui: &mut egui::Ui, label: &str, value: &mut Option<String>) {
    let mut s = value.clone().unwrap_or_default();
    ui.label(label);
    if ui
        .add(egui::TextEdit::singleline(&mut s).desired_width(f32::INFINITY))
        .changed()
    {
        *value = (!s.trim().is_empty()).then_some(s);
    }
    ui.add_space(8.);
}
fn select(ui: &mut egui::Ui, label: &str, value: &mut String, choices: &[&str]) {
    egui::ComboBox::from_label(label)
        .selected_text(value.as_str())
        .show_ui(ui, |ui| {
            for choice in choices {
                ui.selectable_value(value, (*choice).into(), *choice);
            }
        });
    ui.add_space(6.);
}
/// Unchecked is None, never an invented physical limit. The number appears only
/// after the user explicitly elects to provide one.
fn optional_number(ui: &mut egui::Ui, value: &mut Option<f64>, id: impl std::hash::Hash) {
    ui.push_id(id, |ui| {
        ui.horizontal(|ui| {
            let mut known = value.is_some();
            if ui
                .checkbox(&mut known, "")
                .on_hover_text("Renseigner cette valeur ; décocher pour laisser inconnu")
                .changed()
            {
                *value = known.then_some(0.);
            }
            if let Some(v) = value {
                ui.add(egui::DragValue::new(v).speed(0.1).max_decimals(6));
            } else {
                ui.weak("Non renseigné");
            }
        })
    });
}
fn number_row(ui: &mut egui::Ui, label: &str, value: &mut Option<f64>) {
    ui.horizontal(|ui| {
        ui.label(label);
        optional_number(ui, value, label);
    });
}
fn list(ui: &mut egui::Ui, label: &str, values: &mut Vec<String>) {
    // Preserve separators while typing: reconstructing from split_whitespace
    // every frame would erase a trailing space before the next code is typed.
    let id = egui::Id::new(("machine_list_text", label));
    let previous = ui.data_mut(|data| data.get_temp::<(Vec<String>, String)>(id));
    let mut text = match previous {
        Some((model, buffer)) if model == *values => buffer,
        _ => values.join(" "),
    };
    ui.label(label);
    if ui
        .add(
            egui::TextEdit::singleline(&mut text)
                .id(id.with("editor"))
                .desired_width(f32::INFINITY),
        )
        .changed()
    {
        *values = text.split_whitespace().map(str::to_owned).collect();
    }
    ui.data_mut(|data| data.insert_temp(id, (values.clone(), text)));
    ui.add_space(7.);
}

fn choices(ui: &mut egui::Ui, values: &mut Vec<String>, options: &[(&str, &str)]) {
    ui.horizontal_wrapped(|ui| {
        for (key, label) in options {
            let mut checked = values.iter().any(|v| v == key);
            if ui.checkbox(&mut checked, *label).changed() {
                values.retain(|v| v != key);
                if checked {
                    values.push((*key).into());
                }
            }
        }
    });
}
fn spindle(ui: &mut egui::Ui, p: &mut Profile) {
    ui.heading("Broche et outils");
    ui.add_space(12.);
    number_row(ui, "Minimum (tr/min)", &mut p.spindle.min_rpm);
    number_row(ui, "Maximum (tr/min)", &mut p.spindle.max_rpm);
    choices(
        ui,
        &mut p.spindle.directions,
        &[("cw", "Horaire · M3"), ("ccw", "Antihoraire · M4")],
    );
    ui.add_space(12.);
    ui.separator();
    text(
        ui,
        "Type de changeur d'outil (métadonnée)",
        &mut p.tools.changer,
    );
    text(
        ui,
        "Comportement du changement d'outil M6",
        &mut p.tools.m6_behavior,
    );
    ui.checkbox(
        &mut p.tools.m6_preserves_state,
        "M6 conserve position, origines, modes, avance et broche",
    );
    ui.small("À cocher uniquement après vérification du comportement réel des macros M6.");
    ui.add_space(10.);
    ui.label("Arrosage autorisé");
    choices(
        ui,
        &mut p.tools.coolant,
        &[("M7", "M7"), ("M8", "M8"), ("M9", "M9")],
    );
    ui.add_space(10.);
    ui.small(format!(
        "{} outil(s) dans la table. Table et positions du changeur : JSON avancé.",
        p.tools.table.len()
    ));
}
fn review(ui: &mut egui::Ui, p: &Profile) {
    let check = configuration(p);
    ui.heading(check.label());
    ui.add_space(8.);
    ui.label(format!("{} · {}", p.name, p.controller.name));
    ui.monospace(&p.id);
    ui.add_space(12.);
    if let Some(error) = check.error {
        ui.colored_label(egui::Color32::LIGHT_RED, error);
    }
    if !check.missing.is_empty() {
        ui.label("Champs restant à renseigner :");
        for field in &check.missing {
            ui.label(format!("• {field}"));
        }
        ui.add_space(8.);
        ui.small("Vous pouvez enregistrer ce profil partiel et le compléter plus tard.");
    }
    for caution in check.cautions {
        ui.colored_label(egui::Color32::from_rgb(240, 180, 100), caution);
    }
    ui.add_space(14.);
    ui.separator();
    ui.label("Ces contrôles portent sur la configuration. Ils ne certifient pas la machine ni un programme d'usinage.");
    ui.small("La validation d'un fichier G-code reste disponible dans le CLI. Aucune connexion à la machine n'est effectuée.");
}
fn json(ui: &mut egui::Ui, draft: &mut Draft) -> Option<Result<(), String>> {
    ui.heading("Profil JSON complet");
    ui.small("Tous les paramètres sont conservés. Appliquez le JSON pour valider et retrouver les formulaires.");
    let mut result = None;
    ui.horizontal(|ui| {
        if ui.button("Appliquer le JSON").clicked() {
            result = Some(draft.apply_json());
        }
        if ui.button("Annuler les retouches JSON").clicked() {
            result = Some(draft.reset_json());
        }
    });
    ui.add(
        egui::TextEdit::multiline(&mut draft.json)
            .code_editor()
            .desired_rows(28)
            .desired_width(f32::INFINITY),
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_list_preserves_space_between_separate_input_events() {
        let ctx = egui::Context::default();
        let mut values = vec!["G21".to_string()];
        let id = egui::Id::new(("machine_list_text", "test preamble"));
        let frame = |values: &mut Vec<String>, events| {
            let _ = ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| list(ui, "test preamble", values));
                },
            );
        };
        frame(&mut values, vec![]);
        ctx.memory_mut(|memory| memory.request_focus(id.with("editor")));
        frame(
            &mut values,
            vec![
                egui::Event::Key {
                    key: egui::Key::End,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                },
                egui::Event::Text(" ".into()),
            ],
        );
        assert_eq!(
            ctx.data_mut(|data| data.get_temp::<(Vec<String>, String)>(id))
                .unwrap()
                .1,
            "G21 "
        );
        frame(&mut values, vec![egui::Event::Text("G90".into())]);
        assert_eq!(values, ["G21", "G90"]);
    }
}
