//! Repeatable furniture workflows for documentation and headless fabrication planning.
use fablabcam_studio::furniture::{self, Document, ExportOptions, Furniture, Template};
use std::path::Path;

pub const HELP:&str="FabLabCAM — meubles paramétriques (millimètres)\n\n\
fablabcam furniture template cabinet|shelving -o modele.fcm-template.json\n\
fablabcam furniture new modele.fcm-template.json -o meuble.fcm.json [--width mm] [--depth mm] [--height mm] [--thickness mm] [--shelves n] [--profile machine.json] [--columns n --rows n] [--bins yes]\n\
fablabcam furniture validate document.json\n\
fablabcam furniture plan meuble.fcm.json\n\
fablabcam furniture pack meuble.fcm.json -o fabrication.zip\n\n\
Les exports exigent un nouveau fichier. Le dossier ZIP contient des plans, pas de G-code.\n";

pub fn run(args: &[String]) -> Result<String, String> {
    if args.is_empty() || args == ["help"] {
        return Ok(HELP.into());
    }
    let command = args[0].as_str();
    let source = args.get(1).ok_or("Document ou gabarit manquant")?;
    let mut options = std::collections::BTreeMap::new();
    let (pairs, remainder) = args[2..].as_chunks::<2>();
    for pair in pairs {
        let allowed = match command {
            "new" => [
                "-o",
                "--width",
                "--depth",
                "--height",
                "--thickness",
                "--shelves",
                "--profile",
                "--columns",
                "--rows",
                "--bins",
            ]
            .contains(&pair[0].as_str()),
            "template" | "pack" => pair[0] == "-o",
            _ => false,
        };
        if !allowed || options.insert(pair[0].as_str(), pair[1].as_str()).is_some() {
            return Err(format!("Option inconnue ou répétée : {}", pair[0]));
        }
    }
    if !remainder.is_empty() {
        return Err("Valeur d'option manquante".into());
    }
    let destination = || {
        options
            .get("-o")
            .copied()
            .ok_or("Destination -o manquante".to_string())
    };
    match command {
        "template" => {
            let template = match source.as_str() {
                "cabinet" => Template::cabinet(),
                "shelving" => Template::shelving(),
                _ => return Err("Gabarit attendu : cabinet ou shelving".into()),
            };
            furniture::export_file(
                Path::new(destination()?),
                Document::Template(template).to_json()?.as_bytes(),
            )?;
            Ok("Modèle exporté\n".into())
        }
        "new" => {
            let Document::Template(t) = furniture::read_document(Path::new(source))? else {
                return Err("Un modèle réutilisable est requis".into());
            };
            let mut f = Furniture::from_template(t);
            for (key, value) in [
                ("--width", &mut f.dimensions.width),
                ("--depth", &mut f.dimensions.depth),
                ("--height", &mut f.dimensions.height),
                ("--thickness", &mut f.dimensions.thickness),
            ] {
                if let Some(s) = options.get(key) {
                    *value = s.parse().map_err(|_| format!("Nombre invalide : {key}"))?;
                }
            }
            if let Some(s) = options.get("--shelves") {
                f.dimensions.shelves = s.parse().map_err(|_| "Nombre de tablettes invalide")?;
            }
            if options.contains_key("--columns") || options.contains_key("--rows") {
                let parse = |key: &str| {
                    options
                        .get(key)
                        .copied()
                        .unwrap_or("1")
                        .parse::<usize>()
                        .map_err(|_| format!("Nombre invalide : {key}"))
                };
                f.dimensions.composition = Some(Box::new(furniture::Composition::grid(
                    parse("--columns")?,
                    parse("--rows")?,
                )?));
            }
            if let Some(value) = options.get("--bins") {
                if *value != "yes" && *value != "no" {
                    return Err("--bins attend yes ou no".into());
                }
                if *value == "yes" {
                    if f.dimensions.composition.is_none() {
                        f.dimensions.composition = Some(Box::new(
                            furniture::Composition::from_dimensions(&f.dimensions),
                        ));
                    }
                    let scene = furniture::structure(&f.dimensions)?;
                    let root = &mut f.dimensions.composition.as_mut().unwrap().root;
                    for cell in scene.cells.iter().filter(|c| c.leaf) {
                        root.find_mut(&cell.id).unwrap().bin = Some(furniture::Bin {
                            depth: (cell.size[1] - 2.0).min(250.0),
                            ..furniture::Bin::default()
                        });
                    }
                }
            }
            if let Some(s) = options.get("--profile") {
                let json = std::fs::read_to_string(s).map_err(|e| e.to_string())?;
                f.setup.machine = fablabcam_cam::gcode::machine::Profile::from_json(&json)?;
            }
            furniture::export_file(
                Path::new(destination()?),
                Document::Furniture(Box::new(f)).to_json()?.as_bytes(),
            )?;
            Ok("Projet de meuble exporté\n".into())
        }
        "validate" => {
            let doc = furniture::read_document(Path::new(source))?;
            Ok(format!(
                "Document valide : {} · format {} · modèle r{}\n",
                doc.name(),
                furniture::SCHEMA_VERSION,
                doc.template().revision
            ))
        }
        "plan" | "pack" => {
            let Document::Furniture(f) = furniture::read_document(Path::new(source))? else {
                return Err("Un projet de meuble est requis".into());
            };
            if command == "plan" {
                return serde_json::to_string_pretty(&furniture::place(&f)?)
                    .map(|s| s + "\n")
                    .map_err(|e| e.to_string());
            }
            let bytes = furniture::fabrication_zip(&f, &ExportOptions::default())?;
            furniture::export_file(Path::new(destination()?), &bytes)?;
            Ok(format!(
                "Dossier géométrique exporté : {} dalles ; G-code à préparer séparément\n",
                furniture::place(&f)?.sheets.len()
            ))
        }
        _ => Err(format!("Commande inconnue : {command}\n{HELP}")),
    }
}
