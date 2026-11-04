use super::{Action, Workspace};
use crate::furniture::{self, Axis, Bin, Composition, Document, Furniture, Template};

pub(super) fn apply(w: &mut Workspace, action: &Action) -> Result<String, String> {
    if let Action::CreateCabinet { name, width, depth, height, thickness, shelves, back } = action {
        if w.furniture.is_some() {
            return Err("Un meuble est déjà ouvert : créez un nouveau document avant de le remplacer".into());
        }
        let mut f = Furniture::from_template(Template::cabinet());
        f.name = name.clone();
        f.dimensions.width = *width;
        f.dimensions.depth = *depth;
        f.dimensions.height = *height;
        f.dimensions.thickness = *thickness;
        f.dimensions.shelves = *shelves;
        f.dimensions.back = *back;
        f.dimensions.composition = Some(Box::new(Composition::from_dimensions(&f.dimensions)?));
        f.check()?;
        w.furniture = Some(Document::Furniture(Box::new(f)));
        return summary(w);
    }
    let doc = w.furniture.as_mut().ok_or("Aucun meuble ouvert")?;
    let before = doc.clone();
    match action {
        Action::ResizeFurniture { width, depth, height } => {
            let d = doc.dimensions_mut();
            d.width = *width;
            d.depth = *depth;
            d.height = *height;
        }
        Action::DivideCell { cell_id, vertical, count } => {
            let c = composition(doc)?;
            let cell = if cell_id == "root" { Some(&mut c.root) } else { c.root.find_mut(cell_id) }.ok_or("Case absente")?;
            if cell.division.is_some() || cell.bin.is_some() {
                return Err("La division remplacerait un contenu existant ; choisissez une case vide".into());
            }
            cell.divide(if *vertical { Axis::Vertical } else { Axis::Horizontal }, *count)?;
        }
        Action::AddBin { cell_id, depth, thickness, side_gap, top_gap, back_gap } => {
            let c = composition(doc)?;
            let cell = if cell_id == "root" { Some(&mut c.root) } else { c.root.find_mut(cell_id) }.ok_or("Case absente")?;
            if cell.division.is_some() || cell.bin.is_some() {
                return Err("Choisissez une case vide pour le bac".into());
            }
            cell.bin = Some(Bin {
                depth: *depth,
                thickness: *thickness,
                side_gap: *side_gap,
                top_gap: *top_gap,
                back_gap: *back_gap,
                ..Bin::default()
            });
        }
        Action::SetJoint { connection_id, settings } => {
            settings.check()?;
            if !settings.kind.supported() {
                return Err("Assemblage non pris en charge par la fabrication".into());
            }
            let links = furniture::connections(&furniture::structure(doc.dimensions())?.panels);
            if !links.iter().any(|c| &c.id == connection_id) {
                return Err("Liaison absente du meuble".into());
            }
            composition(doc)?.joints.insert(connection_id.clone(), settings.clone());
        }
        _ => return Err("Action de meuble non prise en charge".into()),
    }
    doc.reconcile_edit(&before);
    doc.check()?;
    furniture::structure(doc.dimensions())?;
    summary(w)
}

fn composition(doc: &mut Document) -> Result<&mut Composition, String> {
    let d = doc.dimensions_mut();
    if d.composition.is_none() {
        d.composition = Some(Box::new(Composition::from_dimensions(d)?));
    }
    Ok(d.composition.as_deref_mut().expect("composition initialized"))
}

fn summary(w: &Workspace) -> Result<String, String> {
    let doc = w.furniture.as_ref().ok_or("Meuble absent")?;
    let panels = furniture::panels(doc.template(), doc.dimensions())?;
    Ok(format!("{} : {} panneau(x), dimensions et composition vérifiées ; profil machine à qualifier", doc.name(), panels.len()))
}
