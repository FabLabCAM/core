use fablabcam_studio::furniture::*;

#[test]
fn revision_isolated_projects_roundtrip_and_concurrent_writes() {
    let root = tempfile::tempdir().unwrap();
    let mut library = Library::new(root.path());
    library.reload().unwrap();
    let template = library
        .documents
        .iter()
        .find(|d| d.id() == "caisson-osb")
        .unwrap()
        .clone();
    let mut old = Draft::existing(template.clone());
    let mut changed = Draft::existing(template);
    let mut first = Draft::new(Document::Furniture(Box::new(Furniture::from_template(
        changed.document.template().clone(),
    ))));
    library.save(&mut first).unwrap();
    changed.document.dimensions_mut().width = 700.0;
    library.save(&mut changed).unwrap();
    assert_eq!(changed.document.template().revision, 2);
    assert_eq!(first.document.template().revision, 1);
    assert_eq!(first.document.dimensions().width, 600.0);
    let second = Furniture::from_template(changed.document.template().clone());
    assert_eq!(second.dimensions.width, 700.0);
    assert_eq!(second.template.revision, 2);
    old.document.dimensions_mut().height = 900.0;
    assert!(library.save(&mut old).is_err());
    let encoded = first.document.to_json().unwrap();
    let imported = Document::from_json(&encoded).unwrap();
    assert_eq!(first.document, imported);
    let other_root = tempfile::tempdir().unwrap();
    let mut empty = Library::new(other_root.path());
    let mut fresh = Draft::new(imported);
    empty.save(&mut fresh).unwrap();
    assert!(fresh.document.check().is_ok());
    let other = Library::new(root.path());
    drop(other);
}

#[test]
fn duplicate_and_import_never_replace_originals() {
    let root = tempfile::tempdir().unwrap();
    let mut library = Library::new(root.path());
    library.reload().unwrap();
    let original = library.documents[0].clone();
    let mut collision = Draft::new(original.clone());
    assert!(library.save(&mut collision).is_err());
    let mut copy = Draft::new(original.duplicate());
    assert_ne!(copy.document.id(), original.id());
    library.save(&mut copy).unwrap();
    assert_eq!(
        library.documents.iter().find(|d| d.id() == original.id()),
        Some(&original)
    );
    assert_eq!(copy.document.template().revision, 1);
    let file = root.path().join("export.fcm-template.json");
    export_file(&file, copy.document.to_json().unwrap().as_bytes()).unwrap();
    assert_eq!(read_document(&file).unwrap(), copy.document);
    assert!(export_file(&file, b"overwrite").is_err());
    assert_eq!(read_document(&file).unwrap(), copy.document);
}

#[test]
fn invalid_imports_and_corrupt_catalog_leave_library_untouched() {
    let doc = Document::Template(Template::cabinet());
    let json = doc.to_json().unwrap();
    assert!(
        Document::from_json(&json.replace("\"schema_version\": 2", "\"schema_version\": 99"))
            .unwrap_err()
            .contains("Version de format")
    );
    assert!(Document::from_json("{}").is_err());
    assert!(Document::from_json(&json.replace("\"caisson-osb\"", "\"../outside\"")).is_err());
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("library.json"), "broken").unwrap();
    let mut library = Library::new(root.path());
    assert!(library.reload().is_err());
    let mut draft = Draft::new(doc.duplicate());
    assert!(library.save(&mut draft).is_err());
    assert_eq!(
        std::fs::read_to_string(root.path().join("library.json")).unwrap(),
        "broken"
    );
}

#[test]
fn history_and_save_baseline_track_actual_changes() {
    let original = Document::Template(Template::cabinet());
    let mut draft = Draft::existing(original.clone());
    assert!(!draft.dirty());
    draft.document.dimensions_mut().width = 700.0;
    draft.record(original.clone());
    assert!(draft.dirty());
    draft.undo();
    assert_eq!(draft.document, original);
    assert!(!draft.dirty());
    draft.redo();
    assert_eq!(draft.document.dimensions().width, 700.0);
    assert!(draft.dirty());
    let before = draft.document.clone();
    draft.document.dimensions_mut().width = 800.0;
    draft.record(before);
    assert!(!draft.can_redo());
    let new = Draft::new(original);
    assert!(new.dirty());
}

#[test]
fn typing_a_number_is_one_undo_transaction() {
    let mut d = Draft::existing(Document::Template(Template::cabinet()));
    for value in [7.0, 70.0, 700.0] {
        let before = d.document.clone();
        d.document.dimensions_mut().width = value;
        d.record_group(before, true);
    }
    d.undo();
    assert_eq!(d.document.dimensions().width, 600.0);
    assert!(!d.can_undo());
    d.redo();
    assert_eq!(d.document.dimensions().width, 700.0);
}

#[test]
fn duplicate_keeps_a_long_unicode_name_valid() {
    let mut t = Template::cabinet();
    t.name = "🪵".repeat(50);
    let doc = Document::Template(t);
    doc.check().unwrap();
    let copy = doc.duplicate();
    copy.check().unwrap();
    assert_ne!(doc.id(), copy.id());
    assert!(copy.name().ends_with(" — copie"));
}
