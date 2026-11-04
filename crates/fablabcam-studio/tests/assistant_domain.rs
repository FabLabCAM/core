use fablabcam_studio::{
    assistant::{actions, context::ContextMeta, plan::Step, Action, ContextOptions, ContextSnapshot, Mode, Plan, Workspace},
    shape, Project, Tool, ToolKind,
};
use std::sync::{atomic::AtomicBool, Arc};

fn workspace() -> Workspace {
    let mut project = Project::new();
    for (x, y) in [(10.0, 10.0), (90.0, 10.0), (10.0, 50.0), (90.0, 50.0)] {
        project.add_shape("Plaque", shape::circle((x, y), 1.5).unwrap()).unwrap();
    }
    Workspace { document_id: "document-a".into(), revision: 1, project, script: "circle(10,10,1.5);".into(), furniture: None }
}
fn context(w: &Workspace) -> ContextSnapshot {
    ContextSnapshot::capture(
        w,
        &ContextMeta { page: "CAO".into(), selection: vec![1, 2, 3, 4], ..Default::default() },
        &ContextOptions::default(),
    )
    .unwrap()
}
fn plan(actions: Vec<Action>) -> Plan {
    Plan {
        title: "Essai".into(),
        assumptions: vec![],
        missing: vec![],
        steps: actions.into_iter().map(|action| Step { explanation: "Vérifier".into(), action }).collect(),
    }
}
fn prepare(w: &Workspace, p: Plan, mode: Mode) -> Result<fablabcam_studio::assistant::PreparedPlan, String> {
    actions::prepare(w, &context(w), p, mode, Arc::new(AtomicBool::new(false)))
}

#[test]
fn ask_cannot_prepare_and_plan_cannot_apply() {
    let w = workspace();
    let p = plan(vec![Action::SetCircleRadius { ids: vec![1, 2, 3, 4], radius: 2.0 }]);
    assert!(prepare(&w, p.clone(), Mode::Ask).is_err());
    let result = prepare(&w, p, Mode::Plan).unwrap();
    assert!(actions::authorize(&result, &w, &context(&w), Mode::Plan, false).is_err());
    assert!(actions::authorize(&result, &w, &context(&w), Mode::Agent, false).is_ok());
    assert_eq!(w.project.shapes[0].entity, fablabcam_model::Entity::Circle { cx: 10.0, cy: 10.0, r: 1.5 });
    for (before, after) in w.project.shapes.iter().zip(&result.after.project.shapes) {
        let fablabcam_model::Entity::Circle { cx, cy, .. } = before.entity else { panic!() };
        assert_eq!(after.entity, fablabcam_model::Entity::Circle { cx, cy, r: 2.0 });
    }
}

#[test]
fn failed_batch_never_mutates_source_and_undo_redo_is_atomic() {
    let w = workspace();
    let fingerprint = w.fingerprint();
    assert!(prepare(
        &w,
        plan(vec![Action::MoveShapes { ids: vec![1], dx: 20.0, dy: 0.0 }, Action::SetCircleRadius { ids: vec![999], radius: 2.0 }]),
        Mode::Agent
    )
    .is_err());
    assert_eq!(fingerprint, w.fingerprint());
    let p = prepare(
        &w,
        plan(vec![Action::MoveShapes { ids: vec![1], dx: 20.0, dy: 0.0 }, Action::MoveShapes { ids: vec![2], dx: -20.0, dy: 0.0 }]),
        Mode::Agent,
    )
    .unwrap();
    let mut studio = fablabcam_studio::Studio::new(w.project.clone());
    studio
        .edit(|project| {
            *project = p.after.project.clone();
            Ok(())
        })
        .unwrap();
    assert!(studio.undo());
    assert_eq!(studio.project(), &w.project);
    assert!(studio.redo());
    assert_eq!(studio.project(), &p.after.project);
}

#[test]
fn new_document_selection_revision_and_stop_invalidate_approval() {
    let w = workspace();
    let p = prepare(&w, plan(vec![Action::MoveShapes { ids: vec![1], dx: 1.0, dy: 0.0 }]), Mode::Agent).unwrap();
    for change in 0..4 {
        let mut current = w.clone();
        let mut c = context(&w);
        match change {
            0 => current.document_id = "other-document".into(),
            1 => current.revision += 1,
            2 => current.script.push_str("// edit"),
            _ => c.token = "changed-selection".into(),
        }
        assert!(actions::authorize(&p, &current, &c, Mode::Agent, false).is_err());
    }
    assert!(actions::authorize(&p, &w, &context(&w), Mode::Agent, true).is_err());
}

#[test]
fn script_edit_and_execution_are_distinct_and_replay_is_explicit() {
    let w = workspace();
    let edited = prepare(&w, plan(vec![Action::ReplaceScript { code: "circle(10,10,2.0);".into() }]), Mode::Agent).unwrap();
    assert_eq!(edited.after.project, w.project);
    assert!(prepare(&w, plan(vec![Action::RunScript { code: "circle(1,1,1);".into() }]), Mode::Plan).is_err());
    let first = prepare(&edited.after, plan(vec![Action::RunScript { code: edited.after.script.clone() }]), Mode::Agent).unwrap();
    assert_eq!(first.after.project.shapes.len(), 5);
    let second = prepare(&first.after, plan(vec![Action::RunScript { code: first.after.script.clone() }]), Mode::Agent).unwrap();
    assert_eq!(second.after.project.shapes.len(), 6);
    assert!(prepare(&w, plan(vec![Action::ReplaceScript { code: "let = ;".into() }]), Mode::Agent).is_err());
}

