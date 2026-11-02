use fablabcam_geom as geom;
use fablabcam_model::Entity;
use fablabcam_studio::{dxf, shape, svgio};

fn dxf_doc(entities: &str) -> String {
    format!("0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n")
}

fn area(e: &Entity) -> f64 {
    geom::area(&e.to_pline().unwrap())
}

#[test]
fn dxf_ellipse_complete_et_arc_d_ellipse() {
    let full = dxf_doc("0\nELLIPSE\n8\nE\n10\n10\n20\n5\n11\n6\n21\n0\n40\n0.5\n41\n0\n42\n6.283185307179586\n");
    let r = dxf::parse_dxf(&full).unwrap();
    assert!(r.ignored.is_empty(), "{:?}", r.ignored);
    let (layer, e) = &r.shapes[0];
    assert_eq!(layer, "E");
    assert!(shape::is_closed(e));
    // aire π a b = π × 6 × 3, à la tolérance de la polyligne près
    assert!((area(e) - std::f64::consts::PI * 18.0).abs() < 0.2, "{}", area(e));
    let (lo, hi) = shape::bounds(e).unwrap();
    // sommets sur la courbe : les extrêmes sont atteints à la tolérance des polylignes près
    let t = fablabcam_studio::curves::TOL + 1e-6;
    assert!((lo[0] - 4.0).abs() < t && (hi[0] - 16.0).abs() < t && (hi[1] - 8.0).abs() < t, "{lo:?} {hi:?}");
    let half = dxf_doc("0\nELLIPSE\n10\n0\n20\n0\n11\n4\n21\n0\n40\n0.5\n41\n0\n42\n3.141592653589793\n");
    let e = &dxf::parse_dxf(&half).unwrap().shapes[0].1;
    assert!(!shape::is_closed(e));
    let (lo, hi) = shape::bounds(e).unwrap();
    assert!(lo[1] > -1e-9 && (hi[1] - 2.0).abs() < t && (lo[0] + 4.0).abs() < t);
}

#[test]
fn dxf_spline_par_points_de_controle_et_d_ajustement() {
    // Bézier cubique : nœuds 0 0 0 0 1 1 1 1
    let ctrl = "10\n0\n20\n0\n10\n0\n20\n10\n10\n10\n20\n10\n10\n10\n20\n0\n";
    let knots = "40\n0\n40\n0\n40\n0\n40\n0\n40\n1\n40\n1\n40\n1\n40\n1\n";
    let doc = dxf_doc(&format!("0\nSPLINE\n8\nS\n70\n8\n71\n3\n72\n8\n73\n4\n{knots}{ctrl}"));
    let r = dxf::parse_dxf(&doc).unwrap();
    assert!(r.ignored.is_empty(), "{:?}", r.ignored);
    let e = &r.shapes[0].1;
    let (lo, hi) = shape::bounds(e).unwrap();
    // la courbe part de (0,0), finit en (10,0) et monte à 7,5 (3/4 de la hauteur du polygone)
    assert!(lo[0].abs() < 1e-9 && (hi[0] - 10.0).abs() < 1e-9 && (hi[1] - 7.5).abs() < 1e-3, "{lo:?} {hi:?}");
    // points d'ajustement seuls : polyligne passant par eux, signalée
    let fit = dxf_doc("0\nSPLINE\n70\n0\n71\n3\n74\n3\n11\n0\n21\n0\n11\n5\n21\n5\n11\n10\n21\n0\n");
    let r = dxf::parse_dxf(&fit).unwrap();
    assert_eq!(r.shapes.len(), 1);
    assert!(r.ignored[0].contains("points d'ajustement"));
    // nœuds incohérents : erreur avec le numéro de ligne
    let bad = dxf_doc(&format!("0\nSPLINE\n71\n3\n40\n0\n40\n1\n{ctrl}"));
    let err = dxf::parse_dxf(&bad).unwrap_err();
    assert!(err.contains("ligne") && err.contains("nœuds"), "{err}");
}

