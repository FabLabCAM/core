use fablabcam_cam::gcode::machine::{Profile, Status};
use fablabcam_studio::{
    gcode_store::Store,
    machine_profiles::{configuration, export_file, read_profile, Draft, Library},
};
use std::{fs, path::PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        // Clock resolution alone cannot isolate tests running in parallel.
        let path = std::env::temp_dir().join(format!(
            "fablabcam-profile-ui-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn library(&self) -> Library {
        Library::new(&self.0)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn demo() -> Profile {
    Profile::from_json(include_str!("../../../profiles/mach3-mill-demo.json")).unwrap()
}

#[test]
fn guided_creation_keeps_unknown_machine_parameters_unknown() {
    let temp = Temp::new();
    let mut lib = temp.library();
    lib.reload().unwrap();
    lib.create().unwrap();
    let draft = lib.draft.as_ref().unwrap();
    assert!(draft.dirty());
    assert!(draft
        .profile
        .axes
        .values()
        .all(|a| a.min.is_none() && a.max.is_none() && a.max_feed.is_none()));
    assert!(draft.profile.initial_position_mm.is_none());
    assert!(draft.profile.work_offsets_mm.is_empty());
    assert_eq!(configuration(&draft.profile).label(), "À configurer");
    lib.save().unwrap();
    assert!(!lib.dirty());
    let mut reopened = temp.library();
    reopened.reload().unwrap();
    assert_eq!(
        reopened.draft.as_ref().unwrap().profile,
        lib.draft.as_ref().unwrap().profile
    );
}
#[test]
fn navigation_and_refresh_cannot_discard_unsaved_or_unapplied_json() {
    let temp = Temp::new();
    let mut lib = temp.library();
    lib.create().unwrap();
    lib.save().unwrap();
    let id = lib.draft.as_ref().unwrap().profile.id.clone();
    lib.draft.as_mut().unwrap().profile.name = "Retouchée".into();
    assert!(lib.reload().is_err());
    assert!(lib.select(&id).is_err());
    assert!(lib.create().is_err());
    assert!(lib.duplicate().is_err());
    assert!(lib.stage_import(demo(), None, false).is_err());
    lib.discard();
    assert_eq!(lib.draft.as_ref().unwrap().profile.name, "Nouvelle machine");
    lib.draft.as_mut().unwrap().json = "{invalide".into();
    assert!(lib.dirty());
    assert!(lib.save().is_err());
    assert!(lib.reload().is_err());
    assert!(lib.draft.as_mut().unwrap().apply_json().is_err());
    assert_eq!(
        Store::new(&temp.0).profile(&id).unwrap().name,
        "Nouvelle machine"
    );
}
#[test]
fn json_edit_is_atomic_and_keeps_advanced_fields() {
    let mut p = demo();
    p.tools.table.insert(
        8,
        fablabcam_cam::gcode::machine::Tool {
            diameter_mm: 3.,
            length_mm: 21.,
            kind: "endmill".into(),
        },
    );
    p.tools.positions = Some(12);
    p.sources.push("https://example.org/machine-manual".into());
    let mut d = Draft::existing(p.clone());
    d.profile.name = "Atelier partagé".into();
    d.sync_json().unwrap();
    d.json = d.json.replace("Atelier partagé", "Atelier métal");
    d.apply_json().unwrap();
    assert_eq!(d.profile.name, "Atelier métal");
    assert_eq!(d.profile.tools, p.tools);
    assert_eq!(d.profile.sources, p.sources);
    let before = d.profile.clone();
    d.json = "{}".into();
    assert!(d.apply_json().is_err());
    assert_eq!(d.profile, before);
    assert!(d.ready().is_err());
    d.reset_json().unwrap();
    d.ready().unwrap();
}
#[test]
fn registered_id_cannot_change_even_through_json() {
    let mut d = Draft::existing(demo());
    let old = d.profile.clone();
    let mut renamed = old.clone();
    renamed.id = "another".into();
    d.json = renamed.to_json().unwrap();
    assert!(d.apply_json().is_err());
    assert_eq!(d.profile, old);
    d.reset_json().unwrap();
    d.profile.id = "another".into();
    assert!(d.ready().is_err());
}
#[test]
fn importing_a_collision_requires_a_copy_or_checked_replacement() {
    let temp = Temp::new();
    let store = Store::new(&temp.0);
    store.save_profile(&demo(), false).unwrap();
    let mut lib = temp.library();
    lib.reload().unwrap();
    let mut incoming = demo();
    incoming.name = "Nom importé".into();
    let expected = lib.import_conflict(&incoming).unwrap().unwrap();
    lib.stage_import(incoming.clone(), None, false).unwrap();
    assert!(lib.save().is_err());
    lib.discard();
    lib.stage_import(incoming.clone(), None, true).unwrap();
    let copied = lib.draft.as_ref().unwrap().profile.id.clone();
    assert_ne!(copied, incoming.id);
    lib.save().unwrap();
    assert_eq!(store.profile(&incoming.id).unwrap(), demo());
    lib.stage_import(incoming.clone(), Some(expected), false)
        .unwrap();
    assert_eq!(store.profile(&incoming.id).unwrap(), demo());
    lib.save().unwrap();
    assert_eq!(store.profile(&incoming.id).unwrap(), incoming);
    assert_eq!(store.profiles().unwrap().len(), 2);
}
#[test]
fn competing_editors_do_not_silently_overwrite_each_other() {
    let temp = Temp::new();
    let store = Store::new(&temp.0);
    store.save_profile(&demo(), false).unwrap();
    let (mut a, mut b) = (temp.library(), temp.library());
    a.reload().unwrap();
    b.reload().unwrap();
    a.draft.as_mut().unwrap().profile.location = Some("Atelier A".into());
    a.save().unwrap();
    b.draft.as_mut().unwrap().profile.location = Some("Atelier B".into());
    assert!(b.save().unwrap_err().contains("Conflit"));
    assert!(b.dirty());
    assert_eq!(
        store.profile(&demo().id).unwrap().location,
        Some("Atelier A".into())
    );
    b.discard();
    b.reload().unwrap();
    assert_eq!(b.draft.unwrap().profile.location, Some("Atelier A".into()));
}
#[test]
fn import_replacement_detects_a_change_after_confirmation() {
    let temp = Temp::new();
    let store = Store::new(&temp.0);
    store.save_profile(&demo(), false).unwrap();
    let mut lib = temp.library();
    lib.reload().unwrap();
    let expected = lib.import_conflict(&demo()).unwrap();
    lib.stage_import(demo(), expected, false).unwrap();
    let mut external = demo();
    external.name = "Modification CLI".into();
    store.save_profile(&external, true).unwrap();
    assert!(lib.save().is_err());
    assert_eq!(store.profile(&external.id).unwrap(), external);
}
#[test]
fn export_roundtrip_is_exact_and_refuses_overwrites() {
    let temp = Temp::new();
    fs::create_dir_all(&temp.0).unwrap();
    let path = temp.0.join("machine.json");
    let mut p = demo();
    p.description = "Caractères Unicode : école, ø, Ω".into();
    export_file(&p, &path).unwrap();
    assert_eq!(read_profile(&path).unwrap(), p);
    let before = fs::read(&path).unwrap();
    p.name = "Nouveau nom".into();
    assert!(export_file(&p, &path).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    p.id = "../escape".into();
    assert!(export_file(&p, &temp.0.join("rejected.json")).is_err());
    assert!(!temp.0.join("rejected.json").exists());
}
#[test]
fn malformed_and_oversized_imports_leave_existing_profile_intact() {
    let temp = Temp::new();
    fs::create_dir_all(&temp.0).unwrap();
    let path = temp.0.join("bad.json");
    fs::write(&path, "{}").unwrap();
    assert!(read_profile(&path).is_err());
    fs::write(&path, " ".repeat(1_048_577)).unwrap();
    assert!(read_profile(&path).unwrap_err().contains("1 Mio"));
    fs::write(
        &path,
        demo()
            .to_json()
            .unwrap()
            .replace("\"schema_version\": 1", "\"schema_version\": 2"),
    )
    .unwrap();
    assert!(read_profile(&path).is_err());
}
#[test]
fn editing_a_profile_preserves_archived_program_snapshots() {
    let temp = Temp::new();
    let store = Store::new(&temp.0);
    let p = demo();
    store.save_profile(&p, false).unwrap();
    let program = "G21 G90 G17 G40 G49 G80 G94 G91.1\nG0 X10\nM30\n";
    assert_eq!(
        store
            .archive("piece", program, &p, "Test atelier")
            .unwrap()
            .status,
        Status::Valid
    );
    let mut lib = temp.library();
    lib.reload().unwrap();
    lib.draft
        .as_mut()
        .unwrap()
        .profile
        .axes
        .get_mut(&'X')
        .unwrap()
        .max = Some(5.);
    lib.save().unwrap();
    assert_eq!(store.revision("piece", 1).unwrap().profile, p);
}
#[test]
fn configuration_status_is_never_a_program_validation() {
    let mut p = demo();
    assert_eq!(configuration(&p).label(), "Champs renseignés");
    p.safety.safe_z_mm = Some(200.);
    assert!(!configuration(&p).cautions.is_empty());
    p.axes.get_mut(&'X').unwrap().min = Some(200.);
    assert_eq!(configuration(&p).label(), "Erreur de configuration");
    assert_eq!(
        configuration(&Profile::mach3_mill()).label(),
        "À configurer"
    );
}
#[test]
fn duplicate_and_cancel_never_change_the_source_profile() {
    let temp = Temp::new();
    let store = Store::new(&temp.0);
    store.save_profile(&demo(), false).unwrap();
    let mut lib = temp.library();
    lib.reload().unwrap();
    lib.duplicate().unwrap();
    assert!(lib.dirty());
    assert_ne!(lib.draft.as_ref().unwrap().profile.id, demo().id);
    lib.discard();
    assert!(lib.draft.is_none());
    assert_eq!(store.profiles().unwrap(), vec![demo()]);
}
