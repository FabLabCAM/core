use fablabcam_backplot::*;
use std::f64::consts::PI;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

// ───────────── lecture du G-code ─────────────

#[test]
fn modal_motion_and_kinds() {
    let bp = parse_gcode("G21 G90\nG0 X0 Y0 Z3\nX10 Y0\nG1 Z-1 F300\nX20 F800\nY10\nG0 Z3\n");
    let kinds: Vec<Kind> = bp.segs.iter().map(|s| s.kind).collect();
    // la première position (G0 Z3) n'est qu'un point de départ
    assert_eq!(kinds, vec![Kind::Rapid, Kind::Plunge, Kind::Feed, Kind::Feed, Kind::Rapid]);
    assert_eq!(bp.segs[0].from, [0.0, 0.0, 3.0]);
    assert_eq!(bp.segs[0].to, [10.0, 0.0, 3.0]);
    assert!(close(bp.segs[1].feed, 300.0) && close(bp.segs[2].feed, 800.0));
    assert_eq!(bp.segs[2].line, 5);
}

#[test]
fn comments_case_and_packed_words() {
    let bp = parse_gcode("(en-tête)\ng0 x0 y0 z3 ; retrait\nG1X10Y5(milieu)F200\n");
    assert_eq!(bp.segs.len(), 1);
    assert_eq!(bp.segs[0].to, [10.0, 5.0, 3.0]);
    assert!(bp.warnings.is_empty());
}

#[test]
fn semicircle_arc_and_direction() {
    let bp = parse_gcode("G0 X-5 Y0 Z0\nG3 X5 Y0 I5 J0 F500\n");
    let s = &bp.segs[0];
    assert_eq!(s.kind, Kind::Arc);
    assert!(s.ccw && s.center == Some([0.0, 0.0]));
    assert!(close(s.sweep(), PI) && close(s.len, 5.0 * PI));
    // anti-horaire depuis (-5,0) : passe par (0,-5)
    let mid = s.point_at(0.5);
    assert!(close(mid[0], 0.0) && close(mid[1], -5.0), "{mid:?}");
    let d = s.direction_at(0.0).unwrap();
    assert!(close(d[0], 0.0) && close(d[1], -1.0));
    // horaire : passe par (0,5)
    let cw = parse_gcode("G0 X-5 Y0 Z0\nG2 X5 Y0 I5 J0 F500\n");
    assert!(close(cw.segs[0].point_at(0.5)[1], 5.0));
    assert!(close(cw.segs[0].sweep(), -PI));
}

#[test]
fn full_circle_and_helix() {
    let bp = parse_gcode("G0 X10 Y0 Z0\nG3 X10 Y0 I-10 J0 F500\nG3 X10 Y0 Z-2 I-10 J0\n");
    assert!(close(bp.segs[0].sweep(), 2.0 * PI), "{}", bp.segs[0].sweep());
    assert!(close(bp.segs[0].len, 20.0 * PI));
    let helix = &bp.segs[1];
    assert!(close(helix.len, (20.0 * PI).hypot(2.0)));
    assert!(close(helix.point_at(0.5)[2], -1.0));
    assert!(close(helix.point_at(0.25)[2], -0.5));
}

#[test]
fn inches_incremental_and_unknown_codes() {
    let bp = parse_gcode("G20\nG0 X0 Y0 Z0\nG1 X1 F10\nG91\nG1 Y1\nG90\nG99\nG65 P1\nG18\nB5\n");
    assert!(close(bp.segs[0].to[0], 25.4) && close(bp.segs[0].feed, 254.0));
    assert!(close(bp.segs[1].to[1], 25.4), "incrémental : {:?}", bp.segs[1]);
    let w = bp.warnings.join("|");
    // G18 est désormais géré (arcs du plan ZX) : plus d'avertissement
    assert!(w.contains("G65 ignoré") && !w.contains("G18") && w.contains("axe B"), "{w}");
    assert!(!w.contains("G99"));
}

