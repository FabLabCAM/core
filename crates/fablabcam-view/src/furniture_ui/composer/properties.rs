use super::*;
impl ComposerUi {
    pub(super) fn properties(
        &mut self,
        ui: &mut egui::Ui,
        c: &mut Composition,
        scene: &Structure,
        t: f64,
    ) {
        ui.strong("Propriétés et actions");
        if let Some(cell) = c.root.find_mut(&self.selected) {
            ui.text_edit_singleline(&mut cell.name);
            let bounds = scene.cells.iter().find(|x| x.id == cell.id).unwrap();
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
                if let Some(source) = &self.clipboard {
                    if ui.button("Coller une copie dans cette case").clicked() {
                        cell.copy_contents_from(source);
                    }
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
                self.clipboard = Some(cell.clone());
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
            if let Some(bin) = repeat {
                let mut count = 0;
                for b in scene.cells.iter().filter(|b| b.leaf) {
                    if b.size[0] > 2.0 * (bin.thickness + bin.side_gap)
                        && b.size[2] > bin.top_gap + bin.thickness
                        && b.size[1] >= bin.depth + bin.back_gap
                    {
                        if let Some(target) = c.root.find_mut(&b.id).filter(|c| c.bin.is_none()) {
                            let mut next = bin.clone();
                            next.id = new_id();
                            target.bin = Some(next);
                            count += 1;
                        }
                    }
                }
                self.message = format!(
                    "{count} bacs ajoutés ; les cases occupées ou incompatibles sont conservées."
                );
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
            let name = c
                .panel_names
                .entry(p.id.clone())
                .or_insert_with(|| p.name.clone());
            ui.text_edit_singleline(name);
            ui.label(format!("{} × {} × {} mm", p.width, p.height, p.thickness));
            ui.weak("Les panneaux du caisson suivent les dimensions extérieures. Le dos se règle dans les dimensions.");
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
fn parent<'a>(c: &'a Cell, id: &str) -> Option<&'a Cell> {
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
