//! Jalon M4b : sens de fraisage, surcoupe de coin, passe de finition, incrément final, transitions.

use fablabcam_cam::*;
use fablabcam_geom as geom;

fn base() -> Params {
    Params { tool_diameter: 6.0, target_depth: -1.0, depth_increment: 0.0, ..Params::default() }
}

fn arcs(t: &Toolpath) -> Vec<bool> {
    t.moves.iter().filter_map(|m| if let Move::Arc { ccw, .. } = m { Some(*ccw) } else { None }).collect()
}

/// Points XY atteints en coupe (extrémités des Feed/Arc) et départs en rapide.
fn cut_points(t: &Toolpath) -> Vec<(f64, f64)> {
    t.moves.iter().filter_map(|m| match m {
        Move::Feed { x: Some(x), y: Some(y), .. } | Move::Arc { x, y, .. } => Some((*x, *y)),
        _ => None,
    }).collect()
}

fn rapids_xy(t: &Toolpath) -> Vec<(f64, f64)> {
    t.moves.iter().filter_map(|m| if let Move::Rapid { x: Some(x), y: Some(y), .. } = m { Some((*x, *y)) } else { None }).collect()
}

fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).hypot(a.1 - b.1) < 1e-6
}

// ───────────── sens de fraisage ─────────────

#[test]
fn climb_goes_cw_outside_and_ccw_inside_with_a_cw_spindle() {
    let c = geom::circle(0.0, 0.0, 20.0);
    let out = profile(&c, Side::Outside, &Params { milling: MillingDirection::Climb, ..base() }).unwrap();
    let inn = profile(&c, Side::Inside, &Params { milling: MillingDirection::Climb, ..base() }).unwrap();
    assert!(arcs(&out).iter().all(|a| !*a), "extérieur en avalant : horaire (G2), comme G41 + M3");
    assert!(arcs(&inn).iter().all(|a| *a), "intérieur en avalant : anti-horaire (G3)");
}

#[test]
fn conventional_is_the_opposite() {
    let c = geom::circle(0.0, 0.0, 20.0);
    let p = Params { milling: MillingDirection::Conventional, ..base() };
    // le logiciel de référence : profil extérieur conventionnel en G3 (fichier réel tirroire-haut)
    assert!(arcs(&profile(&c, Side::Outside, &p).unwrap()).iter().all(|a| *a));
    assert!(arcs(&profile(&c, Side::Inside, &p).unwrap()).iter().all(|a| !*a));
}

#[test]
fn a_ccw_spindle_flips_climb() {
    let c = geom::circle(0.0, 0.0, 20.0);
    let p = Params { milling: MillingDirection::Climb, spindle_cw: false, ..base() };
    assert!(arcs(&profile(&c, Side::Outside, &p).unwrap()).iter().all(|a| *a));
    let g = emit(&profile(&c, Side::Outside, &p).unwrap(), &p, &Post::default());
    assert!(g.contains("M4 S1000") && !g.contains("M3 "), "{g}");
}

#[test]
fn mixed_keeps_the_geometric_direction() {
    let c = geom::circle(0.0, 0.0, 20.0);
    let p = Params { milling: MillingDirection::Mixed, ..base() };
    assert!(arcs(&profile(&c, Side::Inside, &p).unwrap()).iter().all(|a| *a)); // anti-horaire d'origine
}

#[test]
fn on_the_line_profiles_are_never_reoriented() {
    let mut cw = geom::circle(0.0, 0.0, 20.0);
    cw = geom::reversed(&cw);
    let p = Params { milling: MillingDirection::Climb, ..base() };
    assert!(arcs(&profile(&cw, Side::On, &p).unwrap()).iter().all(|a| !*a));
}

#[test]
fn pocket_rings_follow_the_milling_direction_around_walls_and_islands() {
    let outer = geom::rectangle(0.0, 0.0, 80.0, 60.0);
    let island = geom::circle(40.0, 30.0, 8.0);
    let run = |m: MillingDirection| pocket_region(&outer, std::slice::from_ref(&island), &Params { milling: m, ..base() }).unwrap();
    // premier anneau autour de l'îlot : arc de rayon 11 centré sur l'îlot
    let island_ring_ccw = |t: &Toolpath| {
        let mut pos = (0.0, 0.0);
        for m in &t.moves {
            match m {
                Move::Rapid { x, y, .. } | Move::Feed { x, y, .. } => {
                    if let Some(x) = x { pos.0 = *x }
                    if let Some(y) = y { pos.1 = *y }
                }
                Move::Arc { ccw, x, y, i, j, .. } => {
                    if near((pos.0 + i, pos.1 + j), (40.0, 30.0)) && ((pos.0 - 40.0).hypot(pos.1 - 30.0) - 11.0).abs() < 1e-6 {
                        return *ccw;
                    }
                    pos = (*x, *y);
                }
                _ => {}
            }
        }
        panic!("pas d'anneau autour de l'îlot");
    };
    // premier anneau de la paroi : horaire s'il part du coin haut-gauche (3,57) vers le coin haut-droit,
    // anti-horaire s'il part du coin bas-gauche (3,3) vers le coin bas-droit
    let outer_ring_cw = |t: &Toolpath| cut_points(t)[0].1 > 50.0;
    let climb = run(MillingDirection::Climb);
    // en avalant : la paroi est à droite du sens de parcours (anneau de paroi anti-horaire, anneau
    // d'îlot horaire) ; en conventionnel, l'inverse
    assert!(!island_ring_ccw(&climb));
    assert!(!outer_ring_cw(&climb));
    let conv = run(MillingDirection::Conventional);
    assert!(island_ring_ccw(&conv));
    assert!(outer_ring_cw(&conv));
}

