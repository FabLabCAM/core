use fablabcam_geom::{self as geom, PlineSource};
use fablabcam_model::Entity;
use fablabcam_studio::{dxf, shape, svgio, Editor, Project};

fn area(e: &Entity) -> f64 {
    geom::area(&e.to_pline().unwrap()).abs()
}

#[test]
fn rectangle_circle_et_formes_degenerees() {
    let r = shape::rectangle((80.0, 60.0), (0.0, 0.0)).unwrap();
    assert!((area(&r) - 4800.0).abs() < 1e-9);
    assert_eq!(shape::bounds(&r).unwrap(), ([0.0, 0.0], [80.0, 60.0]));
    assert!(matches!(shape::circle((0.0, 0.0), 0.0), Err(shape::EditError::Degenerate(_))));
    assert!(shape::rectangle((0.0, 0.0), (10.0, 0.0)).is_err());
    assert!(shape::line((1.0, 1.0), (1.0, 1.0)).is_err());
    assert!(shape::polyline(&[(0.0, 0.0), (1.0, 0.0)], true).is_err());
    assert!(shape::arc((0.0, 0.0), 5.0, 0.0, 0.0).is_err());
    let c = shape::circle((5.0, 5.0), 10.0).unwrap();
    assert_eq!(c, Entity::Circle { cx: 5.0, cy: 5.0, r: 10.0 });
}

#[test]
fn deplacement_et_suppression() {
    let mut p = Project::new();
    let a = p.add_shape("0", shape::rectangle((0.0, 0.0), (10.0, 10.0)).unwrap()).unwrap();
    let b = p.add_shape("0", shape::circle((20.0, 20.0), 3.0).unwrap()).unwrap();
    let c = p.add_shape("0", shape::line((0.0, 0.0), (5.0, 5.0)).unwrap()).unwrap();
    let before_c = p.shape(c).unwrap().entity.clone();
    p.move_shapes(&[a, b], 10.0, -5.0).unwrap();
    assert_eq!(shape::bounds(&p.shape(a).unwrap().entity).unwrap(), ([10.0, -5.0], [20.0, 5.0]));
    assert_eq!(p.shape(b).unwrap().entity, Entity::Circle { cx: 30.0, cy: 15.0, r: 3.0 });
    assert_eq!(p.shape(c).unwrap().entity, before_c);
    assert!(p.move_shapes(&[99], 1.0, 1.0).is_err());
    assert!(p.delete_shapes(&[a]).is_empty());
    assert!(p.shape(a).is_none());
}

#[test]
fn survol_distance_au_contour() {
    let r = shape::rectangle((0.0, 0.0), (10.0, 10.0)).unwrap();
    assert!((shape::distance(&r, (5.0, 11.0)) - 1.0).abs() < 1e-9);
    assert!(shape::distance(&r, (5.0, 5.0)) > 4.9); // le centre est loin du contour
}

#[test]
fn undo_redo() {
    let mut ed = Editor::new(Project::new());
    let id = ed.edit(|p| p.add_shape("0", shape::circle((0.0, 0.0), 1.0).unwrap())).unwrap();
    let after = ed.project.clone();
    assert!(ed.undo());
    assert!(ed.project.shapes.is_empty());
    assert!(ed.redo());
    assert_eq!(ed.project, after);
    assert!(ed.project.shape(id).is_some());
    // une nouvelle action vide le « rétablir »
    ed.undo();
    ed.edit(|p| p.add_shape("0", shape::circle((1.0, 1.0), 2.0).unwrap())).unwrap();
    assert!(!ed.can_redo());
    assert!(!ed.redo());
}

#[test]
fn modification_en_echec_ne_laisse_pas_de_trace() {
    let mut ed = Editor::new(Project::new());
    assert!(ed.edit(|p| p.move_shapes(&[7], 1.0, 1.0)).is_err());
    assert!(!ed.can_undo());
}

const DXF: &str = "0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n8\nContour\n90\n4\n70\n1\n10\n0\n20\n0\n10\n10\n20\n0\n42\n0.5\n10\n10\n20\n10\n10\n0\n20\n10\n0\nCIRCLE\n8\nTrous\n10\n5\n20\n5\n40\n2\n0\nARC\n8\nTrous\n10\n0\n20\n0\n40\n3\n50\n90\n51\n180\n0\nSPLINE\n8\n0\n0\nENDSEC\n0\nEOF\n";

#[test]
fn dxf_import_entites_gerees_et_signalees() {
    let r = dxf::parse_dxf(DXF).unwrap();
    assert_eq!(r.shapes.len(), 3);
    assert_eq!(r.shapes[0].0, "Contour");
    match &r.shapes[0].1 {
        Entity::Polyline(p) => {
            assert!(p.is_closed() && p.vertex_count() == 4);
            assert!((p.at(1).bulge - 0.5).abs() < 1e-12);
        }
        e => panic!("{e:?}"),
    }
    assert_eq!(r.shapes[1].1, Entity::Circle { cx: 5.0, cy: 5.0, r: 2.0 });
    assert_eq!(r.shapes[2].1, Entity::Arc { cx: 0.0, cy: 0.0, r: 3.0, start_deg: 90.0, sweep_deg: 90.0 });
    assert_eq!(r.ignored.len(), 1);
    assert!(r.ignored[0].starts_with("SPLINE"));
}

