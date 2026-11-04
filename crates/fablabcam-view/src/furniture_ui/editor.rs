use super::{Document, FurnitureUi, Pending};
use eframe::egui;
use fablabcam_studio::furniture::panels_with_cutter;

impl FurnitureUi {
    pub(super) fn editor(&mut self, ui: &mut egui::Ui) {
        let Some(mut draft) = self.draft.take() else {
            ui.heading("Choisissez un modèle ou un meuble");
            ui.label("Créez, personnalisez, puis préparez vos dalles pour la CNC.");
            return;
        };
        let previous = draft.document.clone();
        ui.heading(if draft.document.is_template() {
            "Éditeur de modèle"
        } else {
            "Assistant guidé"
        });
        ui.label("Nom");
        ui.text_edit_singleline(draft.document.name_mut());
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!draft.dirty(), egui::Button::new("Dupliquer"))
                .on_hover_text("Enregistrez le document avant de le dupliquer")
                .clicked()
            {
                self.pending = Some(Pending::Duplicate);
            }
            ui.checkbox(&mut self.exploded, "Vue éclatée");
            ui.checkbox(&mut self.composing, "Composer le meuble");
            if !draft.document.is_template() && ui.button("Enregistrer comme modèle…").clicked()
            {
                self.pending = Some(Pending::New(draft.document.as_template()));
            }
        });
        if let Document::Template(t) = &mut draft.document {
            if self.composing {
                ui.collapsing("Dimensions extérieures", |ui| {
                    super::fields::dimensions(ui, &mut t.defaults)
                });
            } else {
                super::fields::dimensions(ui, &mut t.defaults);
            }
            super::fields::limits(ui, t);
            ui.weak(format!(
                "Révision actuelle : {} · une modification enregistrée crée la révision suivante",
                t.revision
            ));
        } else {
            ui.horizontal_wrapped(|ui| {
                for (i, name) in ["1. Meuble", "2. Matière & CNC", "3. Découpe", "4. Dossier"]
                    .iter()
                    .enumerate()
                {
                    ui.selectable_value(&mut self.step, i, *name);
                }
            });
            if let Document::Furniture(f) = &mut draft.document {
                match self.step {
                    0 => {
                        if self.composing {
                            ui.collapsing("Dimensions extérieures", |ui| {
                                super::fields::dimensions(ui, &mut f.dimensions)
                            });
                        } else {
                            super::fields::dimensions(ui, &mut f.dimensions);
                        }
                    }
                    1 => super::fields::setup(ui, &mut f.setup, &self.machines),
                    2 => self.layout_panel(ui, f),
                    _ => self.dossier_panel(ui, f),
                }
            }
        }
        if self.composing && (self.step == 0 || draft.document.is_template()) {
            let cutter = match &draft.document {
                Document::Furniture(f) => f.setup.cutter,
                _ => 6.0,
            };
            self.composer.show(ui, &mut draft.document, cutter);
        }
        draft.document.reconcile_edit(&previous);
        if let Err(e) = draft.document.check() {
            ui.colored_label(egui::Color32::LIGHT_RED, e);
        }
        if self.step < 2 || draft.document.is_template() {
            super::preview::assembly(
                ui,
                draft.document.template(),
                draft.document.dimensions(),
                self.exploded,
                260.0,
                match &draft.document {
                    Document::Furniture(f) => f.setup.cutter,
                    _ => 6.0,
                },
            );
            ui.collapsing("Liste des pièces", |ui| {
                let cutter = match &draft.document {
                    Document::Furniture(f) => f.setup.cutter,
                    _ => 6.0,
                };
                if let Ok(parts) = panels_with_cutter(
                    draft.document.template(),
                    draft.document.dimensions(),
                    cutter,
                ) {
                    egui::Grid::new("furniture-cut-list")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("Repère");
                            ui.strong("Pièce");
                            ui.strong("Cotes finies (mm)");
                            ui.end_row();
                            for p in parts {
                                ui.label(p.id);
                                ui.label(p.name);
                                ui.label(format!("{} × {} × {}", p.width, p.height, p.thickness));
                                ui.end_row();
                            }
                        });
                }
            });
        }
        if !draft.document.is_template() {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(self.step > 0, egui::Button::new("Précédent"))
                    .clicked()
                {
                    self.step -= 1;
                }
                if ui
                    .add_enabled(self.step < 3, egui::Button::new("Suivant"))
                    .clicked()
                {
                    self.step += 1;
                }
            });
        }
        let editing = ui.ctx().wants_keyboard_input() || ui.input(|i| i.pointer.any_down());
        let key = ui.ctx().memory(|m| m.focus()).map(|id| {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            id.hash(&mut hasher);
            hasher.finish()
        });
        draft.record_group_scoped(previous, editing, key);
        self.draft = Some(draft);
        // Actions queued while the draft is borrowed must observe its restored
        // dirty state, just like normal library navigation.
        if matches!(self.pending, Some(Pending::Duplicate | Pending::New(_))) && !self.dirty() {
            let action = self.pending.take().expect("queued editor action");
            self.execute(action);
        }
    }
}
