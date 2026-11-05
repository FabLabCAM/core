use fablabcam_studio::studio::demo_project;
use fablabcam_studio::{dxf, persist, shape, Studio};

fn dxf_with_units(code: i64) -> String {
    format!("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n{code}\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nCIRCLE\n10\n1\n20\n2\n40\n0.5\n0\nENDSEC\n0\nEOF\n")
}

#[test]
fn dxf_en_pouces_converti_en_mm() {
    let r = dxf::parse_dxf(&dxf_with_units(1)).unwrap();
    assert_eq!(r.shapes[0].1, fablabcam_model::Entity::Circle { cx: 25.4, cy: 50.8, r: 12.7 });
    assert!(r.notes[0].contains("pouces"), "{:?}", r.notes);
    // mm ou sans unité : tel quel, sans remarque
    for c in [4, 0] {
        let r = dxf::parse_dxf(&dxf_with_units(c)).unwrap();
        assert_eq!(r.shapes[0].1, fablabcam_model::Entity::Circle { cx: 1.0, cy: 2.0, r: 0.5 });
        assert!(r.notes.is_empty());
    }
    let r = dxf::parse_dxf(&dxf_with_units(5)).unwrap();
    assert_eq!(r.shapes[0].1, fablabcam_model::Entity::Circle { cx: 10.0, cy: 20.0, r: 5.0 });
    assert!(dxf::parse_dxf(&dxf_with_units(77)).unwrap().notes[0].contains("inconnue"));
    // l'écriture reste en mm : l'aller-retour garde les valeurs converties
    let back = dxf::parse_dxf(&dxf::write_dxf(&r.shapes)).unwrap();
    assert_eq!(back.shapes[0].1, r.shapes[0].1);
    let _ = shape::bounds(&back.shapes[0].1);
}

#[test]
fn gcode_en_pouces() {
    let mm = Studio::new(demo_project());
    let mut p = demo_project();
    p.gcode_inches = true;
    let inch = Studio::new(p.clone());
    let g = inch.gcode();
    assert!(g.lines().nth(1).unwrap().starts_with("G20"), "{g}");
    assert!(mm.gcode().lines().nth(1).unwrap().starts_with("G21"));
    // relu (G20 → mm), le trajet est le même à la résolution de 4 décimales de pouce près (0,0025 mm)
    let bi = fablabcam_backplot::parse_gcode(&g);
    let bm = mm.backplot();
    let (li, hi) = bi.bounds().unwrap();
    let (lm, hm) = bm.bounds().unwrap();
    assert!(((hi[0] - li[0]) - (hm[0] - lm[0])).abs() < 0.005, "{} {}", hi[0] - li[0], hm[0] - lm[0]);
    assert!(g.contains("Z0.1181"), "dégagement 3 mm = 0,1181 po");
    // les avances aussi (800 mm/min → 31,4961 po/min)
    assert!(g.contains("F31.4961"), "avance");
    // post-processeur : {$units} → G20
    let out = inch.gcode_with_post(&fablabcam_cam::PostDef::default());
    assert!(out.gcode.contains("\nG20 G90 G17 G94 G40 G49 G80 G91.1\n"), "{}", &out.gcode[..200]);
    // sauvegarde
    let back = persist::load(&persist::save(&p)).unwrap();
    assert!(back.gcode_inches);
    assert!(!persist::save(&demo_project()).contains("GcodeUnits"));
}
