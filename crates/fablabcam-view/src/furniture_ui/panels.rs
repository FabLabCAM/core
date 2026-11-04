use super::{Document, FurnitureUi, Pending, Template};
use eframe::egui;

impl FurnitureUi {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.models, true, "Mes modèles");
            ui.selectable_value(&mut self.models, false, "Mes meubles");
            if ui.button("Nouveau").clicked() {
                self.new_document();
            }
            if ui.button("Importer…").clicked() {
                self.import();
            }
            if ui
                .add_enabled(self.draft.is_some(), egui::Button::new("Enregistrer"))
                .clicked()
            {
                self.save();
            }
            if ui
                .add_enabled(self.draft.is_some(), egui::Button::new("Exporter…"))
                .clicked()
            {
                self.export_native();
            }
            if ui
                .add_enabled(
                    self.draft.as_ref().is_some_and(|d| d.can_undo()),
                    egui::Button::new("Annuler"),
                )
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(
                    self.draft.as_ref().is_some_and(|d| d.can_redo()),
                    egui::Button::new("Rétablir"),
                )
                .clicked()
            {
                self.redo();
            }
            if self.dirty() {
                ui.colored_label(egui::Color32::YELLOW, "* Modifications non enregistrées");
            }
        });
    }
    pub(super) fn library_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading(if self.models {
            "Mes modèles"
        } else {
            "Mes meubles"
        });
        if self.models {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Créer un caisson").clicked() {
                    self.request(Pending::New(Template::cabinet()));
                }
                if ui.button("Créer une étagère").clicked() {
                    self.request(Pending::New(Template::shelving()));
                }
            });
        } else {
            ui.weak("Utilisez un modèle pour créer un nouveau meuble.");
        }
        let docs = self
            .library
            .as_ref()
            .map(|l| l.documents.clone())
            .unwrap_or_default();
        egui::ScrollArea::vertical()
            .id_source("furniture-library")
            .show(ui, |ui| {
                for doc in docs
                    .into_iter()
                    .filter(|d| d.is_template() == self.models)
                    .collect::<Vec<_>>()
                {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        let selected = self
                            .draft
                            .as_ref()
                            .is_some_and(|d| d.document.id() == doc.id());
                        if ui
                            .selectable_label(selected, egui::RichText::new(doc.name()).strong())
                            .clicked()
                        {
                            self.request(Pending::Select(doc.clone()));
                        }
                        let d = doc.dimensions();
                        ui.label(format!(
                            "{} × {} × {} mm · ép. {} mm · modèle r{}",
                            d.width,
                            d.depth,
                            d.height,
                            d.thickness,
                            doc.template().revision
                        ));
                        super::preview::assembly(
                            ui,
                            doc.template(),
                            d,
                            false,
                            130.0,
                            match &doc {
                                Document::Furniture(f) => f.setup.cutter,
                                _ => 6.0,
                            },
                        );
                        ui.horizontal(|ui| {
                            if ui.button("Modifier").clicked() {
                                self.request(Pending::Select(doc.clone()));
                            }
                            if let Document::Template(t) = &doc {
                                if ui.button("Utiliser ce modèle").clicked() {
                                    self.request(Pending::Use(t.clone()));
                                }
                            }
                        });
                    });
                    ui.add_space(8.0);
                }
            });
    }
    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none().inner_margin(16.0).show(ui, |ui| {
            ui.heading("Meubles pour le FabLab");
            ui.weak("Modèles réutilisables · projets de meubles · préparation des dalles");
            ui.add_enabled_ui(!self.busy(), |ui| {
                self.toolbar(ui);
                self.path_panel(ui);
            });
            if !self.message.is_empty() {
                ui.colored_label(
                    if self.error {
                        egui::Color32::LIGHT_RED
                    } else {
                        egui::Color32::LIGHT_GREEN
                    },
                    &self.message,
                );
            }
            ui.separator();
            ui.add_enabled_ui(!self.busy(), |ui| {
                if self.composing && self.draft.is_some() {
                    ui.collapsing("Bibliothèque de meubles et modèles", |ui| {
                        self.library_panel(ui)
                    });
                    egui::ScrollArea::vertical()
                        .id_source("furniture-full-editor")
                        .show(ui, |ui| self.editor(ui));
                    return;
                }
                ui.columns(2, |cols| {
                    self.library_panel(&mut cols[0]);
                    egui::ScrollArea::vertical()
                        .id_source("furniture-editor")
                        .show(&mut cols[1], |ui| self.editor(ui));
                });
            });
        });
    }
}