#[test]
fn arc_without_ij_or_with_radius_falls_back_to_a_line_with_a_warning() {
    let bp = parse_gcode("G0 X0 Y0 Z0\nG2 X10 Y0 R5 F100\nG3 X20 Y0 F100\n");
    assert_eq!(bp.segs.len(), 2);
    assert!(bp.segs.iter().all(|s| s.kind == Kind::Feed));
    assert_eq!(bp.warnings.len(), 2);
}

#[test]
fn inconsistent_arc_radii_are_reported() {
    let bp = parse_gcode("G0 X0 Y0 Z0\nG2 X20 Y0 I5 J0 F100\n");
    assert!(bp.warnings.iter().any(|w| w.contains("rayons")));
}

#[test]
fn canned_drilling_g81_and_g83() {
    let simple = parse_gcode("G0 X0 Y0 Z3\nG81 X10 Y10 Z-4 R3 F300\nX20\nG80\n");
    // 1er trou : rapide XY, plongée R→Z, remontée ; 2e trou : idem
    let kinds: Vec<Kind> = simple.segs.iter().map(|s| s.kind).collect();
    assert_eq!(kinds, vec![Kind::Rapid, Kind::Plunge, Kind::Rapid, Kind::Rapid, Kind::Plunge, Kind::Rapid]);
    assert!(close(simple.segs[1].to[2], -4.0) && close(simple.segs[1].len, 7.0));
    assert_eq!(simple.segs[3].to, [20.0, 10.0, 3.0]);
    // sans position XY de départ connue, le premier trou n'invente pas de trajet depuis l'origine
    let unknown = parse_gcode("G0 Z3\nG81 X10 Y10 Z-4 R3 F300\nG80\n");
    assert_eq!(unknown.segs.iter().map(|s| s.kind).collect::<Vec<_>>(), vec![Kind::Plunge, Kind::Rapid]);

    let peck = parse_gcode("G0 X0 Y0 Z3\nG83 X10 Y10 Z-4 R3 Q2 F300\nG80\n");
    let depths: Vec<f64> = peck.segs.iter().filter(|s| s.kind == Kind::Plunge).map(|s| s.to[2]).collect();
    assert_eq!(depths, vec![1.0, -1.0, -3.0, -4.0]);
}

#[test]
fn unknown_start_position_is_not_drawn_from_the_origin() {
    // seul Z est connu au départ : le premier déplacement XY ne trace pas de rapide depuis (0,0)
    let bp = parse_gcode("G0 Z3\nG0 X50 Y40\nG1 Z-1 F100\nG1 X60\n");
    assert_eq!(bp.segs.len(), 2);
    assert_eq!(bp.segs[0].kind, Kind::Plunge);
    assert_eq!(bp.segs[0].from, [50.0, 40.0, 3.0]);
    // mais X et Y donnés dès la première ligne sont bien le point de départ
    let bp = parse_gcode("G0 X5 Y5 Z0\nG1 X10 F100\n");
    assert_eq!(bp.segs[0].from, [5.0, 5.0, 0.0]);
}

// ───────────── statistiques et positions ─────────────

#[test]
fn stats_lengths_time_and_levels() {
    let bp = parse_gcode("G0 X0 Y0 Z3\nG0 X100 Y0\nG1 Z-1 F600\nG1 X0 F1200\nG1 Z-2 F600\nG1 X100\nG0 Z3\n");
    let st = bp.stats(6000.0);
    // rapides : X0→100 (100) et la remontée finale de -2 à 3 (5)
    assert!(close(st.rapid_len, 105.0), "{}", st.rapid_len);
    // coupe : plongée 3→-1 (4), X100→0 (100), plongée -1→-2 (1), X0→100 (100)
    assert!(close(st.cut_len, 205.0), "{}", st.cut_len);
    // l'avance est modale : le dernier G1 X100 roule à F600 (réglé par `G1 Z-2 F600`)
    let expected = 4.0 / 600.0 * 60.0 + 100.0 / 1200.0 * 60.0 + 1.0 / 600.0 * 60.0 + 100.0 / 600.0 * 60.0 + 105.0 / 6000.0 * 60.0;
    assert!(close(st.time_s, expected), "{} vs {}", st.time_s, expected);
    assert_eq!(st.cut_levels, vec![-1.0, -2.0]);
    assert!(close(st.z_max, 3.0) && close(st.z_min, -2.0));
}

