use fablabcam_studio::{furniture::*, persist, shape, OpKind, Project, Studio, Tool, ToolKind};
#[test]
fn pocket_island_cannot_satisfy_required_material_removal() {
    let mut p = Project::new();
    p.add_tool(Tool::new("endmill", ToolKind::EndMill, 6.0))
        .unwrap();
    let outer = p
        .add_shape(
            "outer",
            shape::rectangle((10.0, 10.0), (110.0, 110.0)).unwrap(),
        )
        .unwrap();
    let inner = p
        .add_shape(
            "slot",
            shape::rectangle((40.0, 40.0), (60.0, 60.0)).unwrap(),
        )
        .unwrap();
    p.furniture_requirements.push(CamRequirement {
        layer: "slot".into(),
        depth: 8.0,
        pocket: true,
        inside: true,
    });
    let op = p
        .add_operation(OpKind::Pocket, "pocket", vec![outer, inner], "endmill")
        .unwrap();
    p.update_operation(op, |o| o.depth = -8.0).unwrap();
    let studio = Studio::new(p.clone());
    assert!(studio.computed[&op].error.is_none());
    assert!(check_cam_export(&studio, &studio.gcode())
        .unwrap_err()
        .contains("ce seul contour"));
    p.update_operation(op, |o| o.shapes = vec![inner]).unwrap();
    let studio = Studio::new(p);
    assert!(studio.computed[&op].error.is_none());
    check_cam_export(&studio, &studio.gcode()).unwrap();
}
#[test]
fn furniture_requirement_text_roundtrips_xml_special_characters() {
    let mut p = Project::new();
    p.furniture_requirements.push(CamRequirement {
        layer: "bad]]>layer<&\"".into(),
        depth: 8.0,
        pocket: true,
        inside: true,
    });
    let roundtrip = persist::load(&persist::save(&p)).unwrap();
    assert_eq!(roundtrip.furniture_requirements, p.furniture_requirements);
}
#[test]
fn maximum_unicode_name_can_be_saved_as_template() {
    let mut f = Furniture::from_template(Template::cabinet());
    f.name = "é".repeat(100);
    let t = Document::Furniture(Box::new(f)).as_template();
    assert!(t.name.len() <= 200);
    t.check().unwrap();
}
