use super::*;
pub(super) fn face(
    ui: &mut egui::Ui,
    scene: &Structure,
    selected: &mut String,
    width: f64,
    height: f64,
) {
    let w = ui.available_width().max(80.0);
    let h = (w * height as f32 / width as f32).clamp(160.0, 300.0);
    let (area, response) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    let scale = ((w - 10.0) / width as f32).min((h - 10.0) / height as f32);
    let map = |x: f64, z: f64| {
        area.left_bottom() + egui::vec2(5.0 + x as f32 * scale, -5.0 - z as f32 * scale)
    };
    let rect = |o: [f64; 3], s: [f64; 3]| {
        egui::Rect::from_two_pos(map(o[0], o[2]), map(o[0] + s[0], o[2] + s[2]))
    };
    let painter = ui.painter_at(area);
    for c in scene.cells.iter().filter(|c| c.leaf) {
        let r = rect(c.origin, c.size);
        painter.rect_filled(
            r,
            0.0,
            if c.id == *selected {
                egui::Color32::from_rgb(32, 110, 124)
            } else {
                egui::Color32::from_gray(40)
            },
        );
        if response.clicked()
            && response
                .interact_pointer_pos()
                .is_some_and(|p| r.contains(p))
        {
            *selected = c.id.clone();
        }
        if r.width() > 45.0 && r.height() > 30.0 {
            painter.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                format!("{:.1} × {:.1}", c.size[0], c.size[2]),
                egui::FontId::proportional(10.0),
                egui::Color32::WHITE,
            );
        }
    }
    for p in scene.panels.iter().filter(|p| p.normal != 1) {
        let r = rect(p.origin, p.size);
        painter.rect_filled(
            r,
            0.0,
            if p.id == *selected {
                egui::Color32::LIGHT_BLUE
            } else {
                egui::Color32::from_rgb(189, 157, 109)
            },
        );
        if response.clicked()
            && response
                .interact_pointer_pos()
                .is_some_and(|q| r.contains(q))
        {
            *selected = p.id.clone();
        }
    }
}
