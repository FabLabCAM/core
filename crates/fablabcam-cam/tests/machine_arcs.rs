use fablabcam_cam::gcode::machine::*;
const PRE: &str = "G21 G90 G17 G40 G49 G80 G94 G91.1\n";
fn demo() -> Profile { Profile::from_json(include_str!("../../../profiles/mach3-mill-demo.json")).unwrap() }
fn report(body: &str, p: &Profile) -> Report { validate(&format!("{PRE}{body}\nM30\n"),p) }
#[test]
fn center_radius_tolerance_follows_program_units() {
    let mut p = demo();
    p.initial_position_mm = Some([25.4, 0., 5.]);
    // Mach3 Mill 10.7.3.2: 0.0002 inch in G20, 0.002 mm in G21.
    let inch = report("G20\nG3 X0 Y1.0001 I-1 J0 F10", &p);
    assert_eq!(inch.status, Status::Valid, "{:?}", inch.diagnostics);
    for body in ["G3 X0 Y25.40254 I-25.4 J0 F254", "G20\nG3 X0 Y1.0003 I-1 J0 F10"] {
        let r = report(body, &p);
        assert!(r.diagnostics.iter().any(|d| d.code == "arc"), "{body}: {:?}", r.diagnostics);
    }
}
#[test]
fn zero_end_radius_is_invalid_even_within_radius_tolerance() {
    let mut p = demo();
    // Exact block reported by Mach3 in #23 (displayed line 7393).
    p.initial_position_mm = Some([85.866, 36.550, 0.]);
    for body in [
        "G2 X85.866 Y36.549 I0 J-0.001 F100",
        "G90.1\nG3 X85.866 Y36.549 I85.866 J36.549 F100",
    ] {
        let r = report(body, &p);
        assert_eq!(r.status, Status::Invalid, "{body}: {:?}", r.diagnostics);
        assert!(r.diagnostics.iter().any(|d| d.code == "arc"));
    }
}
#[test] fn circular_extrema_detect_travel_beyond_endpoints() {
    let mut p = demo(); p.initial_position_mm = Some([1.,0.,5.]); p.axes.get_mut(&'Y').unwrap().max = Some(0.5);
    let r = report("G3 X-1 Y0 I-1 J0 F100",&p);
    assert_eq!(r.status,Status::Invalid); assert!(r.diagnostics.iter().any(|d| d.code=="travel" && d.line==2));
    assert!((r.statistics.bounds_max_mm.unwrap()[1]-1.).abs()<1e-10);
}
#[test] fn circles_helices_and_all_planes() {
    for (plane,start,body) in [
        ("G17",[1.,0.,5.],"G3 X1 Y0 I-1 J0 F100"),
        ("G18",[0.,5.,1.],"G3 Z1 X0 K-1 I0 F100"),
        ("G19",[5.,1.,0.],"G3 Y1 Z0 J-1 K0 F100"),
    ] {
        let mut p = demo(); p.initial_position_mm = Some(start);
        let r = report(&format!("{plane}\n{body}"),&p);
        assert_eq!(r.status,Status::Valid,"{:?}",r.diagnostics);
        assert!((r.statistics.cutting_length_mm-std::f64::consts::TAU).abs()<1e-10);
    }
    let mut p = demo(); p.initial_position_mm = Some([1.,0.,0.]);
    let r = report("G3 X1 Y0 Z2 I-1 J0 F100",&p);
    assert!((r.statistics.cutting_length_mm-std::f64::consts::TAU.hypot(2.)).abs()<1e-10);
    p.safety.allow_helical_arcs = false; assert_eq!(report("G3 X1 Y0 Z2 I-1 J0 F100",&p).status,Status::Invalid);
}
#[test] fn radius_short_long_and_absolute_centers() {
    let mut p = demo(); p.initial_position_mm = Some([1.,0.,5.]);
    for (body,length) in [("G3 X0 Y1 R1 F100",std::f64::consts::FRAC_PI_2),("G3 X0 Y1 R-1 F100",3.*std::f64::consts::FRAC_PI_2),("G90.1\nG3 X0 Y1 I0 J0 F100",std::f64::consts::FRAC_PI_2)] {
        let r = report(body,&p); assert_eq!(r.status,Status::Valid,"{:?}",r.diagnostics); assert!((r.statistics.cutting_length_mm-length).abs()<1e-10);
    }
    for body in ["G3 X0 Y1 R0.1 F100","G3 X0 Y1 I0 J0 F100","G3 X0 Y2 I-1 J0 F100","G3 X0 Y1 I-1 J0 R1 F100","G3 X0 Y1 I-1 J0 K2 F100","G90.1\nG3 X0 Y1 I0 F100"] { assert_eq!(report(body,&p).status,Status::Invalid,"{body}"); }
}
#[test] fn conversion_preserves_geometry_and_refuses_loss() {
    let from = demo(); let mut to = from.clone(); to.id = "custom-controller".into();
    to.controller.dialect = "custom-literal".into(); to.semantics = DialectSemantics::Rs274Mill; to.format.line_numbers = LineNumbers::Required;
    to.format.comments = vec![";".into()]; to.format.uppercase = true;
    let source = format!("{PRE}(test)\ng71\ng1 x10 F100\nM30\n");
    let c = convert(&source,&from,&to);
    assert!(c.output.is_some(),"{:?}",c.unsupported);
    assert_eq!(c.source_report.statistics.cutting_length_mm,c.target_report.unwrap().statistics.cutting_length_mm);
    let output = c.output.unwrap(); assert!(output.contains("N1 ")); assert!(output.contains(";test")); assert!(!output.contains("G71"));
    to.format.max_decimals = 2;
    let c = convert(&format!("{PRE}G1 X1.123 F100\nM30"),&from,&to); assert!(c.output.is_none()); assert!(c.unsupported.iter().any(|d| d.code=="precision_loss"));
    assert!(convert(&format!("{PRE}M6\nM30"),&from,&to).output.is_none());
    assert!(convert(&format!("{PRE}(MSG,hello)\nM30"),&from,&to).output.is_none());
    to.work_offsets_mm.insert("G54".into(),[1.,0.,0.]); assert!(convert(&source,&from,&to).output.is_none());
}

#[test] fn unit_aliases_require_the_mach3_semantics_model() {
    let mut p = demo(); p.semantics = DialectSemantics::Rs274Mill;
    let source = format!("{PRE}G1 X1 F100\nG70 X2\nM30");
    let r = validate(&source,&p);
    assert_eq!(r.status,Status::Incomplete);
    assert_eq!(r.statistics.bounds_max_mm.unwrap()[0],1.);
    assert!(convert(&source,&p,&demo()).output.is_none());
    let mut json = serde_json::to_value(p).unwrap(); json["semantics"] = "turning".into();
    assert!(Profile::from_json(&json.to_string()).is_err());
}
