use fablabcam_geom::{self as geom, PlineSource};

#[test]
fn offsets_accept_repeated_dxf_positions_without_changing_the_drawing() {
    let shape = geom::pline_from(
        &[
            (0., 0., 0.),
            (0., 0., 0.),
            (10., 0., 0.),
            (10., 10., 0.),
            (0., 10., 0.),
            (0., 0., 0.),
        ],
        true,
    );
    let clean = geom::rectangle(0., 0., 10., 10.);
    for offset in [geom::inset, geom::outset] {
        let got = offset(&shape, 1.);
        let expected = offset(&clean, 1.);
        assert_eq!(got.len(), expected.len());
        assert!((geom::area(&got[0]) - geom::area(&expected[0])).abs() < 1e-8);
        assert!((geom::perimeter(&got[0]) - geom::perimeter(&expected[0])).abs() < 1e-8);
    }
    assert_eq!(shape.vertex_count(), 6);
    let line = geom::pline_from(&[(0., 0., 0.), (0., 0., 0.), (10., 0., 0.)], false);
    let got = geom::offset_open(&line, 1.);
    assert_eq!(got.len(), 1);
    assert!((geom::perimeter(&got[0]) - 10.).abs() < 1e-8);
}

#[test]
fn cleaning_repeated_positions_preserves_the_outgoing_arc_bulge() {
    let shape = geom::pline_from(&[(-5., 0., 0.), (-5., 0., 1.), (5., 0., 1.)], true);
    let got = geom::outset(&shape, 1.);
    assert_eq!(got.len(), 1);
    assert!((geom::area(&got[0]) - 36. * std::f64::consts::PI).abs() < 1e-8);
}
