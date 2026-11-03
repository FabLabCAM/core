use fablabcam_cam::gcode::machine::{Profile, Status};
use fablabcam_studio::gcode_store::Store;
use std::{fs, path::PathBuf};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        // Clock resolution alone cannot isolate tests running in parallel.
        let p = std::env::temp_dir().join(format!(
            "fablabcam-store-{}-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn demo() -> Profile { Profile::from_json(include_str!("../../../profiles/mach3-mill-demo.json")).unwrap() }
const PROGRAM: &str = "G21 G90 G17 G40 G49 G80 G94 G91.1\nG0 X10\nM30\n";
#[test] fn profile_crud_is_strict_and_archive_is_a_snapshot() {
    let temp = Temp::new(); let store = Store::new(&temp.0); let mut p = demo();
    assert!(store.profiles().unwrap().is_empty());
    store.save_profile(&p,false).unwrap(); assert!(store.save_profile(&p,false).is_err());
    assert_eq!(store.profile(&p.id).unwrap(),p); assert_eq!(store.profiles().unwrap().len(),1);
    let first = store.archive("piece",PROGRAM,&p,"FabLab").unwrap();
    assert_eq!(first.revision,1); assert_eq!(first.status,Status::Valid);
    p.axes.get_mut(&'X').unwrap().max = Some(5.); store.save_profile(&p,true).unwrap();
    let second = store.archive("piece",PROGRAM,&p,"FabLab").unwrap();
    assert_eq!(second.revision,2); assert_eq!(second.status,Status::Invalid);
    store.delete_profile(&p.id).unwrap(); assert!(store.profiles().unwrap().is_empty());
    let snapshot = store.revision("piece",1).unwrap(); assert_eq!(snapshot.program,PROGRAM); assert_eq!(snapshot.profile,demo());
    assert_eq!(store.revisions("piece").unwrap().len(),2);
    assert!(store.profile("../piece").is_err()); assert!(store.archive("../bad",PROGRAM,&p,"a").is_err());
    assert!(store.archive("piece",PROGRAM,&p,"").is_err());
    fs::write(temp.0.join("archives/piece/00000001/program.nc"),"G0 X999").unwrap();
    assert!(store.revision("piece",1).unwrap_err().contains("SHA-256"));
    assert!(store.revisions("piece").is_err());
}
#[test] fn concurrent_revisions_are_unique_and_incomplete_writes_stay_hidden() {
    let temp = Temp::new(); let store = Store::new(&temp.0);
    let threads: Vec<_> = (0..8).map(|_| { let store=store.clone(); std::thread::spawn(move || store.archive("piece",PROGRAM,&demo(),"test").unwrap().revision) }).collect();
    let mut revisions: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect(); revisions.sort_unstable();
    assert_eq!(revisions,(1..=8).collect::<Vec<_>>());
    fs::create_dir(temp.0.join("archives/piece/.pending-interrupted")).unwrap();
    assert_eq!(store.revisions("piece").unwrap().len(),8);
    assert_eq!(store.archive("piece",PROGRAM,&demo(),"test").unwrap().revision,9);
}
