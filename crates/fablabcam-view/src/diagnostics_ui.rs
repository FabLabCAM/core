//! Native preferences for the diagnostics service; no panic/core logic in egui.
use eframe::egui;
use fablabcam_studio::diagnostics::{self, core_dump, LogLevel};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

#[derive(Default)]
pub struct DiagnosticsUi {
    pub startup_notice: Option<String>,
    message: String,
    core: Option<core_dump::CoreStatus>,
    export_job: Option<Receiver<Result<Option<PathBuf>, String>>>,
}

impl DiagnosticsUi {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Diagnostics").default_open(true).show(ui, |ui| {
            if let Some(error) = &self.startup_notice { ui.colored_label(egui::Color32::YELLOW, error); }
            let Some(service) = diagnostics::active() else {
                ui.label("Journal indisponible. Les erreurs restent écrites dans le terminal.");
                return;
            };
            let mut settings = service.settings();
            let previous = settings.clone();
            egui::ComboBox::from_label("Niveau du journal")
                .selected_text(if settings.level == LogLevel::Debug { "Détaillé" } else { "Normal" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut settings.level, LogLevel::Normal, "Normal");
                    ui.selectable_value(&mut settings.level, LogLevel::Debug, "Détaillé");
                });
            ui.checkbox(&mut settings.full_backtrace, "Traces complètes lors d'une erreur fatale");
            if settings != previous {
                self.message = match service.configure(&settings, true) {
                    Ok(()) => "Réglages enregistrés et appliqués à cette session".into(),
                    Err(e) => format!("Réglages non enregistrés : {e}"),
                };
            }
            ui.small("Le niveau détaillé enregistre les paramètres de calcul, sans les échanges ni les clés de l'assistant.");
            ui.label(format!("Dossier : {}", service.directory().display()));
            if let Some(warning) = service.warning() { ui.colored_label(egui::Color32::YELLOW, warning); }
            ui.horizontal(|ui| {
                if ui.button("Ouvrir le dossier des diagnostics").clicked() {
                    self.message = open_directory(service.directory()).err().unwrap_or_default();
                }
                if ui.add_enabled(self.export_job.is_none(), egui::Button::new("Exporter le rapport…")).clicked() {
                    let (tx, rx) = mpsc::channel();
                    let service = service.clone();
                    std::thread::spawn(move || {
                        let result = crate::desktop::save_dialog("Exporter les diagnostics texte", "fablabcam-diagnostics.txt", None)
                            .and_then(|destination| match destination {
                                Some(path) => service.export(&path).map(|()| Some(path)),
                                None => Ok(None),
                            });
                        let _ = tx.send(result);
                    });
                    self.export_job = Some(rx);
                }
            });
            ui.small("Export des journaux et rapports de panne uniquement. Relisez les messages avant de les partager.");
            self.poll_export(ui.ctx());
            ui.separator();
            self.core_controls(ui, service);
            if !self.message.is_empty() { ui.label(&self.message); }
        });
    }

    fn core_controls(&mut self, ui: &mut egui::Ui, service: &diagnostics::Diagnostics) {
        if self.core.is_none() {
            self.core = Some(core_dump::status());
        }
        ui.strong("Core dump — mode avancé");
        ui.label(if service.core_armed() {
            "Armé pour la prochaine panique de cette session"
        } else {
            "Désactivé"
        });
        if let Some(core) = &self.core {
            ui.label(&core.summary);
            if !core.destination.is_empty() {
                ui.label(format!("Destination attendue : {}", core.destination));
            }
        }
        ui.small("Le dump peut occuper plusieurs Gio et contenir des clés API ou des données de projet. Il reste local et n'est jamais inclus dans l'export. L'armement ne garantit pas sa création par macOS.");
        ui.horizontal(|ui| {
            let armed = service.core_armed();
            let available = self.core.as_ref().is_some_and(|c| c.available);
            if ui.add_enabled(armed || available, egui::Button::new(if armed { "Désarmer" } else { "Armer pour cette session" })).clicked() {
                self.message = match service.set_core_armed(!armed) {
                    Ok(()) => if armed { "Capture core désarmée ; limite du processus restaurée" } else { "Capture armée. Une panique enregistrera le rapport puis arrêtera le processus." }.into(),
                    Err(e) => e,
                };
                self.core = Some(core_dump::status());
            }
            if ui.button("Vérifier les préconditions").clicked() { self.core = Some(core_dump::status()); }
        });
    }

    fn poll_export(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.export_job else { return };
        match rx.try_recv() {
            Ok(result) => {
                self.message = match result {
                    Ok(Some(path)) => format!("Rapport exporté : {}", path.display()),
                    Ok(None) => "Export annulé".into(),
                    Err(e) => format!("Export impossible : {e}"),
                };
                self.export_job = None;
            }
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(100))
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.message = "Le dialogue d'export s'est interrompu".into();
                self.export_job = None;
            }
        }
    }
}

fn open_directory(path: &std::path::Path) -> Result<(), String> {
    let command = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(command)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Ouverture du dossier : {e}"))
}
