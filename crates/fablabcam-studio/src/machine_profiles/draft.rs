use fablabcam_cam::gcode::machine::Profile;

/// Form state and raw JSON are separate until the user applies the JSON.
/// A malformed raw edit must neither alter typed fields nor disappear on save.
pub struct Draft {
    pub profile: Profile,
    baseline: Option<Profile>,
    pub json: String,
    json_origin: String,
    imported: bool,
}
impl Draft {
    pub fn new(profile: Profile) -> Self {
        Self::imported(profile, None)
    }
    pub fn existing(profile: Profile) -> Self {
        let mut draft = Self::imported(profile.clone(), Some(profile));
        draft.imported = false;
        draft
    }
    pub fn imported(profile: Profile, baseline: Option<Profile>) -> Self {
        let json = serde_json::to_string_pretty(&profile).expect("finite profile");
        Self {
            profile,
            baseline,
            json: json.clone(),
            json_origin: json,
            imported: true,
        }
    }
    pub fn baseline(&self) -> Option<&Profile> {
        self.baseline.as_ref()
    }
    pub fn dirty(&self) -> bool {
        self.imported || self.baseline.as_ref() != Some(&self.profile) || self.json_pending()
    }
    pub fn json_pending(&self) -> bool {
        self.json != self.json_origin
    }
    pub fn sync_json(&mut self) -> Result<(), String> {
        if self.json_pending() {
            return Err("Appliquez ou annulez les modifications JSON".into());
        }
        self.reset_json()
    }
    pub fn reset_json(&mut self) -> Result<(), String> {
        self.json = serde_json::to_string_pretty(&self.profile).map_err(|e| e.to_string())?;
        self.json_origin = self.json.clone();
        Ok(())
    }
    pub fn apply_json(&mut self) -> Result<(), String> {
        let profile = Profile::from_json(&self.json)?;
        if self
            .baseline
            .as_ref()
            .is_some_and(|old| old.id != profile.id)
        {
            return Err("Identifiant immuable : utilisez Dupliquer pour créer une copie".into());
        }
        self.profile = profile;
        self.reset_json()
    }
    pub fn ready(&self) -> Result<(), String> {
        if self.json_pending() {
            return Err("Le JSON a des modifications non appliquées".into());
        }
        if self
            .baseline
            .as_ref()
            .is_some_and(|old| old.id != self.profile.id)
        {
            return Err("Identifiant immuable : utilisez Dupliquer".into());
        }
        self.profile.check()
    }
}