#[test]
fn svg_ellipse() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><ellipse cx="50" cy="20" rx="10" ry="5"/></svg>"#;
    let r = svgio::parse_svg(svg).unwrap();
    assert!(r.ignored.is_empty(), "{:?}", r.ignored);
    let e = &r.shapes[0].1;
    assert!(shape::is_closed(e));
    let (lo, hi) = shape::bounds(e).unwrap();
    // y retourné : centre à 100 - 20 = 80
    let t = fablabcam_studio::curves::TOL + 1e-6;
    assert!((lo[0] - 40.0).abs() < t && (hi[0] - 60.0).abs() < t && ((lo[1] + hi[1]) / 2.0 - 80.0).abs() < t, "{lo:?} {hi:?}");
    // écart d'aire ≤ périmètre × tolérance (flèche des cordes)
    assert!((area(e) - std::f64::consts::PI * 50.0).abs() < 49.0 * fablabcam_studio::curves::TOL);
}

#[test]
fn texte_en_contours() {
    use fablabcam_studio::text::{default_font, outlines, TextStyle};
    let Some(path) = default_font() else {
        eprintln!("aucune police système : test sauté");
        return;
    };
    let font = std::fs::read(path).unwrap();
    let style = TextStyle { height: 10.0, ..TextStyle::default() };
    let h = outlines(&font, "H", (0.0, 0.0), &style).unwrap().0;
    assert_eq!(h.len(), 1, "H : un seul contour");
    let (lo, hi) = shape::bounds(&h[0]).unwrap();
    assert!(lo[1].abs() < 0.05 && (hi[1] - 10.0).abs() < 0.05, "hauteur de majuscule 10 : {lo:?} {hi:?}");
    // « o » : contour extérieur + trou, le trou est à l'intérieur
    let o = outlines(&font, "o", (0.0, 0.0), &style).unwrap().0;
    assert_eq!(o.len(), 2);
    let (a, b) = (o[0].to_pline().unwrap(), o[1].to_pline().unwrap());
    assert!(geom::is_inside(&a, &b) || geom::is_inside(&b, &a));
    // deux lignes : la seconde est plus bas ; l'espacement décale les lettres
    let two = outlines(&font, "H\nH", (0.0, 0.0), &style).unwrap().0;
    let lows: Vec<f64> = two.iter().map(|e| shape::bounds(e).unwrap().0[1]).collect();
    assert!(lows[1] < lows[0] - 10.0);
    let tight = outlines(&font, "HH", (0.0, 0.0), &style).unwrap().0;
    let wide = outlines(&font, "HH", (0.0, 0.0), &TextStyle { spacing: 5.0, ..style.clone() }).unwrap().0;
    assert!((shape::bounds(&wide[1]).unwrap().0[0] - shape::bounds(&tight[1]).unwrap().0[0] - 5.0).abs() < 1e-9);
    assert!(outlines(&font, "", (0.0, 0.0), &style).is_err());
    // caractère absent (privé Unicode) : signalé, le reste est dessiné
    let (s, missing) = outlines(&font, "H\u{E000}", (0.0, 0.0), &style).unwrap();
    assert_eq!((s.len(), missing.as_str()), (1, "\u{E000}"));
    assert!(outlines(b"pas une police", "H", (0.0, 0.0), &style).is_err());
    assert!(outlines(&font, "H", (0.0, 0.0), &TextStyle { height: 0.0, ..style }).is_err());
}

#[test]
fn texte_usinable_en_poche_avec_ilots() {
    use fablabcam_studio::text::{default_font, outlines, TextStyle};
    let Some(path) = default_font() else { return };
    let font = std::fs::read(path).unwrap();
    let letters = outlines(&font, "Ob", (0.0, 0.0), &TextStyle { height: 20.0, ..TextStyle::default() }).unwrap().0;
    let plines: Vec<geom::Pline> = letters.iter().map(|e| e.to_pline().unwrap()).collect();
    let p = fablabcam_cam::Params { tool_diameter: 1.0, target_depth: -1.0, depth_increment: 1.0, ..fablabcam_cam::Params::default() };
    let t = fablabcam_cam::pocket_shapes(&plines, &p).unwrap();
    assert!(t.moves.len() > 50);
}
