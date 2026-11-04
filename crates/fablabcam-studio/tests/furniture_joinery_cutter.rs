use fablabcam_geom as geom;
use fablabcam_studio::furniture::*;
fn pline(c: &Contour) -> geom::Pline {
    geom::pline_from(
        &c.iter().map(|p| (p[0], p[1], p[2])).collect::<Vec<_>>(),
        true,
    )
}
#[test]
fn round_cutter_clears_tab_shoulders_on_both_axes_and_ends() {
    for axis in [0, 1] {
        for high in [false, true] {
            for relief in [CornerRelief::Dogbone, CornerRelief::Tbone] {
                let mut origin = [0.0, 0.0, 200.0];
                origin[axis] = 20.0;
                let male = Panel::rectangular("M", "Male", origin, [100.0, 100.0, 20.0], 2);
                let mut fo = [0.0; 3];
                fo[axis] = if high { 120.0 } else { 0.0 };
                let mut fs = [100.0, 100.0, 400.0];
                fs[axis] = 20.0;
                let female = Panel::rectangular("F", "Female", fo, fs, axis);
                let mut c = Composition::from_dimensions(&Dimensions::default()).unwrap();
                c.joint_default = JointSettings {
                    kind: JointKind::Tabs,
                    count: 1,
                    width: 20.0,
                    relief,
                    ..Default::default()
                };
                let parts = machining_preview(vec![male, female], &c, 6.0).unwrap();
                let ideal = pline(&parts[0].outline);
                // Reconstruct material after profiling with a circular cutter.
                let stock = geom::outset(&ideal, 3.0)
                    .into_iter()
                    .flat_map(|p| geom::inset(&p, 3.0))
                    .collect::<Vec<_>>();
                for side in [39.5, 60.5] {
                    let end = if high { 100.5 } else { -0.5 };
                    let point = if axis == 0 { (end, side) } else { (side, end) };
                    assert!(
                        !stock.iter().any(|p| geom::contains_point(p, point)),
                        "shoulder collision axis={axis} high={high} relief={relief:?}"
                    );
                }
            }
        }
    }
}
#[test]
fn short_tongue_reduction_is_millable_and_clears_all_free_corners() {
    let male = Panel::rectangular("M", "Male", [20.0, 0.0, 200.0], [100.0, 100.0, 20.0], 2);
    let female = Panel::rectangular("F", "Female", [0.0, 0.0, 0.0], [20.0, 100.0, 400.0], 0);
    let mut c = Composition::from_dimensions(&Dimensions::default()).unwrap();
    c.joint_default = JointSettings {
        kind: JointKind::TongueGroove,
        engagement: 2.0,
        ..Default::default()
    };
    let parts = machining_preview(vec![male, female], &c, 6.0).unwrap();
    assert_eq!(parts[0].features.len(), 2);
    for f in &parts[0].features {
        let p = pline(&f.contour);
        let params = fablabcam_cam::Params {
            tool_diameter: 6.0,
            target_depth: -f.depth,
            ..Default::default()
        };
        fablabcam_cam::pocket(&p, &params).unwrap();
        let cleared = geom::inset(&p, 3.0)
            .into_iter()
            .flat_map(|p| geom::outset(&p, 3.0))
            .collect::<Vec<_>>();
        for point in [
            (-1.999, 0.001),
            (-1.999, 99.999),
            (-0.001, 0.001),
            (-0.001, 99.999),
        ] {
            assert!(
                cleared.iter().any(|p| geom::contains_point(p, point)),
                "uncleared tongue {point:?}"
            );
        }
    }
}

#[test]
fn continuous_slot_checks_both_dimensions_before_corner_relief() {
    let male = Panel::rectangular("M", "Male", [20.0, 0.0, 200.0], [100.0, 2.0, 20.0], 2);
    let female = Panel::rectangular("F", "Female", [0.0, 0.0, 0.0], [20.0, 100.0, 400.0], 0);
    for kind in [JointKind::Dado, JointKind::TongueGroove] {
        let mut c = Composition::from_dimensions(&Dimensions::default()).unwrap();
        c.joint_default = JointSettings {
            kind,
            ..Default::default()
        };
        assert!(
            machining_preview(vec![male.clone(), female.clone()], &c, 6.0)
                .unwrap_err()
                .contains("Portée")
        );
    }
}
