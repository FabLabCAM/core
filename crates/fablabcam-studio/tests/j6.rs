use fablabcam_studio::sim::{auto_stock, default_stock};
use fablabcam_studio::studio::demo_project;
use fablabcam_studio::{persist, CamSettings, LeadChoice, Operation, Stock, Studio, TabShape};

fn op_id(s: &Studio, name: &str) -> u32 {
    s.project().ops.iter().find(|o| o.name == name).unwrap().id
}

/// G-code obtenu après modification d'une opération de la démo ; erreur de calcul s'il y en a une.
fn with(name: &str, f: impl FnOnce(&mut Operation)) -> Result<String, String> {
    let mut s = Studio::new(demo_project());
    let id = op_id(&s, name);
    s.edit(|p| p.update_operation(id, f)).unwrap();
    match &s.computed[&id].error {
        Some(e) => Err(e.clone()),
        None => Ok(s.gcode()),
    }
}

fn demo_gcode() -> String {
    Studio::new(demo_project()).gcode()
}

#[test]
fn reglages_par_defaut_inchanges() {
    // les valeurs par défaut reproduisent l'ancien calcul et n'ajoutent rien au .cb
    assert_eq!(with("Découpe", |o| o.cam = CamSettings::default()).unwrap(), demo_gcode());
    let xml = persist::save(&demo_project());
    for k in ["Clearance=", "Surface=", "LevelFirst=", "TabShape=", "LeadRadius="] {
        assert!(!xml.contains(k), "{k}");
    }
}

#[test]
fn plan_de_degagement_et_surface() {
    let g = with("Découpe", |o| {
        o.cam.clearance = 10.0;
        o.cam.surface = 2.0;
        o.depth = -2.0;
    })
    .unwrap();
    assert!(g.contains("Z10"), "dégagement à 10");
    // première passe : surface 2, incrément 2 → Z0
    let i = g.find("(Operation : Découpe)").unwrap();
    assert!(g[i..].contains("Z0\n") || g[i..].contains("Z0 "), "{}", &g[i..i + 300]);
    assert!(with("Découpe", |o| o.cam.clearance = -1.0).unwrap_err().contains("dégagement"));
    assert!(with("Découpe", |o| { o.cam.surface = -5.0; }).unwrap_err().contains("sous la surface"));
}

#[test]
fn ordre_finition_surcoupe_et_transition_changent_le_trajet() {
    let base = demo_gcode();
    assert_ne!(with("Poche centrale", |o| o.cam.level_first = true).unwrap(), base);
    assert_ne!(with("Poche centrale", |o| o.cam.finish_stepover = 0.5).unwrap(), base);
    assert_ne!(with("Poche centrale", |o| o.cam.final_increment = 0.2).unwrap(), base);
    assert_ne!(with("Poche centrale", |o| { o.cam.level_first = true; o.cam.max_crossover = 0.0 }).unwrap(), with("Poche centrale", |o| o.cam.level_first = true).unwrap());
    assert_ne!(with("Découpe", |o| { o.side = fablabcam_studio::OpSide::Inside; o.cam.corner_overcut = true }).unwrap(), with("Découpe", |o| o.side = fablabcam_studio::OpSide::Inside).unwrap());
    assert!(with("Poche centrale", |o| o.cam.max_crossover = 1.5).unwrap_err().contains("transition"));
    assert!(with("Poche centrale", |o| o.cam.finish_stepover = -1.0).unwrap_err().contains("finition"));
}

#[test]
fn broche_avances_et_entrees() {
    assert!(with("Découpe", |o| o.cam.spindle_ccw = true).unwrap().contains("M4"));
    assert!(with("Découpe", |o| o.feed = Some(1234.0)).unwrap().contains("F1234"));
    let tangent = with("Poche centrale", |o| o.lead = LeadChoice::Tangent).unwrap();
    assert_ne!(with("Poche centrale", |o| { o.lead = LeadChoice::Tangent; o.cam.lead_radius = 2.0 }).unwrap(), tangent);
    let spiral = with("Poche centrale", |o| o.lead = LeadChoice::Spiral).unwrap();
    assert_ne!(with("Poche centrale", |o| { o.lead = LeadChoice::Spiral; o.cam.lead_angle = 5.0 }).unwrap(), spiral);
    assert!(with("Poche centrale", |o| o.cam.lead_angle = 95.0).unwrap_err().contains("rampe"));
}

#[test]
fn forme_et_nombre_des_ponts() {
    let tri = demo_gcode();
    let square = with("Découpe", |o| o.cam.tab_shape = TabShape::Square).unwrap();
    assert_ne!(square, tri);
    assert_ne!(with("Découpe", |o| o.cam.tab_shape = TabShape::Skip).unwrap(), square);
    assert_ne!(with("Découpe", |o| { o.cam.tab_min = 8; o.cam.tab_max = 8 }).unwrap(), tri);
    assert_ne!(with("Découpe", |o| o.cam.tab_height = 1.0).unwrap(), tri);
    assert!(with("Découpe", |o| { o.cam.tab_min = 5; o.cam.tab_max = 2 }).unwrap_err().contains("minimal"));
    assert!(with("Découpe", |o| o.cam.tab_width = 0.0).unwrap_err().contains("largeur"));
    // sans ponts, leurs réglages ne sont pas contrôlés
    assert!(with("Découpe", |o| { o.tabs = false; o.cam.tab_width = 0.0 }).is_ok());
}

