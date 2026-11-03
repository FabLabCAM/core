use super::{MachineUi, Transfer};
use crate::desktop;
use eframe::egui;
use fablabcam_studio::machine_profiles::{export_file, read_profile};
use std::{path::PathBuf, sync::mpsc};

impl MachineUi {
    pub(super) fn start_import(&mut self) {
        self.spawn(Transfer::Import, || {
            desktop::open_dialog("Importer un profil machine JSON", &["json"], None)
        });
    }
    pub fn export_profile(&mut self) {
        if self.busy() || !self.available {
            return;
        }
        let Some(draft) = &self.library.draft else {
            return;
        };
        if let Err(e) = draft.ready() {
            self.feedback(Err(e), "");
            return;
        }
        let profile = draft.profile.clone();
        let name = format!("{}.json", profile.id);
        self.spawn(Transfer::Export(Box::new(profile)), move || {
            desktop::save_dialog("Exporter le profil JSON (nouveau fichier)", &name, None)
        });
    }
    fn spawn(
        &mut self,
        purpose: Transfer,
        f: impl FnOnce() -> super::DialogReply + Send + 'static,
    ) {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        self.dialog = Some((purpose, rx));
        self.message = "Choisissez le fichier dans le dialogue système…".into();
        self.error = false;
    }
    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some((_, rx)) = &self.dialog else {
            return;
        };
        let reply = match rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Le dialogue système s'est interrompu".into())
            }
        };
        let (purpose, _) = self.dialog.take().expect("active dialog");
        match reply {
            Ok(Some(path)) => {
                self.path = path.display().to_string();
                match purpose {
                    Transfer::Import => self.import_path(path),
                    Transfer::Export(profile) => self.write_export(&profile, path),
                }
            }
            Ok(None) => {
                self.message = "Dialogue annulé".into();
                self.error = false;
            }
            Err(e) => {
                self.message = format!("{e} — utilisez Fichier JSON par chemin");
                self.error = true;
            }
        }
    }
    pub(super) fn import_path(&mut self, path: PathBuf) {
        if self.library.dirty() {
            self.feedback(
                Err("Enregistrez ou abandonnez le profil avant l'import".into()),
                "",
            );
            return;
        }
        let incoming = match read_profile(&path) {
            Ok(p) => p,
            Err(e) => {
                self.feedback(Err(e), "");
                return;
            }
        };
        match self.library.import_conflict(&incoming) {
            Ok(Some(old)) => self.collision = Some((incoming, old)),
            Ok(None) => {
                let result = self.library.stage_import(incoming, None, false);
                if self.feedback(
                    result,
                    "Profil importé en brouillon : vérifiez puis enregistrez",
                ) {
                    self.wizard = None;
                    self.section = super::Section::Review;
                }
            }
            Err(e) => {
                self.feedback(Err(e), "");
            }
        }
    }
    pub(super) fn write_export(
        &mut self,
        profile: &fablabcam_cam::gcode::machine::Profile,
        path: PathBuf,
    ) {
        let path = desktop::with_extension(path, "json");
        let result = export_file(profile, &path);
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        self.feedback(result, &format!("Profil exporté : {name}"));
    }
}