// ───────────── surcoupe de coin ─────────────

fn min_dist(t: &Toolpath, to: (f64, f64)) -> f64 {
    cut_points(t).iter().map(|p| (p.0 - to.0).hypot(p.1 - to.1)).fold(f64::INFINITY, f64::min)
}

#[test]
fn inside_corner_overcut_makes_the_cutter_touch_the_corner() {
    let slot = geom::rectangle(0.0, 0.0, 40.0, 30.0);
    let plain = profile(&slot, Side::Inside, &base()).unwrap();
    let over = profile(&slot, Side::Inside, &Params { corner_overcut: true, ..base() }).unwrap();
    // sans surcoupe le centre de l'outil reste à r·√2 du coin ; avec, à exactement r : le bord de
    // l'outil atteint le coin (dog-bone)
    assert!((min_dist(&plain, (0.0, 0.0)) - 3.0 * 2f64.sqrt()).abs() < 1e-9);
    assert!((min_dist(&over, (0.0, 0.0)) - 3.0).abs() < 1e-9, "{}", min_dist(&over, (0.0, 0.0)));
    for corner in [(40.0, 0.0), (40.0, 30.0), (0.0, 30.0)] {
        assert!((min_dist(&over, corner) - 3.0).abs() < 1e-9);
    }
}

#[test]
fn outside_corners_of_a_convex_shape_need_no_overcut() {
    let r = geom::rectangle(0.0, 0.0, 40.0, 30.0);
    let a = profile(&r, Side::Outside, &base()).unwrap();
    let b = profile(&r, Side::Outside, &Params { corner_overcut: true, ..base() }).unwrap();
    assert_eq!(a.moves.len(), b.moves.len());
}

#[test]
fn reflex_corner_of_an_outside_profile_is_overcut() {
    let l = geom::pline_from(&[(0.0, 0.0, 0.0), (40.0, 0.0, 0.0), (40.0, 20.0, 0.0), (20.0, 20.0, 0.0), (20.0, 40.0, 0.0), (0.0, 40.0, 0.0)], true);
    let plain = profile(&l, Side::Outside, &base()).unwrap();
    let over = profile(&l, Side::Outside, &Params { corner_overcut: true, ..base() }).unwrap();
    assert!((min_dist(&plain, (20.0, 20.0)) - 3.0 * 2f64.sqrt()).abs() < 1e-6, "{}", min_dist(&plain, (20.0, 20.0)));
    assert!((min_dist(&over, (20.0, 20.0)) - 3.0).abs() < 1e-6, "{}", min_dist(&over, (20.0, 20.0)));
}

#[test]
fn overcut_depends_on_the_angle_of_the_corner() {
    // coin à 60° : le coin est à r / sin(30°) = 2r du sommet du trajet ; l'outil doit passer à r du coin
    let tri = geom::pline_from(&[(0.0, 0.0, 0.0), (60.0, 0.0, 0.0), (30.0, 30.0 * 3f64.sqrt(), 0.0)], true);
    let t = profile(&tri, Side::Inside, &Params { corner_overcut: true, ..base() }).unwrap();
    assert!((min_dist(&t, (0.0, 0.0)) - 3.0).abs() < 1e-6, "{}", min_dist(&t, (0.0, 0.0)));
}

// ───────────── incrément final ─────────────