#[test]
fn reglages_sauvegardes_et_recharges() {
    let mut s = Studio::new(demo_project());
    let id = op_id(&s, "Découpe");
    let cam = CamSettings {
        clearance: 8.5,
        surface: 1.0,
        level_first: true,
        finish_stepover: 0.3,
        finish_at_target_only: false,
        final_increment: 0.25,
        corner_overcut: true,
        max_crossover: 0.5,
        spindle_ccw: true,
        tab_shape: TabShape::Skip,
        tab_width: 4.0,
        tab_height: 1.5,
        tab_min: 2,
        tab_max: 4,
        tab_distance: 25.0,
        tab_target: fablabcam_studio::TabTarget::Outer,
        tab_threshold: 8.0,
        lead_angle: 4.0,
        lead_radius: 2.5,
    };
    s.edit(|p| p.update_operation(id, |o| o.cam = cam.clone())).unwrap();
    let xml = persist::save(s.project());
    let back = Studio::new(persist::load(&xml).unwrap());
    assert_eq!(back.project().op(op_id(&back, "Découpe")).unwrap().cam, cam);
    assert_eq!(persist::save(back.project()), xml);
    assert_eq!(back.gcode(), s.gcode());
}

#[test]
fn edition_du_brut() {
    let mut s = Studio::new(demo_project());
    let auto = auto_stock(&s).unwrap();
    assert_eq!(default_stock(&s), Some(auto));
    let manual = Stock { min: [-20.0, -20.0], max: [100.0, 80.0], top: 1.0, bottom: -6.0 };
    s.edit(|p| p.set_stock(Some(manual))).unwrap();
    assert_eq!(default_stock(&s), Some(manual));
    assert_eq!(auto_stock(&s), Some(auto), "le calcul automatique ignore le brut saisi");
    // sauvegardé, et annulable
    let back = persist::load(&persist::save(s.project())).unwrap();
    assert_eq!(back.stock, Some(manual));
    s.undo();
    assert_eq!(s.project().stock, None);
    // valeurs incohérentes refusées
    for bad in [Stock { max: [-30.0, 80.0], ..manual }, Stock { top: -7.0, ..manual }, Stock { bottom: f64::NAN, ..manual }] {
        assert!(s.edit(|p| p.set_stock(Some(bad))).is_err());
    }
    // brut automatique : le dessus suit la surface la plus haute des opérations
    let id = op_id(&s, "Découpe");
    s.edit(|p| p.update_operation(id, |o| { o.cam.surface = 2.0; o.cam.clearance = 5.0 })).unwrap();
    assert_eq!(auto_stock(&s).unwrap().top, 2.0);
}

#[test]
fn methode_et_seuil_des_ponts() {
    use fablabcam_studio::TabTarget;
    let sans = with("Découpe", |o| o.tabs = false).unwrap();
    let auto = demo_gcode();
    assert_ne!(auto, sans);
    // la découpe est un profil extérieur : « intérieurs seulement » ne pose aucun pont
    assert_eq!(with("Découpe", |o| o.cam.tab_target = TabTarget::Inner).unwrap(), sans);
    assert_eq!(with("Découpe", |o| o.cam.tab_target = TabTarget::Outer).unwrap(), auto);
    // contour plus court que le seuil : pas de pont
    assert_eq!(with("Découpe", |o| o.cam.tab_threshold = 10_000.0).unwrap(), sans);
    assert!(with("Découpe", |o| o.cam.tab_threshold = -1.0).unwrap_err().contains("seuil"));
    // sauvegardés
    let mut s = Studio::new(demo_project());
    let id = op_id(&s, "Découpe");
    s.edit(|p| p.update_operation(id, |o| { o.cam.tab_target = TabTarget::Outer; o.cam.tab_threshold = 12.5 })).unwrap();
    let back = persist::load(&persist::save(s.project())).unwrap();
    let c = &back.op(back.ops.iter().find(|o| o.name == "Découpe").unwrap().id).unwrap().cam;
    assert_eq!((c.tab_target, c.tab_threshold), (TabTarget::Outer, 12.5));
}

#[test]
fn methode_manuelle_des_ponts() {
    use fablabcam_studio::TabTarget;
    let sans = with("Découpe", |o| o.tabs = false).unwrap();
    // manuel sans point posé : aucun pont (et non plus le placement automatique)
    assert_eq!(with("Découpe", |o| o.cam.tab_target = TabTarget::Manual).unwrap(), sans);
    // un point posé : un pont à cet endroit
    let un = with("Découpe", |o| {
        o.cam.tab_target = TabTarget::Manual;
        o.tab_points = vec![[40.0, -3.0]];
    })
    .unwrap();
    assert_ne!(un, sans);
    assert_ne!(un, demo_gcode());
    // sauvegardé
    let mut s = Studio::new(demo_project());
    let id = op_id(&s, "Découpe");
    s.edit(|p| p.update_operation(id, |o| o.cam.tab_target = TabTarget::Manual)).unwrap();
    let back = persist::load(&persist::save(s.project())).unwrap();
    assert_eq!(back.ops.iter().find(|o| o.name == "Découpe").unwrap().cam.tab_target, TabTarget::Manual);
}
