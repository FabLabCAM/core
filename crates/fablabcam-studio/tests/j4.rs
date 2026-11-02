use fablabcam_studio::studio::demo_project;
use fablabcam_studio::{persist, Studio};

fn decoupe(s: &Studio) -> u32 {
    s.project().ops.iter().find(|o| o.name == "Découpe").unwrap().id
}

#[test]
fn poser_et_retirer_un_pont() {
    let mut s = Studio::new(demo_project());
    let id = decoupe(&s);
    assert_eq!(s.edit(|p| p.toggle_tab_point(id, [40.0, -5.0], 1.0)), Ok(true));
    assert_eq!(s.edit(|p| p.toggle_tab_point(id, [10.0, -5.0], 1.0)), Ok(true));
    assert_eq!(s.project().op(id).unwrap().tab_points.len(), 2);
    // un clic à moins du rayon retire le pont le plus proche
    assert_eq!(s.edit(|p| p.toggle_tab_point(id, [40.4, -5.3], 1.0)), Ok(false));
    assert_eq!(s.project().op(id).unwrap().tab_points, vec![[10.0, -5.0]]);
    assert!(s.edit(|p| p.toggle_tab_point(9999, [0.0, 0.0], 1.0)).is_err());
}

#[test]
fn les_ponts_manuels_changent_le_gcode_et_s_annulent() {
    let mut s = Studio::new(demo_project());
    let id = decoupe(&s);
    let auto = s.gcode();
    s.edit(|p| p.toggle_tab_point(id, [40.0, -5.0], 1.0)).unwrap();
    let manuel = s.gcode();
    assert_ne!(auto, manuel);
    assert!(s.computed[&id].error.is_none(), "{:?}", s.computed[&id].error);
    s.undo();
    assert_eq!(s.gcode(), auto);
    assert!(s.project().op(id).unwrap().tab_points.is_empty());
}

#[test]
fn les_ponts_manuels_sont_sauvegardes() {
    let mut s = Studio::new(demo_project());
    let id = decoupe(&s);
    s.edit(|p| p.toggle_tab_point(id, [40.0, -5.0], 1.0)).unwrap();
    s.edit(|p| p.toggle_tab_point(id, [-5.5, 30.25], 1.0)).unwrap();
    let xml = persist::save(s.project());
    assert!(xml.contains("TabPoints=\"40,-5 -5.5,30.25\""), "{xml}");
    let back = Studio::new(persist::load(&xml).unwrap());
    assert_eq!(back.project().op(decoupe(&back)).unwrap().tab_points, vec![[40.0, -5.0], [-5.5, 30.25]]);
    assert_eq!(back.gcode(), s.gcode());
    assert_eq!(persist::save(back.project()), xml);
}

#[test]
fn deplacer_les_formes_deplace_les_ponts() {
    let mut s = Studio::new(demo_project());
    let id = decoupe(&s);
    s.edit(|p| p.toggle_tab_point(id, [40.0, -5.0], 1.0)).unwrap();
    let shapes = s.project().op(id).unwrap().shapes.clone();
    s.edit(|p| p.move_shapes(&shapes, 2.0, 3.0)).unwrap();
    assert_eq!(s.project().op(id).unwrap().tab_points, vec![[42.0, -2.0]]);
}

#[test]
fn matiere_laissee_eloigne_l_outil_et_est_sauvegardee() {
    let mut s = Studio::new(demo_project());
    let id = decoupe(&s);
    let avant = s.gcode();
    s.edit(|p| p.update_operation(id, |o| o.stock_to_leave = 0.5)).unwrap();
    assert!(s.computed[&id].error.is_none());
    assert_ne!(s.gcode(), avant);
    let xml = persist::save(s.project());
    assert!(xml.contains("StockToLeave=\"0.5\""), "{xml}");
    let back = Studio::new(persist::load(&xml).unwrap());
    assert_eq!(back.project().op(decoupe(&back)).unwrap().stock_to_leave, 0.5);
    assert_eq!(back.gcode(), s.gcode());
    // retour à 0 : G-code d'origine, attribut absent
    s.edit(|p| p.update_operation(id, |o| o.stock_to_leave = 0.0)).unwrap();
    assert_eq!(s.gcode(), avant);
    assert!(!persist::save(s.project()).contains("StockToLeave"));
}

