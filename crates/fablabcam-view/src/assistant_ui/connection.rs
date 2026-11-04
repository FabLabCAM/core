use super::*;
use assistant::{storage, Provider};

impl AssistantUi {
    pub(super) fn settings(&mut self, ctx: &egui::Context) -> Option<UiAction> {
        if !self.settings {
            return None;
        }
        let mut open = self.settings;
        let mut action = None;
        let mut c = self.session.connection();
        let identity = c.identity();
        if self.secret_identity != identity {
            self.secret.clear();
            self.secret_identity = identity.clone();
        }
        egui::Window::new("Connexion de l'assistant").open(&mut open).default_width(440.0).show(ctx, |ui| {
            ui.strong(c.provider.label());
            ui.label("Modèle (identifiant exact du fournisseur)");
            ui.text_edit_singleline(&mut c.model);
            ui.label("Adresse du service");
            ui.text_edit_singleline(&mut c.endpoint);
            ui.small("Seul ce fournisseur reçoit les éléments cochés. Aucun repli automatique vers un autre service.");
            if c.provider == Provider::Ollama {
                ui.small("Ollama doit être démarré et le modèle déjà installé. HTTP est permis sur votre réseau local.");
            }
            ui.label("Clé API pour cette session");
            ui.add(egui::TextEdit::singleline(&mut self.secret).password(true).hint_text("Clé du coffre ou de l'environnement si vide"));
            ui.horizontal_wrapped(|ui| {
                if ui.button("Enregistrer dans le coffre système").clicked() {
                    self.message = match storage::save_secret(&c, &self.secret) {
                        Ok(()) => "Clé enregistrée dans le coffre système".into(),
                        Err(e) => e,
                    };
                }
                if ui.button("Retirer du coffre").clicked() {
                    self.message = match storage::delete_secret(&c) {
                        Ok(()) => "Clé retirée du coffre ; une variable d'environnement peut encore être active".into(),
                        Err(e) => e,
                    };
                    self.secret.clear();
                }
            });
            ui.small(format!("Variable reconnue : {}. La clé n'est jamais enregistrée dans la conversation.", c.provider.env()));
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Délai maximal");
                ui.add(egui::DragValue::new(&mut c.timeout_secs).clamp_range(5..=300).suffix(" s"));
            });
            ui.horizontal(|ui| {
                ui.label("Limite de génération");
                ui.add(egui::DragValue::new(&mut c.max_tokens).clamp_range(128..=16384).suffix(" tokens"));
            });
            ui.horizontal(|ui| {
                ui.label("Tours Agent maximum");
                ui.add(egui::DragValue::new(&mut c.max_turns).clamp_range(1..=8));
            });
            ui.checkbox(&mut self.session.debug, "Détails des requêtes (sans contenu ni secrets)");
            ui.label(if c.tools {
                "Appels d'outils : vérifiés sur ce modèle"
            } else {
                "Appels d'outils : à vérifier. Ask reste disponible."
            });
            ui.small("Le test envoie une petite demande synthétique, sans votre projet. Le fournisseur peut la facturer.");
            if ui.add_enabled(self.job.is_none(), egui::Button::new("Tester la connexion et les outils")).clicked() {
                action = Some(UiAction::Probe);
            }
            ui.label(&self.message);
        });
        if c.identity() != identity {
            c.tools = false;
            self.secret.clear();
            self.secret_identity = c.identity();
            self.prepared = None;
            self.plan = None;
            self.last_reply = None;
        }
        self.session.set_connection(c);
        self.settings = open;
        if !open {
            self.persist();
        }
        action
    }
}
