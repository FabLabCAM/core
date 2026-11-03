//! Furniture UI. Domain rules, persistence and export remain in fablabcam-studio.
mod dossier;
mod editor;
mod fields;
mod panels;
mod preview;
mod transfer;
mod windows;

use fablabcam_cam::gcode::machine::Profile;
use fablabcam_studio::furniture::{Document, Draft, ExportOptions, Furniture, Library, Template};
use std::{path::PathBuf, sync::mpsc::Receiver};

enum Pending {
    Select(Document),
    New(Template),
    Use(Template),
    Duplicate,
    Import,
    Quit,
}
enum Transfer {
    Import,
    Export(Vec<u8>),
}
type Reply = Result<Option<PathBuf>, String>;

pub struct FurnitureUi {
    pub open: bool,
    library: Option<Library>,
    draft: Option<Draft>,
    models: bool,
    step: usize,
    sheet: usize,
    exploded: bool,
    pending: Option<Pending>,
    incoming: Option<Document>,
    dialog: Option<(Transfer, Receiver<Reply>)>,
    options: ExportOptions,
    path: String,
    pub message: String,
    error: bool,
    quit_ready: bool,
    pub handoff: Option<(fablabcam_studio::Project, Profile)>,
    machines: Vec<Profile>,
}
impl FurnitureUi {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self {
            open: false,
            library: root.map(Library::new),
            draft: None,
            models: true,
            step: 0,
            sheet: 0,
            exploded: false,
            pending: None,
            incoming: None,
            dialog: None,
            options: ExportOptions::default(),
            path: String::new(),
            message: String::new(),
            error: false,
            quit_ready: false,
            handoff: None,
            machines: Vec::new(),
        }
    }
    pub fn open(&mut self) {
        self.open = true;
        if let Some(library) = &mut self.library {
            let result = library.reload();
            self.feedback(result, "Bibliothèque de meubles chargée");
        } else {
            self.feedback(
                Err("Dossier de données introuvable : définir FABLABCAM_FURNITURE_STORE".into()),
                "",
            );
        }
        if let Some(root) = crate::desktop::machine_store() {
            let mut library = fablabcam_studio::machine_profiles::Library::new(root);
            match library.reload() {
                Ok(()) => self.machines = library.profiles,
                Err(e) => {
                    self.feedback(Err(e), "");
                }
            }
        }
    }
    pub fn dirty(&self) -> bool {
        self.draft.as_ref().is_some_and(Draft::dirty)
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some() || self.incoming.is_some() || self.dialog.is_some()
    }
    pub fn request_quit(&mut self) {
        self.request(Pending::Quit);
    }
    pub fn take_quit(&mut self) -> bool {
        std::mem::take(&mut self.quit_ready)
    }
    pub fn new_document(&mut self) {
        if self.models {
            self.request(Pending::New(Template::cabinet()));
        } else {
            self.models = true;
            self.message = "Choisissez un modèle, puis Utiliser ce modèle".into();
        }
    }
    pub fn import(&mut self) {
        self.request(Pending::Import);
    }
    pub fn undo(&mut self) {
        if let Some(d) = &mut self.draft {
            d.undo();
        }
    }
    pub fn redo(&mut self) {
        if let Some(d) = &mut self.draft {
            d.redo();
        }
    }
    pub fn save(&mut self) -> bool {
        let result = match (&mut self.library, &mut self.draft) {
            (Some(l), Some(d)) => l.save(d),
            _ => Err("Aucun document à enregistrer".into()),
        };
        self.feedback(result, "Enregistré dans la bibliothèque locale")
    }
    fn feedback(&mut self, result: Result<(), String>, success: &str) -> bool {
        match result {
            Ok(()) => {
                self.message = success.into();
                self.error = false;
                true
            }
            Err(e) => {
                self.message = e;
                self.error = true;
                false
            }
        }
    }
    fn request(&mut self, action: Pending) {
        if self.busy() {
            return;
        }
        if self.dirty() {
            self.pending = Some(action);
        } else {
            self.execute(action);
        }
    }
    fn execute(&mut self, action: Pending) {
        match action {
            Pending::Select(doc) => {
                // Saving the unsaved draft may have replaced this library snapshot.
                let doc = self
                    .library
                    .as_ref()
                    .and_then(|l| l.documents.iter().find(|d| d.id() == doc.id()))
                    .cloned()
                    .unwrap_or(doc);
                self.models = doc.is_template();
                self.draft = Some(Draft::existing(doc));
            }
            Pending::New(mut t) => {
                t.id = fablabcam_studio::furniture::new_id();
                t.name = "Mon modèle de meuble".into();
                self.models = true;
                self.draft = Some(Draft::new(Document::Template(t)));
            }
            Pending::Use(t) => {
                let t = self
                    .library
                    .as_ref()
                    .and_then(|l| {
                        l.documents
                            .iter()
                            .find(|d| d.id() == t.id && d.is_template())
                    })
                    .map(|d| d.template().clone())
                    .unwrap_or(t);
                self.models = false;
                self.draft = Some(Draft::new(Document::Furniture(Box::new(
                    Furniture::from_template(t),
                ))));
            }
            Pending::Duplicate => {
                if let Some(d) = &self.draft {
                    self.draft = Some(Draft::new(d.document.duplicate()));
                }
            }
            Pending::Import => {
                self.start_import();
                return;
            }
            Pending::Quit => {
                self.quit_ready = true;
                return;
            }
        }
        self.step = 0;
        self.sheet = 0;
        self.error = false;
        self.message = String::new();
    }
}
