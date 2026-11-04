use fablabcam_geom::{self as geom, PlineSource};
use fablabcam_model::Entity;
use fablabcam_studio::{dxf, persist, OpKind, OpSide, Project, Studio, Tool, ToolKind};

fn source() -> String {
    let e = geom::pline_from(
        &[
            (0., 70., 0.),
            (36., 70., 0.),
            (36., 60., 0.),
            (11., 60., 0.),
            (11., 40., 0.),
            (34., 40., 0.),
            (34., 30., 0.),
            (11., 30., 0.),
            (11., 10., 0.),
            (37., 10., 0.),
            (37., 0., 0.),
            (0., 0., 0.),
            (0., 70., 0.),
        ],
        true,
    );
    let mut shapes = vec![(
        "fixture".into(),
        Entity::Polyline(geom::rectangle(-10., 90., 300., 30.)),
    )];
    for n in 0..19 {
        let points: Vec<_> = e
            .iter_vertexes()
            .map(|v| (v.x + 45. * n as f64, v.y, v.bulge))
            .collect();
        shapes.push((
            "fixture".into(),
            Entity::Polyline(geom::pline_from(&points, true)),
        ));
    }
    dxf::write_dxf(&shapes)
}

fn imported() -> Project {
    let mut project = Project::new();
    for (layer, entity) in dxf::parse_dxf(&source()).unwrap().shapes {
        project.add_shape(&layer, entity).unwrap();
    }
    project
        .add_tool(Tool::new("Fraise 3 mm", ToolKind::EndMill, 3.))
        .unwrap();
    project
}

#[test]
fn profile_nineteen_imported_closed_letters_preserves_geometry_and_history() {
    let mut studio = Studio::new(imported());
    let shapes = studio.project().shapes.clone();
    let ids: Vec<_> = shapes.iter().skip(1).map(|s| s.id).collect();
    let id = studio
        .edit(|p| p.add_operation(OpKind::Profile, "Lettres", ids.clone(), "Fraise 3 mm"))
        .unwrap();
    for side in [OpSide::Outside, OpSide::Inside, OpSide::On] {
        studio
            .edit(|p| p.update_operation(id, |o| o.side = side))
            .unwrap();
        assert!(
            studio.computed[&id].error.is_none(),
            "{:?}",
            studio.computed[&id].error
        );
        assert!(!studio.computed[&id].path.as_ref().unwrap().moves.is_empty());
        studio.check_computed().unwrap();
    }
    assert_eq!(studio.project().shapes, shapes);
    let loaded = Studio::new(persist::load(&persist::save(studio.project())).unwrap());
    loaded.check_computed().unwrap();
    assert_eq!(loaded.gcode(), studio.gcode());
    for _ in 0..3 {
        assert!(studio.undo());
    }
    assert!(studio.project().ops.is_empty());
    assert_eq!(studio.project().shapes, shapes);
    assert!(studio.redo());
    studio.check_computed().unwrap();
}

#[test]
fn isolated_shapes_and_frame_also_compute() {
    let project = imported();
    for shape in &project.shapes {
        let mut p = project.clone();
        p.add_operation(OpKind::Profile, "Seul", vec![shape.id], "Fraise 3 mm")
            .unwrap();
        Studio::new(p).check_computed().unwrap();
    }
}

#[test]
fn failed_calculation_cannot_export_stale_paths_and_remains_editable() {
    let mut project = imported();
    let shape = project.shapes[1].id;
    let id = project
        .add_operation(OpKind::Profile, "E", vec![shape], "Fraise 3 mm")
        .unwrap();
    let mut studio = Studio::new(project);
    studio.check_computed().unwrap();
    studio
        .edit(|p| p.update_operation(id, |o| o.step_down = 0.))
        .unwrap();
    assert!(
        studio.computed[&id].path.is_some(),
        "retain previous preview"
    );
    assert!(studio.check_computed().is_err());
    assert!(fablabcam_studio::furniture::check_cam_export(&studio, &studio.gcode()).is_err());
    assert!(studio.undo());
    studio.check_computed().unwrap();
}