#[test]
fn matiere_laissee_trop_negative_est_signalee() {
    let mut s = Studio::new(demo_project());
    let id = decoupe(&s);
    let avant = s.gcode();
    s.edit(|p| p.update_operation(id, |o| o.stock_to_leave = -10.0)).unwrap();
    let c = &s.computed[&id];
    assert!(c.error.as_deref().is_some_and(|e| e.contains("surcoupe")), "{:?}", c.error);
    assert_eq!(s.gcode(), avant, "la trajectoire précédente est conservée");
}

#[test]
fn en_tete_et_pied_d_operation() {
    let mut s = Studio::new(demo_project());
    let id = decoupe(&s);
    s.edit(|p| p.update_operation(id, |o| {
        o.header = "M8\n(avant coupe)\n\n".into();
        o.footer = "M9\r\nG4 P2".into();
    }))
    .unwrap();
    let g = s.gcode();
    let lines: Vec<&str> = g.lines().collect();
    let at = lines.iter().position(|l| *l == "(Operation : Découpe)").unwrap();
    assert_eq!(&lines[at + 1..at + 3], ["M8", "(avant coupe)"]);
    let end = lines.iter().position(|l| *l == "M30").unwrap();
    assert_eq!(&lines[end - 3..end - 1], ["M9", "G4 P2"]);
    assert_eq!(lines[end - 1], "M5");
    // sauvegarde : sauts de ligne conservés
    let xml = persist::save(s.project());
    let back = Studio::new(persist::load(&xml).unwrap());
    let o = back.project().op(decoupe(&back)).unwrap().clone();
    assert_eq!((o.header.as_str(), o.footer.as_str()), ("M8\n(avant coupe)\n\n", "M9\r\nG4 P2"));
    assert_eq!(back.gcode(), g);
    // les autres opérations ne sont pas touchées
    assert_eq!(g.matches("M8").count(), 1);
}

#[test]
fn post_processeur_avec_en_tete_et_pied_d_operation() {
    use fablabcam_cam::PostDef;
    let mut s = Studio::new(demo_project());
    let id = decoupe(&s);
    s.edit(|p| p.update_operation(id, |o| { o.header = "M8".into(); o.footer = "M9".into(); })).unwrap();
    let out = s.gcode_with_post(&PostDef::default());
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    let l: Vec<&str> = out.gcode.lines().collect();
    // fraise, foret, fraise : un changement à chaque changement d'outil (3), plus la table d'outils d'en-tête
    assert_eq!(l.iter().filter(|x| x.ends_with("M6")).count(), 3, "{}", out.gcode);
    assert!(l.contains(&"( T2 : 3 )"));
    let at = l.iter().position(|x| *x == "( Découpe )").unwrap();
    assert!(l[at..].contains(&"M8") && l[at..].contains(&"M9"));
    assert_eq!(*l.last().unwrap(), "M30");
}

#[test]
fn pont_accroche_au_contour_seulement() {
    let mut s = Studio::new(demo_project());
    let id = decoupe(&s); // rectangle 0..80 × 0..60, fraise Ø6
    // clic à 2 mm du bord bas : le pont est ramené sur le contour (y = 0)
    assert_eq!(s.edit(|p| p.click_tab(id, [40.0, 2.0], 1.0, 1.0)), Ok(Some(true)));
    assert_eq!(s.project().op(id).unwrap().tab_points, vec![[40.0, 0.0]]);
    // clic sur la trajectoire (décalée d'un rayon, y = -3) : accepté aussi
    assert_eq!(s.edit(|p| p.click_tab(id, [20.0, -3.0], 1.0, 1.0)), Ok(Some(true)));
    // clic au milieu de la pièce : refusé, rien ne change
    assert_eq!(s.edit(|p| p.click_tab(id, [40.0, 30.0], 1.0, 1.0)), Ok(None));
    assert_eq!(s.project().op(id).unwrap().tab_points.len(), 2);
    // clic sur un pont existant : retiré
    assert_eq!(s.edit(|p| p.click_tab(id, [40.3, 0.2], 1.0, 1.0)), Ok(Some(false)));
    assert_eq!(s.project().op(id).unwrap().tab_points, vec![[20.0, 0.0]]);
}
