use super::{FurnitureUi, Transfer};
use crate::desktop;
use eframe::egui;
use fablabcam_studio::furniture::{export_file, fabrication_zip, read_document, Furniture};
use std::{path::PathBuf, sync::mpsc};

impl FurnitureUi {
    pub(super) fn start_import(&mut self) {
        self.spawn(Transfer::Import, || {
            desktop::open_dialog("Importer un modèle ou un meuble", &["json"], None)
        });
    }
    pub fn export_native(&mut self) {
        if self.busy() {
            return;
        }
        let Some(d) = &self.draft else {
            return;
        };
        let name = format!("{}.{}", d.document.id(), d.document.extension());
        match d.document.to_json() {
            Ok(json) => self.spawn(Transfer::Export(json.into_bytes()), move || {
                desktop::save_dialog("Exporter le document natif (nouveau fichier)", &name, None)
            }),
            Err(e) => {
                self.feedback(Err(e), "");
            }
        }
    }
    pub(super) fn export_dossier(&mut self, f: &Furniture) {
        match fabrication_zip(f, &self.options) {
            Ok(bytes) => self.spawn(Transfer::Export(bytes), || {
                desktop::save_dialog(
                    "Exporter le dossier de fabrication (nouveau fichier)",
                    "fabrication.zip",
                    None,
                )
            }),
            Err(e) => {
                self.feedback(Err(e), "");
            }
        }
    }
    fn spawn(&mut self, transfer: Transfer, f: impl FnOnce() -> super::Reply + Send + 'static) {
        if self.dialog.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        self.dialog = Some((transfer, rx));
        self.message = "Choisissez la destination dans le dialogue système…".into();
        self.error = false;
    }
    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some((_, rx)) = &self.dialog else {
            return;
        };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
                return;
            }
            Err(_) => Err("Dialogue interrompu : utilisez le champ de chemin".into()),
        };
        let (transfer, _) = self.dialog.take().expect("active dialog");
        match result {
            Ok(Some(path)) => {
                self.path = path.display().to_string();
                match transfer {
                    Transfer::Import => self.import_path(path),
                    Transfer::Export(bytes) => {
                        let r = export_file(&path, &bytes);
                        self.feedback(r, "Export terminé");
                    }
                }
            }
            Ok(None) => {
                self.message = "Dialogue annulé".into();
                self.error = false;
            }
            Err(e) => {
                self.feedback(Err(e), "");
            }
        }
    }
    pub(super) fn import_path(&mut self, path: PathBuf) {
        if self.dirty() {
            self.feedback(
                Err("Enregistrez le document courant avant d'importer par chemin".into()),
                "",
            );
            return;
        }
        match read_document(&path) {
            Ok(doc) => self.incoming = Some(doc),
            Err(e) => {
                self.feedback(Err(e), "");
            }
        }
    }
    pub(super) fn path_panel(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Import / export par chemin", |ui| {
            ui.label("Chemin complet du fichier — l'export exige un nouveau nom");
            ui.text_edit_singleline(&mut self.path);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Analyser l'import").clicked() {
                    self.import_path(PathBuf::from(&self.path));
                }
                if ui.button("Exporter le natif ici").clicked() {
                    if let Some(d) = &self.draft {
                        let r = d
                            .document
                            .to_json()
                            .and_then(|s| export_file(&PathBuf::from(&self.path), s.as_bytes()));
                        self.feedback(r, "Document exporté");
                    }
                }
                if ui.button("Exporter le ZIP ici").clicked() {
                    if let Some(super::Document::Furniture(f)) =
                        self.draft.as_ref().map(|d| &d.document)
                    {
                        let r = fabrication_zip(f, &self.options)
                            .and_then(|b| export_file(&PathBuf::from(&self.path), &b));
                        self.feedback(r, "Dossier exporté");
                    }
                }
            });
        });
    }
}