#[test]
fn position_at_and_profile() {
    let bp = parse_gcode("G0 X0 Y0 Z0\nG1 X10 F100\nG1 Y10\n");
    assert!(close(bp.total_len(), 20.0));
    let (p, i, d) = bp.position_at(15.0).unwrap();
    assert_eq!((p, i), ([10.0, 5.0, 0.0], 1));
    assert_eq!(d, Some([0.0, 1.0]));
    assert_eq!(bp.position_at(1e9).unwrap().1, 1); // au-delà de la fin : dernier segment
    let prof = bp.z_profile();
    assert_eq!(prof.len(), 4);
    assert!(close(prof[3].0, 20.0));
    assert!(parse_gcode("").position_at(0.0).is_none());
    assert!(parse_gcode("(rien)\nM30").bounds().is_none());
}

#[test]
fn bounds_include_arc_extremes() {
    let bp = parse_gcode("G0 X-5 Y0 Z0\nG3 X5 Y0 I5 J0 F100\n");
    let (lo, hi) = bp.bounds().unwrap();
    assert!(lo[1] < -4.99 && hi[0] > 4.99, "{lo:?} {hi:?}");
}

#[test]
fn cut_z_range_ignores_rapids_and_the_start_of_a_plunge() {
    let bp = parse_gcode("G0 X0 Y0 Z10\nG0 X5\nG1 Z-2 F100\nG1 X20\nG1 Z-4\nG0 Z10\n");
    // plongée 10 → -2 : seul le point bas (-2) compte ; la remontée en rapide est ignorée
    assert_eq!(bp.cut_z_range(), Some((-4.0, -2.0)));
    assert_eq!(bp.stats(5000.0).cut_z, Some((-4.0, -2.0)));
    assert_eq!(parse_gcode("G0 X0 Y0 Z5\nG0 X5").cut_z_range(), None);
}

// ───────────── scène ─────────────

fn lines(prims: &[Prim]) -> Vec<&Prim> {
    prims.iter().filter(|p| matches!(p, Prim::Line { dashed: false, .. })).collect()
}

fn arrows(prims: &[Prim]) -> Vec<&Prim> {
    prims.iter().filter(|p| matches!(p, Prim::Arrow { .. })).collect()
}

#[test]
fn z_gradient_runs_from_purple_at_the_bottom_to_yellow_at_the_top() {
    assert_eq!(color_for_z(-3.0, -3.0, 0.0), [68, 1, 84]);
    assert_eq!(color_for_z(0.0, -3.0, 0.0), [253, 231, 37]);
    let mid = color_for_z(-1.5, -3.0, 0.0);
    assert_eq!(mid, [33, 145, 140]);
    assert_eq!(color_for_z(5.0, -3.0, 0.0), [253, 231, 37]); // borné
}

#[test]
fn each_pass_gets_the_color_of_its_depth() {
    let bp = parse_gcode("G0 Z3\nG0 X0 Y0\nG1 Z0 F100\nG1 X10\nG1 Z-3\nG1 X0\n");
    let sc = build_scene(&bp, &ViewOptions { show_arrows: false, ..Default::default() });
    let ls = lines(&sc);
    let colors: Vec<[u8; 3]> = ls.iter().filter_map(|p| if let Prim::Line { rgb, .. } = p { Some(*rgb) } else { None }).collect();
    assert_eq!(colors.len(), 2);
    assert_ne!(colors[0], colors[1]);
    // l'échelle couvre les seules altitudes de coupe (0 → -3) : la hauteur de dégagement (3) n'écrase
    // pas le dégradé ; la passe à 0 est jaune (haut), celle à -3 violette (fond)
    assert_eq!(colors[0], [253, 231, 37]);
    assert_eq!(colors[1], [68, 1, 84]);
}

