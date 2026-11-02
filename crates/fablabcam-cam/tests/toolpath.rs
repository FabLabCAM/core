use fablabcam_cam::*;
use fablabcam_geom as geom;

fn params() -> Params {
    Params { tool_diameter: 6.0, target_depth: -3.0, depth_increment: 1.0, ..Params::default() }
}

fn feed_zs(t: &Toolpath) -> Vec<f64> {
    t.moves.iter().filter_map(|m| if let Move::Feed { x: None, y: None, z: Some(z), .. } = m { Some(*z) } else { None }).collect()
}

fn xy(m: &Move) -> Option<(f64, f64)> {
    match m {
        Move::Feed { x: Some(x), y: Some(y), .. } | Move::Arc { x, y, .. } => Some((*x, *y)),
        _ => None,
    }
}

fn cutting_points(t: &Toolpath) -> Vec<(f64, f64)> {
    t.moves.iter().filter_map(xy).collect()
}

// ───────────── profil de base ─────────────

#[test]
fn outside_profile_goes_down_by_passes() {
    let t = profile(&geom::rectangle(0.0, 0.0, 100.0, 50.0), Side::Outside, &params()).unwrap();
    assert_eq!(feed_zs(&t), vec![-1.0, -2.0, -3.0]);
    let arcs = t.moves.iter().filter(|m| matches!(m, Move::Arc { .. })).count();
    assert_eq!(arcs, 12); // 4 arcs de coin × 3 passes
}

#[test]
fn inside_profile_too_large_tool() {
    assert_eq!(profile(&geom::rectangle(0.0, 0.0, 5.0, 5.0), Side::Inside, &params()).unwrap_err(), CamError::ToolTooLarge);
}

#[test]
fn open_contour_is_offset_to_the_right_outside_and_to_the_left_inside() {
    // le logiciel de référence (fichier réel TABLETTE-SOUSSOL) : ligne (0,0)→(-510,0) en extérieur usinée à Y = +3
    let line = geom::pline_from(&[(0.0, 0.0, 0.0), (10.0, 0.0, 0.0)], false);
    let ys = |side: Side| cutting_points(&profile(&line, side, &params()).unwrap()).iter().map(|p| p.1).collect::<Vec<_>>();
    assert!(ys(Side::Outside).iter().all(|y| (*y + 3.0).abs() < 1e-9), "{:?}", ys(Side::Outside));
    assert!(ys(Side::Inside).iter().all(|y| (*y - 3.0).abs() < 1e-9), "{:?}", ys(Side::Inside));
    assert!(profile(&line, Side::On, &params()).is_ok());
}

#[test]
fn rejects_target_above_surface() {
    let p = Params { target_depth: 1.0, ..params() };
    assert_eq!(profile(&geom::circle(0.0, 0.0, 10.0), Side::On, &p).unwrap_err(), CamError::NoDepth);
}

#[test]
fn drill_pecks_when_increment_set() {
    let t = drill(&[(10.0, 10.0), (20.0, 10.0)], &params()).unwrap();
    assert_eq!(t.moves.iter().filter(|m| matches!(m, Move::Drill { peck: Some(_), .. })).count(), 2);
    assert_eq!(drill(&[], &params()).unwrap_err(), CamError::EmptyInput);
}

// ───────────── poches ─────────────

#[test]
fn simple_pocket_stays_inside_shape() {
    let p = Params { depth_increment: 0.0, ..params() };
    let t = pocket(&geom::rectangle(0.0, 0.0, 40.0, 30.0), &p).unwrap();
    let pts = cutting_points(&t);
    assert!(pts.len() >= 20, "{} points", pts.len()); // 5 anneaux × 4 coins
    for (x, y) in pts {
        assert!((3.0 - 1e-6..=37.0 + 1e-6).contains(&x) && (3.0 - 1e-6..=27.0 + 1e-6).contains(&y), "({x},{y})");
    }
}

