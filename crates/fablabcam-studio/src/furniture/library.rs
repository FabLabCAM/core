use super::{Document, Draft, Template};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema_version: u32,
    documents: Vec<Document>,
}

pub struct Library {
    root: PathBuf,
    pub documents: Vec<Document>,
}
impl Library {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().into(),
            documents: Vec::new(),
        }
    }
    fn lock(&self) -> Result<File, String> {
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join(".lock"))
            .map_err(|e| e.to_string())?;
        lock.lock().map_err(|e| e.to_string())?;
        Ok(lock)
    }
    fn read(&self) -> Result<Vec<Document>, String> {
        let path = self.root.join("library.json");
        if !path.exists() {
            return Ok(vec![
                Document::Template(Template::cabinet()),
                Document::Template(Template::shelving()),
            ]);
        }
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        let catalog: Catalog =
            serde_json::from_slice(&bytes).map_err(|e| format!("Bibliothèque illisible : {e}"))?;
        super::data::version(catalog.schema_version)?;
        let mut ids = std::collections::HashSet::new();
        for d in &catalog.documents {
            d.check()?;
            if !ids.insert(d.id()) {
                return Err("Bibliothèque invalide : identifiant en double".into());
            }
        }
        Ok(catalog.documents)
    }
    pub fn reload(&mut self) -> Result<(), String> {
        let _lock = self.lock()?;
        self.documents = self.read()?;
        Ok(())
    }
    /// Compare-and-swap while locked prevents an older Studio from losing another edit.
    /// Models increment their revision; existing jobs keep their embedded snapshot.
    pub fn save(&mut self, draft: &mut Draft) -> Result<(), String> {
        draft.document.check()?;
        let _lock = self.lock()?;
        let mut docs = self.read()?;
        let position = docs.iter().position(|d| d.id() == draft.document.id());
        let current = position.map(|i| &docs[i]);
        if current != draft.baseline.as_ref() {
            return Err("Document modifié ailleurs ou identifiant existant : rechargez ou importez une copie".into());
        }
        let mut next = draft.document.clone();
        next.promote_schema();
        if let (Document::Template(t), Some(Document::Template(old))) = (&mut next, current) {
            if &draft.document != current.expect("existing template") {
                t.revision = old
                    .revision
                    .checked_add(1)
                    .ok_or("Révision maximale atteinte")?;
            }
        }
        if let Some(i) = position {
            docs[i] = next.clone();
        } else {
            docs.push(next.clone());
        }
        docs.sort_by(|a, b| a.name().cmp(b.name()).then(a.id().cmp(b.id())));
        let catalog = Catalog {
            schema_version: super::SCHEMA_VERSION,
            documents: docs,
        };
        let bytes = serde_json::to_vec_pretty(&catalog).map_err(|e| e.to_string())?;
        let mut temp = tempfile::NamedTempFile::new_in(&self.root).map_err(|e| e.to_string())?;
        temp.write_all(&bytes)
            .and_then(|()| temp.as_file().sync_all())
            .map_err(|e| e.to_string())?;
        let old = self.root.join("library.json");
        if old.exists() {
            let bytes = fs::read(&old).map_err(|e| e.to_string())?;
            let version: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if version.get("schema_version").and_then(|v| v.as_u64()) == Some(1) {
                match OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(self.root.join("library-schema-1.backup.json"))
                {
                    Ok(mut backup) => {
                        backup
                            .write_all(&bytes)
                            .and_then(|()| backup.sync_all())
                            .map_err(|e| e.to_string())?;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
        temp.persist(self.root.join("library.json"))
            .map_err(|e| e.to_string())?;
        self.documents = catalog.documents;
        *draft = Draft::existing(next);
        Ok(())
    }
    pub fn has_id(&self, id: &str) -> bool {
        self.documents.iter().any(|d| d.id() == id)
    }
}

pub fn read_document(path: &Path) -> Result<Document, String> {
    let file = File::open(path).map_err(|e| format!("Lecture impossible : {e}"))?;
    let mut text = String::new();
    file.take(2_000_001)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    Document::from_json(&text)
}
/// Exports never overwrite a file silently, including in the path fallback UI.
pub fn export_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(bytes)
        .and_then(|()| temp.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    temp.persist_noclobber(path).map_err(|e| {
        format!("Export impossible (choisissez un nouveau nom si le fichier existe) : {e}")
    })?;
    Ok(())
}
