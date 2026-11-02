use fablabcam_studio::script;
use fablabcam_studio::{OpKind, Project, Studio};

#[test]
fn script_dessine_et_cree_les_operations() {
    let code = r#"
        layer("Plaque");
        let p = rect(0, 0, 100, 60.5);
        layer("Trous");
        let trous = [];
        for x in [10, 90] {
            for y in [10, 50] {
                trous.push(circle(x, y, 1.5));
            }
        }
        tool("F6", 6);
        drill_tool("Foret 3", 3);
        profile("Découpe", p, "F6", -3);
        drill("Perçages", trous, "Foret 3", -3);
        print(`${shapes().len()} formes`);
    "#;
    let out = script::run(&Project::new(), code).unwrap();
    assert_eq!(out.log, ["5 formes"]);
    let p = &out.project;
    assert_eq!(p.shapes.iter().filter(|s| s.layer == "Trous").count(), 4);
    assert_eq!(p.ops.len(), 2);
    assert!(p.ops.iter().all(|o| o.depth == -3.0));
    assert_eq!(p.ops[1].kind, OpKind::Drill);
    let s = Studio::new(p.clone());
    assert!(s.computed.values().all(|c| c.error.is_none()));
    assert!(s.gcode().contains("G83"), "incrément par défaut 0,5 : picage");
}

#[test]
fn transformations_copies_et_polyligne() {
    let code = r#"
        let a = polyline([[0, 0], [10, 0], [10, 10]], true);
        let b = copy(a, 20, 0);
        rotate(b, 25, 5, 90);
        scale(a, 0, 0, 2);
        let o = offset(a, 1.5);
        delete(o);
        move(shapes(), 1, 1);
    "#;
    let out = script::run(&Project::new(), code).unwrap();
    assert_eq!(out.project.shapes.len(), 2);
}

#[test]
fn erreur_sans_effet_et_numero_de_ligne() {
    let mut base = Project::new();
    base.add_shape("0", fablabcam_studio::shape::circle((0.0, 0.0), 1.0).unwrap()).unwrap();
    let e = script::run(&base, "circle(0, 0, 2);\nprofile(\"x\", 1, \"outil absent\", -1);").unwrap_err();
    assert!(e.contains("outil absent") && e.contains("line 2"), "{e}");
    // le projet d'origine n'a pas bougé (le script travaille sur une copie)
    assert_eq!(base.shapes.len(), 1);
    assert!(script::run(&base, "circle(0, 0, 0)").unwrap_err().contains("cercle"), "rayon nul refusé");
    assert!(script::run(&base, "rect(0, 0, \"x\", 1)").unwrap_err().contains("nombre attendu"));
    assert!(script::run(&base, "delete(999)").unwrap_err().contains("999"));
    assert!(script::run(&base, "let x = ;").unwrap_err().starts_with("script"));
}

#[test]
fn boucle_infinie_arretee() {
    let e = script::run(&Project::new(), "loop { }").unwrap_err();
    assert!(e.to_lowercase().contains("too many operations") || e.contains("operations"), "{e}");
}
