use fablabcam_geom as geom;
use fablabcam_model::Entity;
use fablabcam_studio::cad::{self, BoolOp, Similarity};
use fablabcam_studio::{shape, Project, Studio};

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn bounds(e: &Entity) -> ([f64; 2], [f64; 2]) {
    shape::bounds(e).unwrap()
}

fn area(e: &Entity) -> f64 {
    geom::area(&e.to_pline().unwrap())
}

#[test]
fn rotation_echelle_miroir() {
    let rect = shape::rectangle((0.0, 0.0), (10.0, 4.0)).unwrap();
    let r = cad::transform(&rect, &Similarity::rotation((0.0, 0.0), 90.0));
    let (lo, hi) = bounds(&r);
    assert!(close(lo[0], -4.0) && close(hi[0], 0.0) && close(lo[1], 0.0) && close(hi[1], 10.0), "{lo:?} {hi:?}");
    let s = cad::transform(&rect, &Similarity::scale((0.0, 0.0), 2.0).unwrap());
    assert!(close(area(&s), 160.0));
    assert!(Similarity::scale((0.0, 0.0), 0.0).is_err());
    // cercle : centre déplacé, rayon multiplié
    let c = cad::transform(&shape::circle((5.0, 0.0), 2.0).unwrap(), &Similarity::scale((0.0, 0.0), 3.0).unwrap());
    assert_eq!(c, Entity::Circle { cx: 15.0, cy: 0.0, r: 6.0 });
    // arc miroir : même tracé reflété, sens inversé
    let arc = shape::arc((0.0, 0.0), 5.0, 0.0, 90.0).unwrap();
    let m = cad::transform(&arc, &Similarity::mirror((0.0, 0.0), 90.0)); // axe vertical
    let Entity::Arc { start_deg, sweep_deg, .. } = m else { panic!() };
    assert!(close(start_deg.rem_euclid(360.0), 180.0) && close(sweep_deg, -90.0));
    let (lo, hi) = bounds(&m);
    assert!(close(lo[0], -5.0) && close(hi[0], 0.0) && close(hi[1], 5.0), "{lo:?} {hi:?}");
    // polyligne avec arc : le miroir garde la même aire
    let rounded = cad::fillet(&rect, 1.0).unwrap().0;
    assert!(close(area(&cad::transform(&rounded, &Similarity::mirror((3.0, 3.0), 30.0))), area(&rounded)));
}

#[test]
fn copies_et_reseaux() {
    let mut p = Project::new();
    let a = p.add_shape("0", shape::circle((0.0, 0.0), 1.0).unwrap()).unwrap();
    let ids = p.array_rect(&[a], 3, 2, 10.0, 5.0).unwrap();
    assert_eq!(ids.len(), 5);
    let centers: Vec<(f64, f64)> = p.shapes.iter().map(|s| match s.entity { Entity::Circle { cx, cy, .. } => (cx, cy), _ => panic!() }).collect();
    assert!(centers.contains(&(20.0, 5.0)) && centers.contains(&(10.0, 0.0)));
    let b = p.add_shape("0", shape::circle((10.0, 0.0), 1.0).unwrap()).unwrap();
    let pol = p.array_polar(&[b], (0.0, 0.0), 4, 360.0).unwrap();
    assert_eq!(pol.len(), 3);
    let last = p.shape(*pol.last().unwrap()).unwrap();
    let Entity::Circle { cx, cy, .. } = last.entity else { panic!() };
    assert!(close(cx, 0.0) && close(cy, -10.0), "{cx} {cy}"); // 270°
    // arc de 90° en 3 exemplaires : 0°, 45°, 90°
    let half = p.array_polar(&[b], (0.0, 0.0), 3, 90.0).unwrap();
    assert_eq!(half.len(), 2, "copies seulement, l'original reste");
    let Entity::Circle { cx, cy, .. } = p.shape(half[0]).unwrap().entity else { panic!() };
    assert!(close(cx, 10.0 / 2f64.sqrt()) && close(cy, cx));
    assert!(p.array_rect(&[a], 0, 2, 1.0, 1.0).is_err() && p.array_polar(&[a], (0.0, 0.0), 1, 360.0).is_err());
    assert!(p.copy_shapes(&[], &Similarity::translation(1.0, 1.0)).is_err());
}