#[test]
fn final_depth_increment_shortens_the_last_pass() {
    let p = |fdi: f64, inc: f64, target: f64| Params { depth_increment: inc, target_depth: target, final_depth_increment: fdi, ..Params::default() };
    assert_eq!(p(0.0, 1.0, -3.0).depth_levels(), vec![-1.0, -2.0, -3.0]);
    let l = p(0.2, 1.0, -3.0).depth_levels();
    assert_eq!(l.len(), 4);
    for (a, b) in l.iter().zip([-1.0, -2.0, -2.8, -3.0]) {
        assert!((a - b).abs() < 1e-9, "{l:?}");
    }
    assert_eq!(p(5.0, 1.0, -3.0).depth_levels(), vec![-3.0]);
    assert_eq!(p(0.2, 1.0, -0.5).depth_levels().last(), Some(&-0.5));
    // la dernière passe ne dépasse jamais l'incrément final
    let l = p(0.5, 0.8, -2.0).depth_levels();
    assert!((l[l.len() - 1] - l[l.len() - 2]).abs() <= 0.5 + 1e-9, "{l:?}");
}

// ───────────── passe de finition ─────────────

#[test]
fn finish_ring_is_cut_last_on_the_wall() {
    let rect = geom::rectangle(0.0, 0.0, 40.0, 30.0);
    let p = Params { finish_stepover: 1.0, max_crossover: 0.0, milling: MillingDirection::Mixed, ..base() };
    let t = pocket(&rect, &p).unwrap();
    let r = rapids_xy(&t);
    assert!(near(r[0], (4.0, 4.0)), "ébauche à rayon + finition : {r:?}");
    assert!(near(*r.last().unwrap(), (3.0, 3.0)), "finition sur la paroi : {r:?}");
    // aucun anneau d'ébauche ne repasse sur la paroi
    assert_eq!(r.iter().filter(|p| near(**p, (3.0, 3.0))).count(), 1);
}

#[test]
fn finish_only_at_target_depth_by_default_and_every_level_on_request() {
    let rect = geom::rectangle(0.0, 0.0, 40.0, 30.0);
    let two = Params { target_depth: -2.0, depth_increment: 1.0, finish_stepover: 1.0, max_crossover: 0.0, milling: MillingDirection::Mixed, ..base() };
    let count = |p: &Params, pt: (f64, f64)| rapids_xy(&pocket(&rect, p).unwrap()).iter().filter(|r| near(**r, pt)).count();
    // niveau -1 : anneaux normaux dès la paroi (3,3) ; niveau -2 : ébauche à (4,4) puis finition (3,3)
    assert_eq!(count(&two, (3.0, 3.0)), 2);
    assert_eq!(count(&two, (4.0, 4.0)), 1);
    let every = Params { finish_at_target_only: false, ..two };
    assert_eq!(count(&every, (3.0, 3.0)), 2);
    assert_eq!(count(&every, (4.0, 4.0)), 2);
}

#[test]
fn finish_with_islands_also_finishes_the_island_wall() {
    let outer = geom::rectangle(0.0, 0.0, 80.0, 60.0);
    let island = geom::circle(40.0, 30.0, 8.0);
    let p = Params { finish_stepover: 1.0, max_crossover: 0.0, ..base() };
    let t = pocket_region(&outer, &[island], &p).unwrap();
    // les deux dernières coupes sont les anneaux de paroi : bord extérieur puis îlot (ou l'inverse)
    let last_two: Vec<_> = rapids_xy(&t).into_iter().rev().take(2).collect();
    let on_outer_wall = |p: &(f64, f64)| (p.0 - 3.0).abs() < 1e-6 || (p.1 - 3.0).abs() < 1e-6 || (p.0 - 77.0).abs() < 1e-6;
    let on_island_wall = |p: &(f64, f64)| ((p.0 - 40.0).hypot(p.1 - 30.0) - 11.0).abs() < 1e-6;
    assert!(last_two.iter().any(on_outer_wall) && last_two.iter().any(on_island_wall), "{last_two:?}");
}

// ───────────── transitions (MaxCrossoverDistance) ─────────────

fn level_first(max_crossover: f64) -> Params {
    Params { cut_ordering: CutOrdering::LevelFirst, max_crossover, ..base() }
}

#[test]
fn rings_are_linked_by_horizontal_moves_when_close_enough() {
    let rect = geom::rectangle(0.0, 0.0, 40.0, 30.0);
    let with = pocket(&rect, &level_first(0.7)).unwrap();
    let without = pocket(&rect, &level_first(0.0)).unwrap();
    // sans transitions : un rapide d'approche par anneau ; avec : un seul (puis tout est enchaîné)
    assert!(rapids_xy(&without).len() >= 5, "{}", rapids_xy(&without).len());
    assert_eq!(rapids_xy(&with).len(), 1, "{:?}", rapids_xy(&with));
    // un seul retrait vers le plan de dégagement en milieu de niveau : aucun entre anneaux
    let retracts = with.moves.iter().filter(|m| matches!(m, Move::Rapid { z: Some(z), .. } if (*z - 3.0).abs() < 1e-9)).count();
    assert!(retracts <= 3, "{retracts}"); // approche + fin de niveau
    // une seule plongée verticale
    let plunges = with.moves.iter().filter(|m| matches!(m, Move::Feed { x: None, y: None, z: Some(_), .. })).count();
    assert_eq!(plunges, 1);
}

