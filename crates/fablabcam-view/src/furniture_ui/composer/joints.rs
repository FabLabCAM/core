use super::*;
impl ComposerUi {
    pub(super) fn joints(
        &mut self,
        ui: &mut egui::Ui,
        c: &mut Composition,
        scene: &Structure,
        cutter: f64,
    ) {
        ui.strong("Assemblages");
        let links = connections(&scene.panels);
        egui::ComboBox::from_id_source("joint-selection")
            .selected_text(if self.joint.is_empty() {
                "Réglage du meuble"
            } else {
                "Liaison particulière"
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.joint, String::new(), "Réglage du meuble");
                for l in &links {
                    let name = |id: &str| {
                        scene
                            .panels
                            .iter()
                            .find(|p| p.id == id)
                            .map_or(id.to_owned(), |p| p.name.clone())
                    };
                    ui.selectable_value(
                        &mut self.joint,
                        l.id.clone(),
                        format!("{} ↔ {}", name(&l.male), name(&l.female)),
                    );
                }
            });
        if !self.joint.is_empty() && !links.iter().any(|l| l.id == self.joint) {
            self.joint.clear();
        }
        if self.joint.is_empty() {
            settings(ui, &mut c.joint_default);
        } else {
            let link = links.iter().find(|l| l.id == self.joint).unwrap();
            ui.label(format!("{} ↔ {}", link.male, link.female));
            ui.label(format!("Face de la mortaise : {:?}", link.female_face));
            let overridden = c.joints.contains_key(&self.joint);
            if !overridden && ui.button("Personnaliser cette liaison").clicked() {
                c.joints.insert(self.joint.clone(), c.joint_default.clone());
            }
            if let Some(s) = c.joints.get_mut(&self.joint) {
                settings(ui, s);
            } else {
                ui.label(format!("Hérite : {}", c.joint_default.kind.label()));
            }
            if overridden && ui.button("Revenir au réglage du meuble").clicked() {
                c.joints.remove(&self.joint);
            }
            if ui.button("Appliquer aux liaisons de cet élément").clicked() {
                let value = c
                    .joints
                    .get(&self.joint)
                    .unwrap_or(&c.joint_default)
                    .clone();
                for target in links
                    .iter()
                    .filter(|x| x.male == link.male || x.female == link.male)
                {
                    c.joints.insert(target.id.clone(), value.clone());
                }
            }
            if let Ok(parts) = panels_for_preview(scene, c, cutter) {
                ui.label("Pièces complémentaires à plat");
                for id in [&link.male, &link.female] {
                    if let Some(p) = parts.iter().find(|p| &p.id == id) {
                        flat(ui, p);
                    }
                }
            }
        }
        ui.weak("Les dimensions restent paramétriques. Les opérations FAO et les retournements sont à préparer.");
    }
}
fn panels_for_preview(
    scene: &Structure,
    c: &Composition,
    cutter: f64,
) -> Result<Vec<Panel>, String> {
    machining_preview(scene.panels.clone(), c, cutter)
}
fn settings(ui: &mut egui::Ui, s: &mut JointSettings) {
    egui::ComboBox::from_id_source("joint-kind")
        .selected_text(s.kind.label())
        .show_ui(ui, |ui| {
            for kind in JointKind::ALL {
                ui.add_enabled_ui(kind.supported(), |ui| {
                    ui.selectable_value(
                        &mut s.kind,
                        kind,
                        format!(
                            "{}{}",
                            kind.label(),
                            if kind.supported() {
                                ""
                            } else {
                                " — non encore pris en charge"
                            }
                        ),
                    );
                });
            }
        });
    vignette(ui, s.kind);
    if s.kind == JointKind::Butt {
        ui.weak("Chants droits ; fixation par collage/vissage à définir.");
        return;
    }
    ui.weak(match s.kind {
        JointKind::Tabs => "Languettes et mortaises traversantes complémentaires.",
        JointKind::Box => "Dents rectangulaires pour une liaison au bord.",
        JointKind::Rabbet => "Rainure ouverte au bord du panneau femelle.",
        JointKind::HalfLap => "Mi-bois d’angle ; poche sur la face arrière du tenon.",
        JointKind::TongueGroove => "Languette réduite ; usinage sur les deux faces.",
        _ => "Engagement d’un panneau dans son panneau complémentaire.",
    });
    properties::number(ui, "Jeu TOTAL de la mortaise", &mut s.clearance);
    if matches!(
        s.kind,
        JointKind::Tabs | JointKind::Box | JointKind::MortiseTenon
    ) {
        properties::number(ui, "Largeur d’un tenon", &mut s.width);
        ui.add(
            egui::DragValue::new(&mut s.count)
                .clamp_range(1..=100)
                .suffix(" tenons"),
        );
        ui.checkbox(&mut s.blind, "Mortaise borgne");
    }
    if s.blind
        || matches!(
            s.kind,
            JointKind::Dado | JointKind::Rabbet | JointKind::HalfLap | JointKind::TongueGroove
        )
    {
        properties::number(ui, "Profondeur d’engagement", &mut s.engagement);
    }
    egui::ComboBox::from_id_source("joint-relief")
        .selected_text(format!("Angles : {:?}", s.relief))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut s.relief, CornerRelief::None, "Sans dégagement");
            ui.selectable_value(&mut s.relief, CornerRelief::Dogbone, "Dogbone");
            ui.selectable_value(&mut s.relief, CornerRelief::Tbone, "T-bone");
        });
}
fn vignette(ui: &mut egui::Ui, kind: JointKind) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(150.0, 50.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(
        egui::Rect::from_min_size(r.min + egui::vec2(8.0, 4.0), egui::vec2(18.0, 42.0)),
        0.0,
        egui::Color32::from_rgb(148, 112, 66),
    );
    p.rect_filled(
        egui::Rect::from_min_size(r.min + egui::vec2(26.0, 20.0), egui::vec2(110.0, 14.0)),
        0.0,
        egui::Color32::from_rgb(214, 181, 124),
    );
    if kind != JointKind::Butt {
        p.rect_filled(
            egui::Rect::from_min_size(r.min + egui::vec2(11.0, 23.0), egui::vec2(20.0, 8.0)),
            0.0,
            egui::Color32::LIGHT_BLUE,
        );
    }
}
pub(super) fn flat(ui: &mut egui::Ui, p: &Panel) {
    ui.label(format!("{} · {:.1} × {:.1} mm", p.name, p.width, p.height));
    let (r, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 100.0),
        egui::Sense::hover(),
    );
    let scale = ((r.width() - 8.0) / p.width as f32).min((r.height() - 8.0) / p.height as f32);
    let map = |v: [f64; 2]| {
        r.left_bottom()
            + egui::vec2(
                4.0 + (v[0] - p.uv_min[0]) as f32 * scale,
                -4.0 - (v[1] - p.uv_min[1]) as f32 * scale,
            )
    };
    ui.painter().add(egui::Shape::line(
        path_points(&p.outline).into_iter().map(map).collect(),
        egui::Stroke::new(2.0_f32, egui::Color32::LIGHT_BLUE),
    ));
    for f in &p.features {
        ui.painter().add(egui::Shape::line(
            path_points(&f.contour).into_iter().map(map).collect(),
            egui::Stroke::new(
                1.0_f32,
                if f.through {
                    egui::Color32::LIGHT_RED
                } else {
                    egui::Color32::YELLOW
                },
            ),
        ));
    }
}
