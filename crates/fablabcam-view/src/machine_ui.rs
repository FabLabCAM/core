//! Atelier: UI state only. Validation and persistence live in fablabcam-studio.
mod fields;
mod panels;
mod transfer;
mod windows;

use eframe::egui;
use fablabcam_cam::gcode::machine::Profile;
use fablabcam_studio::machine_profiles::Library;
use std::{path::PathBuf, sync::mpsc::Receiver};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    General,
    Axes,
    Spindle,
    Codes,
    Json,
    Review,
}
impl Section {
    const ALL: [Self; 6] = [
        Self::General,
        Self::Axes,
        Self::Spindle,
        Self::Codes,
        Self::Json,
        Self::Review,
    ];
    const STEPS: [Self; 4] = [Self::General, Self::Axes, Self::Spindle, Self::Review];
    fn label(self) -> &'static str {
        match self {
            Self::General => "Général",
            Self::Axes => "Axes et sécurité",
            Self::Spindle => "Broche et outils",
            Self::Codes => "G-code",
            Self::Json => "JSON avancé",
            Self::Review => "Vérification",
        }
    }
}
#[derive(Clone)]
enum Pending {
    Select(String),
    New,
    Duplicate,
    Reload,
    Import,
    Discard,
    Quit,
}
enum Transfer {
    Import,
    Export(Box<Profile>),
}
type DialogReply = Result<Option<PathBuf>, String>;

pub struct MachineUi {
    pub open: bool,
    library: Library,
    available: bool,
    loaded: bool,
    section: Section,
    wizard: Option<usize>,
    pending: Option<Pending>,
    collision: Option<(Profile, Profile)>,
    dialog: Option<(Transfer, Receiver<DialogReply>)>,
    path: String,
    pub message: String,
    error: bool,
    quit_ready: bool,
}

impl MachineUi {
    pub fn new(root: Option<PathBuf>) -> Self {
        let available = root.is_some();
        Self {
            open: false,
            library: Library::new(root.unwrap_or_default()),
            available,
            loaded: false,
            section: Section::Axes,
            wizard: None,
            pending: None,
            collision: None,
            dialog: None,
            path: String::new(),
            message: if available {
                String::new()
            } else {
                "Dossier de données introuvable : définir FABLABCAM_MACHINE_STORE".into()
            },
            error: !available,
            quit_ready: false,
        }
    }
    pub fn dirty(&self) -> bool {
        self.library.dirty()
    }
    pub fn open(&mut self) {
        self.open = true;
        if !self.loaded && self.available {
            self.loaded = true;
            let result = self.library.reload();
            self.feedback(result, "Bibliothèque de profils chargée");
        }
    }
    pub fn new_profile(&mut self) {
        self.request(Pending::New);
    }
    pub fn import_profile(&mut self) {
        self.request(Pending::Import);
    }
    pub fn request_quit(&mut self) {
        self.request(Pending::Quit);
    }
    pub fn take_quit(&mut self) -> bool {
        std::mem::take(&mut self.quit_ready)
    }
    pub fn busy(&self) -> bool {
        self.dialog.is_some() || self.pending.is_some() || self.collision.is_some()
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

    pub fn save(&mut self) -> bool {
        if !self.available || self.dialog.is_some() || self.collision.is_some() {
            return false;
        }
        let result = self.library.save();
        let saved = self.feedback(result, "Profil enregistré dans la bibliothèque locale");
        if saved {
            self.wizard = None;
        }
        saved
    }

    fn request(&mut self, action: Pending) {
        if !self.available || self.busy() {
            return;
        }
        if self.library.dirty() {
            self.pending = Some(action);
        } else {
            self.execute(action);
        }
    }
    fn execute(&mut self, action: Pending) {
        let create = matches!(action, Pending::New);
        let result = match action {
            Pending::Select(id) => self.library.select(&id),
            Pending::New => self.library.create(),
            Pending::Duplicate => self.library.duplicate(),
            Pending::Reload => self.library.reload(),
            Pending::Import => {
                self.start_import();
                return;
            }
            Pending::Discard => {
                self.library.discard();
                self.wizard = None;
                return;
            }
            Pending::Quit => {
                self.quit_ready = true;
                return;
            }
        };
        if self.feedback(
            result,
            if create {
                "Nouveau profil : renseignez les paramètres de votre machine"
            } else {
                "Profil chargé"
            },
        ) {
            self.wizard = create.then_some(0);
            self.section = if create {
                Section::General
            } else {
                Section::Axes
            };
        }
    }

    fn change_section(&mut self, section: Section) {
        if let Some(draft) = &mut self.library.draft {
            if draft.json_pending() {
                self.message =
                    "Appliquez ou annulez les modifications JSON avant de changer de rubrique"
                        .into();
                self.error = true;
                return;
            }
            if section == Section::Json {
                if let Err(e) = draft.sync_json() {
                    self.message = e;
                    self.error = true;
                    return;
                }
            }
        }
        self.section = section;
        if self.wizard.is_some() {
            self.wizard = Section::STEPS.iter().position(|s| *s == section);
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.add_enabled_ui(self.available && !self.busy(), |ui| self.atelier(ui));
    }
}
