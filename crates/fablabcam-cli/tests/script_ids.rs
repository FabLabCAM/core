use fablabcam_studio::{persist, shape, Project};
use std::{fs, process::Command};

#[test]
fn script_invalide_ne_cree_ni_necrase_aucun_fichier() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("entree.cb");
    let script = dir.path().join("identifiants.rhai");
    let output = dir.path().join("sortie.cb");
    let mut project = Project::new();
    project.add_shape("0", shape::circle((0.0, 0.0), 10.0).unwrap()).unwrap();
    let original = persist::save(&project);
    fs::write(&input, &original).unwrap();
    for ids in ["4294967297", "[1, -4294967295]"] {
        fs::write(&script, format!("move(1, 5, 0);\ndelete({ids});")).unwrap();
        // Une sortie absente, une sortie existante, et l'entrée utilisée comme
        // sortie doivent toutes être préservées lorsque le script échoue.
        for mode in 0..3 {
            let destination = if mode == 2 { &input } else { &output };
            if mode == 1 {
                fs::write(&output, "sortie existante à conserver").unwrap();
            }
            let before = fs::read(destination).ok();
            let result = Command::new(env!("CARGO_BIN_EXE_fablabcam"))
                .arg("script").arg(&script).arg(&input).arg("-o").arg(destination)
                .output().unwrap();
            assert_eq!(result.status.code(), Some(1), "{ids}, mode {mode}");
            let error = String::from_utf8(result.stderr).unwrap();
            assert!(error.contains("hors plage") && error.contains("line 2"), "{error}");
            assert!(result.stdout.is_empty());
            assert_eq!(fs::read(destination).ok(), before);
            assert_eq!(fs::read_to_string(&input).unwrap(), original);
        }
        fs::remove_file(&output).unwrap();
    }
}