#[test]
fn decalage_ferme_et_ouvert() {
    let rect = shape::rectangle((0.0, 0.0), (10.0, 10.0)).unwrap();
    let out = cad::offset(&rect, 2.0).unwrap();
    let (lo, hi) = bounds(&out[0]);
    assert!(close(lo[0], -2.0) && close(hi[0], 12.0));
    let inn = cad::offset(&rect, -2.0).unwrap();
    assert!(close(area(&inn[0]), 36.0));
    assert!(cad::offset(&rect, -6.0).is_err(), "plus grand que la demi-largeur");
    assert_eq!(cad::offset(&shape::circle((0.0, 0.0), 3.0).unwrap(), 1.0).unwrap(), vec![Entity::Circle { cx: 0.0, cy: 0.0, r: 4.0 }]);
    // ligne vers +X : décalage positif à gauche (Y > 0)
    let line = shape::line((0.0, 0.0), (10.0, 0.0)).unwrap();
    let o = cad::offset(&line, 1.5).unwrap();
    let (lo, hi) = bounds(&o[0]);
    assert!(close(lo[1], 1.5) && close(hi[1], 1.5), "{lo:?}");
    assert!(cad::offset(&line, 0.0).is_err());
}

#[test]
fn joindre_quatre_lignes_en_carre_ferme() {
    let mut s = Studio::new(Project::new());
    let mut ids = Vec::new();
    // ordre et sens mélangés
    for (a, b) in [((0.0, 0.0), (10.0, 0.0)), ((10.0, 10.0), (0.0, 10.0)), ((10.0, 10.0), (10.0, 0.0)), ((0.0, 0.0), (0.0, 10.0))] {
        ids.push(s.edit(|p| p.add_shape("0", shape::line(a, b).unwrap())).unwrap());
    }
    let joined = s.edit(|p| p.join_shapes(&ids, 1e-6)).unwrap();
    assert_eq!(joined.len(), 1);
    let e = &s.project().shape(joined[0]).unwrap().entity;
    assert!(shape::is_closed(e) && close(area(e), 100.0));
    assert_eq!(s.project().shapes.len(), 1);
    s.undo();
    assert_eq!(s.project().shapes.len(), 4);
    // deux chaînes séparées restent deux formes
    let mut p = Project::new();
    let x: Vec<u32> = [((0.0, 0.0), (1.0, 0.0)), ((1.0, 0.0), (2.0, 1.0)), ((5.0, 5.0), (6.0, 5.0)), ((7.0, 5.0), (6.0, 5.0))].iter().map(|(a, b)| p.add_shape("0", shape::line(*a, *b).unwrap()).unwrap()).collect();
    assert_eq!(p.join_shapes(&x, 1e-6).unwrap().len(), 2);
    let c = p.add_shape("0", shape::circle((0.0, 0.0), 1.0).unwrap()).unwrap();
    assert!(p.join_shapes(&[c, x[0]], 1e-6).is_err());
}

#[test]
fn eclater_rectangle_et_cercle() {
    let mut p = Project::new();
    let r = p.add_shape("A", shape::rectangle((0.0, 0.0), (4.0, 2.0)).unwrap()).unwrap();
    let c = p.add_shape("B", shape::circle((0.0, 0.0), 1.0).unwrap()).unwrap();
    let out = p.explode_shapes(&[r, c]).unwrap();
    assert_eq!(out.len(), 6);
    assert_eq!(p.shapes.iter().filter(|s| s.layer == "A").count(), 4, "calque conservé");
    assert!(p.shapes.iter().all(|s| !shape::is_closed(&s.entity)));
}

