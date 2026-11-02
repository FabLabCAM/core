//! Intégration au bureau : boîtes de dialogue de fichiers natives, préférences, fichiers ouverts depuis le
//! Finder (double-clic, « Ouvrir avec », dépôt sur l'icône).
//!
//! Les dialogues passent par les outils du système, sans bibliothèque : `osascript` (macOS), `zenity`
//! (Linux), PowerShell (Windows, non essayé). S'ils manquent, l'appelant garde le champ « chemin ».

use std::path::{Path, PathBuf};
use std::process::Command;

fn applescript_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Interprète la sortie d'un dialogue : chemin choisi, ou `None` si l'utilisateur a annulé.
fn dialog_result(out: std::io::Result<std::process::Output>, tool: &str) -> Result<Option<PathBuf>, String> {
    let out = out.map_err(|e| format!("{tool} indisponible : {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() && !text.is_empty() {
        return Ok(Some(PathBuf::from(text)));
    }
    let err = String::from_utf8_lossy(&out.stderr);
    // annulation : osascript -128, zenity / PowerShell code 1 sans message
    if err.contains("-128") || err.trim().is_empty() {
        Ok(None)
    } else {
        Err(format!("{tool} : {}", err.trim()))
    }
}

/// Dialogue « Ouvrir » limité aux extensions données.
pub fn open_dialog(title: &str, exts: &[&str], dir: Option<&Path>) -> Result<Option<PathBuf>, String> {
    if cfg!(target_os = "macos") {
        let types: Vec<String> = exts.iter().map(|e| applescript_str(e)).collect();
        let loc = dir.map_or(String::new(), |d| format!(" default location (POSIX file {})", applescript_str(&d.display().to_string())));
        let script = format!("POSIX path of (choose file with prompt {} of type {{{}}}{loc})", applescript_str(title), types.join(", "));
        dialog_result(Command::new("osascript").arg("-e").arg(script).output(), "osascript")
    } else if cfg!(windows) {
        let filter = exts.iter().map(|e| format!("*.{e}")).collect::<Vec<_>>().join(";");
        let ps = format!(
            "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Title = '{}'; $d.Filter = 'Fichiers ({filter})|{filter}'; if ($d.ShowDialog() -eq 'OK') {{ $d.FileName }}",
            title.replace('\'', "''")
        );
        dialog_result(Command::new("powershell").args(["-NoProfile", "-Command", &ps]).output(), "PowerShell")
    } else {
        let filter = exts.iter().map(|e| format!("*.{e}")).collect::<Vec<_>>().join(" ");
        let mut c = Command::new("zenity");
        c.args(["--file-selection", "--title", title, "--file-filter", &filter]);
        if let Some(d) = dir {
            c.arg("--filename").arg(format!("{}/", d.display()));
        }
        dialog_result(c.output(), "zenity")
    }
}

/// Dialogue « Enregistrer sous » ; le système demande confirmation avant d'écraser un fichier.
pub fn save_dialog(title: &str, default_name: &str, dir: Option<&Path>) -> Result<Option<PathBuf>, String> {
    if cfg!(target_os = "macos") {
        let loc = dir.map_or(String::new(), |d| format!(" default location (POSIX file {})", applescript_str(&d.display().to_string())));
        let script = format!("POSIX path of (choose file name with prompt {} default name {}{loc})", applescript_str(title), applescript_str(default_name));
        dialog_result(Command::new("osascript").arg("-e").arg(script).output(), "osascript")
    } else if cfg!(windows) {
        let ps = format!(
            "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.SaveFileDialog; $d.Title = '{}'; $d.FileName = '{}'; if ($d.ShowDialog() -eq 'OK') {{ $d.FileName }}",
            title.replace('\'', "''"),
            default_name.replace('\'', "''")
        );
        dialog_result(Command::new("powershell").args(["-NoProfile", "-Command", &ps]).output(), "PowerShell")
    } else {
        let start = dir.map_or(default_name.to_string(), |d| d.join(default_name).display().to_string());
        dialog_result(Command::new("zenity").args(["--file-selection", "--save", "--confirm-overwrite", "--title", title, "--filename", &start]).output(), "zenity")
    }
}

/// Ajoute l'extension si le nom choisi n'en a pas (les dialogues d'enregistrement ne l'imposent pas).
pub fn with_extension(p: PathBuf, ext: &str) -> PathBuf {
    if p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
        p
    } else {
        let mut s = p.into_os_string();
        s.push(format!(".{ext}"));
        PathBuf::from(s)
    }
}