#[test]
fn pocket_with_island_never_touches_the_island() {
    let outer = geom::rectangle(0.0, 0.0, 100.0, 100.0);
    let island = geom::circle(50.0, 50.0, 10.0);
    let p = Params { depth_increment: 0.0, target_depth: -1.0, ..params() };
    let t = pocket_region(&outer, &[island], &p).unwrap();
    let pts = cutting_points(&t);
    let min = pts.iter().map(|(x, y)| (x - 50.0).hypot(y - 50.0)).fold(f64::INFINITY, f64::min);
    // le centre de l'outil reste à ≥ rayon de l'îlot + rayon d'outil (13 mm) du centre
    assert!(min >= 13.0 - 1e-3, "min = {min}");
    // et l'anneau autour de l'îlot existe bien (on s'en approche à moins de 0,1 mm)
    assert!(min <= 13.1, "min = {min}");
    assert!(t.moves.iter().any(|m| matches!(m, Move::Comment(c) if c.contains("1 îlot"))));
}

#[test]
fn pocket_shapes_detects_islands_by_nesting() {
    let shapes = vec![geom::rectangle(0.0, 0.0, 100.0, 100.0), geom::circle(50.0, 50.0, 10.0)];
    let p = Params { depth_increment: 0.0, target_depth: -1.0, ..params() };
    let with = pocket_shapes(&shapes, &p).unwrap();
    let without = pocket(&shapes[0], &p).unwrap();
    // l'îlot ajoute des anneaux autour de lui et retire le centre
    assert!(with.moves.len() != without.moves.len());
    let min = cutting_points(&with).iter().map(|(x, y)| (x - 50.0).hypot(y - 50.0)).fold(f64::INFINITY, f64::min);
    assert!(min >= 13.0 - 1e-3);
}

#[test]
fn island_leaving_less_than_a_tool_width_blocks_the_pocket() {
    // 5 mm de chaque côté de l'îlot, outil de 6 mm : rien n'est usinable
    let outer = geom::rectangle(0.0, 0.0, 30.0, 30.0);
    let island = geom::rectangle(5.0, 5.0, 20.0, 20.0);
    let p = Params { depth_increment: 0.0, ..params() };
    assert_eq!(pocket_region(&outer, &[island], &p).unwrap_err(), CamError::ToolTooLarge);
    // avec un outil de 4 mm, l'anneau passe et reste à ≥ 2 mm du bord
    let t = pocket_region(&outer, &[geom::rectangle(5.0, 5.0, 20.0, 20.0)], &Params { tool_diameter: 4.0, ..p }).unwrap();
    let min = cutting_points(&t).iter().map(|&(x, y)| x.min(y).min(30.0 - x).min(30.0 - y)).fold(f64::INFINITY, f64::min);
    assert!(min >= 2.0 - 1e-6, "{min}");
}

#[test]
fn cut_ordering_changes_plunge_sequence() {
    let shape = geom::rectangle(0.0, 0.0, 40.0, 30.0);
    let depth_first = pocket(&shape, &Params { target_depth: -2.0, ..params() }).unwrap();
    let level_first = pocket(&shape, &Params { target_depth: -2.0, cut_ordering: CutOrdering::LevelFirst, ..params() }).unwrap();
    let monotone = |t: &Toolpath| feed_zs(t).windows(2).all(|w| w[1] <= w[0] + 1e-9);
    assert!(monotone(&level_first), "{:?}", feed_zs(&level_first));
    assert!(!monotone(&depth_first), "{:?}", feed_zs(&depth_first));
}

// ───────────── tabs ─────────────

fn tabs(style: TabStyle) -> Tabs {
    Tabs { method: TabMethod::Automatic, width: 4.0, height: 1.0, min: 4, max: 4, distance: 0.0, size_threshold: 4.0, style, positions: vec![] }
}

#[test]
fn square_tabs_lift_only_the_final_pass() {
    let p = Params { tabs: tabs(TabStyle::Square), ..params() };
    let t = profile(&geom::rectangle(0.0, 0.0, 100.0, 50.0), Side::Outside, &p).unwrap();
    // niveaux -1, -2 (≥ cible+hauteur = -2) : sans tab ; -3 : 4 tabs → 4 montées à -2
    let ups: Vec<f64> = feed_zs(&t).into_iter().filter(|z| (*z + 2.0).abs() < 1e-9).collect();
    // le niveau -2 est lui-même une plongée à -2 ; on compte les montées au dernier niveau
    let all = feed_zs(&t);
    assert_eq!(all.iter().filter(|z| (**z + 3.0).abs() < 1e-9).count(), 4 + 1, "{all:?}"); // plongée + 4 redescentes
    assert!(ups.len() >= 5, "{all:?}");
    assert!(all.iter().all(|z| *z >= -3.0 - 1e-9 && *z <= 0.0));
}

