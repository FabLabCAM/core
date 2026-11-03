//! Profile editing use cases shared by the native UI and tests.
//! Drawing state and machine configuration have independent lifetimes.
mod configuration;
mod draft;
mod exchange;

use crate::gcode_store::Store;
pub use configuration::{configuration, Configuration};
pub use draft::Draft;
pub use exchange::{export_file, read_profile};
use fablabcam_cam::gcode::machine::Profile;
use std::path::{Path, PathBuf};

pub struct Library {
    store: Store,
    root: PathBuf,
    pub profiles: Vec<Profile>,
    pub draft: Option<Draft>,
}

impl Library {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            store: Store::new(&root),
            root: root.as_ref().into(),
            profiles: Vec::new(),
            draft: None,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn dirty(&self) -> bool {
        self.draft.as_ref().is_some_and(Draft::dirty)
    }

    pub fn reload(&mut self) -> Result<(), String> {
        self.require_clean()?;
        let default = Profile::default();
        self.store.initialize_empty_library(&default)?;
        let profiles = self.store.profiles()?;
        let current = self.draft.as_ref().map(|d| d.profile.id.clone());
        self.draft = current
            .and_then(|id| profiles.iter().find(|p| p.id == id))
            .or_else(|| profiles.iter().find(|p| p.id == default.id))
            .or(profiles.first())
            .map(|p| Draft::existing(p.clone()));
        self.profiles = profiles;
        Ok(())
    }

    pub fn select(&mut self, id: &str) -> Result<(), String> {
        self.require_clean()?;
        self.draft = Some(Draft::existing(self.store.profile(id)?));
        Ok(())
    }

    pub fn create(&mut self) -> Result<(), String> {
        self.require_clean()?;
        let mut profile = Profile::default();
        profile.id = self.unique_id(&profile.id)?;
        self.draft = Some(Draft::new(profile));
        Ok(())
    }

    pub fn duplicate(&mut self) -> Result<(), String> {
        self.require_clean()?;
        let mut profile = self
            .draft
            .as_ref()
            .ok_or("Sélectionnez un profil")?
            .profile
            .clone();
        profile.id = self.unique_id(&profile.id)?;
        profile.name += " — copie";
        self.draft = Some(Draft::new(profile));
        Ok(())
    }

    /// Only the explicit UI discard action calls this; navigation never does.
    pub fn discard(&mut self) {
        self.draft = self
            .draft
            .as_ref()
            .and_then(|d| d.baseline().cloned())
            .map(Draft::existing);
    }

    pub fn save(&mut self) -> Result<(), String> {
        let draft = self.draft.as_ref().ok_or("Aucun profil à enregistrer")?;
        draft.ready()?;
        self.store
            .save_profile_checked(&draft.profile, draft.baseline())?;
        let saved = draft.profile.clone();
        self.profiles.retain(|p| p.id != saved.id);
        self.profiles.push(saved.clone());
        self.profiles
            .sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        self.draft = Some(Draft::existing(saved));
        Ok(())
    }

    /// Import is staged for inspection. Saving is always a separate action.
    /// `expected` is the profile displayed in the duplicate-ID confirmation.
    pub fn stage_import(
        &mut self,
        mut profile: Profile,
        expected: Option<Profile>,
        copy: bool,
    ) -> Result<(), String> {
        self.require_clean()?;
        profile.check()?;
        if copy {
            profile.id = self.unique_id(&profile.id)?;
            profile.name += " — copie";
            self.draft = Some(Draft::new(profile));
        } else {
            if expected.as_ref().is_some_and(|old| old.id != profile.id) {
                return Err("Identifiant de remplacement incohérent".into());
            }
            self.draft = Some(Draft::imported(profile, expected));
        }
        Ok(())
    }

    pub fn import_conflict(&self, profile: &Profile) -> Result<Option<Profile>, String> {
        // Listing under Store's lock propagates corrupt-file and permission errors.
        Ok(self
            .store
            .profiles()?
            .into_iter()
            .find(|p| p.id == profile.id))
    }

    fn unique_id(&self, base: &str) -> Result<String, String> {
        let profiles = self.store.profiles()?;
        // IDs are ASCII (enforced by Profile::check), leaving room for the suffix.
        let stem = &base[..base.len().min(68)];
        for index in 1..=100_000 {
            let id = format!("{stem}-{index}");
            if profiles.iter().all(|p| p.id != id) {
                return Ok(id);
            }
        }
        Err("Impossible de trouver un identifiant disponible".into())
    }

    fn require_clean(&self) -> Result<(), String> {
        if self.dirty() {
            Err("Enregistrez ou abandonnez les modifications du profil en cours".into())
        } else {
            Ok(())
        }
    }
}