#[test]
fn a_tiny_crossover_distance_links_nothing() {
    let rect = geom::rectangle(0.0, 0.0, 40.0, 30.0);
    let t = pocket(&rect, &level_first(0.05)).unwrap();
    assert!(rapids_xy(&t).len() >= 5);
}

#[test]
fn linked_pocket_cuts_the_same_area_as_the_unlinked_one() {
    // mêmes anneaux : les extrémités des anneaux de l'un se retrouvent dans l'autre
    let rect = geom::rectangle(0.0, 0.0, 40.0, 30.0);
    let a: Vec<_> = cut_points(&pocket(&rect, &level_first(0.7)).unwrap()).into_iter().map(|p| (p.0.round(), p.1.round())).collect();
    let b: Vec<_> = cut_points(&pocket(&rect, &level_first(0.0)).unwrap()).into_iter().map(|p| (p.0.round(), p.1.round())).collect();
    for corner in [(3.0, 3.0), (37.0, 3.0), (37.0, 27.0), (3.0, 27.0)] {
        assert!(a.contains(&corner) && b.contains(&corner), "{corner:?}");
    }
}

#[test]
fn links_never_gouge_an_island() {
    let outer = geom::rectangle(0.0, 0.0, 80.0, 60.0);
    let island = geom::circle(40.0, 30.0, 8.0);
    let t = pocket_region(&outer, &[island], &level_first(0.9)).unwrap();
    // échantillonne chaque segment droit : le centre de l'outil reste à ≥ 11 mm du centre de l'îlot
    let mut pos = (0.0, 0.0);
    let mut min = f64::INFINITY;
    for m in &t.moves {
        match m {
            Move::Rapid { x, y, .. } => {
                if let Some(x) = x { pos.0 = *x }
                if let Some(y) = y { pos.1 = *y }
            }
            Move::Feed { x, y, .. } => {
                let (nx, ny) = (x.unwrap_or(pos.0), y.unwrap_or(pos.1));
                for k in 0..=20 {
                    let tt = k as f64 / 20.0;
                    let (px, py) = (pos.0 + (nx - pos.0) * tt, pos.1 + (ny - pos.1) * tt);
                    min = min.min((px - 40.0).hypot(py - 30.0));
                }
                pos = (nx, ny);
            }
            Move::Arc { x, y, .. } => pos = (*x, *y),
            _ => {}
        }
    }
    assert!(min >= 11.0 - 2e-3, "{min}");
}

#[test]
fn tangent_lead_out_disables_links() {
    let rect = geom::rectangle(0.0, 0.0, 40.0, 30.0);
    let lead = Lead { kind: LeadKind::Tangent, tangent_radius: 1.0, ..Lead::default() };
    let p = Params { lead_in: lead.clone(), lead_out: lead, ..level_first(0.7) };
    let t = pocket(&rect, &p).unwrap();
    assert!(rapids_xy(&t).len() >= 5);
}

#[test]
fn stepover_feedrate_is_used_for_links() {
    let rect = geom::rectangle(0.0, 0.0, 40.0, 30.0);
    let p = Params { stepover_feedrate: 450.0, ..level_first(0.7) };
    let t = pocket(&rect, &p).unwrap();
    assert!(t.moves.iter().any(|m| matches!(m, Move::Feed { f, .. } if (*f - 450.0).abs() < 1e-9)));
}

// ───────────── styles ─────────────

#[test]
fn style_values_for_m4b_are_read() {
    let lib = StyleLibrary::parse(include_str!("../../../tests/fixtures/styles-demo.xml")).unwrap();
    let base = Params::from_style(&lib.resolve("").unwrap());
    assert_eq!(base.max_crossover, 0.7); // observé dans le style de base
    assert!(!base.corner_overcut && base.spindle_cw);
    assert_eq!(base.finish_stepover, 0.0);
    assert!(Params::from_style(&lib.resolve("cutout+tabs+corners").unwrap()).corner_overcut);
    let mut s = lib.resolve("").unwrap();
    for (k, v) in [("MillingDirection", "Conventional"), ("FinishStepover", "0.5"), ("FinalDepthIncrement", "0.1"), ("SpindleDirection", "CCW"), ("StepoverFeedrate", "300"), ("FinishStepoverAtTargetDepth", "False")] {
        s.set(k, v);
    }
    let p = Params::from_style(&s);
    assert_eq!(p.milling, MillingDirection::Conventional);
    assert_eq!((p.finish_stepover, p.final_depth_increment, p.stepover_feedrate), (0.5, 0.1, 300.0));
    assert!(!p.spindle_cw && !p.finish_at_target_only);
}