#[test]
fn triangle_tabs_follow_the_slope_of_the_triangle() {
    let p = Params { tabs: tabs(TabStyle::Triangle), ..params() };
    let t = profile(&geom::rectangle(0.0, 0.0, 100.0, 50.0), Side::Outside, &p).unwrap();
    // au dernier niveau (z = -3, tab triangulaire de 4 mm de base et 1 mm de haut) : 4 montées vers -2
    // et 4 descentes vers -3, chacune sur la demi-base (2 mm) comme dans le G-code du logiciel de référence
    let (mut pos, mut ups, mut downs) = ((0.0, 0.0), Vec::new(), Vec::new());
    let mut z = 0.0;
    for m in &t.moves {
        if let Move::Feed { x: Some(x), y: Some(y), z: Some(nz), .. } = m {
            let d = (x - pos.0).hypot(y - pos.1);
            if *nz > z + 1e-9 && (*nz + 2.0).abs() < 1e-9 { ups.push(d); }
            if *nz < z - 1e-9 && (*nz + 3.0).abs() < 1e-9 && z > -3.0 + 1e-9 { downs.push(d); }
        }
        match m {
            Move::Feed { x, y, z: nz, .. } | Move::Rapid { x, y, z: nz } => {
                if let Some(x) = x { pos.0 = *x; }
                if let Some(y) = y { pos.1 = *y; }
                if let Some(nz) = nz { z = *nz; }
            }
            Move::Arc { x, y, z: nz, .. } => {
                pos = (*x, *y);
                if let Some(nz) = nz { z = *nz; }
            }
            _ => {}
        }
    }
    assert_eq!(ups.len(), 4, "{ups:?}");
    assert_eq!(downs.len(), 4, "{downs:?}");
    assert!(ups.iter().chain(&downs).all(|d| (d - 2.0).abs() < 1e-6), "{ups:?} {downs:?}");
}

#[test]
fn skip_tabs_use_rapid_across_the_top() {
    let p = Params { tabs: tabs(TabStyle::Skip), ..params() };
    let t = profile(&geom::rectangle(0.0, 0.0, 100.0, 50.0), Side::Outside, &p).unwrap();
    let rapids = t.moves.iter().filter(|m| matches!(m, Move::Rapid { x: Some(_), z: None, .. })).count();
    assert!(rapids > 4, "{rapids}"); // 4 tabs + l'approche
}

#[test]
fn no_tabs_without_method_or_below_threshold_or_for_wrong_side() {
    let base = Params { tabs: tabs(TabStyle::Square), ..params() };
    let count_up = |p: &Params, shape: &geom::Pline, side: Side| {
        profile(shape, side, p).map(|t| feed_zs(&t).iter().filter(|z| (**z + 3.0).abs() < 1e-9).count()).unwrap()
    };
    let rect = geom::rectangle(0.0, 0.0, 100.0, 50.0);
    assert_eq!(count_up(&base, &rect, Side::Outside), 5);
    assert_eq!(count_up(&Params { tabs: Tabs { method: TabMethod::None, ..base.tabs.clone() }, ..base.clone() }, &rect, Side::Outside), 1);
    assert_eq!(count_up(&Params { tabs: Tabs { size_threshold: 1000.0, ..base.tabs.clone() }, ..base.clone() }, &rect, Side::Outside), 1);
    assert_eq!(count_up(&Params { tabs: Tabs { method: TabMethod::AutomaticInner, ..base.tabs.clone() }, ..base.clone() }, &rect, Side::Outside), 1);
    assert_eq!(count_up(&Params { tabs: Tabs { method: TabMethod::AutomaticOuter, ..base.tabs.clone() }, ..base.clone() }, &rect, Side::Outside), 5);
}

