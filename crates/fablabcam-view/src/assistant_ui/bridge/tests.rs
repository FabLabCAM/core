use super::*;
use assistant::{plan::Step, Action, Plan};
use std::sync::{atomic::AtomicBool, Arc};

fn app() -> App {
    let mut app = App::new(Default::default(), Some(1));
    app.assistant.path = None;
    app.assistant.session = Session::default();
    app.furniture = crate::furniture_ui::FurnitureUi::new(None);
    app.machines = crate::machine_ui::MachineUi::new(None);
    let mut c = Connection::for_provider(assistant::Provider::Mistral);
    c.model = "synthetic-model".into();
    c.tools = true;
    app.assistant.session.provider = c.provider;
    app.assistant.session.set_connection(c.clone());
    app.assistant.active_connection = Some(c);
    app
}
fn stage(app: &mut App, actions: Vec<Action>) {
    let w = app.assistant_workspace();
    let context = app.assistant_context(&w).unwrap();
    let plan = Plan {
        title: "Test transaction".into(),
        assumptions: vec![],
        missing: vec![],
        steps: actions.into_iter().map(|action| Step { explanation: "Test".into(), action }).collect(),
    };
    app.assistant.plan = Some(plan.clone());
    app.assistant.prepared = Some(assistant::actions::prepare(&w, &context, plan, Mode::Plan, Arc::new(AtomicBool::new(false))).unwrap());
    app.assistant.context = Some(context);
}

#[test]
fn one_ui_lot_undo_redo_restores_drawing_script_and_furniture() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.studio.edit(|p| p.add_shape("Test", fablabcam_studio::shape::circle((10.0, 20.0), 1.5)?)).unwrap();
    let before = app.assistant_workspace();
    stage(
        &mut app,
        vec![
            Action::SetCircleRadius { ids: vec![1], radius: 2.0 },
            Action::ReplaceScript { code: "circle(10,20,2);".into() },
            Action::CreateCabinet {
                name: "Test".into(),
                width: 600.0,
                depth: 400.0,
                height: 800.0,
                thickness: 18.0,
                shelves: 2,
                back: true,
            },
        ],
    );
    app.assistant_apply(&ctx).unwrap();
    let applied = app.assistant_workspace();
    assert_eq!(app.assistant.session.mode, Mode::Agent);
    assert!(app.furniture.open);
    assert!(applied.furniture.is_some());
    assert_ne!(applied.project, before.project);
    app.assistant_restore(false).unwrap();
    let undone = app.assistant_workspace();
    assert_eq!(undone.project, before.project);
    assert_eq!(undone.script, before.script);
    assert!(undone.furniture.is_none());
    app.assistant_restore(true).unwrap();
    let redone = app.assistant_workspace();
    assert_eq!(redone.project, applied.project);
    assert_eq!(redone.script, applied.script);
    assert_eq!(redone.furniture, applied.furniture);
    app.studio.edit(|p| p.add_shape("Test", fablabcam_studio::shape::circle((1.0, 1.0), 1.0)?)).unwrap();
    assert!(app.assistant_restore(false).is_err());
}

#[test]
fn changing_selection_or_provider_invalidates_ui_application() {
    let mut app = app();
    let ctx = egui::Context::default();
    stage(&mut app, vec![Action::ReplaceScript { code: "circle(1,1,1);".into() }]);
    let before = app.script_code.clone();
    app.selection = vec![999];
    assert!(app.assistant_apply(&ctx).is_err());
    assert_eq!(app.script_code, before);
    app.selection.clear();
    app.assistant.session.provider = assistant::Provider::Ollama;
    assert!(app.assistant_apply(&ctx).is_err());
    assert_eq!(app.script_code, before);
}

#[test]
fn page_navigation_keeps_conversation_and_correct_context() {
    let mut app = app();
    let ctx = egui::Context::default();
    let context = app.assistant_context(&app.assistant_workspace()).unwrap();
    app.assistant.session.push(true, "Conversation conservée".into(), &context, &app.assistant.session.connection());
    for (action, page) in [
        (crate::Action::System, "Outils"),
        (crate::Action::View(View::Simulation), "Simulation"),
        (crate::Action::Script, "Scripts"),
        (crate::Action::View(View::Vue3D), "3D"),
        (crate::Action::View(View::Plan), "CAO / Plan 2D"),
    ] {
        app.run(action, &ctx);
        assert_eq!(app.assistant_page(), page);
        assert_eq!(app.assistant.session.entries.len(), 1);
        assert_eq!(app.assistant.session.connection().model, "synthetic-model");
    }
}

#[test]
fn simulation_context_distinguishes_total_and_simulated_distance() {
    let mut app = app();
    app.sim = Some((
        app.studio.rev,
        0.55,
        0.5,
        fablabcam_studio::sim::SimOutput {
            map: fablabcam_studio::sim::Heightmap { min: [0.0, 0.0], res: 0.5, nx: 2, ny: 2, top: 0.0, bottom: -4.0, h: vec![0.0; 4] },
            gouges: vec![],
            total_len: 1000.0,
        },
    ));
    let c = app.assistant_context(&app.assistant_workspace()).unwrap();
    assert_eq!(c.data["simulation"]["total_path_length_mm"], 1000.0);
    assert_eq!(c.data["simulation"]["simulated_path_length_mm"], 550.0);
    assert_eq!(c.data["simulation"]["complete"], false);
    assert_eq!(c.data["simulation"]["stock"]["bottom"], -4.0);
}

#[test]
fn edited_plan_cannot_apply_an_older_preview() {
    let mut app = app();
    stage(&mut app, vec![Action::ReplaceScript { code: "circle(1,1,1);".into() }]);
    let before = app.script_code.clone();
    app.assistant.plan.as_mut().unwrap().steps.clear();
    assert!(app.assistant_apply(&egui::Context::default()).is_err());
    assert_eq!(app.script_code, before);
}
