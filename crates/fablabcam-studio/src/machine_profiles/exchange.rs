use fablabcam_cam::gcode::machine::Profile;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

pub fn read_profile(path: &Path) -> Result<Profile, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut source = String::new();
    file.take(1_048_577)
        .read_to_string(&mut source)
        .map_err(|e| e.to_string())?;
    if source.len() > 1_048_576 {
        return Err("Le profil dépasse 1 Mio".into());
    }
    Profile::from_json(&source)
}

/// Export never replaces an existing file, including a profile in the store.
/// The store has its own locked, checked replacement operation.
pub fn export_file(profile: &Profile, path: &Path) -> Result<(), String> {
    let json = profile.to_json()?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("Export impossible (un fichier existant n'est pas remplacé) : {e}"))?;
    if let Err(e) = file
        .write_all(json.as_bytes())
        .and_then(|()| file.sync_all())
    {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(e.to_string());
    }
    Ok(())
}
