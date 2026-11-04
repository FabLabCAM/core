//! Issue #23: the exported program must establish its own arc convention.
//! These are interpreter tests with a synthetic machine, not Mach3 machine trials.
use fablabcam_cam::{gcode::machine::{validate, Profile, Status}, ArcCenter, Post, PostDef};
use fablabcam_studio::{shape, OpKind, Project, Studio, Tool, ToolKind};

fn studio(inches: bool) -> Studio {
    let mut p = Project::new();
    p.gcode_inches = inches;
    let id = p.add_shape("Contour", shape::circle((25., 25.), 10.).unwrap()).unwrap();
    p.add_tool(Tool::new("Tool 3 mm", ToolKind::EndMill, 3.)).unwrap();
    let op = p.add_operation(OpKind::Profile, "Profile", vec![id], "Tool 3 mm").unwrap();
    p.update_operation(op, |o| { o.depth = -1.; o.step_down = 1.; }).unwrap();
    Studio::new(p)
}

fn machine(inches: bool, arc_mode: &str) -> Profile {
    let mut p = Profile::from_json(include_str!("../../../profiles/mach3-mill-demo.json")).unwrap();
    // Deliberately inherit incompatible controller defaults. M6 is assumed to
    // preserve state only in this synthetic test; real macros must be checked.
    p.defaults.arc_distance = arc_mode.into();
    p.defaults.units = "G20".into();
    p.defaults.distance = "G91".into();
    p.defaults.plane = "G18".into();
    p.defaults.feed_mode = "G93".into();
    p.tools.m6_preserves_state = true;
    if inches { p.safety.required_preamble[0] = "G20".into(); }
    p
}

#[test]
fn integrated_exports_reset_inherited_modes_before_motion() {
    for inches in [false, true] {
        let s = studio(inches);
        for gcode in [s.gcode(), s.gcode_with_post(&PostDef::default()).gcode] {
            let r = validate(&gcode, &machine(inches, "G90.1"));
            assert_eq!(r.status, Status::Valid, "{:#?}\n{gcode}", r.diagnostics);
            // A complete circle of the programmed scale, not a displaced arc.
            let lo = r.statistics.bounds_min_mm.unwrap();
            let hi = r.statistics.bounds_max_mm.unwrap();
            assert!(lo[0] >= -0.01 && hi[0] > 30. && hi[0] < 40.);
            assert!((lo[2] + 1.).abs() < 0.003);
            let bp = fablabcam_backplot::parse_gcode(&gcode);
            assert!(bp.warnings.is_empty(), "{:?}", bp.warnings);
            let (plot_lo, plot_hi) = bp.bounds().unwrap();
            assert!((plot_lo[2] - lo[2]).abs() < 1e-8);
            assert!((plot_hi[0] - hi[0]).abs() < 0.003);
        }
    }
}

#[test]
fn direct_cam_emitter_declares_incremental_arc_centers() {
    let s = studio(false);
    let computed = s.computed.values().next().unwrap();
    let g = fablabcam_cam::emit(computed.path.as_ref().unwrap(), computed.params.as_ref().unwrap(), &Post::default());
    let r = validate(&g, &machine(false, "G90.1"));
    assert_eq!(r.status, Status::Valid, "{:#?}", r.diagnostics);
}

#[test]
fn configurable_post_declares_the_selected_center_mode() {
    let s = studio(false);
    let def = PostDef { arc_center: ArcCenter::Absolute, ..PostDef::default() };
    let out = s.gcode_with_post(&def);
    assert!(out.warnings.is_empty());
    let mut p = machine(false, "G91.1");
    *p.safety.required_preamble.last_mut().unwrap() = "G90.1".into();
    let r = validate(&out.gcode, &p);
    assert_eq!(r.status, Status::Valid, "{:#?}", r.diagnostics);
}
