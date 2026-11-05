//! Rounded arc centers must not coincide with either endpoint (#23).
use fablabcam_cam::*;
use fablabcam_cam::gcode::machine::{validate, Profile, Report, Status};

fn path(start: (f64, f64), end: (f64, f64), offset: (f64, f64), ccw: bool) -> Toolpath {
    Toolpath { moves: vec![
        Move::Rapid { x: Some(start.0), y: Some(start.1), z: Some(0.) },
        Move::Arc { ccw, x: end.0, y: end.1, z: Some(-0.2), i: offset.0, j: offset.1, f: 100. },
    ] }
}

fn outputs(path: &Toolpath, decimals: usize) -> Vec<(String, bool)> {
    let p = Params::default();
    let mut result = vec![(emit(path, &p, &Post { decimals, ..Post::default() }), false)];
    for absolute in [false, true] {
        let def = PostDef {
            number_format: format!("0.{}", "#".repeat(decimals)),
            arc_center: if absolute { ArcCenter::Absolute } else { ArcCenter::IncrementalFromStart },
            ..PostDef::default()
        };
        let prog = Program { name: "rounding".into(), date: String::new(), file_header: String::new(), file_footer: String::new(), inches: false, mops: vec![Mop {
            name: "arc".into(), header: String::new(), footer: String::new(),
            tool: ToolInfo { index: 1, name: "T1".into(), diameter: 3., comment: String::new(), change: String::new() },
            path, params: &p,
        }] };
        result.push((render_post(&def, &prog).gcode, absolute));
    }
    result
}

fn check(g: &str, absolute: bool) -> Report {
    let mut p = Profile::from_json(include_str!("../../../profiles/mach3-mill-demo.json")).unwrap();
    p.tools.m6_preserves_state = true;
    if absolute { *p.safety.required_preamble.last_mut().unwrap() = "G90.1".into(); }
    let r = validate(g, &p);
    assert_eq!(r.status, Status::Valid, "{g}\n{:?}", r.diagnostics);
    assert!(g.contains("Z-0.2"), "helical endpoint lost: {g}");
    assert!(g.contains("F100"), "feed lost: {g}");
    r
}

#[test]
fn tiny_arcs_are_linearized_when_rounding_collapses_a_radius() {
    for decimals in [3, 4] {
        let q = 10_f64.powi(-(decimals as i32));
        for ccw in [false, true] {
            for r in [0.0007, 0.0002] {
                let r = r * q / 0.001;
                // Valid semicircle; at 3 decimals the first case becomes the
                // photographed block, the second rounds both I/J to zero.
                let path = path((85.866, 36.550), (85.866, 36.550 - 2. * r), (0., -r), ccw);
                for (g, absolute) in outputs(&path, decimals) {
                    assert!(!g.lines().any(|l| l.starts_with("G2 ") || l.starts_with("G3 ")), "{g}");
                    check(&g, absolute);
                }
            }
        }
    }
}

#[test]
fn partial_arc_rounded_closed_does_not_become_a_full_circle() {
    let a: f64 = 0.00003;
    let path = path((20., 10.), (10. + 10. * a.cos(), 10. + 10. * a.sin()), (-10., 0.), true);
    for (g, absolute) in outputs(&path, 3) {
        assert!(!g.lines().any(|l| l.starts_with("G3 ")), "{g}");
        check(&g, absolute);
    }
}

#[test]
fn representable_circles_and_helices_keep_their_arcs() {
    let path = path((20., 10.), (20., 10.), (-10., 0.), true);
    for (g, absolute) in outputs(&path, 3) {
        assert!(g.lines().any(|l| l.starts_with("G3 ")), "{g}");
        check(&g, absolute);
    }
}

#[test]
fn major_arc_rounded_closed_preserves_its_sweep_and_helical_depth() {
    let a: f64 = 0.00003;
    // Same endpoints as the short CCW arc, but clockwise means almost a full
    // turn. A single replacement chord would silently remove the contour.
    let path = path((20., 10.), (10. + 10. * a.cos(), 10. + 10. * a.sin()), (-10., 0.), false);
    for (g, absolute) in outputs(&path, 3) {
        let r = check(&g, absolute);
        assert!((r.statistics.cutting_length_mm - std::f64::consts::TAU * 10.).abs() < 0.01, "{:?}\n{:?}\n{g}", r.statistics, r.diagnostics);
        assert!(r.statistics.bounds_min_mm.unwrap()[0] < 0.002);
        assert!(r.statistics.bounds_max_mm.unwrap()[1] > 19.998);
        assert!(!g.lines().any(|l| l.starts_with("G2 ")), "{g}");
        // Subdivision must interpolate Z, not defer all descent to the end.
        assert!(g.lines().any(|l| l.contains("Z-0.1 ") || l.ends_with("Z-0.1")));
    }
}
