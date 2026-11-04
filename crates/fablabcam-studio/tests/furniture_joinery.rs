use fablabcam_studio::furniture::*;
fn model() -> (Template, Dimensions) {
    let t = Template::cabinet();
    let mut d = t.defaults.clone();
    d.composition = Some(Box::new(Composition::from_dimensions(&d).unwrap()));
    (t, d)
}
fn select(d: &mut Dimensions, male: &str, female: &str, settings: JointSettings) -> Connection {
    let link = connections(&structure(d).unwrap().panels)
        .into_iter()
        .find(|l| l.male == male && l.female == female)
        .unwrap();
    d.composition
        .as_mut()
        .unwrap()
        .joints
        .insert(link.id.clone(), settings);
    link
}
#[test]
fn tabs_change_both_panels_and_apply_total_clearance_once() {
    let (t, mut d) = model();
    let s = JointSettings {
        kind: JointKind::Tabs,
        relief: CornerRelief::None,
        ..JointSettings::default()
    };
    select(&mut d, "T01", "C1", s);
    let parts = panels(&t, &d).unwrap();
    let male = parts.iter().find(|p| p.id == "T01").unwrap();
    let female = parts.iter().find(|p| p.id == "C1").unwrap();
    assert_eq!(male.width, 580.0);
    assert_eq!(male.uv_min, [-20.0, 0.0]);
    assert_eq!(female.features.len(), 3);
    for f in &female.features {
        let pts = &f.contour;
        assert!((pts[1][0] - pts[0][0] - 20.2).abs() < 1e-8);
        assert!((pts[2][1] - pts[1][1] - 20.2).abs() < 1e-8);
        assert!(f.through);
        assert_eq!(f.depth, 20.0);
    }
}
#[test]
fn families_produce_geometry_and_require_compatible_contacts() {
    for kind in JointKind::ALL
        .into_iter()
        .filter(|k| k.supported() && *k != JointKind::Butt)
    {
        let (t, mut d) = model();
        select(
            &mut d,
            "SUP",
            "C1",
            JointSettings {
                kind,
                relief: CornerRelief::None,
                ..JointSettings::default()
            },
        );
        let p = panels(&t, &d).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        assert!(!p.iter().find(|p| p.id == "C1").unwrap().features.is_empty());
        assert!(p.iter().find(|p| p.id == "SUP").unwrap().width > 560.0);
    }
}
#[test]
fn tooling_depth_unsupported_and_orphan_references_are_rejected() {
    let (t, mut d) = model();
    let l = select(
        &mut d,
        "T01",
        "C1",
        JointSettings {
            kind: JointKind::Tabs,
            width: 4.0,
            clearance: 0.0,
            ..JointSettings::default()
        },
    );
    assert!(panels(&t, &d).is_err());
    let s = d
        .composition
        .as_mut()
        .unwrap()
        .joints
        .get_mut(&l.id)
        .unwrap();
    s.width = 20.0;
    s.blind = true;
    s.engagement = 20.0;
    assert!(panels(&t, &d).unwrap_err().contains("borgne"));
    d.composition
        .as_mut()
        .unwrap()
        .joints
        .get_mut(&l.id)
        .unwrap()
        .kind = JointKind::Dovetail;
    assert!(panels(&t, &d).unwrap_err().contains("non encore"));
    let c = d.composition.as_mut().unwrap();
    c.joints.clear();
    c.joints.insert("missing".into(), JointSettings::default());
    assert!(panels(&t, &d).unwrap_err().contains("sans pièces"));
}
#[test]
fn mirrored_faces_have_identical_dimensions_and_explicit_setup() {
    let (t, mut d) = model();
    for side in ["C1", "C2"] {
        select(
            &mut d,
            "T01",
            side,
            JointSettings {
                kind: JointKind::MortiseTenon,
                blind: true,
                relief: CornerRelief::Tbone,
                ..JointSettings::default()
            },
        );
    }
    let p = panels(&t, &d).unwrap();
    let left = p.iter().find(|p| p.id == "C1").unwrap();
    let right = p.iter().find(|p| p.id == "C2").unwrap();
    assert_eq!(left.features[0].contour, right.features[0].contour);
    assert_ne!(left.features[0].face, right.features[0].face);
    assert!(left.features[0].contour.iter().any(|p| p[2] != 0.0));
}
#[test]
fn bin_connections_never_attach_the_removable_bin_to_the_frame() {
    let (_, mut d) = model();
    let c = d.composition.as_mut().unwrap();
    c.root.division.as_mut().unwrap().children[0].bin = Some(Bin::default());
    let p = structure(&d).unwrap().panels;
    for l in connections(&p) {
        let a = p.iter().find(|p| p.id == l.male).unwrap();
        let b = p.iter().find(|p| p.id == l.female).unwrap();
        assert_eq!(a.assembly, b.assembly);
    }
}
