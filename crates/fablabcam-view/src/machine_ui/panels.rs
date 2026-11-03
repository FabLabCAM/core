use super::{fields, MachineUi, Pending, Section};
use eframe::egui::{self, Color32, RichText};
use fablabcam_studio::machine_profiles::configuration;

impl MachineUi {
    pub(super) fn atelier(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none().inner_margin(16.).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading(if self.wizard.is_some() {
                    "Créer un profil machine"
                } else {
                    "Profils machine · Atelier"
                });
                ui.separator();
                if ui.button("Nouveau profil…").clicked() {
                    self.request(Pending::New);
                }
                if ui.button("Importer JSON…").clicked() {
                    self.request(Pending::Import);
                }
                if ui
                    .add_enabled(
                        self.library.draft.is_some(),
                        egui::Button::new("Exporter JSON…"),
                    )
                    .clicked()
                {
                    self.export_profile();
                }
            });
            ui.small(if self.wizard.is_some() {
                "Un modèle de départ, puis les paramètres de votre machine."
            } else {
                "Bibliothèque locale · les profils sont indépendants du dessin ouvert."
            });
        });
        egui::TopBottomPanel::bottom("machine_footer").show_inside(ui, |ui| self.footer(ui));
        egui::SidePanel::left("machine_library")
            .resizable(true)
            .default_width(215.)
            .width_range(175.0..=300.)
            .show_inside(ui, |ui| self.library_panel(ui));
        if ui.available_width() > 850. && self.wizard.is_none() {
            egui::SidePanel::right("machine_summary")
                .resizable(false)
                .exact_width(200.)
                .show_inside(ui, |ui| self.summary(ui));
        }
        egui::CentralPanel::default().show_inside(ui, |ui| self.editor(ui));
    }
    fn library_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.);
        if let Some(step) = self.wizard {
            ui.strong("CRÉATION GUIDÉE");
            ui.add_space(15.);
            for (i, section) in Section::STEPS.into_iter().enumerate() {
                if ui
                    .selectable_label(i == step, format!("{}  {}", i + 1, section.label()))
                    .clicked()
                {
                    self.change_section(section);
                }
                ui.add_space(9.);
            }
            ui.separator();
            ui.small("Les limites physiques du modèle sont inconnues. Renseignez vos valeurs ou complétez le profil plus tard.");
            return;
        }
        ui.horizontal(|ui| {
            ui.strong("MES PROFILS");
            ui.weak(self.library.profiles.len().to_string());
        });
        ui.add_space(10.);
        let selected = self.library.draft.as_ref().map(|d| d.profile.id.clone());
        let mut choose = None;
        egui::ScrollArea::vertical()
            .id_source("machine_list")
            .max_height((ui.available_height() - 105.).max(80.))
            .show(ui, |ui| {
                for p in &self.library.profiles {
                    let label = format!("{}\n{}", p.name, configuration(p).label());
                    if ui
                        .add_sized(
                            [ui.available_width(), 49.],
                            egui::SelectableLabel::new(selected.as_deref() == Some(&p.id), label),
                        )
                        .on_hover_text(&p.id)
                        .clicked()
                    {
                        choose = Some(p.id.clone());
                    }
                    ui.add_space(5.);
                }
            });
        if let Some(id) = choose {
            if selected.as_ref() != Some(&id) {
                self.request(Pending::Select(id));
            }
        }
        ui.add_space(12.);
        ui.separator();
        if ui.button("Actualiser la bibliothèque").clicked() {
            self.request(Pending::Reload);
        }
        if ui
            .add_enabled(
                self.library.draft.is_some(),
                egui::Button::new("Dupliquer le profil"),
            )
            .clicked()
        {
            self.request(Pending::Duplicate);
        }
    }
    fn editor(&mut self, ui: &mut egui::Ui) {
        let Some(draft) = &self.library.draft else {
            ui.add_space(32.);
            ui.heading("Votre bibliothèque de machines");
            ui.label("Créez un profil avec l'assistant ou importez un fichier JSON existant.");
            ui.add_space(14.);
            if ui.button("Créer mon premier profil").clicked() {
                self.request(Pending::New);
            }
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.strong(&draft.profile.name);
            if draft.dirty() {
                ui.colored_label(
                    Color32::from_rgb(240, 180, 100),
                    "Modifications non enregistrées",
                );
            }
        });
        ui.add_space(10.);
        if self.wizard.is_none() {
            ui.horizontal_wrapped(|ui| {
                for s in Section::ALL {
                    if ui.selectable_label(self.section == s, s.label()).clicked() {
                        self.change_section(s);
                    }
                }
            });
        } else {
            ui.small(format!("Étape {} / 4", self.wizard.unwrap_or(0) + 1));
        }
        ui.separator();
        ui.add_space(8.);
        let mut result = None;
        egui::ScrollArea::both()
            .id_source(("machine_form", self.section as u8))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width().max(390.));
                if let Some(draft) = &mut self.library.draft {
                    result = fields::show(ui, draft, self.section);
                }
            });
        if let Some(result) = result {
            self.feedback(
                result,
                "JSON appliqué ; les paramètres du formulaire sont actualisés",
            );
        }
    }
    fn footer(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.);
        ui.horizontal_wrapped(|ui| {
            if self.wizard.is_some_and(|s| s > 0) && ui.button("Précédent").clicked() {
                self.change_section(Section::STEPS[self.wizard.unwrap() - 1]);
            }
            if self.wizard.is_some_and(|s| s < 3) {
                if primary(ui, "Continuer") {
                    self.change_section(Section::STEPS[self.wizard.unwrap() + 1]);
                }
            } else if ui
                .add_enabled(
                    self.library.draft.is_some(),
                    egui::Button::new(RichText::new("Enregistrer le profil").strong())
                        .fill(Color32::from_rgb(160, 80, 20)),
                )
                .clicked()
            {
                self.save();
            }
            if ui
                .add_enabled(
                    self.library.dirty(),
                    egui::Button::new(if self.wizard.is_some() {
                        "Abandonner la création"
                    } else {
                        "Annuler les modifications"
                    }),
                )
                .clicked()
            {
                self.request(Pending::Discard);
            }
            if self
                .library
                .draft
                .as_ref()
                .is_some_and(|d| d.json_pending())
            {
                ui.colored_label(Color32::from_rgb(240, 180, 100), "JSON à appliquer");
            }
        });
        if !self.message.is_empty() {
            ui.colored_label(
                if self.error {
                    Color32::LIGHT_RED
                } else {
                    ui.visuals().text_color()
                },
                &self.message,
            );
        }
        egui::CollapsingHeader::new("Fichier JSON par chemin / emplacement du magasin").show(ui,|ui|{
            ui.horizontal(|ui|{
                ui.label("Fichier JSON");ui.add(egui::TextEdit::singleline(&mut self.path).desired_width(420.));
                if ui.button("Importer ce fichier").clicked(){self.import_path(PathBuf::from(&self.path));}
                if ui.button("Exporter vers ce fichier").clicked(){
                    if let Some(d)=&self.library.draft{
                        match d.ready(){Ok(())=>{let p=d.profile.clone();self.write_export(&p,PathBuf::from(&self.path));},Err(e)=>{self.feedback(Err(e),"");}}
                    }
                }
            });
            ui.small("L'export crée un nouveau fichier ; choisissez un autre nom si le fichier existe.");
            ui.small(format!("Magasin : {}",self.library.root().display()));
        });
        ui.add_space(5.);
    }
    fn summary(&self, ui: &mut egui::Ui) {
        let Some(d) = &self.library.draft else {
            return;
        };
        let p = &d.profile;
        ui.add_space(12.);
        ui.strong("CONFIGURATION");
        ui.add_space(10.);
        let check = configuration(p);
        ui.label(check.label());
        ui.small(format!("{} · {}", p.controller.name, p.machine_kind));
        ui.add_space(20.);
        ui.strong("Courses renseignées");
        for a in ['X', 'Y', 'Z'] {
            let course = p
                .axes
                .get(&a)
                .and_then(|v| v.min.zip(v.max))
                .map(|(min, max)| format!("{:.3} mm", max - min))
                .unwrap_or_else(|| "Inconnue".into());
            ui.label(format!("{a}  {course}"));
        }
        ui.add_space(18.);
        ui.separator();
        ui.small("Repère machine en mm. Les limites ne sont pas déduites du dessin.");
        ui.add_space(18.);
        ui.label("La configuration ne remplace pas la validation du G-code.");
        ui.small("Aucun pilotage machine.");
    }
}
use std::path::PathBuf;
fn primary(ui: &mut egui::Ui, label: &str) -> bool {
    ui.add(egui::Button::new(RichText::new(label).strong()).fill(Color32::from_rgb(160, 80, 20)))
        .clicked()
}
