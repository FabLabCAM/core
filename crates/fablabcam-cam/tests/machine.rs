use fablabcam_cam::gcode::machine::*;
const PRE: &str = "G21 G90 G17 G40 G49 G80 G94 G91.1\n";
fn demo() -> Profile { Profile::from_json(include_str!("../../../profiles/mach3-mill-demo.json")).unwrap() }
fn program(body: &str) -> String { format!("{PRE}{body}\nM30\n") }
fn has(r: &Report, code: &str) -> bool { r.diagnostics.iter().any(|d| d.code == code) }
fn valid(body: &str) -> Report {
    let r = validate(&program(body),&demo());
    assert_eq!(r.status,Status::Valid,"{:#?}",r.diagnostics); r
}
#[test] fn profiles_are_valid_and_strict() {
    Profile::mach3_mill().check().unwrap(); demo().check().unwrap();
    let mut json = serde_json::to_value(demo()).unwrap(); json["max_feedd"] = 100.into();
    assert!(Profile::from_json(&json.to_string()).is_err());
    let mut p = demo(); p.id = "../escape".into(); assert!(p.check().is_err());
    p = demo(); p.axes.get_mut(&'X').unwrap().min = Some(101.); assert!(p.check().is_err());
    p = demo(); p.defaults.units = "G22".into(); assert!(p.check().is_err());
}
#[test] fn fixture_is_valid_with_exact_statistics() {
    let r = validate(include_str!("fixtures/machine-valid.nc"),&demo());
    assert_eq!(r.status,Status::Valid,"{:?}",r.diagnostics);
    assert_eq!(r.statistics.tools,vec![1]);
    assert_eq!(r.statistics.bounds_min_mm,Some([0.,0.,0.]));
    assert_eq!(r.statistics.bounds_max_mm,Some([10.,10.,5.]));
    assert!((r.statistics.cutting_length_mm-(15.+5.*std::f64::consts::PI)).abs()<1e-8);
    assert!((r.statistics.rapid_length_mm-15.).abs()<1e-8);
    assert!((r.statistics.estimated_seconds-(2.45+std::f64::consts::PI/2.)).abs()<1e-8);
}
#[test] fn invalid_codes_positions_and_preamble_are_precise() {
    let r = validate(include_str!("fixtures/machine-invalid.nc"),&demo());
    assert_eq!(r.status,Status::Invalid);
    assert!(r.diagnostics.iter().any(|d| d.code=="travel" && d.line==3 && d.column==4 && d.word.as_deref()==Some("X")));
    assert!(r.diagnostics.iter().any(|d| d.code=="unsupported_code" && d.line==4 && d.word.as_deref()==Some("G123")));
    let r = validate("G0 X1\nM30",&demo()); assert!(has(&r,"preamble"));
}
#[test] fn recovery_keeps_lines_and_f64_precision() {
    let p = parse("G1 Xno\n  G1 X0.123456789123\nG2 X2 (unfinished\nG1 X3\n");
    assert_eq!(p.blocks.len(),4);
    assert_eq!(p.blocks[1].words[1].value,0.123456789123);
    assert!(p.diagnostics.iter().any(|d| d.line==1 && d.column==4));
    assert!(p.diagnostics.iter().any(|d| d.code=="comment" && d.line==3));
    assert_eq!(p.blocks[3].words[1].value,3.);
}
#[test] fn mode_order_and_incremental_units() {
    let r = valid("G1 X10 F600\nG91 X2\nG1 X-1\nG20 G90 X1 F10");
    assert!((r.statistics.bounds_max_mm.unwrap()[0]-25.4).abs()<1e-10);
    let r = valid("X10 G1 F600\nX5"); assert!((r.statistics.cutting_length_mm-15.).abs()<1e-10);
    assert!(has(&validate(&program("G1 X1 F600\nG20 X2"),&demo()),"feed_missing"));
}
#[test] fn modal_conflicts_duplicate_words_and_line_numbers() {
    for body in ["G0 G1 X1 F100","G20 G21","G90 G91","G0 X1 X2","M3 M5 S1000","M7 M9","M3 M3 S1000","G90.1 G91.1"] {
        let r = validate(&program(body),&demo()); assert_eq!(r.status,Status::Invalid,"{body}");
    }
    for body in ["N-1 G0 X1","N000001 G0 X1","N100000 G0 X1","G0 N1 X1","T1.5"] {
        assert_eq!(validate(&program(body),&demo()).status,Status::Invalid,"{body}");
    }
}
#[test] fn limits_spindle_feed_axis_and_safety() {
    for (body,code) in [("G1 X1 F4000","feed_limit"),("S25000 M3","spindle_speed"),("S500 M3","spindle_speed"),("G1 X1","feed_missing"),("G1 X1 F0","feed"),("G0 A10","axis")] {
        let r = validate(&program(body),&demo()); assert!(has(&r,code),"{body}: {:?}",r.diagnostics);
    }
    let mut p = demo(); p.axes.get_mut(&'Z').unwrap().max_feed = Some(100.);
    assert!(has(&validate(&program("G1 Z0 F300"),&p),"axis_feed_limit"));
    p = demo(); p.spindle.directions = vec!["cw".into()]; assert!(has(&validate(&program("M4 S1000"),&p),"spindle_direction"));
    p.safety.forbid_rapid_below_safe_z = true;
    assert!(has(&validate(&program("G0 Z4"),&p),"rapid_below_safe_z"));
}
#[test] fn reference_and_unknown_semantics_are_never_complete() {
    let r = validate(include_str!("fixtures/machine-valid.nc"),&Profile::mach3_mill());
    assert_eq!(r.status,Status::Incomplete,"{:?}",r.diagnostics);
    for body in ["G81 X1 Z-2 R5 F100","G68 X0 Y0 R45","G43 H1","M6","M98 P1","G0 X#1","/ G0 X2","G4 P2"] {
        let r = validate(&program(body),&demo()); assert_ne!(r.status,Status::Valid,"{body}"); assert!(!r.statistics.complete,"{body}");
    }
    let r = validate(&program("G1 X1 F100\nM6\nG0 X20"),&demo());
    assert_eq!(r.statistics.bounds_max_mm.unwrap()[0],1.);
}
#[test] fn positions_and_machine_offsets() {
    let mut p = demo(); p.work_offsets_mm.insert("G54".into(),[10.,20.,0.]);
    let r = validate(&program("G0 X1 Y2\nG53 G0 X3"),&p);
    assert_eq!(r.status,Status::Valid); assert_eq!(r.statistics.bounds_max_mm,Some([11.,22.,5.]));
    p.initial_position_mm = None;
    let r = validate(&program("G0 X1 Y2 Z5\nG1 X2 F100"),&p);
    assert_eq!(r.status,Status::Incomplete); assert_eq!(r.statistics.cutting_length_mm,1.);
    assert!(has(&validate(&program("G55 G0 X1"),&demo()),"work_offset"));
}
#[test] fn inverse_time_and_per_revolution() {
    let r = valid("G93 G1 X10 F2"); assert!((r.statistics.estimated_seconds-30.).abs()<1e-9);
    assert!(has(&validate(&program("G93 G1 X10 F2\nX20"),&demo()),"feed_missing"));
    let r = valid("M3 S1000\nG95 G1 X10 F0.1"); assert!((r.statistics.estimated_seconds-6.).abs()<1e-9);
}
#[test] fn comments_and_stops_are_dialect_sensitive() {
    valid("% this entire line is ignored G123\nG0 X1 // G123");
    assert!(has(&validate(&program("; not verified for Mach3"),&demo()),"comment_style"));
    assert!(has(&validate(&program("G0 X1 (nested (bad))"),&demo()),"comment"));
    assert_eq!(validate(&program("M0"),&demo()).status,Status::Incomplete);
    assert!(has(&validate(&program("M30\nG0 X1"),&demo()),"after_end"));
}
