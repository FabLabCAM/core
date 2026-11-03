use super::*;

pub(super) fn profile_command(a: &Args) -> Result<Output,String> {
    match a.pos.get(1).map(String::as_str) {
        Some("init") => { a.check(3,&[])?; write(&a.pos[2],&Profile::default().to_json()?)?; json(&serde_json::json!({"created":a.pos[2]}),0) }
        Some("add"|"update") => { a.check(3,&["--store"])?; let p = profile(&a.pos[2])?; a.store()?.save_profile(&p,a.pos[1] == "update")?; json(&p,0) }
        Some("list") => { a.check(2,&["--store"])?; json(&a.store()?.profiles()?,0) }
        Some("show") => { a.check(3,&["--store"])?; json(&a.store()?.profile(&a.pos[2])?,0) }
        Some("delete") => { a.check(3,&["--store"])?; a.store()?.delete_profile(&a.pos[2])?; json(&serde_json::json!({"deleted":a.pos[2]}),0) }
        _ => Err(HELP.into()),
    }
}
pub(super) fn archive_command(a: &Args) -> Result<Output,String> {
    match a.pos.get(1).map(String::as_str) {
        Some("add") => {
            a.check(4,&["--store","--profile","--author"])?;
            let revision = a.store()?.archive(&a.pos[2],&read(&a.pos[3])?,&profile(a.option("--profile")?)?,a.option("--author")?)?;
            json(&revision,0)
        }
        Some("list") => { a.check(3,&["--store"])?; json(&a.store()?.revisions(&a.pos[2])?,0) }
        Some("show"|"export") => {
            let export = a.pos[1] == "export";
            a.check(4,if export { &["--store","-o"] } else { &["--store"] })?;
            let n: u64 = a.pos[3].parse().map_err(|_| "Révision entière requise")?;
            let archive = a.store()?.revision(&a.pos[2],n)?;
            if export { write(a.option("-o")?,&archive.program)?; json(&archive.metadata,0) } else { json(&archive,0) }
        }
        _ => Err(HELP.into()),
    }
}
