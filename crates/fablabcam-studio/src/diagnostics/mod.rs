//! Process diagnostics, independent of the UI and geometry engines (ADR 0006).
//! Only explicit domain events enter this journal; provider HTTP traffic does not.

pub mod cli;
pub mod core_dump;
mod files;
mod panic_report;
mod settings;

use serde_json::{json, Value};
pub use settings::{default_directory, LogLevel, Settings};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

pub const BUILD_ID: &str = match option_env!("FABLABCAM_BUILD_ID") {
    Some(id) => id,
    None => "local",
};

static ACTIVE: OnceLock<Arc<Diagnostics>> = OnceLock::new();

pub struct Diagnostics {
    directory: PathBuf,
    detailed: AtomicBool,
    full_backtrace: AtomicBool,
    core_armed: AtomicBool,
    previous_limit: Mutex<Option<core_dump::SavedLimit>>,
    journal: Mutex<Option<files::Journal>>,
    warning: Mutex<Option<String>>,
}

impl Diagnostics {
    /// Open a service without installing a global hook (also useful for tests).
    pub fn open(directory: PathBuf) -> Result<Arc<Self>, String> {
        files::prepare_directory(&directory)
            .map_err(|e| format!("Dossier des diagnostics : {e}"))?;
        let (settings, mut warning) = match settings::load(&directory) {
            Ok(s) => (s, None),
            Err(e) => (Settings::default(), Some(e)),
        };
        if let Err(e) = files::prune_panics(&directory) {
            warning = Some(e.to_string());
        }
        let journal = files::Journal::open(&directory)
            .map_err(|e| format!("Journal des diagnostics : {e}"))?;
        let diagnostics = Arc::new(Self {
            directory,
            detailed: AtomicBool::new(settings.level == LogLevel::Debug),
            full_backtrace: AtomicBool::new(settings.full_backtrace),
            core_armed: AtomicBool::new(false),
            previous_limit: Mutex::new(None),
            journal: Mutex::new(Some(journal)),
            warning: Mutex::new(warning),
        });
        diagnostics.record(
            false,
            "session_start",
            json!({"os": std::env::consts::OS, "arch": std::env::consts::ARCH}),
        );
        Ok(diagnostics)
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn settings(&self) -> Settings {
        Settings {
            level: if self.detailed.load(Ordering::Relaxed) {
                LogLevel::Debug
            } else {
                LogLevel::Normal
            },
            full_backtrace: self.full_backtrace.load(Ordering::Relaxed),
        }
    }

    pub fn configure(&self, settings: &Settings, persist: bool) -> Result<(), String> {
        if persist {
            settings::save(&self.directory, settings)?;
        }
        self.detailed
            .store(settings.level == LogLevel::Debug, Ordering::Relaxed);
        self.full_backtrace
            .store(settings.full_backtrace, Ordering::Relaxed);
        self.record(
            false,
            "settings_changed",
            json!({"level": settings.level, "full_backtrace": settings.full_backtrace}),
        );
        Ok(())
    }

    pub fn warning(&self) -> Option<String> {
        self.warning.lock().ok().and_then(|w| w.clone())
    }

    pub fn record(&self, detailed: bool, event: &str, fields: Value) {
        if detailed && !self.detailed.load(Ordering::Relaxed) {
            return;
        }
        let entry = json!({"time_unix_ms": now_ms(), "pid": std::process::id(), "version": env!("CARGO_PKG_VERSION"), "build": BUILD_ID, "event": event, "fields": fields});
        let result = self
            .journal
            .lock()
            .map_err(|_| "Verrou du journal indisponible".to_string())
            .and_then(|mut guard| {
                guard
                    .as_mut()
                    .ok_or_else(|| "Journal fermé".to_string())?
                    .write(&entry)
                    .map_err(|e| e.to_string())
            });
        if let Err(e) = result {
            // Best effort reporting, never propagate a logging failure into CAM.
            use std::io::Write;
            let _ = writeln!(std::io::stderr(), "[FabLabCAM diagnostics] {e}");
            if let Ok(mut warning) = self.warning.lock() {
                *warning = Some(format!("Écriture des diagnostics : {e}"));
            }
        }
    }

    pub fn core_armed(&self) -> bool {
        self.core_armed.load(Ordering::Acquire)
    }

    pub fn set_core_armed(&self, armed: bool) -> Result<(), String> {
        let mut previous = self
            .previous_limit
            .lock()
            .map_err(|_| "Réglage core indisponible")?;
        if armed {
            if previous.is_none() {
                *previous = Some(core_dump::arm()?);
            }
            self.core_armed.store(true, Ordering::Release);
        } else {
            self.core_armed.store(false, Ordering::Release);
            if let Some(limit) = previous.as_ref() {
                core_dump::restore(limit)?;
                *previous = None;
            }
        }
        self.record(
            false,
            "core_mode",
            json!({"armed": armed, "virtual_limit_bytes": core_dump::CORE_LIMIT}),
        );
        Ok(())
    }

    pub fn export(&self, destination: &Path) -> Result<(), String> {
        let _guard = self.journal.lock().map_err(|_| "Journal indisponible")?;
        files::export(&self.directory, destination)
    }

    pub fn shutdown(&self) {
        if let Err(e) = self.set_core_armed(false) {
            if let Ok(mut warning) = self.warning.lock() {
                *warning = Some(e);
            }
        }
        self.record(false, "session_end", json!({}));
        if let Ok(mut journal) = self.journal.lock() {
            *journal = None;
        }
    }
}

pub fn initialize(directory: PathBuf) -> Result<Arc<Diagnostics>, String> {
    if let Some(active) = ACTIVE.get() {
        return Ok(active.clone());
    }
    let service = Diagnostics::open(directory)?;
    ACTIVE
        .set(service.clone())
        .map_err(|_| "Diagnostics déjà initialisés")?;
    panic_report::install(service.clone());
    Ok(service)
}

pub fn active() -> Option<&'static Arc<Diagnostics>> {
    ACTIVE.get()
}

pub fn record(detailed: bool, event: &str, fields: Value) {
    if let Some(service) = ACTIVE.get() {
        service.record(detailed, event, fields);
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}