#[test]
fn a_ramp_becomes_a_gradient_of_short_lines() {
    let bp = parse_gcode("G0 X10 Y0 Z0\nG3 X10 Y0 Z-2 I-10 J0 F500\n");
    let sc = build_scene(&bp, &ViewOptions { show_arrows: false, show_rapids: false, ..Default::default() });
    let ls = lines(&sc);
    assert!(ls.len() >= 3, "hélice = dégradé : {} lignes", ls.len());
}

#[test]
fn arrows_follow_the_travel_direction_and_density() {
    let bp = parse_gcode("G0 X0 Y0 Z0\nG1 X100 F100\n");
    let sc = build_scene(&bp, &ViewOptions { arrow_spacing_mm: 10.0, ..Default::default() });
    let a = arrows(&sc);
    assert_eq!(a.len(), 10, "une flèche par 10 mm sur 100 mm");
    for p in a {
        let Prim::Arrow { dir, pos, .. } = p else { unreachable!() };
        assert_eq!(*dir, [1.0, 0.0]);
        assert!(pos[0] > 0.0 && pos[0] < 100.0);
    }
    // sens inversé : flèches vers -x
    let back = parse_gcode("G0 X100 Y0 Z0\nG1 X0 F100\n");
    let sc = build_scene(&back, &ViewOptions { arrow_spacing_mm: 10.0, ..Default::default() });
    assert!(arrows(&sc).iter().all(|p| matches!(p, Prim::Arrow { dir, .. } if *dir == [-1.0, 0.0])));
}

#[test]
fn arrows_on_a_ccw_circle_are_tangent_and_counter_clockwise() {
    let bp = parse_gcode("G0 X10 Y0 Z0\nG3 X10 Y0 I-10 J0 F500\n");
    let sc = build_scene(&bp, &ViewOptions { arrow_spacing_mm: 8.0, ..Default::default() });
    let a = arrows(&sc);
    assert!(a.len() >= 7);
    for p in a {
        let Prim::Arrow { pos, dir, .. } = p else { unreachable!() };
        // tangent : perpendiculaire au rayon ; anti-horaire : produit vectoriel (r × d) > 0
        assert!((pos[0] * dir[0] + pos[1] * dir[1]).abs() < 1e-6);
        assert!(pos[0] * dir[1] - pos[1] * dir[0] > 0.0);
    }
    let cw = parse_gcode("G0 X10 Y0 Z0\nG2 X10 Y0 I-10 J0 F500\n");
    let sc = build_scene(&cw, &ViewOptions { arrow_spacing_mm: 8.0, ..Default::default() });
    assert!(arrows(&sc).iter().all(|p| matches!(p, Prim::Arrow { pos, dir, .. } if pos[0] * dir[1] - pos[1] * dir[0] < 0.0)));
}

#[test]
fn toggles_hide_rapids_arrows_and_plunges() {
    let bp = parse_gcode("G0 X0 Y0 Z3\nG0 X5 Y5\nG1 Z-1 F100\nG1 X20\nG0 Z3\n");
    let all = build_scene(&bp, &ViewOptions::default());
    assert!(all.iter().any(|p| matches!(p, Prim::Line { dashed: true, .. })));
    assert!(all.iter().any(|p| matches!(p, Prim::Dot { rgb, .. } if *rgb == fablabcam_backplot::scene::PLUNGE_RGB)));
    let none = build_scene(&bp, &ViewOptions { show_rapids: false, show_arrows: false, show_plunges: false, ..Default::default() });
    assert!(none.iter().all(|p| !matches!(p, Prim::Line { dashed: true, .. } | Prim::Arrow { .. })));
    assert!(none.iter().all(|p| !matches!(p, Prim::Dot { rgb, .. } if *rgb == fablabcam_backplot::scene::PLUNGE_RGB)));
}

