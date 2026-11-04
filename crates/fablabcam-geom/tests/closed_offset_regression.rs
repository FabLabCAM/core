use fablabcam_geom::{self as geom, PlineSource};

fn letter_e(repeat_closure: bool) -> geom::Pline {
    let mut points = vec![
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
    ];
    if repeat_closure {
        points.push(points[0]);
    }
    geom::pline_from(&points, true)
}

#[test]
fn closed_letter_with_explicit_closing_vertex_matches_clean_contour() {
    // Issue #24: a DXF closed polyline also repeats its first vertex. This is a
    // synthetic orthogonal E, not a copy of the user's drawing or metadata.
    let repeated = letter_e(true);
    let before = repeated.clone();
    for offset in [geom::inset, geom::outset] {
        for distance in [0.5, 1.5, 3.] {
            let actual = offset(&repeated, distance);
            let expected = offset(&letter_e(false), distance);
            assert!(!actual.is_empty());
            assert_eq!(actual.len(), expected.len());
            for (a, e) in actual.iter().zip(&expected) {
                assert!((geom::area(a) - geom::area(e)).abs() < 1e-7);
                assert!((geom::perimeter(a) - geom::perimeter(e)).abs() < 1e-7);
                assert!(a
                    .iter_vertexes()
                    .all(|v| v.x.is_finite() && v.y.is_finite() && v.bulge.is_finite()));
            }
        }
    }
    let vertices = |p: &geom::Pline| {
        p.iter_vertexes()
            .map(|v| (v.x, v.y, v.bulge))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        vertices(&repeated),
        vertices(&before),
        "offset must not edit the source drawing"
    );
}
