use super::{number_row, optional_number, text};
use eframe::egui;
use fablabcam_cam::gcode::machine::Profile;

pub(super) fn show(ui: &mut egui::Ui, p: &mut Profile) {
    ui.heading("Courses de la machine");
    ui.small("Coordonnées machine : mm pour XYZ, degrés pour les axes rotatifs. Une case décochée laisse la valeur inconnue.");
    ui.add_space(10.);
    egui::Grid::new("machine_axes")
        .num_columns(4)
        .spacing([16., 8.])
        .striped(true)
        .show(ui, |ui| {
            ui.strong("Axe");
            ui.strong("Minimum");
            ui.strong("Maximum");
            ui.strong("Avance max / min");
            ui.end_row();
            for (a, axis) in &mut p.axes {
                ui.strong(a.to_string());
                optional_number(ui, &mut axis.min, (*a, "min"));
                optional_number(ui, &mut axis.max, (*a, "max"));
                optional_number(ui, &mut axis.max_feed, (*a, "feed"));
                ui.end_row();
            }
        });
    ui.add_space(16.);
    ui.separator();
    ui.heading("Vitesses et dégagement");
    number_row(ui, "Avance globale (mm/min)", &mut p.max_feed_mm_min);
    number_row(ui, "Vitesse rapide (mm/min)", &mut p.rapid_mm_min);
    number_row(ui, "Z de sécurité (mm)", &mut p.safety.safe_z_mm);
    ui.checkbox(
        &mut p.safety.forbid_rapid_below_safe_z,
        "Refuser les déplacements rapides sous le Z de sécurité",
    );
    ui.checkbox(
        &mut p.safety.allow_helical_arcs,
        "Autoriser les arcs hélicoïdaux",
    );
    ui.add_space(12.);
    egui::CollapsingHeader::new("Position initiale et origines pièce (mm)")
        .default_open(true)
        .show(ui, |ui| {
            point(ui, "Position initiale machine", &mut p.initial_position_mm);
            ui.small("Renseignez seulement une position initiale effectivement connue.");
            for key in ["G54", "G55", "G56", "G57", "G58", "G59"] {
                let mut value = p.work_offsets_mm.get(key).copied();
                point(ui, key, &mut value);
                match value {
                    Some(v) => {
                        p.work_offsets_mm.insert(key.into(), v);
                    }
                    None => {
                        p.work_offsets_mm.remove(key);
                    }
                }
            }
            ui.small("Origine pièce : décalage par rapport à l'origine machine.");
        });
    egui::CollapsingHeader::new("Métadonnées de positionnement").show(ui, |ui| {
        text(
            ui,
            "Convention des directions positives",
            &mut p.positive_directions,
        );
        number_row(
            ui,
            "Accélération (mm/s², non simulée)",
            &mut p.acceleration_mm_s2,
        );
        point(ui, "Position de parc", &mut p.safety.park_position_mm);
        point(
            ui,
            "Position de changement d'outil",
            &mut p.safety.tool_change_position_mm,
        );
        ui.small("Ces métadonnées ne déclenchent aucun déplacement.");
    });
}
fn point(ui: &mut egui::Ui, label: &str, value: &mut Option<[f64; 3]>) {
    ui.push_id(label, |ui| {
        ui.horizontal_wrapped(|ui| {
            let mut known = value.is_some();
            if ui.checkbox(&mut known, label).changed() {
                *value = known.then_some([0.; 3]);
            }
            if let Some(p) = value {
                for (axis, v) in ['X', 'Y', 'Z'].into_iter().zip(p) {
                    ui.label(axis.to_string());
                    ui.add(egui::DragValue::new(v).speed(0.1).max_decimals(6));
                }
            } else {
                ui.weak("Non renseignée");
            }
        })
    });
}
