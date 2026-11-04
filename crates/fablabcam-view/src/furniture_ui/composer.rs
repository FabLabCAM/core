mod drawing;
mod joints;
mod properties;
use eframe::egui;
use fablabcam_studio::furniture::*;

pub(super) struct ComposerUi {
    pub(super) selected: String,
    columns: usize,
    rows: usize,
    count: usize,
    clipboard: Option<CellClipboard>,
    delete: Option<String>,
    joint: String,
    show_joints: bool,
    last_scene: Option<Structure>,
    pub message: String,
}
impl Default for ComposerUi {
    fn default() -> Self {
        Self {
            selected: String::new(),
            columns: 5,
            rows: 6,
            count: 2,
            clipboard: None,
            delete: None,
            joint: String::new(),
            show_joints: false,
            last_scene: None,
            message: String::new(),
        }
    }
}
impl ComposerUi {
    pub(super) fn show(&mut self, ui: &mut egui::Ui, doc: &mut Document, cutter: f64) {
        let template = doc.template().clone();
        let d = doc.dimensions_mut();
        if d.composition.is_none() {
            ui.label("Convertir les tablettes actuelles en éléments modifiables. Les dimensions et les repères sont conservés.");
            if ui.button("Activer la composition").clicked() {
                match Composition::from_dimensions(d) {
                    Ok(c) => d.composition = Some(Box::new(c)),
                    Err(e) => self.message = e,
                };
            }
            return;
        }
        let mut resolved = d.clone();
        let composition = d.composition.as_mut().unwrap();
        if self.selected.is_empty() {
            self.selected = composition.root.id.clone();
        }
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.show_joints, false, "Structure et cases");
            ui.selectable_value(&mut self.show_joints, true, "Assemblages");
            ui.separator();
            ui.label("Grille");
            ui.add(
                egui::DragValue::new(&mut self.columns)
                    .clamp_range(1..=32)
                    .suffix(" colonnes"),
            );
            ui.add(
                egui::DragValue::new(&mut self.rows)
                    .clamp_range(1..=32)
                    .suffix(" rangées"),
            );
            if ui.button("Créer la grille…").clicked() {
                self.delete = Some("grid".into());
            }
        });
        if let Some(id) = self.delete.clone() {
            let description = if id == "grid" {
                format!(
                    "Remplacer toute la structure par {} × {} cases ?",
                    self.columns, self.rows
                )
            } else {
                format!(
                    "Supprimer cet élément et ses descendants ({}) ?",
                    properties::affected(&composition.root, &id)
                )
            };
            egui::Frame::group(ui.style()).show(ui,|ui| {
                ui.colored_label(egui::Color32::YELLOW,description);
                ui.label("Les liaisons des éléments supprimés seront retirées. Annuler reste disponible.");
                ui.horizontal(|ui| {
                    if ui.button("Confirmer la suppression / remplacement").clicked() {
                        if id=="grid" {
                            match Composition::grid(self.columns,self.rows) {Ok(mut next)=>{next.joint_default=composition.joint_default.clone();**composition=next;self.selected=composition.root.id.clone();},Err(e)=>self.message=e}
                        }else if let Some(c)=composition.root.find_mut(&id) {c.clear();}
                        else {properties::remove_divider(&mut composition.root,&id);}
                        self.delete=None;
                    }
                    if ui.button("Conserver").clicked(){self.delete=None;}
                });
            });
        }
        resolved.composition = Some(composition.clone());
        let result = structure(&resolved);
        let error = result.as_ref().err().cloned();
        let scene = match result {
            Ok(scene) => {
                self.last_scene = Some(scene.clone());
                Some(scene)
            }
            Err(_) => self
                .last_scene
                .clone()
                .filter(|s| s.cells.first().is_some_and(|b| b.id == composition.root.id)),
        };
        // Keep the same controls/focus while typing temporarily invalid numbers.
        // Moving to a different recovery form midway through "200" would lose
        // focus at "2" and prevent users from completing the value.
        match &scene {
            Some(scene) => {
                ui.columns(3, |cols| {
                    cols[0].strong("Éléments du meuble");
                    egui::ScrollArea::vertical()
                        .id_source("composition-tree")
                        .max_height(420.0)
                        .show(&mut cols[0], |ui| {
                            for id in ["C1", "C2", "SUP", "INF", "DOS"] {
                                if let Some(p) = scene.panels.iter().find(|p| p.id == id) {
                                    ui.selectable_value(&mut self.selected, p.id.clone(), &p.name);
                                }
                            }
                            tree(ui, &composition.root, &mut self.selected, 0);
                        });
                    cols[1].strong("Vue de face — cliquez une case");
                    let pair = connections(&scene.panels)
                        .into_iter()
                        .find(|l| self.show_joints && l.id == self.joint);
                    drawing::face(
                        &mut cols[1],
                        scene,
                        &mut self.selected,
                        d.width,
                        d.height,
                        pair.as_ref(),
                    );
                    if self.show_joints {
                        self.joints(&mut cols[2], composition, scene, cutter);
                    } else {
                        self.properties(&mut cols[2], composition, scene, &resolved, cutter);
                    }
                });
            }
            None => self.recovery(ui, composition, &resolved),
        }
        if let Some(e) = error {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                format!("{e} — aperçu précédent conservé pendant la saisie."),
            );
        }
        if !self.message.is_empty() {
            ui.colored_label(egui::Color32::LIGHT_BLUE, &self.message);
        }
        match panels_with_cutter(&template, d, cutter) {
            Ok(parts) => {
                ui.weak(format!(
                    "{} pièces · toutes les cotes sont en mm · fraisage : Ø {cutter}",
                    parts.len()
                ));
            }
            Err(e) => {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    format!("Fabrication à corriger : {e}"),
                );
            }
        }
    }
    fn recovery(&mut self, ui: &mut egui::Ui, c: &mut Composition, dimensions: &Dimensions) {
        ui.label("Corrigez les dimensions globales ou les propriétés ci-dessous.");
        tree(ui, &c.root, &mut self.selected, 0);
        if let Some(parent) = properties::parent(&c.root, &self.selected) {
            let id = parent.id.clone();
            let division = c.root.find_mut(&id).unwrap().division.as_mut().unwrap();
            let p = division
                .dividers
                .iter_mut()
                .find(|p| p.id == self.selected)
                .unwrap();
            properties::number(ui, "Position", &mut p.offset);
            properties::optional_number(
                ui,
                "Épaisseur propre",
                &mut p.thickness,
                dimensions.thickness,
            );
            properties::optional_number(ui, "Profondeur propre", &mut p.depth, dimensions.depth);
            ui.selectable_value(
                &mut division.mode,
                PositionMode::Equal,
                "Répartition régulière",
            );
        }
        if let Some(t) = c.panel_thickness.get_mut(&self.selected) {
            properties::number(ui, "Épaisseur du panneau", t);
            if ui.button("Revenir à l’épaisseur globale").clicked() {
                c.panel_thickness.remove(&self.selected);
            }
        }
        if let Some(cell) = c.root.find_mut(&self.selected) {
            if let Some(d) = &mut cell.division {
                for divider in &mut d.dividers {
                    properties::number(ui, &divider.name, &mut divider.offset);
                }
                ui.selectable_value(
                    &mut d.mode,
                    PositionMode::Equal,
                    "Revenir à une répartition régulière",
                );
            }
            if ui.button("Retirer le contenu de cette case…").clicked() {
                self.delete = Some(cell.id.clone());
            }
            if let Some(b) = &mut cell.bin {
                properties::bin_fields(ui, b);
            }
        }
    }
}
fn tree(ui: &mut egui::Ui, c: &Cell, selected: &mut String, depth: usize) {
    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * 10.0);
        ui.selectable_value(
            selected,
            c.id.clone(),
            format!("{}{}", if c.bin.is_some() { "▣ " } else { "□ " }, c.name),
        );
    });
    if let Some(d) = &c.division {
        for (i, child) in d.children.iter().enumerate() {
            tree(ui, child, selected, depth + 1);
            if let Some(p) = d.dividers.get(i) {
                ui.horizontal(|ui| {
                    ui.add_space((depth + 1) as f32 * 10.0);
                    ui.selectable_value(selected, p.id.clone(), &p.name);
                });
            }
        }
    }
}
