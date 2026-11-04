use super::{Document, Draft, FurnitureUi};
use eframe::egui;

impl FurnitureUi {
    pub fn windows(&mut self, ctx: &egui::Context) {
        if self.pending.is_some() {
            let mut choice = None;
            egui::Window::new("Meuble : modifications non enregistrées")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_max_width(520.0);
                    ui.label(
                        "Le document contient des modifications. Choisissez comment continuer.",
                    );
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Enregistrer et continuer").clicked() {
                            choice = Some(0);
                        }
                        if ui.button("Abandonner les modifications").clicked() {
                            choice = Some(1);
                        }
                        if ui.button("Rester sur le document").clicked() {
                            choice = Some(2);
                        }
                    });
                    if self.error {
                        ui.colored_label(egui::Color32::LIGHT_RED, &self.message);
                    }
                });
            match choice {
                Some(0) => {
                    if self.save() {
                        let p = self.pending.take().expect("pending");
                        self.execute(p);
                    }
                }
                Some(1) => {
                    self.draft = None;
                    let p = self.pending.take().expect("pending");
                    self.execute(p);
                }
                Some(2) => {
                    self.pending = None;
                }
                _ => {}
            }
        }
        let Some(doc) = self.incoming.as_ref() else {
            return;
        };
        let collision = self.library.as_ref().is_some_and(|l| l.has_id(doc.id()));
        let mut choice = None;
        egui::Window::new("Aperçu de l'import")
            .collapsible(false)
            .resizable(true)
            .default_width(530.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.heading(doc.name());
                ui.label(if doc.is_template() {
                    "Modèle réutilisable"
                } else {
                    "Projet de meuble autonome"
                });
                ui.label(format!(
                    "Format 1 · modèle révision {}",
                    doc.template().revision
                ));
                let d = doc.dimensions();
                ui.label(format!(
                    "{} × {} × {} mm · épaisseur {} mm",
                    d.width, d.depth, d.height, d.thickness
                ));
                super::preview::assembly(
                    ui,
                    doc.template(),
                    d,
                    false,
                    220.0,
                    match &doc {
                        Document::Furniture(f) => f.setup.cutter,
                        _ => 6.0,
                    },
                );
                if collision {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        "Cet identifiant existe déjà. L'import crée une copie indépendante.",
                    );
                }
                ui.horizontal(|ui| {
                    if ui
                        .button(if collision {
                            "Importer une copie"
                        } else {
                            "Ajouter à la bibliothèque"
                        })
                        .clicked()
                    {
                        choice = Some(true);
                    }
                    if ui.button("Annuler l'import").clicked() {
                        choice = Some(false);
                    }
                });
                if self.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, &self.message);
                }
            });
        if let Some(accept) = choice {
            if !accept {
                self.incoming = None;
                return;
            }
            let doc = self.incoming.as_ref().expect("incoming");
            let mut draft = Draft::new(if collision {
                doc.duplicate()
            } else {
                doc.clone()
            });
            let result = self
                .library
                .as_mut()
                .ok_or("Bibliothèque indisponible".to_string())
                .and_then(|l| l.save(&mut draft));
            if self.feedback(result, "Import enregistré") {
                self.models = draft.document.is_template();
                self.draft = Some(draft);
                self.step = 0;
                self.incoming = None;
            }
        }
    }
}
