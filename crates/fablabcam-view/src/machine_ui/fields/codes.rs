use super::{choices, list, select, text};
use eframe::egui;
use fablabcam_cam::gcode::machine::{LineNumbers, Profile};

pub(super) fn show(ui: &mut egui::Ui, p: &mut Profile) {
    ui.heading("G-code et conventions");
    ui.add_space(12.);
    select(ui, "Plan", &mut p.defaults.plane, &["G17", "G18", "G19"]);
    select(ui, "Distances", &mut p.defaults.distance, &["G90", "G91"]);
    select(
        ui,
        "Centres d'arcs",
        &mut p.defaults.arc_distance,
        &["G90.1", "G91.1"],
    );
    select(
        ui,
        "Mode d'avance",
        &mut p.defaults.feed_mode,
        &["G93", "G94", "G95"],
    );
    select(
        ui,
        "Origine par défaut",
        &mut p.defaults.work_offset,
        &["G54", "G55", "G56", "G57", "G58", "G59"],
    );
    list(
        ui,
        "Préambule requis (codes séparés par des espaces)",
        &mut p.safety.required_preamble,
    );
    ui.small("Le préambule est explicite : le choix d'unités ne le réécrit pas automatiquement.");
    ui.add_space(10.);
    egui::CollapsingHeader::new("Codes autorisés et interdits").show(ui,|ui|{
        list(ui,"G autorisés",&mut p.gcodes.allow);list(ui,"G interdits",&mut p.gcodes.deny);
        list(ui,"M autorisés",&mut p.mcodes.allow);list(ui,"M interdits",&mut p.mcodes.deny);
        ui.small("Forme canonique : G1, M3… Autoriser un code ne garantit pas son analyse par le moteur.");
    });
    egui::CollapsingHeader::new("Format des fichiers").show(ui, |ui| {
        egui::ComboBox::from_label("Numéros de ligne")
            .selected_text(match p.format.line_numbers {
                LineNumbers::Optional => "Facultatifs",
                LineNumbers::Required => "Requis",
                LineNumbers::Forbidden => "Interdits",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut p.format.line_numbers,
                    LineNumbers::Optional,
                    "Facultatifs",
                );
                ui.selectable_value(&mut p.format.line_numbers, LineNumbers::Required, "Requis");
                ui.selectable_value(
                    &mut p.format.line_numbers,
                    LineNumbers::Forbidden,
                    "Interdits",
                );
            });
        ui.horizontal(|ui| {
            ui.label("Décimales maximum");
            ui.add(egui::DragValue::new(&mut p.format.max_decimals).clamp_range(0..=12));
        });
        ui.horizontal(|ui| {
            ui.label("Longueur maximum d'une ligne");
            ui.add(egui::DragValue::new(&mut p.format.max_line_length).clamp_range(1..=1_000_000));
        });
        ui.checkbox(&mut p.format.ascii_only, "ASCII uniquement");
        ui.checkbox(&mut p.format.uppercase, "Majuscules requises");
        ui.label("Commentaires autorisés");
        choices(
            ui,
            &mut p.format.comments,
            &[("()", "( )"), ("//", "//"), ("%", "%"), (";", ";")],
        );
        let mut header = p.format.header.is_some();
        if ui.checkbox(&mut header, "En-tête % requis").changed() {
            p.format.header = header.then(|| "%".into());
        }
        list(ui, "Codes de fin (M2 / M30)", &mut p.format.end_codes);
    });
    egui::CollapsingHeader::new("Métadonnées du contrôleur").show(ui, |ui| {
        text(ui, "Dialecte déclaré", &mut p.controller.dialect);
        text(
            ui,
            "Post-processeur déclaré",
            &mut p.controller.postprocessor,
        );
        text(ui, "Type de machine", &mut p.machine_kind);
        list(
            ui,
            "Sources (URLs séparées par des espaces)",
            &mut p.sources,
        );
        ui.small(
            "Le post-processeur déclaré est une métadonnée : il ne change pas le .cbpp du dessin.",
        );
    });
}
