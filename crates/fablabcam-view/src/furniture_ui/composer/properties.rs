use super::*;
impl ComposerUi {
    pub(super) fn properties(
        &mut self,
        ui: &mut egui::Ui,
        c: &mut Composition,
        scene: &Structure,
        dimensions: &Dimensions,
        cutter: f64,
    ) {
        let t = dimensions.thickness;
        let mut paste = None;
        let mut copy = false;
        ui.strong("Propriétés et actions");
        if let Some(cell) = c.root.find_mut(&self.selected) {
            ui.text_edit_singleline(&mut cell.name);
            let Some(bounds) = scene.cells.iter().find(|x| x.id == cell.id) else {
                ui.label("Case à recalculer ; corrigez ses dimensions ou retirez son contenu.");
                if let Some(bin) = &mut cell.bin {
                    bin_fields(ui, bin);
                }
                if ui.button("Retirer le contenu…").clicked() {
                    self.delete = Some(cell.id.clone());
                }
                return;
            };
            ui.label(format!(
                "Utile : {:.1} × {:.1} × {:.1} mm",
                bounds.size[0], bounds.size[1], bounds.size[2]
            ));
            if cell.division.is_none() && cell.bin.is_none() {
                ui.add(
                    egui::DragValue::new(&mut self.count)
                        .clamp_range(2..=32)
                        .suffix(" cases"),
                );
                for (label, axis, count) in [
                    ("Ajouter une tablette", Axis::Horizontal, 2),
                    ("Ajouter une séparation", Axis::Vertical, 2),
                    ("Diviser horizontalement", Axis::Horizontal, self.count),
                    ("Diviser verticalement", Axis::Vertical, self.count),
                ] {
                    if ui.button(label).clicked() {
                        if let Err(e) = cell.divide(axis, count) {
                            self.message = e;
                        }
                    }
                }
                if ui.button("Ajouter un bac").clicked() {
                    cell.bin = Some(Bin {
                        depth: (bounds.size[1] - 2.0).min(250.0),
                        ..Bin::default()
                    });
                }
                if self.clipboard.is_some()
                    && ui.button("Coller une copie dans cette case").clicked()
                {
                    paste = Some(cell.id.clone());
                }
            }
            if let Some(division) = &mut cell.division {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            division.mode == PositionMode::Equal,
                            "Répartition régulière",
                        )
                        .clicked()
                    {
                        division.mode = PositionMode::Equal;
                    }
                    if ui
                        .selectable_label(division.mode == PositionMode::Fixed, "Positions fixes")
                        .clicked()
                    {
                        division.fix_positions(bounds.size[division.axis.coordinate()], t);
                    }
                });
            }
            if ui.button("Copier le contenu").clicked() {
                copy = true;
                self.message =
                    "Sélectionnez une case vide puis collez une copie indépendante.".into();
            }
            if let Some(bin) = &mut cell.bin {
                bin_fields(ui, bin);
            }
            let repeat = cell.bin.clone().filter(|_| {
                ui.button("Répéter ce bac dans les cases compatibles")
                    .clicked()
            });
            if ui.button("Supprimer le contenu…").clicked() {
                self.delete = Some(cell.id.clone());
            }
            let source_id = cell.id.clone();
            if copy || repeat.is_some() {
                match CellClipboard::capture(dimensions, c, &source_id) {
                    Ok(clipboard) => self.clipboard = Some(clipboard),
                    Err(e) => {
                        self.message = e;
                        return;
                    }
                }
            }
            if let Some(target) = paste {
                if let Some(clipboard) = &self.clipboard {
                    if let Err(e) = clipboard.paste(dimensions, c, &target, cutter) {
                        self.message = e;
                    }
                }
            }
            if repeat.is_some() {
                let mut count = 0;
                if let Some(clipboard) = &self.clipboard {
                    for b in scene.cells.iter().filter(|b| b.leaf) {
                        if clipboard.paste(dimensions, c, &b.id, cutter).is_ok() {
                            count += 1;
                        }
                    }
                }
                self.message = format!("{count} bacs ajoutés avec leurs assemblages ; les cases occupées ou incompatibles sont conservées.");
            }
            if source_id == c.root.id {
                ui.separator();
                ui.strong("Orientation des angles du caisson");
                ui.checkbox(&mut c.top_covers_sides, "Dessus continu sur les côtés");
                ui.checkbox(&mut c.bottom_covers_sides, "Dessous continu sous les côtés");
                ui.weak("Le panneau continu reçoit les mortaises. Les dimensions extérieures sont conservées.");
            }
            return;
        }
        if let Some(parent) = parent(&c.root, &self.selected) {
            let parent_id = parent.id.clone();
            let bounds = scene.cells.iter().find(|b| b.id == parent_id).unwrap();
            let d = c
                .root
                .find_mut(&parent_id)
                .unwrap()
                .division
                .as_mut()
                .unwrap();
            let index = d
                .dividers
                .iter()
                .position(|p| p.id == self.selected)
                .unwrap();
            ui.text_edit_singleline(&mut d.dividers[index].name);
            optional_number(ui, "Épaisseur propre", &mut d.dividers[index].thickness, t);
            optional_number(
                ui,
                "Profondeur propre",
                &mut d.dividers[index].depth,
                bounds.size[1],
            );
            if let Some(p) = scene.panels.iter().find(|p| p.id == self.selected) {
                ui.label(format!(
                    "Portée entre les limites de la case : {:.1} × {:.1} mm",
                    p.width, p.height
                ));
            }
            if d.mode == PositionMode::Equal && ui.button("Modifier sa position").clicked() {
                d.fix_positions(bounds.size[d.axis.coordinate()], t);
            }
            if d.mode == PositionMode::Fixed {
                number(ui, "Position depuis la case", &mut d.dividers[index].offset);
            }
            if ui.button("Dupliquer cette séparation").clicked() {
                if let Err(e) = d.duplicate_divider(index, bounds.size[d.axis.coordinate()], t) {
                    self.message = e;
                }
            }
            if ui.button("Supprimer cette séparation…").clicked() {
                self.delete = Some(self.selected.clone());
            }
            return;
        }
        if let Some(p) = scene.panels.iter().find(|p| p.id == self.selected) {
            if p.assembly != "frame" {
                ui.label(format!(
                    "{} · {} × {} × {} mm",
                    p.name, p.width, p.height, p.thickness
                ));
                if let Some(bin) = find_bin(&mut c.root, &p.assembly) {
                    bin_fields(ui, bin);
                }
                return;
            }
            let name = c
                .panel_names
                .entry(p.id.clone())
                .or_insert_with(|| p.name.clone());
            ui.text_edit_singleline(name);
            ui.label(format!("{} × {} × {} mm", p.width, p.height, p.thickness));
            let mut value = c.panel_thickness.get(&p.id).copied();
            optional_number(ui, "Épaisseur propre", &mut value, t);
            match value {
                Some(v) => {
                    c.panel_thickness.insert(p.id.clone(), v);
                }
                None => {
                    c.panel_thickness.remove(&p.id);
                }
            }
            if p.id == "SUP" {
                ui.checkbox(&mut c.top_covers_sides, "Dessus continu sur les côtés");
            }
            if p.id == "INF" {
                ui.checkbox(&mut c.bottom_covers_sides, "Dessous continu sous les côtés");
            }
            ui.weak("Largeur, hauteur et profondeur suivent les dimensions extérieures : ouvrez « Dimensions » pour les modifier. Les cases utiles sont recalculées selon chaque épaisseur.");
        }
    }
}
pub(super) fn number(ui: &mut egui::Ui, label: &str, value: &mut f64) {
    ui.label(label);
    ui.add(egui::DragValue::new(value).speed(0.5).suffix(" mm"));
}
pub(super) fn bin_fields(ui: &mut egui::Ui, b: &mut Bin) {
    ui.separator();
    ui.strong("Bac amovible");
    ui.text_edit_singleline(&mut b.name);
    ui.checkbox(&mut b.bottom_covers_sides, "Fond continu sous les parois");
    for (label, value) in [
        ("Profondeur", &mut b.depth),
        ("Épaisseur", &mut b.thickness),
        ("Jeu de chaque côté", &mut b.side_gap),
        ("Jeu en haut", &mut b.top_gap),
        ("Jeu au dos", &mut b.back_gap),
    ] {
        number(ui, label, value);
    }
}
pub(super) fn parent<'a>(c: &'a Cell, id: &str) -> Option<&'a Cell> {
    let d = c.division.as_ref()?;
    if d.dividers.iter().any(|p| p.id == id) {
        return Some(c);
    }
    d.children.iter().find_map(|c| parent(c, id))
}
pub(super) fn remove_divider(c: &mut Cell, id: &str) {
    let Some(p) = parent(c, id) else {
        return;
    };
    let pid = p.id.clone();
    let d = c.find_mut(&pid).unwrap().division.as_mut().unwrap();
    let i = d.dividers.iter().position(|p| p.id == id).unwrap();
    let _ = d.remove_divider(i);
}

pub(super) fn affected(c: &Cell, id: &str) -> usize {
    if let Some(cell) = c.find(id) {
        return cell.descendants();
    }
    let Some(p) = parent(c, id) else {
        return 0;
    };
    let d = p.division.as_ref().unwrap();
    let i = d.dividers.iter().position(|p| p.id == id).unwrap();
    1 + d.children[i].descendants() + d.children[i + 1].descendants() + 1
}

pub(super) fn optional_number(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<f64>,
    default: f64,
) {
    let mut custom = value.is_some();
    if ui.checkbox(&mut custom, label).changed() {
        *value = custom.then_some(default);
    }
    if let Some(v) = value {
        number(ui, label, v);
    }
}

fn find_bin<'a>(cell: &'a mut Cell, id: &str) -> Option<&'a mut Bin> {
    if let Some(b) = &mut cell.bin {
        return (b.id == id).then_some(b);
    }
    cell.division
        .as_mut()?
        .children
        .iter_mut()
        .find_map(|c| find_bin(c, id))
}
