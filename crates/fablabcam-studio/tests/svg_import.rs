use fablabcam_geom::{self as geom, PlineSource};
use fablabcam_model::Entity;
use fablabcam_studio::{shape, svgio};

fn read(body: &str) -> svgio::SvgImport {
    svgio::parse_svg(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40">{body}</svg>"#
    ))
    .unwrap()
}

fn bounds(e: &Entity, lo: [f64; 2], hi: [f64; 2]) {
    let (a, b) = shape::bounds(e).unwrap();
    for k in 0..2 {
        assert!(
            (a[k] - lo[k]).abs() < 0.02 && (b[k] - hi[k]).abs() < 0.02,
            "{a:?} {b:?}"
        );
    }
}

#[test]
fn nested_matrices_compose_before_y_flip() {
    let r = read(
        r#"<g transform="matrix(0.1,0,0,0.1,-50,-100)"><g transform="translate(500,1000)"><rect width="100" height="100"/></g></g>"#,
    );
    assert_eq!(r.shapes.len(), 1);
    assert!(r.ignored.is_empty(), "{:?}", r.ignored);
    bounds(&r.shapes[0].1, [0.0, 30.0], [10.0, 40.0]);
}

#[test]
fn definition_subtrees_never_become_contours() {
    let r = read(
        r#"<defs><clipPath id="unused"><rect x="500" y="500" width="100" height="100"/><path d="M0 0 H100 V100 Z"/></clipPath><rect width="20" height="20"/></defs><symbol><circle r="500"/></symbol><path d="M0 0 H10 V10 H0 Z"/>"#,
    );
    assert_eq!(r.shapes.len(), 1);
    assert!(r.ignored.is_empty());
    bounds(&r.shapes[0].1, [0.0, 30.0], [10.0, 40.0]);
}