#[test]
fn booleens() {
    let a = shape::rectangle((0.0, 0.0), (10.0, 10.0)).unwrap();
    let b = shape::rectangle((5.0, 5.0), (15.0, 15.0)).unwrap();
    let u = cad::boolean(&a, &b, BoolOp::Union).unwrap();
    assert_eq!(u.len(), 1);
    assert!(close(area(&u[0]), 175.0));
    let d = cad::boolean(&a, &b, BoolOp::Difference).unwrap();
    assert!(close(d.iter().map(area).sum::<f64>(), 75.0));
    let i = cad::boolean(&a, &b, BoolOp::Intersection).unwrap();
    assert!(close(area(&i[0]), 25.0));
    // trou : un carré au milieu d'un autre, différence = contour + trou
    let hole = shape::rectangle((3.0, 3.0), (7.0, 7.0)).unwrap();
    assert_eq!(cad::boolean(&a, &hole, BoolOp::Difference).unwrap().len(), 2);
    let far = shape::rectangle((50.0, 50.0), (60.0, 60.0)).unwrap();
    assert!(cad::boolean(&a, &far, BoolOp::Intersection).is_err());
    assert!(cad::boolean(&a, &shape::line((0.0, 0.0), (1.0, 1.0)).unwrap(), BoolOp::Union).is_err());
    // dans le projet : les deux formes sont remplacées
    let mut p = Project::new();
    let (ia, ib) = (p.add_shape("0", a).unwrap(), p.add_shape("0", b).unwrap());
    assert_eq!(p.boolean_shapes(ia, ib, BoolOp::Union).unwrap().len(), 1);
    assert_eq!(p.shapes.len(), 1);
}

#[test]
fn conges_sur_les_coins() {
    let rect = shape::rectangle((0.0, 0.0), (10.0, 10.0)).unwrap();
    let (f, n) = cad::fillet(&rect, 2.0).unwrap();
    assert_eq!(n, 4);
    // aire = 100 - 4 × (r² - πr²/4)
    let expected = 100.0 - 4.0 * (4.0 - std::f64::consts::PI);
    assert!((area(&f) - expected).abs() < 1e-6, "{}", area(&f));
    assert_eq!(bounds(&f), ([0.0, 0.0], [10.0, 10.0]));
    // rayon trop grand pour les segments : aucun coin
    assert_eq!(cad::fillet(&rect, 6.0).unwrap().1, 0);
    // ligne brisée ouverte : seul le coin intérieur
    let open = shape::polyline(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], false).unwrap();
    assert_eq!(cad::fillet(&open, 1.0).unwrap().1, 1);
    let mut p = Project::new();
    let id = p.add_shape("0", rect).unwrap();
    assert_eq!(p.fillet_shapes(&[id], 2.0).unwrap(), 4);
    assert!(p.fillet_shapes(&[id], 2.0).is_err(), "plus de coin vif");
}

#[test]
fn transformation_dans_le_projet_suit_les_ponts_et_s_annule() {
    let mut s = Studio::new(fablabcam_studio::studio::demo_project());
    let op = s.project().ops.iter().find(|o| o.name == "Découpe").unwrap().id;
    s.edit(|p| p.toggle_tab_point(op, [40.0, -5.0], 1.0)).unwrap();
    let ids = s.project().op(op).unwrap().shapes.clone();
    s.edit(|p| p.transform_shapes(&ids, &Similarity::rotation((0.0, 0.0), 90.0))).unwrap();
    let tp = s.project().op(op).unwrap().tab_points[0];
    assert!(close(tp[0], 5.0) && close(tp[1], 40.0), "{tp:?}");
    assert!(s.computed[&op].error.is_none());
    s.undo();
    assert_eq!(s.project().op(op).unwrap().tab_points, vec![[40.0, -5.0]]);
}
