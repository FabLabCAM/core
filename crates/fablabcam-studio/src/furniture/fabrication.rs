use super::{place, placed_contour, CamRequirement, Document, Furniture, Layout, MachiningFace};
use crate::{dxf, persist, Project, Stock, Tool, ToolKind};
use std::io::{Cursor, Write};

#[derive(Clone, Debug, PartialEq)]
pub struct ExportOptions {
    pub native: bool,
    pub cb: bool,
    pub dxf: bool,
    pub svg: bool,
    pub csv: bool,
    pub pdf: bool,
}
impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            native: true,
            cb: true,
            dxf: true,
            svg: true,
            csv: true,
            pdf: true,
        }
    }
}

/// Geometry and stock are handed to CAM. Speeds, holding tabs, depths and operations
/// must be chosen there. No unreviewed cutter path is presented as machine ready.
pub fn cam_project(job: &Furniture, sheet: usize) -> Result<Project, String> {
    let layout = place(job)?;
    project_for_sheet(job, &layout, sheet, MachiningFace::Front)
}
pub fn cam_project_for_face(
    job: &Furniture,
    sheet: usize,
    face: MachiningFace,
) -> Result<Project, String> {
    let project = project_for_sheet(job, &place(job)?, sheet, face)?;
    if project.shapes.is_empty() {
        return Err("Aucune opération sur cette face".into());
    }
    Ok(project)
}
fn project_for_sheet(
    job: &Furniture,
    layout: &Layout,
    sheet: usize,
    face: MachiningFace,
) -> Result<Project, String> {
    let sheet = layout.sheets.get(sheet).ok_or("Dalle inconnue")?;
    let mut project = Project::new();
    project.machine_profile = Some(job.setup.machine.clone());
    project
        .set_stock(Some(Stock {
            min: [0.0, 0.0],
            max: job.setup.sheet_size,
            top: 0.0,
            bottom: -sheet.placements[0].panel.thickness,
        }))
        .map_err(|e| e.to_string())?;
    project
        .add_tool(Tool::new(
            // Tool names appear in G-code comments; Mach3's reference profile requires ASCII.
            "Fraise meuble - avances a regler",
            ToolKind::EndMill,
            job.setup.cutter,
        ))
        .map_err(|e| e.to_string())?;
    for p in &sheet.placements {
        if face == MachiningFace::Front {
            add_contour(
                &mut project,
                &p.panel.id,
                placed_contour(p, &p.panel.outline, face, job.setup.sheet_size[0]),
            )?;
            if job.dimensions.composition.is_some() {
                project.furniture_requirements.push(CamRequirement {
                    layer: p.panel.id.clone(),
                    depth: p.panel.thickness,
                    pocket: false,
                    inside: false,
                });
            }
        }
        for (i, feature) in p.panel.features.iter().enumerate() {
            let target = if feature.through {
                MachiningFace::Front
            } else {
                feature.face
            };
            if target != face || feature.in_outline {
                continue;
            }
            let layer = format!(
                "{}-J{}-{:?}-{:.3}mm",
                p.panel.id,
                i + 1,
                face,
                feature.depth
            );
            add_contour(
                &mut project,
                &layer,
                placed_contour(p, &feature.contour, face, job.setup.sheet_size[0]),
            )?;
            project.furniture_requirements.push(CamRequirement {
                layer,
                depth: feature.depth,
                pocket: !feature.through,
                inside: true,
            });
        }
    }
    Ok(project)
}