#[test]
fn tool_copy_preserves_source_and_operations() {
    let mut w = workspace();
    let t = Tool::new("Fraise 6", ToolKind::EndMill, 6.0);
    w.project.add_tool(t.clone()).unwrap();
    let p =
        prepare(&w, plan(vec![Action::DuplicateTool { source: "Fraise 6".into(), name: "Fraise 4".into(), diameter: 4.0 }]), Mode::Agent)
            .unwrap();
    assert_eq!(p.after.project.tool("Fraise 6"), w.project.tool("Fraise 6"));
    let copy = p.after.project.tool("Fraise 4").unwrap();
    assert_eq!(copy.diameter, 4.0);
    assert_eq!(copy.feed, t.feed);
    assert_eq!(copy.spindle, t.spindle);
    assert_eq!(p.after.project.ops, w.project.ops);
    assert!(prepare(
        &w,
        plan(vec![Action::DuplicateTool { source: "Fraise 6".into(), name: "Fraise 6".into(), diameter: 4.0 }]),
        Mode::Agent
    )
    .is_err());
}

#[test]
fn cabinet_and_partition_use_existing_geometry_validation() {
    let w = workspace();
    let p = prepare(
        &w,
        plan(vec![Action::CreateCabinet {
            name: "Caisson test".into(),
            width: 600.0,
            depth: 400.0,
            height: 800.0,
            thickness: 18.0,
            shelves: 2,
            back: true,
        }]),
        Mode::Agent,
    )
    .unwrap();
    let d = p.after.furniture.as_ref().unwrap();
    let structure = fablabcam_studio::furniture::structure(d.dimensions()).unwrap();
    assert_eq!(structure.panels.len(), 7);
    let cell = structure.cells.iter().find(|c| c.leaf).unwrap().id.clone();
    let divided = prepare(&p.after, plan(vec![Action::DivideCell { cell_id: cell, vertical: true, count: 2 }]), Mode::Agent).unwrap();
    assert_eq!(fablabcam_studio::furniture::structure(divided.after.furniture.as_ref().unwrap().dimensions()).unwrap().panels.len(), 8);
    assert!(prepare(&p.after, plan(vec![Action::ResizeFurniture { width: -1.0, depth: 400.0, height: 800.0 }]), Mode::Agent).is_err());
}

#[test]
fn context_filters_do_not_leak_omitted_sections_or_claim_simulation_complete() {
    let w = workspace();
    let meta = ContextMeta { simulation: serde_json::json!({"progress":0.58,"complete":false}), ..Default::default() };
    let c = ContextSnapshot::capture(&w, &meta, &ContextOptions::default()).unwrap();
    assert!(c.data.get("script").is_none());
    assert!(c.data.get("furniture").is_none());
    assert_eq!(c.data["simulation"]["complete"], false);
    assert_eq!(c.data["drawing"].as_array().unwrap().len(), 0);
}

#[test]
fn bounded_scripts_and_cancel_leave_project_unchanged() {
    let w = workspace();
    for code in ["loop {}", "let a = \"a\"; loop {a += a;}", "import \"missing-file\" as x;"] {
        assert!(fablabcam_studio::script::run(&w.project, code).is_err());
    }
    assert!(fablabcam_studio::script::run_controlled(&w.project, "circle(1,1,2);", Arc::new(AtomicBool::new(true))).is_err());
    assert_eq!(w.project.shapes.len(), 4);
}

#[test]
fn exports_are_separate_and_require_their_document() {
    use fablabcam_studio::assistant::plan::ExportFormat;
    let w = workspace();
    let export = Action::PrepareExport { format: ExportFormat::Cb };
    assert!(prepare(&w, plan(vec![Action::MoveShapes { ids: vec![1], dx: 1.0, dy: 0.0 }, export.clone()]), Mode::Agent).is_err());
    let p = prepare(&w, plan(vec![export]), Mode::Agent).unwrap();
    assert_eq!(p.before.fingerprint(), p.after.fingerprint());
    assert_eq!(p.exports, vec![ExportFormat::Cb]);
    assert!(prepare(&w, plan(vec![Action::PrepareExport { format: ExportFormat::FurnitureZip }]), Mode::Agent).is_err());
}

#[test]
fn updating_tool_rejects_invalid_values_without_changing_source() {
    let mut w = workspace();
    w.project.add_tool(Tool::new("Fraise", ToolKind::EndMill, 6.0)).unwrap();
    let p = prepare(
        &w,
        plan(vec![Action::UpdateTool { name: "Fraise".into(), diameter: 4.0, feed: 600.0, plunge: 200.0, spindle: 12000.0 }]),
        Mode::Agent,
    )
    .unwrap();
    assert_eq!(p.after.project.tool("Fraise").unwrap().diameter, 4.0);
    assert_eq!(w.project.tool("Fraise").unwrap().diameter, 6.0);
    assert!(prepare(
        &w,
        plan(vec![Action::UpdateTool { name: "Fraise".into(), diameter: -1.0, feed: 600.0, plunge: 200.0, spindle: 12000.0 }]),
        Mode::Agent
    )
    .is_err());
}

#[test]
fn geometry_context_names_radius_and_diameter_with_units() {
    let w = workspace();
    let c = context(&w);
    let g = &c.data["drawing"][0]["geometry"];
    assert_eq!(g["type"], "circle");
    assert_eq!(g["center_mm"], serde_json::json!([10.0, 10.0]));
    assert_eq!(g["radius_mm"], 1.5);
    assert_eq!(g["diameter_mm"], 3.0);
}
