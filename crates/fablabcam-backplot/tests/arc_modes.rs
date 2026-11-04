use fablabcam_backplot::{parse_gcode, Kind};

#[test]
fn incremental_centers_do_not_enable_incremental_endpoints() {
    let bp = parse_gcode("G21 G90 G91.1\nG0 X20 Y10 Z0\nG3 X10 Y20 I-10 J0 F100\nG1 X5 Y20\n");
    assert!(bp.warnings.is_empty(), "{:?}", bp.warnings);
    assert_eq!(bp.segs[0].center, Some([10., 10.]));
    assert_eq!(bp.segs[0].to, [10., 20., 0.]);
    assert_eq!(bp.segs[1].to, [5., 20., 0.]);
}

#[test]
fn absolute_centers_do_not_disable_incremental_endpoints() {
    let bp = parse_gcode("G21 G90\nG0 X20 Y10 Z0\nG91 G90.1\nG3 X-10 Y10 I10 J10 F100\n");
    assert!(bp.warnings.is_empty(), "{:?}", bp.warnings);
    assert_eq!(bp.segs[0].kind, Kind::Arc);
    assert_eq!(bp.segs[0].center, Some([10., 10.]));
    assert_eq!(bp.segs[0].to, [10., 20., 0.]);
    assert!((bp.segs[0].len - 5. * std::f64::consts::PI).abs() < 1e-10);
}

#[test]
fn absolute_centers_follow_units_and_all_arc_planes() {
    for (plane, start, absolute, incremental) in [
        ("G17", "X2 Y1 Z0", "X1 Y2 I1 J1", "X1 Y2 I-1 J0"),
        ("G18", "Z2 X1 Y0", "Z1 X2 K1 I1", "Z1 X2 K-1 I0"),
        ("G19", "Y2 Z1 X0", "Y1 Z2 J1 K1", "Y1 Z2 J-1 K0"),
    ] {
        let a = parse_gcode(&format!("G20 G90 {plane} G90.1\nG0 {start}\nG3 {absolute} F10"));
        let b = parse_gcode(&format!("G20 G90 {plane} G91.1\nG0 {start}\nG3 {incremental} F10"));
        assert!(a.warnings.is_empty() && b.warnings.is_empty());
        assert_eq!(a.segs, b.segs, "{plane}");
        assert!((a.total_len() - 25.4 * std::f64::consts::FRAC_PI_2).abs() < 0.1);
    }
}

#[test]
fn unknown_fractional_codes_are_not_truncated_into_other_modes() {
    let bp = parse_gcode("G90\nG0 X10 Y10 Z0\nG91.2\nG1 X5 Y5 F100");
    assert_eq!(bp.segs[0].to, [5., 5., 0.]);
    assert!(bp.warnings.iter().any(|s| s.contains("G91.2")));
}