// ───────────────────────────── Préférences ─────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Prefs {
    pub snap: bool,
    /// Pas du magnétisme (mm).
    pub snap_step: f64,
    /// Maille de la simulation (mm).
    pub sim_res: f64,
    /// Post-processeur `.cbpp` utilisé à l'export (vide = intégré).
    pub post_path: String,
    /// Fichiers récents, le plus récent en premier.
    pub recent: Vec<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { snap: true, snap_step: 1.0, sim_res: 0.5, post_path: String::new(), recent: Vec::new() }
    }
}

pub const MAX_RECENT: usize = 8;

impl Prefs {
    /// Fichier des préférences ; `FABLABCAM_PREFS` le remplace (tests, captures).
    pub fn file() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os("FABLABCAM_PREFS") {
            return Some(PathBuf::from(p));
        }
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let dir = if cfg!(target_os = "macos") {
            home?.join("Library/Application Support/FabLabCAM")
        } else if cfg!(windows) {
            PathBuf::from(std::env::var_os("APPDATA")?).join("FabLabCAM")
        } else {
            std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| home.map(|h| h.join(".config")))?.join("fablabcam")
        };
        Some(dir.join("preferences.txt"))
    }

    pub fn parse(text: &str) -> Prefs {
        let mut p = Prefs::default();
        for l in text.lines() {
            let Some((k, v)) = l.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "snap" => p.snap = v == "true",
                "snap_step" => p.snap_step = v.parse().ok().filter(|x: &f64| *x > 0.0).unwrap_or(p.snap_step),
                "sim_res" => p.sim_res = v.parse().ok().filter(|x: &f64| *x >= 0.05).unwrap_or(p.sim_res),
                "post_path" => p.post_path = v.to_string(),
                "recent" if !v.is_empty() && p.recent.len() < MAX_RECENT => p.recent.push(v.to_string()),
                _ => {}
            }
        }
        p
    }

    pub fn to_text(&self) -> String {
        let mut s = format!("snap={}\nsnap_step={}\nsim_res={}\npost_path={}\n", self.snap, self.snap_step, self.sim_res, self.post_path);
        for r in &self.recent {
            s += &format!("recent={r}\n");
        }
        s
    }

    pub fn load() -> Prefs {
        Prefs::file().and_then(|f| std::fs::read_to_string(f).ok()).map_or_else(Prefs::default, |t| Prefs::parse(&t))
    }

    pub fn save(&self) -> Result<(), String> {
        let f = Prefs::file().ok_or("dossier des préférences introuvable")?;
        if let Some(d) = f.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{} : {e}", d.display()))?;
        }
        std::fs::write(&f, self.to_text()).map_err(|e| format!("{} : {e}", f.display()))
    }

    /// Place `path` en tête des fichiers récents.
    pub fn touch_recent(&mut self, path: &Path) {
        let s = path.display().to_string();
        self.recent.retain(|r| r != &s);
        self.recent.insert(0, s);
        self.recent.truncate(MAX_RECENT);
    }
}

// ───────────────────── Fichiers ouverts par le Finder (macOS) ─────────────────────

/// Fichiers transmis par macOS (double-clic sur un `.cb`, « Ouvrir avec », dépôt sur l'icône du Dock).
///
/// macOS les envoie au délégué de l'application par `application:openFiles:`. winit (0.29) ne le traite pas ;
/// on ajoute donc cette méthode à `NSObject` avant le démarrage, ce qui la rend disponible sur le délégué de
/// winit. Seul `NSApplication` envoie ce message : l'ajout est sans effet sur les autres objets.
#[cfg(target_os = "macos")]
pub mod finder {
    use eframe::egui;
    use objc2::runtime::{AnyClass, AnyObject, Sel};
    use objc2::{class, msg_send, sel};
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    static QUEUE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
    static CTX: OnceLock<egui::Context> = OnceLock::new();

