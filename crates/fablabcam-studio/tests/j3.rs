use fablabcam_backplot::parse_gcode;
use fablabcam_studio::sim::{default_stock, Cutter, simulate, GougeKind, Sim};
use fablabcam_studio::studio::demo_project;
use fablabcam_studio::view3d::{sorted_projected, surface_triangles, Camera3};
use fablabcam_studio::{shape, OpKind, Project, Stock, Studio, Tool, ToolKind};

fn pocket_studio() -> Studio {
    let mut p = Project::new();
    let r = p.add_shape("0", shape::rectangle((10.0, 10.0), (50.0, 40.0)).unwrap()).unwrap();
    p.add_tool(Tool::new("F6", ToolKind::EndMill, 6.0)).unwrap();
    let o = p.add_operation(OpKind::Pocket, "Poche", vec![r], "F6").unwrap();
    p.update_operation(o, |o| { o.depth = -2.0; o.step_down = 1.0; }).unwrap();
    Studio::new(p)
}

#[test]
fn brut_par_defaut_couvre_le_trajet_et_descend_au_point_le_plus_bas() {
    let s = Studio::new(demo_project());
    let st = default_stock(&s).unwrap();
    let (lo, hi) = s.backplot().bounds().unwrap();
    assert!(st.min[0] < lo[0] && st.min[1] < lo[1] && st.max[0] > hi[0] && st.max[1] > hi[1]);
    assert_eq!(st.top, 0.0);
    assert_eq!(st.bottom, -4.0);
    // un brut défini par l'utilisateur est respecté
    let mut p = demo_project();
    p.stock = Some(Stock { min: [0.0, 0.0], max: [90.0, 70.0], top: 0.0, bottom: -5.0 });
    assert_eq!(default_stock(&Studio::new(p)).unwrap().max, [90.0, 70.0]);
}

#[test]
fn poche_fond_a_moins_2_et_exterieur_intact() {
    let out = simulate(&pocket_studio(), 0.5, 1.0).unwrap();
    let m = &out.map;
    // centre de la poche : fond à -2
    assert!((m.height_at(30.0, 25.0) + 2.0).abs() < 1e-3, "{}", m.height_at(30.0, 25.0));
    // près du bord intérieur (2 mm dedans) : l'outil de 6 mm laisse quand même le fond à -2
    assert!((m.height_at(13.0, 25.0) + 2.0).abs() < 1e-3);
    // à l'extérieur : intact
    assert_eq!(m.height_at(5.0, 25.0), 0.0);
    assert_eq!(m.height_at(55.0, 5.0), 0.0);
    assert!(out.gouges.is_empty(), "{:?}", out.gouges);
    let v = m.removed_volume();
    assert!(v > 40.0 * 30.0 * 2.0 * 0.9 && v < 40.0 * 30.0 * 2.0 * 1.05, "volume {v}");
}

#[test]
fn animation_a_50_pourcent_ne_montre_que_la_premiere_moitie() {
    let s = pocket_studio();
    let (half, full) = (simulate(&s, 0.5, 0.5).unwrap(), simulate(&s, 0.5, 1.0).unwrap());
    let (vh, vf) = (half.map.removed_volume(), full.map.removed_volume());
    assert!(vh > 0.0 && vh < vf * 0.75, "{vh} {vf}");
    assert_eq!(simulate(&s, 0.5, 0.0).unwrap().map.removed_volume(), 0.0);
    // monotone
    let q = simulate(&s, 0.5, 0.25).unwrap().map.removed_volume();
    assert!(q <= vh + 1e-9 && vh <= vf + 1e-9);
}

fn small_stock() -> Stock {
    Stock { min: [-10.0, -10.0], max: [30.0, 30.0], top: 0.0, bottom: -5.0 }
}

#[test]
fn fraise_boule_arrondie() {
    let mut sim = Sim::new(&small_stock(), 0.1);
    let bp = parse_gcode("G21 G90\nG0 X10 Y10 Z5\nG1 Z-2 F100\n");
    sim.run(&bp, Cutter { r: 3.0, kind: ToolKind::BallNose, vee: 90.0 }, -2.0, false, "boule", f64::INFINITY);
    let m = &sim.map;
    assert!((m.height_at(10.0, 10.0) + 2.0).abs() < 0.02);
    // à 1,5 mm du centre : z + r - sqrt(r² - d²) = -2 + 3 - 2.598
    assert!((m.height_at(11.5, 10.0) - (-2.0 + 3.0 - (9.0_f64 - 2.25).sqrt())).abs() < 0.05, "{}", m.height_at(11.5, 10.0));
    assert_eq!(m.height_at(14.0, 10.0), 0.0);
    // fraise droite : fond plat
    let mut flat = Sim::new(&small_stock(), 0.1);
    flat.run(&bp, Cutter { r: 3.0, kind: ToolKind::EndMill, vee: 90.0 }, -2.0, false, "plat", f64::INFINITY);
    assert!((flat.map.height_at(12.5, 10.0) + 2.0).abs() < 1e-3);
}

#[test]
fn gouge_sous_le_fond_programme() {
    let mut sim = Sim::new(&small_stock(), 0.2);
    let bp = parse_gcode("G21 G90\nG0 X5 Y5 Z5\nG1 Z-1 F100\nG1 X15 Z-3.5\n");
    sim.run(&bp, Cutter { r: 1.0, kind: ToolKind::EndMill, vee: 90.0 }, -2.0, false, "trop profond", f64::INFINITY);
    let g = &sim.gouges[0];
    assert_eq!(g.kind, GougeKind::BelowFloor);
    assert!(g.pos[2] < -2.0 && g.pos[0] > 5.0 && g.pos[0] < 15.0, "{:?}", g.pos);
    assert_eq!(g.op, "trop profond");
}

