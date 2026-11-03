//! Machine command adapter. Reports always go to stdout, including failed validation.
mod commands;
use fablabcam_cam::gcode::machine::{convert, parse, validate, Profile, Status};
use fablabcam_studio::gcode_store::Store;
use std::{fs, io::Write};

pub const HELP: &str = "\
fablabcam machine — profils et contrôle statique de G-code

  machine profile init <sortie.json>                 ID CNC PRO - Mach3 à configurer
  machine profile add|update <profil.json> --store <dossier>
  machine profile list --store <dossier>
  machine profile show|delete <id> --store <dossier>
  machine parse <programme.nc>
  machine validate|stats <programme.nc> --profile <profil.json>
  machine convert <programme.nc> --from <source.json> --to <cible.json> -o <sortie.nc>
  machine archive add <id> <programme.nc> --profile <profil.json> --author <nom> --store <dossier>
  machine archive list <id> --store <dossier>
  machine archive show <id> <révision> --store <dossier>
  machine archive export <id> <révision> --store <dossier> -o <sortie.nc>

Rapports JSON : stdout. Codes de sortie : 0 succès, 1 erreur/validation invalide,
2 analyse incomplète ou conversion refusée. Les sorties existantes ne sont pas écrasées.
Les profils du magasin se consultent/exportent avec profile show ; --profile lit un fichier.
";
#[derive(Debug)]
pub struct Output { pub text: String, pub exit_code: i32 }
struct Args { pos: Vec<String>, options: std::collections::BTreeMap<String,String> }
impl Args {
    fn parse(args: &[String]) -> Result<Self,String> {
        let mut a = Self { pos: Vec::new(), options: Default::default() };
        let mut it = args.iter();
        while let Some(s) = it.next() {
            if s.starts_with('-') {
                if !["--profile","--from","--to","--store","--author","-o"].contains(&s.as_str()) { return Err(format!("Option inconnue : {s}")); }
                let v = it.next().ok_or_else(|| format!("{s} attend une valeur"))?;
                if a.options.insert(s.clone(),v.clone()).is_some() { return Err(format!("Option répétée : {s}")); }
            } else { a.pos.push(s.clone()); }
        }
        Ok(a)
    }
    fn option(&self,k: &str) -> Result<&str,String> { self.options.get(k).map(String::as_str).ok_or_else(|| format!("{k} requis")) }
    fn store(&self) -> Result<Store,String> { Ok(Store::new(self.option("--store")?)) }
    fn check(&self, count: usize, options: &[&str]) -> Result<(),String> {
        if self.pos.len() != count || self.options.keys().any(|s| !options.contains(&s.as_str())) { return Err(format!("Arguments inattendus.\n{HELP}")); }
        Ok(())
    }
}
fn read(p: &str) -> Result<String,String> { fs::read_to_string(p).map_err(|e| format!("{p} : {e}")) }
fn profile(p: &str) -> Result<Profile,String> { Profile::from_json(&read(p)?) }
fn write(p: &str, value: &str) -> Result<(),String> {
    let mut f = fs::OpenOptions::new().write(true).create_new(true).open(p).map_err(|e| format!("{p} : {e}"))?;
    f.write_all(value.as_bytes()).and_then(|()| f.sync_all()).map_err(|e| e.to_string())
}
fn json<T: serde::Serialize>(value: &T, exit_code: i32) -> Result<Output,String> {
    Ok(Output { text: serde_json::to_string_pretty(value).map_err(|e| e.to_string())?+"\n", exit_code })
}
pub fn run(args: &[String]) -> Result<Output,String> {
    if args.is_empty() || args == ["help"] || args == ["--help"] { return Ok(Output { text: HELP.into(), exit_code: 0 }); }
    let a = Args::parse(args)?;
    match a.pos.first().map(String::as_str) {
        Some("profile") => commands::profile_command(&a),
        Some("archive") => commands::archive_command(&a),
        Some("parse") => {
            a.check(2,&[])?;
            let parsed = parse(&read(&a.pos[1])?);
            let code = if parsed.diagnostics.iter().any(|d| d.severity == fablabcam_cam::gcode::machine::Severity::Error) {1} else if !parsed.diagnostics.is_empty() {2} else {0};
            json(&parsed,code)
        }
        Some("validate"|"stats") => {
            a.check(2,&["--profile"])?;
            let report = validate(&read(&a.pos[1])?,&profile(a.option("--profile")?)?);
            let code = match report.status { Status::Valid => 0, Status::Invalid => 1, Status::Incomplete => 2 };
            json(&report,code)
        }
        Some("convert") => {
            a.check(2,&["--from","--to","-o"])?;
            let destination = a.option("-o")?;
            let result = convert(&read(&a.pos[1])?,&profile(a.option("--from")?)?,&profile(a.option("--to")?)?);
            let code = if let Some(ref output) = result.output { write(destination,output)?; 0 } else { 2 };
            json(&result,code)
        }
        _ => Err(HELP.into()),
    }
}
