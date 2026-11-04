use fablabcam_studio::{furniture::*, persist};
use std::io::{Cursor, Read};

#[test]
fn fabrication_bundle_has_consistent_geometry_units_and_machine_snapshot() {
    let f = Furniture::from_template(Template::cabinet());
    let bytes = fabrication_zip(&f, &ExportOptions::default()).unwrap();
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    assert_eq!(zip.len(), 15);
    let mut native = String::new();
    zip.by_name("meuble.fcm.json")
        .unwrap()
        .read_to_string(&mut native)
        .unwrap();
    assert_eq!(
        Document::from_json(&native).unwrap(),
        Document::Furniture(Box::new(f.clone()))
    );
    let mut csv = String::new();
    zip.by_name("liste-de-debit.csv")
        .unwrap()
        .read_to_string(&mut csv)
        .unwrap();
    assert_eq!(csv.lines().count(), 7);
    assert!(csv.contains("largeur_mm"));
    assert!(csv.contains("560;580;20"));
    let mut pdf = Vec::new();
    zip.by_name("notice-montage.pdf")
        .unwrap()
        .read_to_end(&mut pdf)
        .unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    let mut all_layers = std::collections::HashSet::new();
    for i in 0..3 {
        let mut cb = String::new();
        zip.by_name(&format!("dalle-{:02}.cb", i + 1))
            .unwrap()
            .read_to_string(&mut cb)
            .unwrap();
        let p = persist::load(&cb).unwrap();
        assert_eq!(p.machine_profile, Some(f.setup.machine.clone()));
        assert_eq!(p.stock.unwrap().max, [1500.0, 800.0]);
        assert_eq!(p.stock.unwrap().bottom, -20.0);
        assert!(p.ops.is_empty());
        for s in p.shapes {
            assert!(all_layers.insert(s.layer));
        }
        let mut svg = String::new();
        zip.by_name(&format!("dalle-{:02}.svg", i + 1))
            .unwrap()
            .read_to_string(&mut svg)
            .unwrap();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(xml.root_element().attribute("width"), Some("1500mm"));
        let mut dxf = String::new();
        zip.by_name(&format!("dalle-{:02}.dxf", i + 1))
            .unwrap()
            .read_to_string(&mut dxf)
            .unwrap();
        assert_eq!(
            fablabcam_studio::dxf::parse_dxf(&dxf).unwrap().shapes.len(),
            2
        );
    }
    assert_eq!(all_layers.len(), 6);
    assert!(!zip.file_names().any(|n| n.unwrap().ends_with(".nc")));
}

#[test]
fn changed_job_recomputes_and_invalid_layout_never_exports() {
    let mut f = Furniture::from_template(Template::cabinet());
    let first = cam_project(&f, 0).unwrap();
    f.dimensions.width = 700.0;
    let second = cam_project(&f, 0).unwrap();
    assert_ne!(first, second);
    f.dimensions.height = 1600.0;
    assert!(fabrication_zip(&f, &ExportOptions::default()).is_err());
}

#[test]
fn xml_extension_rejects_corrupt_machine_and_old_cb_remains_supported() {
    let f = Furniture::from_template(Template::cabinet());
    let p = cam_project(&f, 0).unwrap();
    let xml = persist::save(&p);
    let corrupt = xml.replace(
        "&quot;schema_version&quot;:1",
        "&quot;schema_version&quot;:99",
    );
    assert_ne!(xml, corrupt);
    assert!(persist::load(&corrupt).is_err());
    assert!(
        persist::load(&persist::save(&fablabcam_studio::Project::new()))
            .unwrap()
            .machine_profile
            .is_none()
    );
}

#[test]
fn cam_export_requires_operations_and_a_complete_machine_report() {
    use fablabcam_studio::{OpKind, Studio};
    let f = Furniture::from_template(Template::cabinet());
    let mut project = cam_project(&f, 0).unwrap();
    assert!(check_cam_export(&Studio::new(project.clone()), "M30")
        .unwrap_err()
        .contains("opérations"));
    let shape = project.shapes[0].id;
    let tool = project.tools[0].name.clone();
    project
        .add_operation(OpKind::Profile, "Contour", vec![shape], &tool)
        .unwrap();
    let studio = Studio::new(persist::load(&persist::save(&project)).unwrap());
    assert!(check_cam_export(&studio, &studio.gcode())
        .unwrap_err()
        .contains("Export bloqué"));
    // Existing non-furniture projects retain the original export contract.
    assert!(check_cam_export(&Studio::default(), "M30").is_ok());
}

#[test]
fn configured_fictional_machine_can_export_a_generated_furniture_toolpath() {
    use fablabcam_cam::gcode::machine::Profile;
    use fablabcam_studio::{OpKind, Studio};
    let mut f = Furniture::from_template(Template::cabinet());
    let mut machine =
        Profile::from_json(include_str!("../../../profiles/mach3-mill-demo.json")).unwrap();
    for (axis, limit) in [('X', 1500.0), ('Y', 800.0)] {
        let a = machine.axes.get_mut(&axis).unwrap();
        a.min = Some(0.0);
        a.max = Some(limit);
    }
    // This synthetic controller preserves its declared modes and position at M6.
    // This is a software fixture, never a suggested configuration of a real CNC.
    machine.tools.m6_preserves_state = true;
    machine.safety.required_preamble.clear();
    f.setup.machine = machine;
    let mut p = cam_project(&f, 0).unwrap();
    let ids = p.shapes.iter().map(|s| s.id).collect();
    let tool = p.tools[0].name.clone();
    p.add_operation(OpKind::Profile, "Contour", ids, &tool)
        .unwrap();
    let studio = Studio::new(persist::load(&persist::save(&p)).unwrap());
    let code = studio.gcode();
    assert!(code.contains("G1"));
    assert!(code.is_ascii());
    check_cam_export(&studio, &code).unwrap();
}
