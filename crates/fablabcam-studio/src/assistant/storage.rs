//! Local conversation storage and OS credentials are deliberately separate.
use super::{Connection, Session};
use sha2::{Digest, Sha256};
use std::{io::Write, path::Path};

pub fn save(path: &Path, session: &Session) -> Result<(), String> {
    let parent = path.parent().ok_or("Dossier de conversation absent")?;
    std::fs::create_dir_all(parent).map_err(|_| "Impossible de créer le dossier de conversation")?;
    let bytes = serde_json::to_vec_pretty(session).map_err(|_| "Historique non sérialisable")?;
    if bytes.len() > 4_000_000 {
        return Err("Historique supérieur à 4 Mo : effacez la conversation".into());
    }
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|_| "Écriture d'historique impossible")?;
    file.write_all(&bytes).and_then(|_| file.as_file().sync_all()).map_err(|_| "Écriture d'historique impossible")?;
    file.persist(path).map_err(|_| "Enregistrement d'historique impossible")?;
    Ok(())
}

pub fn load(path: &Path) -> Result<Session, String> {
    if !path.exists() {
        return Ok(Session::default());
    }
    let metadata = std::fs::metadata(path).map_err(|_| "Historique illisible")?;
    if metadata.len() > 4_000_000 {
        return Err("Historique trop volumineux".into());
    }
    let s = std::fs::read(path).map_err(|_| "Historique illisible")?;
    serde_json::from_slice(&s).map_err(|_| "Historique invalide ; original conservé".into())
}

fn credential(connection: &Connection) -> Result<keyring::Entry, String> {
    // Bind a stored secret to its endpoint, never silently forward it to a new host.
    let account = format!("{}-{:x}", connection.provider.label(), Sha256::digest(connection.endpoint.as_bytes()));
    keyring::Entry::new("org.fablabcam.assistant", &account).map_err(|_| "Coffre système indisponible".into())
}

pub fn save_secret(connection: &Connection, secret: &str) -> Result<(), String> {
    if secret.trim().is_empty() {
        return Err("Clé vide".into());
    }
    credential(connection)?.set_password(secret).map_err(|_| "Impossible d'enregistrer dans le coffre système".into())
}

pub fn secret(connection: &Connection) -> Result<String, String> {
    if connection.endpoint.trim_end_matches('/') == connection.provider.endpoint().trim_end_matches('/') {
        if let Ok(value) = std::env::var(connection.provider.env()) {
            if !value.is_empty() {
                return Ok(value);
            }
        }
    }
    match credential(connection)?.get_password() {
        Ok(value) => Ok(value),
        Err(keyring::Error::NoEntry) => Ok(String::new()),
        Err(_) => Err("Coffre système indisponible : utilisez une clé pour cette session ou une variable d'environnement".into()),
    }
}

pub fn delete_secret(connection: &Connection) -> Result<(), String> {
    match credential(connection)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Suppression de la clé impossible dans le coffre système".into()),
    }
}

pub fn redact(text: &str, secret: &str) -> String {
    if secret.is_empty() {
        text.into()
    } else {
        text.replace(secret, "[clé masquée]")
    }
}

/// Keep a possible credential prefix between chunks so split tokens cannot leak.
pub struct StreamRedactor {
    secret: String,
    pending: String,
}
impl StreamRedactor {
    pub fn new(secret: &str) -> Self {
        Self { secret: secret.into(), pending: String::new() }
    }
    pub fn push(&mut self, text: &str) -> String {
        if self.secret.is_empty() {
            return text.into();
        }
        self.pending.push_str(text);
        self.pending = redact(&self.pending, &self.secret);
        let keep = self.secret.char_indices().map(|(i, _)| i).rev().find(|&n| self.pending.ends_with(&self.secret[..n])).unwrap_or(0);
        let split = self.pending.len() - keep;
        let rest = self.pending.split_off(split);
        std::mem::replace(&mut self.pending, rest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_credentials_are_not_emitted() {
        let mut redactor = StreamRedactor::new("clé-secrète");
        let mut output = redactor.push("Avant clé-");
        assert_eq!(output, "Avant ");
        output += &redactor.push("secrè");
        output += &redactor.push("te après");
        assert_eq!(output, "Avant [clé masquée] après");
    }
}
