use fablabcam_studio::studio::demo_project;
use fablabcam_studio::{persist, shape, OpKind, Project, Studio, Tool, ToolKind};

#[test]
fn table_d_outils() {
    let mut p = Project::new();
    let mut t = Tool::new("Fraise 6 mm", ToolKind::EndMill, 6.0);
    t.flutes = 2;
    t.feed = 800.0;
    t.plunge = 300.0;
    t.spindle = 12000.0;
    p.add_tool(t.clone()).unwrap();
    // le numéro d'outil (0 = auto) est attribué à l'ajout
    let stored = Tool { index: 1, ..t.clone() };
    assert_eq!(p.tool("Fraise 6 mm"), Some(&stored));
    assert!(matches!(p.add_tool(t), Err(shape::EditError::DuplicateName(_))));
    assert!(p.add_tool(Tool::new("zéro", ToolKind::EndMill, 0.0)).is_err());
    assert_eq!(p.tools.len(), 1);
}

#[test]
fn suppression_d_un_outil_utilise() {
    let mut p = demo_project();
    let err = p.delete_tool("Fraise 6 mm", None).unwrap_err();
    assert!(err.to_string().contains("Poche centrale"), "{err}");
    assert_eq!(p.tools.len(), 2);
    p.add_tool(Tool::new("Fraise 3 mm", ToolKind::EndMill, 3.0)).unwrap();
    p.delete_tool("Fraise 6 mm", Some("Fraise 3 mm")).unwrap();
    assert!(p.ops.iter().all(|o| o.tool != "Fraise 6 mm"));
    // renommage : les opérations suivent
    let mut t = p.tool("Fraise 3 mm").unwrap().clone();
    t.name = "F3".into();
    p.update_tool("Fraise 3 mm", t).unwrap();
    assert!(p.ops.iter().any(|o| o.tool == "F3"));
}

#[test]
fn changer_d_outil_recalcule_les_trajectoires() {
    let mut s = Studio::new(demo_project());
    let id = s.project().ops[0].id;
    let before = s.computed[&id].path.clone().unwrap().moves.len();
    assert_eq!(s.computed[&id].params.as_ref().unwrap().tool_diameter, 6.0);
    s.edit(|p| p.add_tool(Tool::new("Fraise 3 mm", ToolKind::EndMill, 3.0))).unwrap();
    s.edit(|p| p.update_operation(id, |o| o.tool = "Fraise 3 mm".into())).unwrap();
    let c = &s.computed[&id];
    assert_eq!(c.params.as_ref().unwrap().tool_diameter, 3.0);
    assert!(c.path.as_ref().unwrap().moves.len() > before, "plus d'anneaux avec une fraise plus petite");
}

#[test]
fn ilots_poche() {
    let s = Studio::new(demo_project());
    let g = s.gcode();
    assert!(g.contains("1 îlot") || g.contains("1 ilot"), "{g}");
}

#[test]
fn liste_ordre_et_desactivation() {
    let mut s = Studio::new(demo_project());
    let g0 = s.gcode();
    assert!(g0.find("Operation : Poche centrale") < g0.find("Operation : Perçages"));
    let drill = s.project().ops[1].id;
    s.edit(|p| p.move_operation(drill, 0)).unwrap();
    let g1 = s.gcode();
    assert!(g1.find("Operation : Perçages") < g1.find("Operation : Poche centrale"));
    s.edit(|p| p.update_operation(drill, |o| o.enabled = false)).unwrap();
    assert!(!s.gcode().contains("Perçages"));
    let copy = s.edit(|p| p.duplicate_operation(drill)).unwrap();
    assert_eq!(s.project().ops.iter().position(|o| o.id == copy), Some(1));
    s.edit(|p| p.delete_operation(copy)).unwrap();
    assert_eq!(s.project().ops.len(), 3);
}

