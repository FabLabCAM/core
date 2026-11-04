use fablabcam_studio::{furniture::*, persist};
fn project() -> Furniture {
    let mut t = Template::cabinet();
    t.defaults.composition = Some(Box::new(Composition::from_dimensions(&t.defaults)));
    let mut f = Furniture::from_template(t);
    let link = connections(&structure(&f.dimensions).unwrap().panels)
        .into_iter()
        .find(|l| l.male == "T01" && l.female == "C1")
        .unwrap();
    f.dimensions.composition.as_mut().unwrap().joints.insert(
        link.id,
        JointSettings {
            kind: JointKind::MortiseTenon,
            blind: true,
            ..JointSettings::default()
        },
    );
    f
}
#[test]
fn fabrication_keeps_pockets_faces_depths_and_cb_requirements() {
    let f = project();
    let files = fabrication_files(&f, &ExportOptions::default()).unwrap();
    assert!(files.iter().any(|(name, _)| name == "operations.json"));
    assert!(files.iter().any(|(name, _)| name.ends_with("-verso.cb")));
    let mut pockets = 0;
    for (name, bytes) in files.iter().filter(|(name, _)| name.ends_with(".cb")) {
        let p = persist::load(std::str::from_utf8(bytes).unwrap()).unwrap();
        assert!(!p.furniture_requirements.is_empty(), "{name}");
        for r in &p.furniture_requirements {
            assert!(p.shapes.iter().any(|s| s.layer == r.layer));
            if r.pocket {
                pockets += 1;
                assert_eq!(r.depth, 8.0);
            }
        }
        assert_eq!(
            persist::load(&persist::save(&p))
                .unwrap()
                .furniture_requirements,
            p.furniture_requirements
        );
    }
    assert_eq!(pockets, 3);
}
#[test]
fn rotated_mirrored_features_remain_inside_placement_and_sheet() {
    let f = project();
    let layout = place(&f).unwrap();
    for s in layout.sheets {
        for p in s.placements {
            for path in
                std::iter::once(&p.panel.outline).chain(p.panel.features.iter().map(|f| &f.contour))
            {
                for face in [MachiningFace::Front, MachiningFace::Back] {
                    for q in path_points(&placed_contour(&p, path, face, f.setup.sheet_size[0])) {
                        assert!(
                            q[0] >= f.setup.margin - 1e-7
                                && q[0] <= f.setup.sheet_size[0] - f.setup.margin + 1e-7
                        );
                        assert!(
                            q[1] >= f.setup.margin - 1e-7
                                && q[1] <= f.setup.sheet_size[1] - f.setup.margin + 1e-7
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn different_thicknesses_never_share_a_stock_sheet() {
    let mut f = project();
    let c = f.dimensions.composition.as_mut().unwrap();
    c.root.division.as_mut().unwrap().children[0].bin = Some(Bin::default());
    let layout = place(&f).unwrap();
    for (i, s) in layout.sheets.iter().enumerate() {
        let t = s.placements[0].panel.thickness;
        assert!(s.placements.iter().all(|p| p.panel.thickness == t));
        assert_eq!(cam_project(&f, i).unwrap().stock.unwrap().bottom, -t);
    }
}
#[test]
fn saving_legacy_upgrades_explicitly_and_keeps_geometry() {
    let native = Document::Template(Template::cabinet())
        .to_json()
        .unwrap()
        .replace("\"schema_version\": 2", "\"schema_version\": 1");
    let old = Document::from_json(&native).unwrap();
    assert_eq!(old.template().schema_version, 1);
    let migrated = Document::from_json(&old.to_json().unwrap()).unwrap();
    assert_eq!(migrated.template().schema_version, 2);
    assert_eq!(
        panels(old.template(), old.dimensions()).unwrap(),
        panels(migrated.template(), migrated.dimensions()).unwrap()
    );
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("library.json");
    let bytes = format!("{{\"schema_version\":1,\"documents\":[{native}]}}");
    std::fs::write(&path, &bytes).unwrap();
    let mut library = Library::new(root.path());
    library.reload().unwrap();
    let mut draft = Draft::existing(library.documents[0].clone());
    *draft.document.name_mut() = "Meuble migré".into();
    library.save(&mut draft).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.path().join("library-schema-1.backup.json")).unwrap(),
        bytes
    );
}
