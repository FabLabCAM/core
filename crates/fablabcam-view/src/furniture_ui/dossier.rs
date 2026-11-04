use super::FurnitureUi;
use eframe::egui;
use fablabcam_studio::furniture::{cam_project, fabrication_files, place};

impl FurnitureUi {
    pub(super) fn layout_panel(
        &mut self,
        ui: &mut egui::Ui,
        f: &fablabcam_studio::furniture::Furniture,
    ) {
        match place(f) {
            Ok(layout) => {
                self.sheet = self.sheet.min(layout.sheets.len() - 1);
                ui.horizontal_wrapped(|ui| {
                    for i in 0..layout.sheets.len() {
                        ui.selectable_value(&mut self.sheet, i, format!("Dalle {}", i + 1));
                    }
                });
                let sheet = &layout.sheets[self.sheet];
                ui.label(format!(
                    "{} dalles · occupation de cette dalle : {:.1} %",
                    layout.sheets.len(),
                    sheet.occupied_ratio * 100.0
                ));
                super::preview::sheet(ui, f, sheet);
                for p in &sheet.placements {
                    ui.label(format!(
                        "{} · {} × {} mm · X {} / Y {}{}",
                        p.panel.id,
                        p.panel.width,
                        p.panel.height,
                        p.x,
                        p.y,
                        if p.rotated { " · rotation 90°" } else { "" }
                    ));
                }
                ui.weak("Occupation = surface finie des pièces / surface totale. Placement heuristique, optimum non garanti.");
            }
            Err(e) => {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
            }
        }
    }
    pub(super) fn dossier_panel(
        &mut self,
        ui: &mut egui::Ui,
        f: &fablabcam_studio::furniture::Furniture,
    ) {
        self.layout_panel(ui, f);
        ui.separator();
        ui.strong("Contenu du dossier");
        ui.horizontal_wrapped(|ui| {
            for (value, label) in [
                (&mut self.options.native, "Projet natif"),
                (&mut self.options.cb, "CB"),
                (&mut self.options.dxf, "DXF"),
                (&mut self.options.svg, "SVG"),
                (&mut self.options.csv, "CSV"),
                (&mut self.options.pdf, "Notice PDF"),
            ] {
                ui.checkbox(value, label);
            }
        });
        match fabrication_files(f, &self.options) {
            Ok(files) => {
                ui.collapsing(format!("Aperçu : {} fichiers", files.len()), |ui| {
                    for (name, _) in &files {
                        ui.label(name);
                    }
                });
                if ui.button("Exporter le dossier ZIP…").clicked() {
                    self.export_dossier(f);
                }
                if ui.button("Préparer cette dalle en FAO").clicked() {
                    match cam_project(f, self.sheet) {
                        Ok(p) => self.handoff = Some((p, f.setup.machine.clone())),
                        Err(e) => {
                            self.feedback(Err(e), "");
                        }
                    }
                }
            }
            Err(e) => {
                ui.colored_label(egui::Color32::LIGHT_RED, format!("Export bloqué : {e}"));
            }
        }
        ui.label(format!(
            "Machine : {} · post déclaré : {}",
            f.setup.machine.name, f.setup.machine.controller.postprocessor
        ));
        ui.weak("Le dossier contient des plans. Réglez les opérations, le bridage et le post-processeur dans la FAO, puis simulez et contrôlez le G-code.");
    }
}