#[test]
fn scientific_and_decimal_path_coordinates_are_identical() {
    for (a, b) in [
        ("M0 0 L1e1 0 L10 10 Z", "M0 0 L10 0 L10 10 Z"),
        (
            "m1E+1-2e-1 c-19.51116,-1.25e-4 -19.55834,0 -39.26619,0",
            "m10 -0.2 c-19.51116,-0.000125 -19.55834,0 -39.26619,0",
        ),
        ("M0 0 1e1 0 10 1e1z", "M0 0 10 0 10 10z"),
    ] {
        let ra = read(&format!(r#"<path d="{a}"/>"#));
        let rb = read(&format!(r#"<path d="{b}"/>"#));
        assert!(!ra.shapes.is_empty());
        assert_eq!(ra.shapes, rb.shapes);
    }
}

#[test]
fn local_transform_lists_and_siblings_keep_their_own_frames() {
    let r = read(
        r#"<g transform="translate(5 3)"><rect width="2" height="4" transform="rotate(90) scale(2 1)"/></g><rect width="1" height="1"/>"#,
    );
    bounds(&r.shapes[0].1, [1.0, 33.0], [5.0, 37.0]);
    bounds(&r.shapes[1].1, [0.0, 39.0], [1.0, 40.0]);
}

#[test]
fn reflected_circular_arcs_keep_exact_bulges_and_sweep() {
    let r = read(r#"<path transform="translate(20 0) scale(-2 2)" d="M0 0 A5 5 0 0 1 10 0"/>"#);
    let p = r.shapes[0].1.to_pline().unwrap();
    assert_eq!(p.vertex_count(), 2);
    assert!((p.at(0).bulge - 1.0).abs() < 1e-9);
    assert!((geom::perimeter(&p) - 10.0 * std::f64::consts::PI).abs() < 1e-8);
    bounds(&r.shapes[0].1, [0.0, 40.0], [20.0, 50.0]);
}

#[test]
fn nonuniform_circles_and_arcs_are_elliptical_polylines() {
    let r = read(
        r#"<circle cx="10" cy="10" r="5" transform="scale(2 1)"/><path transform="scale(2 1)" d="M5 10 A5 5 0 1 1 15 10 A5 5 0 1 1 5 10 Z"/>"#,
    );
    assert_eq!(r.shapes.len(), 2);
    for (_, e) in r.shapes {
        bounds(&e, [10.0, 25.0], [30.0, 35.0]);
        let p = e.to_pline().unwrap();
        assert!(p.is_closed());
        assert!(p.vertex_count() > 8);
        for v in p.iter_vertexes() {
            assert_eq!(v.bulge, 0.0);
            assert!(
                ((v.x - 20.0).powi(2) / 100.0 + (v.y - 30.0).powi(2) / 25.0 - 1.0).abs() < 1e-8
            );
        }
    }
}

#[test]
fn malformed_visible_geometry_is_an_error_not_partial_success() {
    for bad in [
        r#"<g transform="translate(oops)"><rect width="1" height="1"/></g>"#,
        r#"<path id="broken" d="M0 0 L10 0 L2e- 5 Z"/>"#,
        r#"<path d="M0 0 L1e999 0 L10 10 Z"/>"#,
    ] {
        let text = format!(r#"<svg><rect width="1" height="1"/>{bad}</svg>"#);
        assert!(svgio::parse_svg(&text).is_err(), "{bad}");
    }
}

#[test]
fn root_transform_and_rotation_center_are_applied() {
    let r = svgio::parse_svg(r#"<svg viewBox="0 0 40 40" transform="translate(2 3)"><line x1="10" y1="10" x2="20" y2="10" transform="rotate(90 10 10)"/></svg>"#).unwrap();
    bounds(&r.shapes[0].1, [12.0, 17.0], [12.0, 27.0]);
}

#[test]
fn closing_arc_is_preserved_during_roundtrip() {
    let r = read(r#"<path d="M0 0 A5 5 0 0 1 10 0 A5 5 0 0 1 0 0 Z"/>"#);
    let p = r.shapes[0].1.to_pline().unwrap();
    assert!(p.is_closed());
    assert!((geom::perimeter(&p) - 10.0 * std::f64::consts::PI).abs() < 1e-8);
    let back = svgio::parse_svg(&svgio::write_svg(&r.shapes)).unwrap();
    assert_eq!(back.shapes.len(), 1);
    assert!(
        (geom::perimeter(&back.shapes[0].1.to_pline().unwrap()) - geom::perimeter(&p)).abs() < 1e-6
    );
}

#[test]
fn transformed_primitives_share_the_same_frame() {
    let r = read(
        r#"<g transform="translate(3 4) scale(2)"><circle cx="5" cy="6" r="2"/><ellipse cx="5" cy="6" rx="2" ry="1"/><polygon points="0,0 1e1,0 10,10"/></g>"#,
    );
    assert_eq!(
        r.shapes[0].1,
        Entity::Circle {
            cx: 13.0,
            cy: 24.0,
            r: 4.0
        }
    );
    bounds(&r.shapes[1].1, [9.0, 22.0], [17.0, 26.0]);
    bounds(&r.shapes[2].1, [3.0, 16.0], [23.0, 36.0]);
}

#[test]
fn skewed_arcs_keep_endpoints_without_circular_bulges() {
    let r = read(r#"<path transform="skewX(45)" d="M0 0 A5 5 0 0 1 10 0"/>"#);
    let p = r.shapes[0].1.to_pline().unwrap();
    assert!(!p.is_closed());
    assert_eq!((p.at(0).x, p.at(0).y), (0.0, 40.0));
    let end = p.at(p.vertex_count() - 1);
    assert_eq!((end.x, end.y), (10.0, 40.0));
    assert!(p.iter_vertexes().all(|v| v.bulge == 0.0));
}

#[test]
fn quadratic_smooth_controls_and_compound_subpaths_are_preserved() {
    let a = read(r#"<path d="M0 0 Q5 10 10 0 T20 0 M30 0 h2 v2 z"/>"#);
    let b = read(r#"<path d="M0 0 Q5 10 10 0 Q15 -10 20 0 M30 0 h2 v2 z"/>"#);
    assert_eq!(a.shapes, b.shapes);
    assert_eq!(a.shapes.len(), 2);
    assert!(!a.shapes[0].1.to_pline().unwrap().is_closed());
    assert!(a.shapes[1].1.to_pline().unwrap().is_closed());
}

#[test]
fn unused_resources_are_skipped_but_used_effects_are_diagnosed() {
    let r = read(
        r##"<defs><g transform="bad"><path d="broken"/></g></defs><g clip-path="url(#clip)"><rect width="10" height="10"/></g><use href="#missing"/>"##,
    );
    assert_eq!(r.shapes.len(), 1);
    assert_eq!(r.ignored.len(), 2);
    assert!(r.ignored[0].contains("clip-path"));
    assert!(r.ignored[1].contains("<use>"));
}

#[test]
fn display_none_skips_subtree_and_css_transform_is_reported() {
    let r = read(
        r#"<g style="display:inline;display : none"><path d="broken"/></g><rect width="1" height="2" style="transform : rotate(30deg)"/>"#,
    );
    assert_eq!(r.shapes.len(), 1);
    assert_eq!(r.ignored.len(), 1);
    assert!(r.ignored[0].contains("transformation CSS"));
}
