use fablabcam_studio::{script, shape, Editor, Project, Tool, ToolKind};

fn project() -> Project {
    let mut p = Project::new();
    assert_eq!(p.add_shape("0", shape::circle((0.0, 0.0), 10.0).unwrap()).unwrap(), 1);
    p.add_tool(Tool::new("F2", ToolKind::EndMill, 2.0)).unwrap();
    p
}

#[test]
fn references_hors_plage_refusees_par_toutes_les_actions() {
    let base = project();
    let actions = [
        "delete(IDS)", "move(IDS, 1, 2)", "rotate(IDS, 0, 0, 90)",
        "scale(IDS, 0, 0, 2)", "copy(IDS, 20, 0)", "offset(IDS, 1)",
        "profile(\"P\", IDS, \"F2\", -1)", "pocket(\"P\", IDS, \"F2\", -1)",
        "drill(\"P\", IDS, \"F2\", -1)", "engrave(\"P\", IDS, \"F2\", -1)",
    ];
    let invalid = [
        ("4294967296", "4294967296"), ("4294967297", "4294967297"),
        ("-1", "-1"), ("-4294967295", "-4294967295"),
        ("9223372036854775807", "9223372036854775807"),
        ("(-9223372036854775807 - 1)", "-9223372036854775808"),
    ];
    for action in actions {
        for (expression, value) in invalid {
            for ids in [expression.to_string(), format!("[1, {expression}]")] {
                let code = format!("move(1, 5, 0);\n{};", action.replace("IDS", &ids));
                let error = script::run(&base, &code).expect_err(&code);
                assert!(error.contains("identifiant") && error.contains("hors plage"), "{code}: {error}");
                assert!(error.contains(value) && error.contains("line 2"), "{code}: {error}");
            }
        }
    }
}

#[test]
fn echec_preserve_le_projet_et_les_deux_historiques() {
    let base = project();
    let mut editor = Editor::new(base.clone());
    editor.edit(|p| p.move_shapes(&[1], 2.0, 0.0)).unwrap();
    let moved = editor.project.clone();
    assert!(editor.undo());
    for ids in ["4294967297", "[1, -4294967295]"] {
        let error = editor.edit(|p| -> Result<(), String> {
            let out = script::run(p, &format!("circle(20, 20, 2); move(1, 5, 0); delete({ids});"))?;
            *p = out.project;
            Ok(())
        }).unwrap_err();
        assert!(error.contains("hors plage"), "{error}");
        assert_eq!(editor.project, base);
        assert!(!editor.can_undo());
        assert!(editor.can_redo());
    }
    assert!(editor.redo());
    assert_eq!(editor.project, moved);
    assert!(editor.undo());
    assert_eq!(editor.project, base);
}

#[test]
fn bornes_u32_et_identifiants_valides_conserves() {
    // Références d'un projet déjà construit : aucune allocation d'identifiant
    // n'est nécessaire pour déplacer une forme située à une borne numérique.
    for id in [0, 1, u32::MAX] {
        let mut base = project();
        base.shapes[0].id = id;
        for reference in [id.to_string(), format!("[{id}]")] {
            let out = script::run(&base, &format!("move({reference}, 3, 4);")).unwrap();
            assert_eq!(out.project.shapes[0].id, id);
            assert_eq!(out.project.shapes[0].entity, shape::circle((3.0, 4.0), 10.0).unwrap());
        }
    }
}

#[test]
fn types_non_entiers_et_references_absentes_restent_refuses() {
    let base = project();
    for value in ["1.0", "\"1\"", "true", "()", "[1.0]", "[1, \"1\"]"] {
        let error = script::run(&base, &format!("delete({value});")).unwrap_err();
        assert!(error.contains("identifiant"), "{value}: {error}");
    }
    for id in [0, 2, u32::MAX] {
        for reference in [id.to_string(), format!("[1, {id}]")] {
            let error = script::run(&base, &format!("delete({reference});")).unwrap_err();
            assert!(error.contains(&id.to_string()) && !error.contains("hors plage"), "{error}");
        }
    }
}