#[test]
fn z_filter_keeps_only_the_requested_passes() {
    let bp = parse_gcode("G0 Z3\nG0 X0 Y0\nG1 Z-1 F100\nG1 X10\nG0 Z3\nG0 X0 Y5\nG1 Z-2 F100\nG1 X10\nG0 Z3\n");
    let o = ViewOptions { show_arrows: false, show_rapids: false, ..Default::default() };
    assert_eq!(lines(&build_scene(&bp, &o)).len(), 2);
    let only_deep = ViewOptions { z_range: Some((-2.5, -1.5)), ..o.clone() };
    let scene = build_scene(&bp, &only_deep);
    let ls = lines(&scene);
    assert_eq!(ls.len(), 1);
    assert!(matches!(ls[0], Prim::Line { pts, .. } if close(pts[0][1], 5.0)));
    assert!(lines(&build_scene(&bp, &ViewOptions { z_range: Some((-5.0, -4.0)), ..o })).is_empty());
}

#[test]
fn animation_clips_the_path_and_shows_the_tool() {
    let bp = parse_gcode("G0 X0 Y0 Z0\nG1 X100 F100\nG1 Y100\n");
    let sc = build_scene(&bp, &ViewOptions { upto: Some(150.0), show_arrows: false, ..Default::default() });
    let drawn: f64 = sc.iter().filter_map(|p| if let Prim::Line { pts, .. } = p { Some(pts.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum::<f64>()) } else { None }).sum();
    assert!(close(drawn, 150.0), "{drawn}");
    let tool = sc.iter().find_map(|p| if let Prim::Dot { pos, ring: true, .. } = p { Some(*pos) } else { None }).unwrap();
    assert!(close(tool[0], 100.0) && close(tool[1], 50.0), "{tool:?}");
    // pas de marqueur de fin tant que le trajet n'est pas terminé
    assert!(!sc.iter().any(|p| matches!(p, Prim::Dot { rgb, .. } if *rgb == fablabcam_backplot::scene::END_RGB)));
    let full = build_scene(&bp, &ViewOptions::default());
    assert!(full.iter().any(|p| matches!(p, Prim::Dot { rgb, .. } if *rgb == fablabcam_backplot::scene::END_RGB)));
}

#[test]
fn color_by_type_distinguishes_lines_and_arcs() {
    let bp = parse_gcode("G0 X0 Y0 Z0\nG1 X10 F100\nG3 X20 Y10 I0 J10\n");
    let sc = build_scene(&bp, &ViewOptions { color_by: ColorBy::Type, show_arrows: false, ..Default::default() });
    let cols: Vec<[u8; 3]> = sc.iter().filter_map(|p| if let Prim::Line { rgb, dashed: false, .. } = p { Some(*rgb) } else { None }).collect();
    assert_eq!(cols.len(), 2);
    assert_ne!(cols[0], cols[1]);
}

// ───────────── SVG ─────────────

#[test]
fn svg_contains_the_expected_elements() {
    let bp = parse_gcode("G0 Z3\nG0 X0 Y0\nG1 Z-1 F100\nG3 X20 Y0 I10 J0\nG1 Z-2\nG1 X0\nG0 Z3\n");
    let svg = svg::to_svg(&bp, &ViewOptions::default(), &svg::SvgOptions { title: "Plaque <test>".into(), ..Default::default() });
    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
    assert!(svg.contains("Plaque &lt;test&gt;"), "le titre est échappé");
    assert!(svg.contains("stroke-dasharray=\"4 3\""), "rapides en pointillés");
    assert!(svg.contains("<polygon"), "flèches de sens");
    assert!(svg.contains("linearGradient") && svg.contains("(haut des coupes)") && svg.contains("(fond)"));
    assert!(svg.contains("Profil de profondeur"));
    assert!(svg.contains("niveau(x) de coupe"));
    assert!(svg.contains("<circle")); // départ/fin/plongée
}