pub fn fabrication_files(
    job: &Furniture,
    options: &ExportOptions,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let layout = place(job)?;
    if ![
        options.native,
        options.cb,
        options.dxf,
        options.svg,
        options.csv,
        options.pdf,
    ]
    .into_iter()
    .any(|v| v)
    {
        return Err("Sélectionnez au moins un format d'export".into());
    }
    let mut files = Vec::new();
    if options.native {
        files.push((
            "meuble.fcm.json".into(),
            Document::Furniture(Box::new(job.clone()))
                .to_json()?
                .into_bytes(),
        ));
    }
    files.push((
        "profil-machine.json".into(),
        job.setup.machine.to_json()?.into_bytes(),
    ));
    files.push((
        "placement.json".into(),
        serde_json::to_vec_pretty(&layout).map_err(|e| e.to_string())?,
    ));
    let machining_note = if job.dimensions.composition.is_some() {
        "Assemblages : consulter operations.json et placement.json pour les faces, profondeurs et repères.\nSi un verso existe : préparer le verso avant la découpe extérieure du recto. Retourner autour de Y ; miroir X = largeur de dalle - X, Y inchangé. Définir le bridage, les pions de retournement et les origines dans les deux montages.\n"
    } else {
        ""
    };
    let manifest=format!("FabLabCAM {} — dossier géométrique\nUnités : mm. Dalles après pré-débit : {} × {}.\nModèle : {} révision {}.\nMachine : {}.\nPost-processeur déclaré : {}.\nAucun G-code dans ce dossier.\n{machining_note}Ouvrir les .cb dans Studio, choisir les opérations et le post-processeur, régler avances, profondeurs, maintien et origine.\nSimuler et analyser le G-code avec profil-machine.json. Un rapport incomplet ne valide pas l'usinage.\nL'occupation correspond à la surface finie des pièces / surface totale de dalle.\n",
        env!("CARGO_PKG_VERSION"),job.setup.sheet_size[0],job.setup.sheet_size[1],job.template.name,job.template.revision,job.setup.machine.name,job.setup.machine.controller.postprocessor);
    files.push(("LIRE-MOI.txt".into(), manifest.into_bytes()));
    for (i, sheet) in layout.sheets.iter().enumerate() {
        let project = project_for_sheet(job, &layout, i, MachiningFace::Front)?;
        let stem = format!("dalle-{:02}", i + 1);
        if options.cb {
            files.push((format!("{stem}.cb"), persist::save(&project).into_bytes()));
        }
        if options.dxf {
            let shapes = project
                .shapes
                .iter()
                .map(|s| (s.layer.clone(), s.entity.clone()))
                .collect::<Vec<_>>();
            files.push((format!("{stem}.dxf"), dxf::write_dxf(&shapes).into_bytes()));
        }
        if options.svg {
            files.push((
                format!("{stem}.svg"),
                sheet_svg(job, sheet, MachiningFace::Front).into_bytes(),
            ));
        }
    }
    if job.dimensions.composition.is_some() {
        let mut operations = Vec::new();
        for (i, sheet) in layout.sheets.iter().enumerate() {
            for face in [MachiningFace::Front, MachiningFace::Back] {
                let project = project_for_sheet(job, &layout, i, face)?;
                if project.shapes.is_empty() {
                    continue;
                }
                operations.push(serde_json::json!({"sheet":i+1,"face":face,"thickness":sheet.placements[0].panel.thickness,"requirements":project.furniture_requirements,"back_transform":"X = sheet_width - X; Y unchanged"}));
                if face == MachiningFace::Back {
                    if options.svg {
                        files.push((
                            format!("dalle-{:02}-verso.svg", i + 1),
                            sheet_svg(job, sheet, face).into_bytes(),
                        ));
                    }
                    if options.dxf {
                        let shapes = project
                            .shapes
                            .iter()
                            .map(|s| (s.layer.clone(), s.entity.clone()))
                            .collect::<Vec<_>>();
                        files.push((
                            format!("dalle-{:02}-verso.dxf", i + 1),
                            dxf::write_dxf(&shapes).into_bytes(),
                        ));
                    }
                }
                if face == MachiningFace::Back && options.cb {
                    files.push((
                        format!("dalle-{:02}-verso.cb", i + 1),
                        persist::save(&project).into_bytes(),
                    ));
                }
            }
        }
        files.push((
            "operations.json".into(),
            serde_json::to_vec_pretty(&operations).map_err(|e| e.to_string())?,
        ));
        files.push((
            "assemblages.json".into(),
            serde_json::to_vec_pretty(&job.dimensions.composition).map_err(|e| e.to_string())?,
        ));
    }
    if options.csv {
        files.push(("liste-de-debit.csv".into(), cut_list(&layout).into_bytes()));
    }
    if options.pdf {
        files.push((
            "notice-montage.pdf".into(),
            super::manual::pdf(job, &layout),
        ));
    }
    Ok(files)
}

