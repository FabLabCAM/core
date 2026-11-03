use super::{optional_text, select, text};
use eframe::egui;
use fablabcam_cam::gcode::machine::{DialectSemantics, Profile};

pub(super) fn show(ui: &mut egui::Ui, p: &mut Profile, existing: bool) {
    ui.heading("Identité de la machine");
    ui.add_space(12.);
    text(ui, "Nom du profil", &mut p.name);
    ui.add_enabled_ui(!existing, |ui| {
        text(
            ui,
            "Identifiant (lettres ASCII, chiffres, - ou _)",
            &mut p.id,
        )
    });
    if existing {
        ui.small("Identifiant conservé. Utilisez Dupliquer pour créer une autre machine.");
    }
    optional_text(ui, "Emplacement", &mut p.location);
    text(ui, "Contrôleur", &mut p.controller.name);
    optional_text(ui, "Version du contrôleur", &mut p.controller.version);
    egui::ComboBox::from_label("Interprétation du G-code")
        .selected_text(match p.semantics {
            DialectSemantics::Mach3Mill => "Mach3 · fraisage",
            DialectSemantics::Rs274Mill => "RS274 · fraisage",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut p.semantics,
                DialectSemantics::Mach3Mill,
                "Mach3 · fraisage",
            );
            ui.selectable_value(
                &mut p.semantics,
                DialectSemantics::Rs274Mill,
                "RS274 · fraisage",
            );
        });
    ui.small("Le nom du contrôleur ne détermine pas l'interprétation des codes.");
    ui.add_space(10.);
    select(
        ui,
        "Unités par défaut",
        &mut p.defaults.units,
        &["G21", "G20"],
    );
    ui.small("G21 : millimètres ; G20 : pouces. Les limites et positions du profil restent en mm.");
    ui.add_space(10.);
    ui.label("Description");
    ui.add(
        egui::TextEdit::multiline(&mut p.description)
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );
}