#[test]
fn tab_count_follows_the_documented_rule() {
    // doc : périmètre 160 mm, distance 30 mm → round(160/30) = 5 tabs (entre min 3 et max 6)
    // rectangle décalé « sur le trait » : on part d'un contour de périmètre exactement 160
    let shape = geom::rectangle(0.0, 0.0, 70.0, 10.0);
    let tabs = Tabs { method: TabMethod::Automatic, width: 2.0, height: 1.0, min: 3, max: 6, distance: 30.0, size_threshold: 4.0, style: TabStyle::Square, positions: vec![] };
    let t = profile(&shape, Side::On, &Params { tool_diameter: 2.0, tabs, ..params() }).unwrap();
    let downs = feed_zs(&t).iter().filter(|z| (**z + 3.0).abs() < 1e-9).count();
    assert_eq!(downs, 5 + 1, "{:?}", feed_zs(&t));
}

// ───────────── lead moves ─────────────

#[test]
fn spiral_lead_in_replaces_vertical_plunges_with_a_helix() {
    let lead = Lead { kind: LeadKind::Spiral, spiral_angle: 0.0, tangent_radius: 0.0, feedrate: 0.0 };
    let p = Params { lead_in: lead, ..params() };
    let t = profile(&geom::circle(0.0, 0.0, 20.0), Side::Outside, &p).unwrap();
    // seule plongée verticale : de la descente rapide (1 mm au-dessus) jusqu'à la surface
    assert_eq!(feed_zs(&t), vec![0.0], "plongées verticales");
    // chaque tour descend d'un incrément : arcs hélicoïdaux avec Z décroissant jusqu'à -3
    let zs: Vec<f64> = t.moves.iter().filter_map(|m| if let Move::Arc { z: Some(z), .. } = m { Some(*z) } else { None }).collect();
    assert!(zs.len() >= 6);
    assert!(zs.windows(2).all(|w| w[1] <= w[0] + 1e-9), "{zs:?}");
    assert!((zs.last().unwrap() + 3.0).abs() < 1e-9);
    // passe plate finale : arcs sans Z
    assert!(t.moves.iter().rev().take(6).any(|m| matches!(m, Move::Arc { z: None, .. })));
}

#[test]
fn spiral_with_fixed_angle_needs_more_loops_when_shallow() {
    let steep = Params { lead_in: Lead { kind: LeadKind::Spiral, spiral_angle: 45.0, ..Lead::default() }, ..params() };
    let shallow = Params { lead_in: Lead { kind: LeadKind::Spiral, spiral_angle: 0.3, ..Lead::default() }, ..params() };
    let shape = geom::circle(0.0, 0.0, 20.0);
    let n = |p: &Params| profile(&shape, Side::Outside, p).unwrap().moves.iter().filter(|m| matches!(m, Move::Arc { .. })).count();
    assert!(n(&shallow) > n(&steep));
}

#[test]
fn tangent_lead_in_and_out_are_tangent_arcs_of_the_given_radius() {
    let lead = Lead { kind: LeadKind::Tangent, tangent_radius: 5.0, ..Lead::default() };
    let p = Params { lead_in: lead.clone(), lead_out: lead, target_depth: -1.0, ..params() };
    let t = profile(&geom::circle(0.0, 0.0, 20.0), Side::Outside, &p).unwrap();
    let arcs: Vec<&Move> = t.moves.iter().filter(|m| matches!(m, Move::Arc { .. })).collect();
    // entrée, 2 demi-arcs du cercle, sortie
    assert_eq!(arcs.len(), 4, "{arcs:?}");
    for a in [arcs[0], arcs[3]] {
        let Move::Arc { i, j, .. } = a else { unreachable!() };
        assert!((i.hypot(*j) - 5.0).abs() < 1e-9, "rayon {}", i.hypot(*j));
    }
    // l'entrée aboutit au départ du contour décalé (cercle de rayon 23, sur l'axe X)
    let Move::Arc { x, y, .. } = arcs[0] else { unreachable!() };
    assert!((x.abs() - 23.0).abs() < 1e-9 && y.abs() < 1e-9, "({x},{y})");
    // l'approche se fait dans la zone dégagée : à l'extérieur du cercle de rayon 23
    let approach = t.moves.iter().find_map(|m| if let Move::Rapid { x: Some(x), y: Some(y), .. } = m { Some((*x, *y)) } else { None }).unwrap();
    assert!(approach.0.hypot(approach.1) > 23.0);
}

