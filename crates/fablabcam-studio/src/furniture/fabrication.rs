use super::{place, Document, Furniture, Layout};
use crate::{dxf, persist, shape, Project, Stock, Tool, ToolKind};
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
    project_for_sheet(job, &layout, sheet)
}
fn project_for_sheet(job: &Furniture, layout: &Layout, sheet: usize) -> Result<Project, String> {
    let sheet = layout.sheets.get(sheet).ok_or("Dalle inconnue")?;
    let mut project = Project::new();
    project.machine_profile = Some(job.setup.machine.clone());
    project
        .set_stock(Some(Stock {
            min: [0.0, 0.0],
            max: job.setup.sheet_size,
            top: 0.0,
            bottom: -job.dimensions.thickness,
        }))
        .map_err(|e| e.to_string())?;
    project
        .add_tool(Tool::new(
            "Fraise meuble — régler les avances",
            ToolKind::EndMill,
            job.setup.cutter,
        ))
        .map_err(|e| e.to_string())?;
    for p in &sheet.placements {
        let entity = shape::rectangle((p.x, p.y), (p.x + p.width, p.y + p.height))
            .map_err(|e| e.to_string())?;
        project
            .add_shape(&p.panel.id, entity)
            .map_err(|e| e.to_string())?;
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
    let manifest=format!("FabLabCAM {} — dossier géométrique\nUnités : mm. Dalles après pré-débit : {} × {}.\nModèle : {} révision {}.\nMachine : {}.\nPost-processeur déclaré : {}.\nAucun G-code dans ce dossier.\nOuvrir les .cb dans Studio, choisir les opérations et le post-processeur, régler avances, profondeurs, maintien et origine.\nSimuler et analyser le G-code avec profil-machine.json. Un rapport incomplet ne valide pas l'usinage.\nL'occupation correspond à la surface finie des pièces / surface totale de dalle.\n",
        env!("CARGO_PKG_VERSION"),job.setup.sheet_size[0],job.setup.sheet_size[1],job.template.name,job.template.revision,job.setup.machine.name,job.setup.machine.controller.postprocessor);
    files.push(("LIRE-MOI.txt".into(), manifest.into_bytes()));
    for (i, sheet) in layout.sheets.iter().enumerate() {
        let project = project_for_sheet(job, &layout, i)?;
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
            files.push((format!("{stem}.svg"), sheet_svg(job, sheet).into_bytes()));
        }
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
fn sheet_svg(job: &Furniture, sheet: &super::Sheet) -> String {
    let [w, h] = job.setup.sheet_size;
    let mut out = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}mm" height="{h}mm" viewBox="0 0 {w} {h}"><title>FabLabCAM — dalle, cotes en mm</title><rect width="{w}" height="{h}" fill="#f7f3e9" stroke="#888"/>"##
    );
    for p in &sheet.placements {
        // Geometry is expressed in CNC coordinates (Y up). Labels remain upright.
        let y = h - p.y - p.height;
        out += &format!(
            r##"<rect x="{}" y="{y}" width="{}" height="{}" fill="#d9e8dc" stroke="#315e48"/><text x="{}" y="{}" font-size="20" text-anchor="middle">{} — {} × {} mm</text>"##,
            p.x,
            p.width,
            p.height,
            p.x + p.width / 2.0,
            y + p.height / 2.0,
            p.panel.id,
            p.panel.width,
            p.panel.height
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
                p.panel.name,
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