#[test]
fn parametre_invalide_garde_le_dernier_resultat() {
    let mut s = Studio::new(demo_project());
    let id = s.project().ops[0].id;
    let good = s.computed[&id].path.clone().unwrap().moves.len();
    s.edit(|p| p.update_operation(id, |o| o.step_down = 0.0)).unwrap();
    let c = &s.computed[&id];
    assert!(c.error.as_deref().unwrap().contains("incrément"), "{:?}", c.error);
    assert_eq!(c.path.as_ref().unwrap().moves.len(), good);
    assert!(s.gcode().contains("Poche centrale"), "le G-code garde le dernier calcul valide");
    s.edit(|p| p.update_operation(id, |o| o.step_down = 1.0)).unwrap();
    assert!(s.computed[&id].error.is_none());
}

#[test]
fn forme_supprimee_est_retiree_des_operations() {
    let mut s = Studio::new(demo_project());
    let hole = s.project().ops[1].shapes[0];
    let touched = s.edit(|p| Ok(p.delete_shapes(&[hole]))).unwrap();
    assert_eq!(touched, vec!["Perçages".to_string()]);
    assert_eq!(s.project().ops[1].shapes.len(), 3);
    s.undo();
    assert_eq!(s.project().ops[1].shapes.len(), 4);
}

#[test]
fn gcode_deux_outils_avec_changement() {
    let s = Studio::new(demo_project());
    let g = s.gcode();
    assert_eq!(g.matches("M6 T").count(), 3, "{g}"); // fraise, foret, fraise
    assert!(g.contains("M6 T1") && g.contains("M6 T2"));
    assert!(g.trim_end().ends_with("M30"));
    assert_eq!(g.matches("M30").count(), 1);
}

#[test]
fn sauvegarde_et_rechargement() {
    let s = Studio::new(demo_project());
    let xml = persist::save(s.project());
    let back = persist::load(&xml).unwrap();
    assert_eq!(back.tools, s.project().tools);
    assert_eq!(back.ops.len(), 3);
    assert_eq!(back.shapes.len(), s.project().shapes.len());
    let s2 = Studio::new(back);
    assert_eq!(s2.gcode(), s.gcode(), "G-code identique après rechargement");
    // idempotence du format
    assert_eq!(persist::save(s2.project()), xml);
}

#[test]
fn ancien_document_sans_outils_ni_operations() {
    let p = persist::load("<CADFile><Layers><Layer Name=\"0\"><circle X=\"1\" Y=\"2\" R=\"3\"/></Layer></Layers></CADFile>").unwrap();
    assert_eq!(p.shapes.len(), 1);
    assert!(p.tools.is_empty() && p.ops.is_empty());
}

#[test]
fn operation_sans_forme_ou_outil_inconnu() {
    let mut p = demo_project();
    assert!(p.add_operation(OpKind::Pocket, "x", vec![9999], "Fraise 6 mm").is_err());
    assert!(p.add_operation(OpKind::Pocket, "x", vec![], "inconnu").is_err());
    let s = Studio::new({
        let mut q = Project::new();
        q.add_tool(Tool::new("T", ToolKind::EndMill, 3.0)).unwrap();
        q.add_operation(OpKind::Pocket, "vide", vec![], "T").unwrap();
        q
    });
    assert!(s.computed.values().next().unwrap().error.is_some());
}

#[test]
fn outil_depuis_un_style() {
    let xml = std::fs::read_to_string(format!("{}/../../tests/fixtures/styles-demo.xml", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let lib = fablabcam_cam::StyleLibrary::parse(&xml).unwrap();
    let name = lib.names().next().unwrap().to_string();
    let t = Tool::from_style("Depuis style", &lib, &name).unwrap();
    let expected = fablabcam_cam::Params::from_style(&lib.resolve(&name).unwrap());
    assert_eq!(t.diameter, expected.tool_diameter);
    assert_eq!(t.feed, expected.cut_feedrate);
    assert!(Tool::from_style("x", &lib, "n'existe pas").is_err());
}
