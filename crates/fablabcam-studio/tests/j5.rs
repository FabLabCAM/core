use fablabcam_backplot::parse_gcode;
use fablabcam_studio::sim::{Cutter, Sim};
use fablabcam_studio::studio::demo_project;
use fablabcam_studio::{persist, Project, Stock, Studio, Tool, ToolKind};

fn names(p: &Project) -> Vec<(String, u32)> {
    p.tools.iter().map(|t| (t.name.clone(), t.index)).collect()
}

#[test]
fn numeros_d_outil_uniques_et_automatiques() {
    let mut p = Project::new();
    p.add_tool(Tool::new("A", ToolKind::EndMill, 3.0)).unwrap();
    p.add_tool(Tool::new("B", ToolKind::EndMill, 4.0)).unwrap();
    let mut c = Tool::new("C", ToolKind::Drill, 2.0);
    c.index = 7;
    p.add_tool(c).unwrap();
    assert_eq!(names(&p), [("A".into(), 1), ("B".into(), 2), ("C".into(), 7)]);
    // doublon refusé, message avec le nom du propriétaire
    let mut d = Tool::new("D", ToolKind::EndMill, 5.0);
    d.index = 2;
    assert!(p.add_tool(d).unwrap_err().to_string().contains("déjà utilisé par « B »"));
    // la modification garde le numéro si 0, accepte le sien, refuse celui d'un autre
    let mut b2 = p.tool("B").unwrap().clone();
    b2.index = 0;
    p.update_tool("B", b2).unwrap();
    assert_eq!(p.tool_number("B"), 2);
    let mut b3 = p.tool("B").unwrap().clone();
    b3.index = 7;
    assert!(p.update_tool("B", b3).is_err());
    // le premier numéro libre est réutilisé après suppression, sans renuméroter les autres
    p.delete_tool("A", None).unwrap();
    p.add_tool(Tool::new("E", ToolKind::EndMill, 6.0)).unwrap();
    assert_eq!(names(&p), [("B".into(), 2), ("C".into(), 7), ("E".into(), 1)]);
}

#[test]
fn validation_des_proprietes() {
    let ok = Tool::new("T", ToolKind::EndMill, 6.0);
    let bad = |f: &dyn Fn(&mut Tool)| {
        let mut t = ok.clone();
        f(&mut t);
        Project::new().add_tool(t).unwrap_err().to_string()
    };
    assert!(bad(&|t| t.shank_diameter = -1.0).contains("diamètre de queue"));
    assert!(bad(&|t| t.helix_angle = 95.0).contains("angle d'hélice"));
    assert!(bad(&|t| t.kind = ToolKind::VBit).contains("fraise en V"));
    assert!(bad(&|t| { t.length = 20.0; t.flute_length = 30.0 }).contains("longueur de coupe"));
    assert!(bad(&|t| t.tooth_load = f64::NAN).contains("charge par dent"));
    // une fraise en V est créée à 90° par défaut
    assert_eq!(Tool::new("V", ToolKind::VBit, 6.0).vee_angle, 90.0);
}

fn full_tool() -> Tool {
    let mut t = Tool::new("Fraise \"spéciale\" & <co>", ToolKind::BullNose, 6.0);
    t.index = 12;
    t.flute_length = 18.0;
    t.length = 50.0;
    t.shank_diameter = 6.0;
    t.helix_angle = 30.0;
    t.max_ramp_angle = 3.5;
    t.axial_depth = 6.0;
    t.radial_depth = 2.4;
    t.tooth_load = 0.05;
    t.material = "Carbure".into();
    t.coating = "TiAlN".into();
    t.part_code = "REF-42".into();
    t.notes = "ligne 1\nligne 2 avec \"guillemets\" & <balises>".into();
    t.comment = "bois dur".into();
    t.tool_change = "M5\nM0 (changer l'outil)\nT12 M6".into();
    t
}

#[test]
fn toutes_les_proprietes_sont_sauvegardees() {
    let mut p = demo_project();
    p.add_tool(full_tool()).unwrap();
    let mut v = Tool::new("V 60", ToolKind::VBit, 8.0);
    v.vee_angle = 60.0;
    p.add_tool(v).unwrap();
    let xml = persist::save(&p);
    let back = persist::load(&xml).unwrap();
    assert_eq!(back.tools, p.tools);
    assert_eq!(persist::save(&back), xml, "format idempotent");
    // un outil sans propriété optionnelle n'écrit que le strict nécessaire
    assert!(!xml.contains("Coating=\"\"") && !xml.contains("ShankDiameter=\"0\""));
}

