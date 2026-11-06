//! Native panel; the domain module owns validation, transport and tool execution.
mod bridge;
mod connection;
mod panel;
mod message;
use eframe::egui;
use fablabcam_studio::assistant::{
    self, provider::TurnResult, worker::Job, Connection, ContextSnapshot, Plan, PreparedPlan, Session, Workspace,
};
use std::{path::PathBuf, sync::Arc};

#[derive(Clone, Copy)]
enum UiAction {
    Send,
    Stop,
    Prepare,
    Apply,
    Undo,
    Redo,
    Pin,
    Unpin,
    Probe,
    Clear,
}

pub struct AssistantUi {
    pub open: bool,
    session: Session,
    path: Option<PathBuf>,
    prompt: String,
    message: String,
    partial: String,
    settings: bool,
    secret: String,
    secret_identity: String,
    plan: Option<Plan>,
    prepared: Option<PreparedPlan>,
    last_reply: Option<Box<TurnResult>>,
    context: Option<ContextSnapshot>,
    pinned: Option<ContextSnapshot>,
    job: Option<Job>,
    generation: u64,
    active_connection: Option<Connection>,
    probe: bool,
    turns: u32,
    undo: Option<UndoLot>,
    redone: bool,
    markdown: egui_commonmark::CommonMarkCache,
}

#[derive(Clone)]
struct UndoLot {
    before: Workspace,
    after: Workspace,
    before_project: fablabcam_studio::studio::PreparedProject,
    after_project: fablabcam_studio::studio::PreparedProject,
}

impl AssistantUi {
    pub fn new() -> Self {
        let path = std::env::var_os("FABLABCAM_ASSISTANT_STORE")
            .map(PathBuf::from)
            .or_else(|| crate::desktop::Prefs::file().and_then(|p| p.parent().map(|p| p.join("assistant"))))
            .map(|p| p.join("conversation.json"));
        let loaded = path.as_deref().map(assistant::storage::load).unwrap_or_else(|| Ok(Session::default()));
        let (session, message, path) = match loaded {
            Ok(s) => (s, String::new(), path),
            Err(e) => (Session::default(), e, None),
        };
        Self {
            open: true,
            session,
            path,
            prompt: String::new(),
            message,
            partial: String::new(),
            settings: false,
            secret: String::new(),
            secret_identity: String::new(),
            plan: None,
            prepared: None,
            last_reply: None,
            context: None,
            pinned: None,
            job: None,
            generation: 0,
            active_connection: None,
            probe: false,
            turns: 0,
            undo: None,
            redone: false,
            markdown: Default::default(),
        }
    }
    fn persist(&mut self) {
        if let Some(p) = &self.path {
            if let Err(e) = assistant::storage::save(p, &self.session) {
                self.message = e;
            }
        }
    }
    fn wake(ctx: &egui::Context) -> Arc<dyn Fn() + Send + Sync> {
        let ctx = ctx.clone();
        Arc::new(move || ctx.request_repaint())
    }
    fn key(&self, c: &Connection) -> Result<String, String> {
        if self.secret_identity == c.identity() && !self.secret.is_empty() {
            return Ok(self.secret.clone());
        }
        if c.provider == assistant::Provider::Ollama {
            return assistant::storage::secret(c).or_else(|_| Ok(String::new()));
        }
        assistant::storage::secret(c)
    }
    fn stop(&mut self) {
        self.job = None;
        self.prepared = None;
        self.last_reply = None;
        self.message = "Arrêté — aucune nouvelle action ne sera appliquée".into();
    }
    fn poll(&mut self) {
        let events = self.job.as_ref().map(|j| j.events.try_iter().collect::<Vec<_>>()).unwrap_or_default();
        for event in events {
            match event {
                assistant::worker::Event::Text(text) => self.partial.push_str(&text),
                assistant::worker::Event::Error(e) => {
                    self.message = e;
                    self.job = None;
                    self.prepared = None;
                    self.partial.clear();
                }
                assistant::worker::Event::Prepared(p) => {
                    if serde_json::to_value(&self.plan).ok() != serde_json::to_value(&p.plan).ok() {
                        self.job = None;
                        self.message = "Plan modifié : aperçu abandonné, préparez à nouveau".into();
                        continue;
                    }
                    self.prepared = Some(*p);
                    self.job = None;
                    self.message = "Aperçu calculé — vérifiez les changements avant application".into();
                }
                assistant::worker::Event::Reply(reply) => {
                    self.job = None;
                    self.partial.clear();
                    if self.probe {
                        if let Some(mut c) = self.active_connection.clone() {
                            c.tools = reply.plan.is_some();
                            self.message = if c.tools {
                                "Connexion réussie ; appel d'outil reçu"
                            } else {
                                "Réponse reçue ; capacité Agent non confirmée"
                            }
                            .into();
                            self.session.set_connection(c);
                            self.persist();
                        }
                        self.probe = false;
                        continue;
                    }
                    if let (Some(context), Some(c)) = (&self.context, &self.active_connection) {
                        let text = if reply.text.is_empty() && reply.plan.is_some() {
                            "Proposition structurée à vérifier ci-dessous.".into()
                        } else {
                            reply.text.clone()
                        };
                        self.session.push(false, text, context, c);
                    }
                    self.plan = reply.plan.clone();
                    self.prepared = None;
                    self.message = if self.plan.is_some() {
                        "Proposition reçue — préparer l'aperçu avant d'appliquer"
                    } else {
                        "Réponse terminée"
                    }
                    .into();
                    self.last_reply = Some(reply);
                    self.persist();
                }
            }
        }
    }
}
