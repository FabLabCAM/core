use fablabcam_studio::studio::demo_project;
use fablabcam_studio::{persist, shape, OpKind, Project, Studio, Tool, ToolKind};

#[test]
fn gravure_suit_le_trace_ouvert() {
    let mut p = Project::new();
    let l = p.add_shape("0", shape::polyline(&[(0.0, 0.0), (20.0, 0.0), (20.0, 10.0)], false).unwrap()).unwrap();
    let c = p.add_shape("0", shape::circle((50.0, 0.0), 5.0).unwrap()).unwrap();
    p.add_tool(Tool::new("V", ToolKind::VBit, 6.0)).unwrap();
    let id = p.add_operation(OpKind::Engrave, "Gravure", vec![l, c], "V").unwrap();
    p.update_operation(id, |o| { o.depth = -0.5; o.step_down = 0.5; }).unwrap();
    let s = Studio::new(p);
    assert!(s.computed[&id].error.is_none(), "{:?}", s.computed[&id].error);
    let bp = s.backplot();
    assert!(bp.warnings.is_empty(), "{:?}", bp.warnings);
    // l'outil passe exactement sur les sommets, sans décalage
    let g = s.gcode();
    assert!(g.contains("X20 Y10") || g.contains("X20 Y10 "), "{g}");
    assert!(g.contains("Z-0.5"));
    let (lo, hi) = bp.bounds().unwrap();
    assert!((lo[0] - 0.0).abs() < 1e-6 && (hi[0] - 55.0).abs() < 1e-6 && (lo[1] + 5.0).abs() < 1e-6, "{lo:?} {hi:?}");
    // la matière laissée ne s'applique pas à la gravure
    let mut s2 = Studio::new(s.project().clone());
    s2.edit(|p| p.update_operation(id, |o| o.stock_to_leave = 2.0)).unwrap();
    assert_eq!(s2.gcode(), g);
}

#[test]
fn temporisation_g82() {
    let mut s = Studio::new(demo_project());
    let id = s.project().ops.iter().find(|o| o.kind == OpKind::Drill).unwrap().id;
    s.edit(|p| p.update_operation(id, |o| { o.dwell = 0.5; o.step_down = 0.0; })).unwrap();
    let g = s.gcode();
    assert!(g.contains("G82") && g.contains("P0.5"), "{g}");
    assert!(s.backplot().warnings.is_empty(), "{:?}", s.backplot().warnings);
    // avec picage, la temporisation est ignorée
    s.edit(|p| p.update_operation(id, |o| o.step_down = 1.0)).unwrap();
    let g = s.gcode();
    assert!(g.contains("G83") && !g.contains("G82"));
    // post-processeur : gabarit DrillDwell lu
    let (d, ignored) = fablabcam_cam::PostDef::parse("<PostProcessor><DrillDwell>{$g82} {$x} {$y} {$z} {$p}</DrillDwell></PostProcessor>").unwrap();
    assert!(ignored.is_empty(), "{ignored:?}");
    s.edit(|p| p.update_operation(id, |o| o.step_down = 0.0)).unwrap();
    assert!(s.gcode_with_post(&d).gcode.contains("G82 X5 Y5 Z-4 P0.5"));
}

#[test]
fn pieces_regroupent_les_operations() {
    let mut s = Studio::new(demo_project());
    let ids: Vec<u32> = s.project().ops.iter().map(|o| o.id).collect();
    s.edit(|p| {
        p.update_operation(ids[0], |o| o.part = "Plaque".into())?;
        p.update_operation(ids[1], |o| o.part = "Plaque".into())?;
        p.update_operation(ids[2], |o| o.part = "Cadre".into())
    })
    .unwrap();
    assert_eq!(s.project().parts(), ["Plaque", "Cadre"]);
    let g = s.gcode();
    assert_eq!(g.matches("(Pièce : Plaque)").count(), 1);
    assert!(g.find("(Pièce : Plaque)").unwrap() < g.find("(Pièce : Cadre)").unwrap());
    // désactiver une pièce retire ses opérations du G-code
    assert_eq!(s.edit(|p| Ok::<_, shape::EditError>(p.set_part_enabled("Plaque", false))).unwrap(), 2);
    let g = s.gcode();
    assert!(!g.contains("Poche centrale") && g.contains("Découpe"));
    // sauvegarde
    let xml = persist::save(s.project());
    let back = persist::load(&xml).unwrap();
    assert_eq!(back.parts(), ["Plaque", "Cadre"]);
    assert_eq!(persist::save(&back), xml);
}

#[test]
fn temporisation_sauvegardee() {
    let mut p = demo_project();
    let id = p.ops.iter().find(|o| o.kind == OpKind::Drill).unwrap().id;
    p.update_operation(id, |o| o.dwell = 1.5).unwrap();
    let back = persist::load(&persist::save(&p)).unwrap();
    assert_eq!(back.ops.iter().find(|o| o.kind == OpKind::Drill).unwrap().dwell, 1.5);
}