#[test]
fn tangent_lead_with_angle_descends_along_the_arc() {
    let lead = Lead { kind: LeadKind::Tangent, tangent_radius: 5.0, spiral_angle: 10.0, ..Lead::default() };
    let p = Params { lead_in: lead, target_depth: -1.0, depth_increment: 1.0, ..params() };
    let t = profile(&geom::circle(0.0, 0.0, 20.0), Side::Outside, &p).unwrap();
    assert!(t.moves.iter().any(|m| matches!(m, Move::Arc { z: Some(z), .. } if (*z + 1.0).abs() < 1e-9)));
}

#[test]
fn tangent_radius_too_big_for_an_inside_profile_is_rejected() {
    let lead = Lead { kind: LeadKind::Tangent, tangent_radius: 30.0, ..Lead::default() };
    let p = Params { lead_in: lead, ..params() };
    assert_eq!(profile(&geom::circle(0.0, 0.0, 20.0), Side::Inside, &p).unwrap_err(), CamError::LeadDoesNotFit);
}

#[test]
fn tangent_lead_ignored_for_on_the_line_profiles() {
    let lead = Lead { kind: LeadKind::Tangent, tangent_radius: 5.0, ..Lead::default() };
    let p = Params { lead_in: lead, ..params() };
    let t = profile(&geom::circle(0.0, 0.0, 20.0), Side::On, &p).unwrap();
    assert_eq!(t.moves.iter().filter(|m| matches!(m, Move::Arc { .. })).count(), 6); // 2 demi-arcs × 3 passes
}

#[test]
fn feed_rate_of_lead_moves_is_used_when_set() {
    let lead = Lead { kind: LeadKind::Tangent, tangent_radius: 5.0, feedrate: 123.0, ..Lead::default() };
    let p = Params { lead_in: lead, target_depth: -1.0, ..params() };
    let t = profile(&geom::circle(0.0, 0.0, 20.0), Side::Outside, &p).unwrap();
    assert!(t.moves.iter().any(|m| matches!(m, Move::Arc { f, .. } if (*f - 123.0).abs() < 1e-9)));
}

// ───────────── tabs manuelles ─────────────

#[test]
fn manual_tabs_are_placed_at_the_given_points() {
    let manual = |positions: Vec<(f64, f64)>| Params {
        tabs: Tabs { method: TabMethod::Manual, positions, ..tabs(TabStyle::Square) },
        ..params()
    };
    let count = |p: &Params| {
        let t = profile(&geom::rectangle(0.0, 0.0, 100.0, 50.0), Side::Outside, p).unwrap();
        feed_zs(&t).iter().filter(|z| (**z + 3.0).abs() < 1e-9).count()
    };
    // plongée + une redescente par tab
    assert_eq!(count(&manual(vec![(50.0, -5.0), (50.0, 55.0)])), 1 + 2);
    assert_eq!(count(&manual(vec![(50.0, -5.0)])), 1 + 1);
    // aucune position : pas de tab
    assert_eq!(count(&manual(vec![])), 1);
    // deux points voisins : tabs qui se chevauchent, une seule est gardée
    assert_eq!(count(&manual(vec![(50.0, -5.0), (51.0, -5.0)])), 1 + 1);
}

