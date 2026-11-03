use super::Store;
use sha2::{Digest, Sha256};
use std::{fs::{self, File, OpenOptions}, io::Write, path::Path, time::{SystemTime, UNIX_EPOCH}};

pub(super) fn id(s: &str) -> Result<(), String> {
    if s.is_empty() || s.len() > 80 || !s.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c)) { return Err("Identifiant attendu : 1 à 80 caractères ASCII alphanumériques, - ou _".into()); }
    Ok(())
}
pub(super) fn read(path: &Path) -> Result<String,String> {
    fs::read_to_string(path).map_err(|e| format!("{} : {e}",path.display()))
}
pub(super) fn sha(text: &str) -> String { format!("{:x}",Sha256::digest(text.as_bytes())) }
pub(super) fn now() -> Result<u128,String> { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).map_err(|e| e.to_string()) }
pub(super) fn write_new(path: &Path, content: &str) -> Result<(),String> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| format!("{} : {e}",path.display()))?;
    file.write_all(content.as_bytes()).and_then(|()| file.sync_all()).map_err(|e| e.to_string())
}
pub(super) fn json<T: serde::Serialize>(value: &T) -> Result<String,String> { serde_json::to_string_pretty(value).map(|v| v+"\n").map_err(|e| e.to_string()) }
impl Store {
    pub(super) fn lock(&self) -> Result<File,String> {
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let lock = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(self.root.join(".lock")).map_err(|e| e.to_string())?;
        lock.lock().map_err(|e| e.to_string())?;
        Ok(lock)
    }
}
