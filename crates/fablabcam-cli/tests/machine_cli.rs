use std::{fs, path::PathBuf, process::{Command,Output}};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        // Clock resolution alone cannot isolate tests running in parallel.
        let p = std::env::temp_dir().join(format!(
            "fablabcam-cli-{}-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn run(&self,args: &[&str]) -> Output { Command::new(env!("CARGO_BIN_EXE_fablabcam")).current_dir(&self.0).arg("machine").args(args).output().unwrap() }
}
impl Drop for Temp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
#[test] fn exit_codes_reports_conversion_and_archives() {
    let t = Temp::new();
    fs::write(t.0.join("demo.json"),include_str!("../../../profiles/mach3-mill-demo.json")).unwrap();
    fs::write(t.0.join("valid.nc"),include_str!("../../fablabcam-cam/tests/fixtures/machine-valid.nc")).unwrap();
    assert!(t.run(&["--help"]).status.success());
    assert!(t.run(&["profile","init","reference.json"]).status.success());
    let initialized: serde_json::Value = serde_json::from_str(&fs::read_to_string(t.0.join("reference.json")).unwrap()).unwrap();
    assert_eq!(initialized["name"], "ID CNC PRO - Mach3");
    assert_eq!(initialized["id"], "id-cnc-pro-1500x800");
    assert!(!t.run(&["profile","init","reference.json"]).status.success());
    let out = t.run(&["validate","valid.nc","--profile","demo.json"]);
    assert_eq!(out.status.code(),Some(0),"{}",String::from_utf8_lossy(&out.stdout));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap(); assert_eq!(report["status"],"valid");
    assert_eq!(t.run(&["validate","valid.nc","--profile","reference.json"]).status.code(),Some(2));
    fs::write(t.0.join("invalid.nc"),"G123\nM30").unwrap();
    assert_eq!(t.run(&["validate","invalid.nc","--profile","demo.json"]).status.code(),Some(1));
    assert!(t.run(&["convert","valid.nc","--from","demo.json","--to","demo.json","-o","converted.nc"]).status.success());
    assert!(t.run(&["validate","converted.nc","--profile","demo.json"]).status.success());
    let original = fs::read(t.0.join("converted.nc")).unwrap();
    assert!(!t.run(&["convert","invalid.nc","--from","demo.json","--to","demo.json","-o","converted.nc"]).status.success());
    assert_eq!(fs::read(t.0.join("converted.nc")).unwrap(),original);
    assert!(!t.run(&["convert","valid.nc","--from","demo.json","--to","demo.json","-o","converted.nc"]).status.success());
    assert!(t.run(&["profile","add","demo.json","--store","store"]).status.success());
    assert!(t.run(&["archive","add","part","valid.nc","--profile","demo.json","--author","FabLab","--store","store"]).status.success());
    assert!(t.run(&["archive","export","part","1","--store","store","-o","export.nc"]).status.success());
    assert_eq!(fs::read(t.0.join("valid.nc")).unwrap(),fs::read(t.0.join("export.nc")).unwrap());
    assert!(!t.run(&["validate","valid.nc","--profile","demo.json","extra"]).status.success());
    assert!(!t.run(&["validate","valid.nc","--profile","demo.json","--typo","x"]).status.success());
}