#[test]
fn dxf_invalide_donne_la_ligne() {
    let e = dxf::parse_dxf("0\nSECTION\n2\nENTITIES\nabc\nX\n").unwrap_err();
    assert!(e.contains("ligne 5"), "{e}");
    assert!(dxf::parse_dxf("ceci n'est pas un dxf").is_err());
    assert!(dxf::parse_dxf("0\nSECTION\n2\nHEADER\n0\nENDSEC\n0\nEOF\n").is_err());
}

fn same(a: &Entity, b: &Entity, tol: f64) -> bool {
    match (a, b) {
        (Entity::Polyline(p), Entity::Polyline(q)) => {
            p.is_closed() == q.is_closed()
                && p.vertex_count() == q.vertex_count()
                && (0..p.vertex_count()).all(|i| (p.at(i).x - q.at(i).x).abs() < tol && (p.at(i).y - q.at(i).y).abs() < tol && (p.at(i).bulge - q.at(i).bulge).abs() < tol)
        }
        _ => a == b,
    }
}

#[test]
fn dxf_aller_retour() {
    let r = dxf::parse_dxf(DXF).unwrap();
    let again = dxf::parse_dxf(&dxf::write_dxf(&r.shapes)).unwrap();
    assert_eq!(again.shapes.len(), r.shapes.len());
    for (a, b) in r.shapes.iter().zip(&again.shapes) {
        assert_eq!(a.0, b.0);
        assert!(same(&a.1, &b.1, 1e-6), "{:?} != {:?}", a.1, b.1);
    }
}

#[test]
fn dxf_arc_sens_horaire_conserve_la_geometrie() {
    let cw = Entity::Arc { cx: 0.0, cy: 0.0, r: 5.0, start_deg: 90.0, sweep_deg: -90.0 };
    let back = dxf::parse_dxf(&dxf::write_dxf(&[("0".into(), cw)])).unwrap();
    assert_eq!(back.shapes[0].1, Entity::Arc { cx: 0.0, cy: 0.0, r: 5.0, start_deg: 0.0, sweep_deg: 90.0 });
}

#[test]
fn svg_orientation_y() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><circle cx="20" cy="10" r="4"/></svg>"#;
    let r = svgio::parse_svg(svg).unwrap();
    assert_eq!(r.shapes[0].1, Entity::Circle { cx: 20.0, cy: 90.0, r: 4.0 });
}

#[test]
fn svg_chemins_et_formes() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">
      <rect x="10" y="10" width="20" height="10"/>
      <polyline points="0,0 10,0 10,10"/>
      <path d="M0 0 H10 V10 h-10 z"/>
      <path d="M0 0 C 10 0, 10 10, 0 10"/>
      <path d="M0 0 A5 5 0 0 1 10 0"/>
      <ellipse cx="1" cy="1" rx="1" ry="2"/>
    </svg>"#;
    let r = svgio::parse_svg(svg).unwrap();
    assert_eq!(r.shapes.len(), 6, "l'ellipse est maintenant importée (polyligne)");
    assert!((area(&r.shapes[0].1) - 200.0).abs() < 1e-9);
    assert!((area(&r.shapes[2].1) - 100.0).abs() < 1e-9);
    // demi-cercle de rayon 5 : sweep=1 en SVG = horaire à l'écran → bulge négatif dans le document
    match &r.shapes[4].1 {
        Entity::Polyline(p) => assert!((p.at(0).bulge + 1.0).abs() < 1e-9, "{}", p.at(0).bulge),
        e => panic!("{e:?}"),
    }
    assert!(r.ignored.is_empty(), "{:?}", r.ignored);
}

#[test]
fn svg_aller_retour() {
    let shapes = vec![
        ("0".to_string(), shape::rectangle((2.0, 3.0), (12.0, 9.0)).unwrap()),
        ("0".to_string(), shape::circle((5.0, -4.0), 2.5).unwrap()),
        ("0".to_string(), Entity::Polyline(geom::pline_from(&[(0.0, 0.0, 0.5), (10.0, 0.0, -0.3), (10.0, 5.0, 0.0)], false))),
        ("0".to_string(), shape::arc((0.0, 0.0), 6.0, 30.0, -100.0).unwrap()),
    ];
    let back = svgio::parse_svg(&svgio::write_svg(&shapes)).unwrap();
    assert_eq!(back.shapes.len(), 4);
    for (a, b) in shapes.iter().zip(&back.shapes) {
        match (&a.1, &b.1) {
            (Entity::Circle { .. }, _) => assert!(same(&a.1, &b.1, 1e-3), "{:?} {:?}", a.1, b.1),
            _ => {
                let (pa, pb) = (a.1.to_pline().unwrap(), b.1.to_pline().unwrap());
                assert!((geom::perimeter(&pa) - geom::perimeter(&pb)).abs() < 1e-3, "{:?} {:?}", a.1, b.1);
                let (ea, eb) = (shape::bounds(&a.1).unwrap(), shape::bounds(&b.1).unwrap());
                for k in 0..2 {
                    assert!((ea.0[k] - eb.0[k]).abs() < 1e-3 && (ea.1[k] - eb.1[k]).abs() < 1e-3, "{:?} {:?}", ea, eb);
                }
            }
        }
    }
}
