use super::{files::{self, id, json, read, sha, write_new}, Store};
use fablabcam_cam::gcode::machine::{validate, Profile, Report, Status};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revision {
    pub schema_version: u32,
    pub id: String,
    pub revision: u64,
    pub author: String,
    pub created_unix_seconds: u64,
    pub profile_id: String,
    pub status: Status,
    pub program_sha256: String,
    pub profile_sha256: String,
    pub report_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Archive {
    pub metadata: Revision,
    pub program: String,
    pub profile: Profile,
    pub report: Report,
}
impl Store {
    pub fn archive(&self, archive_id: &str, source: &str, profile: &Profile, author: &str) -> Result<Revision,String> {
        id(archive_id)?;
        if author.trim().is_empty() { return Err("Auteur requis".into()); }
        let profile_text = profile.to_json()?;
        let report = validate(source,profile);
        let report_text = json(&report)?;
        let _lock = self.lock()?;
        let dir = self.root.join("archives").join(archive_id);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let revision = revisions(&dir)?.into_iter().max().unwrap_or(0).checked_add(1).ok_or("Trop de révisions")?;
        let metadata = Revision { schema_version: 1, id: archive_id.into(), revision, author: author.into(), created_unix_seconds: (files::now()?/1_000_000_000) as u64, profile_id: profile.id.clone(), status: report.status, program_sha256: sha(source), profile_sha256: sha(&profile_text), report_sha256: sha(&report_text) };
        let temp = dir.join(format!(".pending-{}-{}",std::process::id(),files::now()?));
        fs::create_dir(&temp).map_err(|e| e.to_string())?;
        let result = (|| {
            write_new(&temp.join("program.nc"),source)?;
            write_new(&temp.join("profile.json"),&profile_text)?;
            write_new(&temp.join("report.json"),&report_text)?;
            write_new(&temp.join("revision.json"),&json(&metadata)?)?;
            fs::rename(&temp,dir.join(format!("{revision:08}"))).map_err(|e| e.to_string())
        })();
        if result.is_err() { let _ = fs::remove_dir_all(&temp); }
        result.map(|()| metadata)
    }
    pub fn revisions(&self, archive_id: &str) -> Result<Vec<Revision>,String> {
        id(archive_id)?;
        let _lock = self.lock()?;
        let dir = self.root.join("archives").join(archive_id);
        if !dir.exists() { return Ok(Vec::new()); }
        revisions(&dir)?.into_iter().map(|n| self.read_revision(archive_id,n).map(|a| a.metadata)).collect()
    }
    pub fn revision(&self, archive_id: &str, revision: u64) -> Result<Archive,String> {
        id(archive_id)?;
        let _lock = self.lock()?;
        self.read_revision(archive_id,revision)
    }
    fn read_revision(&self, archive_id: &str, revision: u64) -> Result<Archive,String> {
        let dir = self.root.join("archives").join(archive_id).join(format!("{revision:08}"));
        let metadata: Revision = serde_json::from_str(&read(&dir.join("revision.json"))?).map_err(|e| e.to_string())?;
        if metadata.schema_version != 1 || metadata.id != archive_id || metadata.revision != revision { return Err("Métadonnées de révision incohérentes".into()); }
        let program = read(&dir.join("program.nc"))?;
        let profile_text = read(&dir.join("profile.json"))?;
        let report_text = read(&dir.join("report.json"))?;
        if sha(&program) != metadata.program_sha256 || sha(&profile_text) != metadata.profile_sha256 || sha(&report_text) != metadata.report_sha256 { return Err("Intégrité SHA-256 de la révision incorrecte".into()); }
        let profile = Profile::from_json(&profile_text)?;
        let report: Report = serde_json::from_str(&report_text).map_err(|e| e.to_string())?;
        if profile.id != metadata.profile_id || report.profile_id != profile.id || report.status != metadata.status { return Err("Profil/rapport de révision incohérent".into()); }
        Ok(Archive { metadata, program, profile, report })
    }
}
fn revisions(dir: &std::path::Path) -> Result<Vec<u64>,String> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".pending-") { continue; }
        let n = name.parse::<u64>().map_err(|_| format!("Révision inconnue : {name}"))?;
        if n == 0 || name != format!("{n:08}") || !entry.file_type().map_err(|e| e.to_string())?.is_dir() { return Err(format!("Révision mal formée : {name}")); }
        out.push(n);
    }
    out.sort_unstable(); Ok(out)
}