#[test]
fn ancien_document_sans_index_est_numerote_dans_l_ordre() {
    let xml = persist::save(&demo_project()).replace(" Index=\"1\"", "").replace(" Index=\"2\"", "");
    assert!(!xml.contains("Index="));
    let p = persist::load(&xml).unwrap();
    assert_eq!(names(&p), [("Fraise 6 mm".into(), 1), ("Foret 3 mm".into(), 2)]);
}

#[test]
fn numero_et_changement_d_outil_dans_le_gcode() {
    let mut s = Studio::new(demo_project());
    s.edit(|p| p.update_tool("Foret 3 mm", { let mut t = p.tool("Foret 3 mm").unwrap().clone(); t.index = 9; t })).unwrap();
    let g = s.gcode();
    assert!(g.contains("M6 T9") && g.contains("M6 T1") && !g.contains("M6 T2"), "{g}");
    // texte propre à l'outil : remplace l'appel M6 par défaut, à chaque changement vers cet outil
    s.edit(|p| p.update_tool("Foret 3 mm", { let mut t = p.tool("Foret 3 mm").unwrap().clone(); t.tool_change = "M5\nM0\nT9 M6 (foret)".into(); t })).unwrap();
    let g = s.gcode();
    let l: Vec<&str> = g.lines().collect();
    let at = l.iter().position(|x| x.starts_with("(Changement d'outil : Foret 3 mm")).unwrap();
    assert_eq!(&l[at + 1..at + 4], ["M5", "M0", "T9 M6 (foret)"], "{g}");
    assert_eq!(g.matches("T9 M6").count(), 1);
}

#[test]
fn post_processeur_utilise_numero_commentaire_et_changement_propre() {
    use fablabcam_cam::PostDef;
    let mut s = Studio::new(demo_project());
    s.edit(|p| p.update_tool("Fraise 6 mm", { let mut t = p.tool("Fraise 6 mm").unwrap().clone(); t.index = 4; t.comment = "bois".into(); t })).unwrap();
    s.edit(|p| p.update_tool("Foret 3 mm", { let mut t = p.tool("Foret 3 mm").unwrap().clone(); t.tool_change = "M0 (foret)\nT{$tool.index} M6".into(); t })).unwrap();
    let def = PostDef { tool_change: "T{$tool.index} M6 {$comment} {$tool.comment} {$endcomment}".into(), ..PostDef::default() };
    let out = s.gcode_with_post(&def);
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    let l: Vec<&str> = out.gcode.lines().collect();
    assert!(l.contains(&"T4 M6 ( bois )"), "{}", out.gcode);
    // le foret (index 2) utilise son propre texte, avec macros développées
    assert!(l.windows(2).any(|w| w == ["M0 ( foret )", "T2 M6"]) || l.windows(2).any(|w| w[0].starts_with("M0") && w[1] == "T2 M6"), "{}", out.gcode);
}

#[test]
fn angle_du_v_modifie_la_simulation() {
    let stock = Stock { min: [-10.0, -10.0], max: [30.0, 30.0], top: 0.0, bottom: -5.0 };
    let bp = parse_gcode("G21 G90\nG0 X10 Y10 Z5\nG1 Z-2 F100\n");
    let cut = |vee: f64| {
        let mut sim = Sim::new(&stock, 0.1);
        sim.run(&bp, Cutter { r: 3.0, kind: ToolKind::VBit, vee }, -2.0, false, "v", f64::INFINITY);
        sim.map
    };
    // à 1 mm de l'axe, le fond est à -2 + 1 / tan(angle/2)
    assert!((cut(90.0).height_at(11.0, 10.0) - (-1.0)).abs() < 0.12); // cellules de 0,1 mm : ±0,05 sur d
    assert!((cut(60.0).height_at(11.0, 10.0) - (-2.0 + 1.0 / 30f64.to_radians().tan())).abs() < 0.12);
    assert!(cut(120.0).height_at(11.0, 10.0) < cut(90.0).height_at(11.0, 10.0));
    assert!(cut(60.0).removed_volume() < cut(90.0).removed_volume());
}
