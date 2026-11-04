//! Worker messages carry a request generation. Dropping a job cancels network work.
use super::{actions, provider, Connection, ContextSnapshot, Mode, Plan, PreparedPlan, Workspace};
use genai::chat::ChatRequest;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    time::{Duration, Instant},
};

pub enum Event {
    Text(String),
    Reply(Box<provider::TurnResult>),
    Prepared(Box<PreparedPlan>),
    Error(String),
}

pub struct Job {
    pub id: u64,
    pub events: Receiver<Event>,
    pub cancelled: Arc<AtomicBool>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}
impl Job {
    pub fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
    pub fn chat(
        id: u64,
        connection: Connection,
        secret: String,
        request: ChatRequest,
        mode: Mode,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let (tx, events) = mpsc::channel();
        let (stop, rx) = tokio::sync::oneshot::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        std::thread::spawn(move || {
            let started = Instant::now();
            let mut redactor = super::storage::StreamRedactor::new(&secret);
            let result = (|| {
                let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|_| "Worker réseau indisponible")?;
                rt.block_on(async {
                    tokio::select! {
                        _=rx=>Err("Requête arrêtée".into()),
                        value=tokio::time::timeout(Duration::from_secs(connection.timeout_secs),
                            provider::chat(&connection,&secret,request,mode,|text|send(&tx,&wake,Event::Text(redactor.push(&text)))))=>
                            value.map_err(|_|"Délai maximal dépassé".to_string())?,
                    }
                })
            })();
            let model = super::storage::redact(&connection.model, &secret).chars().take(128).collect::<String>();
            let actions = result
                .as_ref()
                .ok()
                .and_then(|r| r.plan.as_ref())
                .map(|p| p.steps.iter().map(|s| s.action.label()).collect::<Vec<_>>())
                .unwrap_or_default();
            log(
                id,
                connection.provider.label(),
                mode,
                if result.is_ok() { "complete" } else { "error" },
                started.elapsed(),
                serde_json::json!({"model":model,"actions":actions}),
            );
            let event = match result {
                Ok(mut r) => {
                    r.text = super::storage::redact(&r.text, &secret);
                    if !secret.is_empty() && serde_json::to_string(&r.plan).unwrap_or_default().contains(&secret) {
                        Event::Error("Proposition rejetée : elle contient un identifiant confidentiel".into())
                    } else {
                        Event::Reply(Box::new(r))
                    }
                }
                Err(e) => Event::Error(e),
            };
            send(&tx, &wake, event);
        });
        Self { id, events, cancelled, stop: Some(stop) }
    }
    pub fn prepare(id: u64, w: Workspace, context: ContextSnapshot, plan: Plan, mode: Mode, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (tx, events) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        std::thread::spawn(move || {
            let started = Instant::now();
            let labels = plan.steps.iter().map(|s| s.action.label()).collect::<Vec<_>>();
            let result = actions::prepare(&w, &context, plan, mode, flag);
            log(
                id,
                "domain",
                mode,
                if result.is_ok() { "prepared" } else { "error" },
                started.elapsed(),
                serde_json::json!({"actions":labels}),
            );
            send(
                &tx,
                &wake,
                match result {
                    Ok(p) => Event::Prepared(Box::new(p)),
                    Err(e) => Event::Error(e),
                },
            );
        });
        Self { id, events, cancelled, stop: None }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
    }
}
fn send(tx: &Sender<Event>, wake: &Arc<dyn Fn() + Send + Sync>, event: Event) {
    if tx.send(event).is_ok() {
        wake();
    }
}
fn log(id: u64, provider: &str, mode: Mode, status: &str, duration: Duration, details: serde_json::Value) {
    eprintln!(
        "{}",
        serde_json::json!({"component":"assistant","request":id,"provider":provider,"mode":mode,"status":status,"duration_ms":duration.as_millis(),"details":details})
    );
}