#[test]
fn manual_tab_sits_on_the_projected_point() {
    let p = Params {
        tabs: Tabs { method: TabMethod::Manual, positions: vec![(30.0, -20.0)], ..tabs(TabStyle::Square) },
        ..params()
    };
    let t = profile(&geom::rectangle(0.0, 0.0, 100.0, 50.0), Side::Outside, &p).unwrap();
    // au dernier niveau, la tab relève l'outil à z = -2 ; la montée se fait à x ≈ 30 ± demi-largeur
    let mut pos = (0.0, 0.0);
    let mut lifts = Vec::new();
    let mut z = 0.0;
    for m in &t.moves {
        match m {
            Move::Feed { x, y, z: nz, .. } | Move::Rapid { x, y, z: nz } => {
                if let Some(x) = x { pos.0 = *x; }
                if let Some(y) = y { pos.1 = *y; }
                if let Some(nz) = nz {
                    if (*nz + 2.0).abs() < 1e-9 && z < -2.0 - 1e-9 { lifts.push(pos); }
                    z = *nz;
                }
            }
            Move::Arc { x, y, z: nz, .. } => {
                pos = (*x, *y);
                if let Some(nz) = nz { z = *nz; }
            }
            _ => {}
        }
    }
    assert_eq!(lifts.len(), 1, "{lifts:?}");
    let half = (4.0 + params().tool_diameter) / 2.0;
    assert!((lifts[0].0 - (30.0 - half)).abs() < 1e-6 || (lifts[0].0 - (30.0 + half)).abs() < 1e-6, "{lifts:?}");
}

// ───────────── RoughingClearance ─────────────

fn bbox(t: &Toolpath) -> (f64, f64, f64, f64) {
    let pts = cutting_points(t);
    let f = |g: fn(&(f64, f64)) -> f64, m: fn(f64, f64) -> f64, init: f64| pts.iter().map(g).fold(init, m);
    (f(|p| p.0, f64::min, f64::MAX), f(|p| p.1, f64::min, f64::MAX), f(|p| p.0, f64::max, f64::MIN), f(|p| p.1, f64::max, f64::MIN))
}

#[test]
fn roughing_clearance_moves_the_tool_away_from_the_wall() {
    let rect = geom::rectangle(0.0, 0.0, 100.0, 50.0);
    let with = |c: f64| Params { roughing_clearance: c, ..params() };
    // profil extérieur : axe à r + c du contour (r = 3)
    let (x0, _, x1, _) = bbox(&profile(&rect, Side::Outside, &with(0.0)).unwrap());
    assert!((x0 + 3.0).abs() < 1e-6 && (x1 - 103.0).abs() < 1e-6, "{x0} {x1}");
    let (x0, _, x1, _) = bbox(&profile(&rect, Side::Outside, &with(0.5)).unwrap());
    assert!((x0 + 3.5).abs() < 1e-6 && (x1 - 103.5).abs() < 1e-6, "{x0} {x1}");
    // profil intérieur et poche : axe plus loin de la paroi
    let (x0, _, x1, _) = bbox(&profile(&rect, Side::Inside, &with(0.5)).unwrap());
    assert!((x0 - 3.5).abs() < 1e-6 && (x1 - 96.5).abs() < 1e-6, "{x0} {x1}");
    let (x0, y0, x1, y1) = bbox(&pocket(&rect, &with(0.5)).unwrap());
    assert!((x0 - 3.5).abs() < 1e-6 && (x1 - 96.5).abs() < 1e-6 && (y0 - 3.5).abs() < 1e-6 && (y1 - 46.5).abs() < 1e-6);
    // surcoupe : négatif rapproche l'axe de la paroi
    let (x0, _, _, _) = bbox(&profile(&rect, Side::Outside, &with(-1.0)).unwrap());
    assert!((x0 + 2.0).abs() < 1e-6, "{x0}");
}

#[test]
fn roughing_clearance_cannot_exceed_the_tool_radius_when_negative() {
    let rect = geom::rectangle(0.0, 0.0, 100.0, 50.0);
    let p = Params { roughing_clearance: -3.5, ..params() };
    assert_eq!(profile(&rect, Side::Outside, &p).unwrap_err(), CamError::ClearanceTooNegative);
    assert_eq!(pocket(&rect, &p).unwrap_err(), CamError::ClearanceTooNegative);
}

#[test]
fn roughing_clearance_is_read_from_the_style() {
    let mut style = CamStyle::default();
    style.set("RoughingClearance", "0.25");
    assert_eq!(Params::from_style(&style).roughing_clearance, 0.25);
}
