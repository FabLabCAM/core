use super::{
    files::{self, id, read},
    Store,
};
use fablabcam_cam::gcode::machine::Profile;
use std::{fs, path::Path};

impl Store {
    /// Seed only an empty library, under the same lock as profile writes.
    /// Existing user profiles (including a customized default) are never replaced.
    pub fn initialize_empty_library(&self, profile: &Profile) -> Result<(), String> {
        let text = profile.to_json()?;
        let _lock = self.lock()?;
        let dir = self.root.join("profiles");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_some_and(|v| v == "json") {
                return Ok(());
            }
        }
        write_profile(&dir, &profile.id, &text)
    }
    /// Compare under the same lock as the write, so an editor cannot silently
    /// replace a profile changed by another Studio or by the CLI.
    pub fn save_profile_checked(
        &self,
        profile: &Profile,
        expected: Option<&Profile>,
    ) -> Result<(), String> {
        let text = profile.to_json()?;
        id(&profile.id)?;
        let _lock = self.lock()?;
        let dir = self.root.join("profiles");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.json", profile.id));
        match expected {
            Some(old) => {
                if old.id != profile.id { return Err("L'identifiant d'un profil enregistré ne peut pas changer ; créer une copie".into()); }
                let current = Profile::from_json(&read(&path)?)?;
                if &current != old { return Err("Conflit : le profil a changé sur disque. Exportez votre brouillon puis rechargez la bibliothèque.".into()); }
            }
            None if path.exists() => return Err("Cet identifiant existe déjà ; choisir un autre identifiant ou importer avec remplacement".into()),
            None => {}
        }
        write_profile(&dir, &profile.id, &text)
    }

    pub fn save_profile(&self, profile: &Profile, replace: bool) -> Result<(), String> {
        let text = profile.to_json()?;
        id(&profile.id)?;
        let _lock = self.lock()?;
        let dir = self.root.join("profiles");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.json", profile.id));
        if path.exists() != replace {
            return Err(if replace {
                "Profil inexistant : utiliser add"
            } else {
                "Profil existant : utiliser update"
            }
            .into());
        }
        write_profile(&dir, &profile.id, &text)
    }
    pub fn profile(&self, profile_id: &str) -> Result<Profile, String> {
        id(profile_id)?;
        let _lock = self.lock()?;
        Profile::from_json(&read(
            &self
                .root
                .join("profiles")
                .join(format!("{profile_id}.json")),
        )?)
    }
    pub fn profiles(&self) -> Result<Vec<Profile>, String> {
        let _lock = self.lock()?;
        let dir = self.root.join("profiles");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_some_and(|v| v == "json") {
                out.push(Profile::from_json(&read(&path)?)?);
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }
    pub fn delete_profile(&self, profile_id: &str) -> Result<(), String> {
        id(profile_id)?;
        let _lock = self.lock()?;
        fs::remove_file(
            self.root
                .join("profiles")
                .join(format!("{profile_id}.json")),
        )
        .map_err(|e| e.to_string())
    }
}

/// Caller holds Store's lock; publish only a fully written profile.
fn write_profile(dir: &Path, profile_id: &str, text: &str) -> Result<(), String> {
    let temp = dir.join(format!(".pending-{}-{}", std::process::id(), files::now()?));
    let path = dir.join(format!("{profile_id}.json"));
    let result = files::write_new(&temp, text)
        .and_then(|()| fs::rename(&temp, &path).map_err(|e| e.to_string()));
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
