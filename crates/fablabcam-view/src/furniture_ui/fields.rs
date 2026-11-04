use eframe::egui;
use fablabcam_cam::gcode::machine::Profile;
use fablabcam_studio::furniture::{Dimensions, Setup, Template};

fn number(ui: &mut egui::Ui, label: &str, value: &mut f64) {
    ui.label(label);
    ui.add(egui::DragValue::new(value).speed(1.0).suffix(" mm"));
    ui.end_row();
}
pub(super) fn dimensions(ui: &mut egui::Ui, d: &mut Dimensions) {
    ui.strong("Dimensions extérieures — millimètres");
    egui::Grid::new("furniture-dimensions")
        .striped(true)
        .show(ui, |ui| {
            number(ui, "Largeur", &mut d.width);
            number(ui, "Profondeur", &mut d.depth);
            number(ui, "Hauteur", &mut d.height);
            number(ui, "Épaisseur du panneau", &mut d.thickness);
            ui.label("Tablettes");
            ui.add_enabled(
                d.composition.is_none(),
                egui::DragValue::new(&mut d.shelves).clamp_range(0..=100),
            );
            ui.end_row();
        });
    ui.checkbox(&mut d.back, "Dos rapporté (inclus dans la profondeur)");
    ui.weak("Dessus/dessous entre les côtés. Activez Composer le meuble pour ajouter des éléments et choisir les assemblages.");
}
pub(super) fn limits(ui: &mut egui::Ui, t: &mut Template) {
    ui.collapsing("Paramètres autorisés et règles du modèle",|ui|{
        egui::Grid::new("furniture-limits").show(ui,|ui|{
            ui.strong("Paramètre");ui.strong("Minimum");ui.strong("Maximum");ui.end_row();
            for (name,r) in [("Largeur",&mut t.limits.width),("Profondeur",&mut t.limits.depth),("Hauteur",&mut t.limits.height),("Épaisseur",&mut t.limits.thickness)] {
                ui.label(name);ui.add(egui::DragValue::new(&mut r.min));ui.add(egui::DragValue::new(&mut r.max));ui.end_row();
            }
            ui.label("Tablettes");ui.label("0");ui.add(egui::DragValue::new(&mut t.limits.max_shelves).clamp_range(0..=100));ui.end_row();
        });
        ui.label("Traversants : largeur extérieure − 2 × épaisseur.");
        ui.label("Profondeur des côtés : profondeur extérieure − épaisseur du dos s'il est présent.");
        ui.label("Les tablettes divisent la hauteur intérieure en espaces égaux.");
        ui.weak("Le modèle reste indépendant de la machine : choisissez la CNC dans chaque projet de meuble.");
    });
}
pub(super) fn setup(ui: &mut egui::Ui, s: &mut Setup, machines: &[Profile]) {
    ui.label("Matériau");
    ui.text_edit_singleline(&mut s.material);
    egui::ComboBox::from_id_source("furniture-machine")
        .selected_text(&s.machine.name)
        .show_ui(ui, |ui| {
            for profile in machines {
                if ui
                    .selectable_label(profile == &s.machine, &profile.name)
                    .clicked()
                {
                    s.machine = profile.clone();
                }
            }
        });
    ui.weak("La copie du profil reste attachée à ce projet. Sélectionner à nouveau un profil actualise cette copie.");
    egui::Grid::new("furniture-machine-size")
        .striped(true)
        .show(ui, |ui| {
            number(ui, "Course utile X déclarée", &mut s.working_size[0]);
            number(ui, "Course utile Y déclarée", &mut s.working_size[1]);
            number(ui, "Largeur de dalle après pré-débit", &mut s.sheet_size[0]);
            number(ui, "Hauteur de dalle après pré-débit", &mut s.sheet_size[1]);
            number(ui, "Marge de bord / bridage", &mut s.margin);
            number(ui, "Diamètre de fraise", &mut s.cutter);
            number(ui, "Espacement entre pièces", &mut s.spacing);
        });
    ui.checkbox(&mut s.rotate, "Autoriser la rotation à 90°");
    ui.weak("Vérifiez les courses et le bridage de votre installation. Les valeurs initiales 1 500 × 800 mm sont celles de l'exemple, pas une qualification physique.");
}
