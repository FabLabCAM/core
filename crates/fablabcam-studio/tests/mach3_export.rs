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

#[test]
fn lyon_v1_first_arc_requires_incremental_centers_in_every_export() {
    use fablabcam_cam::{Move, Toolpath};
    use fablabcam_cam::gcode::machine::Severity;
    // Reduced from the first arc (physical line 145). Unknown machine limits
    // stay unknown; this checks interpretation, not real machine clearance.
    let mut p = Profile::from_json(include_str!("../../../profiles/mach3-mill.json")).unwrap();
    p.initial_position_mm = Some([0., 0., 10.]);
    p.work_offsets_mm.insert("G54".into(), [0., 0., 0.]);
    p.tools.m6_preserves_state = true;
    let required = p.safety.required_preamble.clone();
    p.safety.required_preamble = ["G21", "G90", "G17", "G94"].map(str::to_owned).to_vec();
    let old = "G21 G90 G17 G94\nM6 T1\nG0 X781.243 Y198.202 Z3\nG1 Z-1 F500\nG2 X782.743 Y196.702 I0 J-1.5 F1000\nM30\n";
    p.defaults.arc_distance = "G90.1".into();
    let bad = validate(old, &p);
    assert_eq!(bad.status, Status::Invalid);
    assert!(bad.diagnostics.iter().any(|d| d.code == "arc" && d.line == 5));
    p.defaults.arc_distance = "G91.1".into();
    let reference = validate(old, &p);
    assert_eq!(reference.status, Status::Incomplete);
    assert!(!reference.diagnostics.iter().any(|d| d.severity == Severity::Error));
    let expected = 4. + 1.5 * std::f64::consts::FRAC_PI_2;
    assert!((reference.statistics.cutting_length_mm - expected).abs() < 1e-9);

    let mut s = studio(false);
    let computed = s.computed.values_mut().next().unwrap();
    computed.path = Some(Toolpath { moves: vec![
        Move::Rapid { x: Some(781.243), y: Some(198.202), z: Some(3.) },
        Move::Feed { x: None, y: None, z: Some(-1.), f: 500. },
        Move::Arc { ccw: false, x: 782.743, y: 196.702, z: None, i: 0., j: -1.5, f: 1000. },
    ] });
    let direct = fablabcam_cam::emit(computed.path.as_ref().unwrap(), computed.params.as_ref().unwrap(), &Post::default());
    p.safety.required_preamble = required;
    for gcode in [direct, s.gcode(), s.gcode_with_post(&PostDef::default()).gcode] {
        for mode in ["G90.1", "G91.1"] {
            p.defaults.arc_distance = mode.into();
            let r = validate(&gcode, &p);
            assert_eq!(r.status, Status::Incomplete, "{:?}\n{gcode}", r.diagnostics);
            assert!(!r.diagnostics.iter().any(|d| d.severity == Severity::Error));
            assert!((r.statistics.cutting_length_mm - expected).abs() < 1e-9);
            assert_eq!(r.statistics.bounds_max_mm.unwrap()[0], 782.743);
        }
    }
}