pub fn fabrication_zip(job: &Furniture, options: &ExportOptions) -> Result<Vec<u8>, String> {
    let files = fabrication_files(job, options)?;
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    zip.finish()
        .map(|c| c.into_inner())
        .map_err(|e| e.to_string())
}
fn add_contour(project: &mut Project, layer: &str, path: super::Contour) -> Result<(), String> {
    let points = path.iter().map(|v| (v[0], v[1], v[2])).collect::<Vec<_>>();
    project
        .add_shape(
            layer,
            fablabcam_model::Entity::Polyline(fablabcam_geom::pline_from(&points, true)),
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn sheet_svg(job: &Furniture, sheet: &super::Sheet, face: MachiningFace) -> String {
    let [w, h] = job.setup.sheet_size;
    let mut out = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}mm" height="{h}mm" viewBox="0 0 {w} {h}"><title>FabLabCAM — dalle, cotes en mm</title><rect width="{w}" height="{h}" fill="#f7f3e9" stroke="#888"/>"##
    );
    for p in &sheet.placements {
        let mut paths = Vec::new();
        if face == MachiningFace::Front {
            paths.push((&p.panel.outline, "#315e48"));
        }
        paths.extend(
            p.panel
                .features
                .iter()
                .filter(|f| {
                    !f.in_outline
                        && if f.through {
                            face == MachiningFace::Front
                        } else {
                            face == f.face
                        }
                })
                .map(|f| (&f.contour, if f.through { "#c64747" } else { "#a66b00" })),
        );
        for (path, color) in paths {
            let path = placed_contour(p, path, face, w);
            let points = path.iter().map(|v| (v[0], v[1], v[2])).collect::<Vec<_>>();
            let poly = fablabcam_geom::pline_from(&points, true);
            let mut data = format!("M {} {}", path[0][0], h - path[0][1]);
            for seg in fablabcam_geom::segments(&poly) {
                if let Some(c) = seg.arc_center() {
                    let r = (seg.from.0 - c.0).hypot(seg.from.1 - c.1);
                    data += &format!(
                        " A {r} {r} 0 {} {} {} {}",
                        u8::from(seg.sweep().abs() > std::f64::consts::PI),
                        u8::from(seg.sweep() < 0.0),
                        seg.to.0,
                        h - seg.to.1
                    );
                } else {
                    data += &format!(" L {} {}", seg.to.0, h - seg.to.1);
                }
            }
            out += &format!(r#"<path d="{data} Z" fill="none" stroke="{color}"/>"#);
        }
        out += &format!(
            r#"<text x="{}" y="{}" font-size="12">{}</text>"#,
            p.x + 5.0,
            h - p.y - 10.0,
            p.panel.id
        );
    }
    out + "</svg>\n"
}
fn cut_list(layout: &Layout) -> String {
    let mut out =
        "repere;piece;quantite;largeur_mm;hauteur_mm;epaisseur_mm;dalle;x_mm;y_mm;rotation_90\n"
            .to_string();
    for (i, s) in layout.sheets.iter().enumerate() {
        for p in &s.placements {
            out += &format!(
                "{};{};1;{};{};{};{};{};{};{}\n",
                p.panel.id,
                format_args!("\"{}\"", p.panel.name.replace('"', "\"\"")),
                p.panel.width,
                p.panel.height,
                p.panel.thickness,
                i + 1,
                p.x,
                p.y,
                p.rotated
            );
        }
    }
    out
}