#[test]
fn rapide_dans_la_matiere() {
    let mut sim = Sim::new(&small_stock(), 0.2);
    let bp = parse_gcode("G21 G90\nG0 X5 Y5 Z5\nG0 Z-1\n");
    sim.run(&bp, Cutter { r: 1.0, kind: ToolKind::EndMill, vee: 90.0 }, -2.0, false, "rapide", f64::INFINITY);
    assert!(sim.gouges.iter().any(|g| g.kind == GougeKind::RapidInMaterial));
}

#[test]
fn gouge_dans_un_pont_de_maintien() {
    // découpe extérieure avec ponts, puis une plongée au milieu d'un pont
    let mut p = Project::new();
    let r = p.add_shape("0", shape::rectangle((0.0, 0.0), (60.0, 40.0)).unwrap()).unwrap();
    p.add_tool(Tool::new("F6", ToolKind::EndMill, 6.0)).unwrap();
    let o = p.add_operation(OpKind::Profile, "Decoupe", vec![r], "F6").unwrap();
    p.update_operation(o, |o| { o.depth = -3.0; o.step_down = 1.5; o.tabs = true; }).unwrap();
    let s = Studio::new(p);
    let stock = default_stock(&s).unwrap();
    let c = &s.computed[&o];
    let g = fablabcam_cam::emit(c.path.as_ref().unwrap(), c.params.as_ref().unwrap(), &fablabcam_cam::Post { canned_drill: false, ..Default::default() });
    let bp = parse_gcode(&g);
    let mut sim = Sim::new(&stock, 0.25);
    sim.run(&bp, Cutter { r: 3.0, kind: ToolKind::EndMill, vee: 90.0 }, -3.0, true, "Decoupe", f64::INFINITY);
    assert!(sim.gouges.is_empty(), "{:?}", sim.gouges);
    // cherche une cellule du couloir (à ~3 mm du bord du rectangle) restée au-dessus du fond
    let m = &sim.map;
    let mut found = None;
    for iy in 0..m.ny {
        for ix in 0..m.nx {
            let (x, y) = (m.min[0] + (ix as f64 + 0.5) * m.res, m.min[1] + (iy as f64 + 0.5) * m.res);
            let d = (-x).max(0.0).max(x - 60.0).hypot((-y).max(0.0).max(y - 40.0)); // distance au rectangle (0 dedans)
            if (d - 3.0).abs() < 0.2 && m.at(ix, iy) > -2.0 && found.is_none() {
                found = Some((x, y));
            }
        }
    }
    let (x, y) = found.expect("un pont devrait rester dans le couloir");
    let second = parse_gcode(&format!("G21 G90\nG0 X{x} Y{y} Z5\nG1 Z-3 F100\n"));
    sim.run(&second, Cutter { r: 1.0, kind: ToolKind::EndMill, vee: 90.0 }, -3.0, false, "Perçage fautif", f64::INFINITY);
    assert!(sim.gouges.iter().any(|g| g.kind == GougeKind::IntoTab && g.op == "Perçage fautif"), "{:?}", sim.gouges);
}

#[test]
fn camera_orbitale_z_reste_vertical() {
    let mut cam = Camera3::fit([0.0, 0.0, -2.0], [80.0, 60.0, 0.0]);
    let a = cam.project([40.0, 30.0, -1.0], 500.0);
    let b = cam.project([40.0, 30.0, 5.0], 500.0);
    assert!(b[1] > a[1], "un point plus haut est plus haut à l'écran");
    assert!((b[0] - a[0]).abs() < 1e-9, "Z reste vertical : même abscisse");
    cam.orbit(1.3, 0.2);
    let (a2, b2) = (cam.project([40.0, 30.0, -1.0], 500.0), cam.project([40.0, 30.0, 5.0], 500.0));
    assert!((b2[0] - a2[0]).abs() < 1e-9 && b2[1] > a2[1]);
    cam.orbit(0.0, 100.0);
    assert!(cam.pitch < std::f64::consts::FRAC_PI_2);
    // le centre de la cible se projette à l'origine
    let t = cam.project(cam.target, 500.0);
    assert!(t[0].abs() < 1e-9 && t[1].abs() < 1e-9);
}

#[test]
fn surface_ombree_et_tri_du_fond_vers_l_avant() {
    let out = simulate(&pocket_studio(), 1.0, 1.0).unwrap();
    let tris = surface_triangles(&out.map, 40);
    assert!(tris.len() > 100 && tris.len() <= 2 * 40 * 40);
    let cam = Camera3::fit([0.0, 0.0, -2.0], [60.0, 50.0, 0.0]);
    let v = sorted_projected(&cam, &tris, 500.0);
    assert_eq!(v.len(), tris.len());
    // le fond de poche est plus sombre que la surface
    let lum = |c: [u8; 3]| c[0] as u32 + c[1] as u32 + c[2] as u32;
    let (min, max) = tris.iter().fold((u32::MAX, 0), |(a, b), t| (a.min(lum(t.1)), b.max(lum(t.1))));
    assert!(max > min + 30);
}

#[test]
fn performance_poche_exemple() {
    let s = pocket_studio();
    let t = std::time::Instant::now();
    let out = simulate(&s, 0.2, 1.0).unwrap();
    let el = t.elapsed().as_secs_f64();
    eprintln!("simulation 0,2 mm : {el:.2} s ({}x{} cellules)", out.map.nx, out.map.ny);
    if !cfg!(debug_assertions) {
        assert!(el < 5.0, "{el} s");
    }
}
