use super::{MachineUi, Section};
use eframe::egui;

impl MachineUi {
    pub fn windows(&mut self, ctx: &egui::Context) {
        self.unsaved_window(ctx);
        self.collision_window(ctx);
    }
    fn unsaved_window(&mut self, ctx: &egui::Context) {
        if self.pending.is_none() {
            return;
        }
        let mut choice = None;
        egui::Window::new("Profil : modifications non enregistrées")
            .collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER,[0.,0.]).show(ctx,|ui|{
                ui.set_max_width(480.);
                ui.label("Le profil contient des modifications. Que souhaitez-vous faire avant de continuer ?");
                ui.horizontal_wrapped(|ui|{
                    if ui.button("Enregistrer et continuer").clicked(){choice=Some(0);}
                    if ui.button("Abandonner les modifications").clicked(){choice=Some(1);}
                    if ui.button("Rester sur le profil").clicked(){choice=Some(2);}
                });
                if self.error{ui.colored_label(egui::Color32::LIGHT_RED,&self.message);}
            });
        match choice {
            Some(0) => {
                if self.save() {
                    let action = self.pending.take().expect("pending action");
                    self.execute(action);
                }
            }
            Some(1) => {
                self.library.discard();
                self.wizard = None;
                let action = self.pending.take().expect("pending action");
                self.execute(action);
            }
            Some(2) => {
                self.pending = None;
            }
            _ => {}
        }
    }
    fn collision_window(&mut self, ctx: &egui::Context) {
        let Some((incoming, old)) = &self.collision else {
            return;
        };
        let mut choice = None;
        egui::Window::new("Import : identifiant déjà présent").collapsible(false).resizable(false)
            .anchor(egui::Align2::CENTER_CENTER,[0.,0.]).show(ctx,|ui|{
                ui.set_max_width(520.);
                ui.label(format!("Identifiant : {}",incoming.id));
                ui.label(format!("Bibliothèque : {}",old.name));
                ui.label(format!("Fichier importé : {}",incoming.name));
                ui.small("Le remplacement sera appliqué uniquement après Enregistrer. Les archives existantes sont conservées.");
                ui.horizontal_wrapped(|ui|{
                    if ui.button("Garder les deux (copie)").clicked(){choice=Some(0);}
                    if ui.button("Remplacer ce profil").clicked(){choice=Some(1);}
                    if ui.button("Annuler l'import").clicked(){choice=Some(2);}
                });
            });
        let Some(choice) = choice else {
            return;
        };
        let (incoming, old) = self.collision.take().expect("collision");
        if choice == 2 {
            self.message = "Import annulé".into();
            self.error = false;
            return;
        }
        let result = self
            .library
            .stage_import(incoming, (choice == 1).then_some(old), choice == 0);
        if self.feedback(
            result,
            "Import préparé : vérifiez le profil puis enregistrez",
        ) {
            self.wizard = None;
            self.section = Section::Review;
        }
    }
}
