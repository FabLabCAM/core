//! Ordinary script execution shares the bounded engine with the assistant and keeps egui responsive.
use fablabcam_studio::{script, studio::PreparedProject, Project};
use eframe::egui;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
    Arc,
};

type ResultMessage = Result<(PreparedProject, Vec<String>), String>;
pub struct ScriptJob {
    pub document: String,
    pub revision: u64,
    pub code: String,
    pub events: Receiver<ResultMessage>,
    cancelled: Arc<AtomicBool>,
}
impl ScriptJob {
    pub fn start(document: String, revision: u64, project: Project, code: String, ctx: egui::Context) -> Self {
        let (tx, events) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let script = code.clone();
        std::thread::spawn(move || {
            let result = script::run_controlled(&project, &script, flag.clone()).and_then(|out| {
                let prepared = PreparedProject::new(out.project);
                if flag.load(Ordering::Relaxed) {
                    return Err("Script arrêté".into());
                }
                Ok((prepared, out.log))
            });
            if tx.send(result).is_ok() {
                ctx.request_repaint();
            }
        });
        Self { document, revision, code, events, cancelled }
    }
}
impl Drop for ScriptJob {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}