#[test]
fn svg_of_an_empty_program_says_so_and_reports_warnings_otherwise() {
    let empty = svg::to_svg(&parse_gcode("(rien)"), &ViewOptions::default(), &svg::SvgOptions::default());
    assert!(empty.contains("Aucun mouvement"));
    let w = svg::to_svg(&parse_gcode("G0 X0 Y0 Z0\nG65 P1\nG1 X5 F10\n"), &ViewOptions::default(), &svg::SvgOptions::default());
    assert!(w.contains('⚠') && w.contains("G65"));
}

#[test]
fn html_embarque_les_segments_et_echappe_le_titre() {
    let bp = parse_gcode("G21 G90\nG0 Z5\nG1 X0 Y0 F100\nG1 Z-1\nG1 X10 F200\nG3 X0 Y0 I-5 J0\n");
    let json = fablabcam_backplot::web::to_json(&bp, "a</script>b");
    assert!(json.contains("\"segs\":[[") && !json.contains("</script>"));
    assert_eq!(json.matches("],[").count() + 1, bp.segs.len());
    let html = fablabcam_backplot::web::to_html(&bp, "<x>&");
    assert!(html.contains("<title>&lt;x&gt;&amp; — backplot</title>"));
    assert!(!html.contains("__DATA__") && !html.contains("__TITLE__"));
}

// ───────────── arcs dans les plans verticaux (G18 / G19, G-code de Fusion) ─────────────

fn on_circle(p: &[f64; 3], u: usize, v: usize, c: (f64, f64), r: f64) -> bool {
    ((p[u] - c.0).hypot(p[v] - c.1) - r).abs() < 1e-6
}

#[test]
fn g19_arc_is_a_quarter_circle_in_the_yz_plane() {
    // entrée de Fusion : de Y-4.8 Z-9.4, quart de cercle de rayon 0,6 centré en (Y-4.2, Z-9.4) jusqu'à Z-10
    let bp = parse_gcode("G0 X260.6 Y-4.8 Z-9.4\nG1 Z-9.4 F333\nG19 G3 Y-4.2 Z-10. J0.6 K0. F1000.\nG17 G1 Y-3.6");
    let arc: Vec<_> = bp.segs.iter().filter(|s| s.line == 3).collect();
    assert!(arc.len() > 4, "{arc:?}");
    assert!(bp.warnings.is_empty(), "{:?}", bp.warnings);
    for s in &arc {
        assert!(on_circle(&s.to, 1, 2, (-4.2, -9.4), 0.6), "{:?}", s.to);
        assert!((s.to[0] - 260.6).abs() < 1e-9 && s.to[2] <= -9.4 + 1e-9 && s.to[1] >= -4.8 - 1e-9);
    }
    let end = arc.last().unwrap().to;
    assert!((end[1] + 4.2).abs() < 1e-9 && (end[2] + 10.0).abs() < 1e-9);
    // retour au plan XY : la ligne suivante est un simple segment
    assert_eq!(bp.segs.last().unwrap().to, [260.6, -3.6, -10.0]);
}

#[test]
fn g18_arcs_go_down_and_back_up_in_the_zx_plane() {
    // entrée et sortie de Fusion dans le plan ZX (centre en Z-5.4, X-4.2, rayon 0,6)
    let bp = parse_gcode("G0 X-4.8 Y769 Z-5.4\nG18 G2 X-4.2 Z-6. I0.6 K0. F1000.\nG1 X-3.6\nG1 X-4.2\nG18 G3 X-4.8 Z-5.4 I0. K0.6");
    for line in [2, 5] {
        let arc: Vec<_> = bp.segs.iter().filter(|s| s.line == line).collect();
        assert!(arc.len() > 4);
        for s in &arc {
            assert!(on_circle(&s.to, 2, 0, (-5.4, -4.2), 0.6), "ligne {line} : {:?}", s.to);
            assert!(s.to[2] <= -5.4 + 1e-9 && s.to[0] <= -4.2 + 1e-9, "sous le centre, côté X négatif : {:?}", s.to);
        }
    }
    let low = bp.segs.iter().filter(|s| s.line == 2).map(|s| s.to[2]).fold(f64::INFINITY, f64::min);
    assert!((low + 6.0).abs() < 1e-9);
}
