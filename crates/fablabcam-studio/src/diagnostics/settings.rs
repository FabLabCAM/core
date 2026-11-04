use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    #[default]
    Normal,
    Debug,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub level: LogLevel,
    pub full_backtrace: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            level: LogLevel::Normal,
            full_backtrace: true,
        }
    }
}

pub fn default_directory() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("FABLABCAM_DIAGNOSTICS_DIR").filter(|v| !v.is_empty()) {
        return Some(dir.into());
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let base = if cfg!(target_os = "macos") {
        home?.join("Library/Logs/FabLabCAM")
    } else if cfg!(windows) {
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("FabLabCAM/Diagnostics")
    } else {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|p| p.join(".local/state")))?
            .join("fablabcam/diagnostics")
    };
    Some(base)
}

pub fn load(directory: &Path) -> Result<Settings, String> {
    match std::fs::read(directory.join("settings.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| format!("Réglages de diagnostic illisibles : {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(format!("Lecture des réglages de diagnostic : {e}")),
    }
}

pub fn save(directory: &Path, settings: &Settings) -> Result<(), String> {
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut file, settings).map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(directory.join("settings.json"))
        .map_err(|e| e.to_string())?;
    Ok(())
}