    extern "C" fn open_files(_this: *mut AnyObject, _cmd: Sel, app: *mut AnyObject, files: *mut AnyObject) {
        // SAFETY : `files` est le NSArray<NSString*> fourni par AppKit pour la durée de l'appel.
        unsafe {
            let n: usize = msg_send![files, count];
            let mut q = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
            for i in 0..n {
                let s: *mut AnyObject = msg_send![files, objectAtIndex: i];
                let c: *const std::os::raw::c_char = msg_send![s, UTF8String];
                if !c.is_null() {
                    q.push(PathBuf::from(std::ffi::CStr::from_ptr(c).to_string_lossy().into_owned()));
                }
            }
            // NSApplicationDelegateReplySuccess
            let _: () = msg_send![app, replyToOpenOrPrint: 0usize];
        }
        if let Some(c) = CTX.get() {
            c.request_repaint();
        }
    }

    /// À appeler avant `eframe::run_native`.
    pub fn install() {
        let cls: &AnyClass = class!(NSObject);
        let imp: unsafe extern "C" fn() = unsafe { std::mem::transmute(open_files as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject)) };
        // SAFETY : signature « v@:@@ » conforme à application:openFiles: ; `class_addMethod` ne remplace pas une
        // méthode existante.
        unsafe {
            objc2::ffi::class_addMethod(cls as *const AnyClass as *mut objc2::ffi::objc_class, sel!(application:openFiles:).as_ptr(), Some(imp), b"v@:@@\0".as_ptr().cast());
        }
    }

    pub fn set_context(ctx: &egui::Context) {
        let _ = CTX.set(ctx.clone());
    }

    /// Fichiers reçus depuis le dernier appel.
    pub fn take() -> Vec<PathBuf> {
        std::mem::take(&mut *QUEUE.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

#[cfg(not(target_os = "macos"))]
pub mod finder {
    use eframe::egui;
    pub fn install() {}
    pub fn set_context(_: &egui::Context) {}
    pub fn take() -> Vec<std::path::PathBuf> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_aller_retour() {
        let p = Prefs { snap: false, snap_step: 0.5, sim_res: 0.25, post_path: "/x/mach3.cbpp".into(), recent: vec!["/a.cb".into(), "/b b.cb".into()] };
        assert_eq!(Prefs::parse(&p.to_text()), p);
        // valeurs absentes ou absurdes : défaut
        assert_eq!(Prefs::parse("snap_step=-2\nsim_res=0\nbidule=1"), Prefs::default());
    }

    #[test]
    fn fichiers_recents_sans_doublon_et_bornes() {
        let mut p = Prefs::default();
        for i in 0..12 {
            p.touch_recent(Path::new(&format!("/f{i}.cb")));
        }
        p.touch_recent(Path::new("/f5.cb"));
        assert_eq!(p.recent.len(), MAX_RECENT);
        assert_eq!(p.recent[0], "/f5.cb");
        assert_eq!(p.recent.iter().filter(|r| *r == "/f5.cb").count(), 1);
    }

    #[test]
    fn extension_ajoutee_si_absente() {
        assert_eq!(with_extension(PathBuf::from("/d/piece"), "cb"), PathBuf::from("/d/piece.cb"));
        assert_eq!(with_extension(PathBuf::from("/d/piece.CB"), "cb"), PathBuf::from("/d/piece.CB"));
        assert_eq!(with_extension(PathBuf::from("/d/prog.v2"), "nc"), PathBuf::from("/d/prog.v2.nc"));
    }

    #[cfg(unix)]
    #[test]
    fn resultat_de_dialogue() {
        use std::os::unix::process::ExitStatusExt;
        use std::process::{ExitStatus, Output};
        let out = |code: i32, so: &str, se: &str| Ok(Output { status: ExitStatus::from_raw(code << 8), stdout: so.into(), stderr: se.into() });
        assert_eq!(dialog_result(out(0, "/Users/x/a.cb\n", ""), "t").unwrap(), Some(PathBuf::from("/Users/x/a.cb")));
        assert_eq!(dialog_result(out(1, "", "execution error: User canceled. (-128)"), "t").unwrap(), None);
        assert_eq!(dialog_result(out(1, "", ""), "t").unwrap(), None);
        assert!(dialog_result(out(1, "", "syntax error"), "t").unwrap_err().contains("syntax error"));
        assert!(dialog_result(Err(std::io::Error::new(std::io::ErrorKind::NotFound, "absent")), "zenity").unwrap_err().contains("zenity"));
    }
}
