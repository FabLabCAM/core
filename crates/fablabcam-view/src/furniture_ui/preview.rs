use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use fablabcam_studio::furniture::{
    panels_with_cutter, path_points, placed_contour, Dimensions, Furniture, MachiningFace, Sheet,
    Template,
};

pub(super) fn assembly(
    ui: &mut egui::Ui,
    t: &Template,
    d: &Dimensions,
    exploded: bool,
    height: f32,
    cutter: f64,
) {
    let Ok(mut parts) = panels_with_cutter(t, d, cutter) else {
        return;
    };
    let (area, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().max(100.0), height),
        Sense::hover(),
    );
    let painter = ui.painter_at(area);
    painter.rect_filled(area, 6.0, Color32::from_rgb(27, 35, 34));
    let iso = |p: [f64; 3]| [(p[0] + 0.48 * p[1]) as f32, (-p[2] - 0.28 * p[1]) as f32];
    let extra = if exploded {
        d.width.max(d.height) * 0.15
    } else {
        0.0
    };
    for p in &mut parts {
        for axis in 0..3 {
            let center = [d.width, d.depth, d.height][axis] / 2.0;
            p.origin[axis] += if p.origin[axis] + p.size[axis] / 2.0 >= center {
                extra
            } else {
                -extra
            };
        }
    }
    parts.sort_by(|a, b| {
        b.origin[1]
            .total_cmp(&a.origin[1])
            .then(a.origin[2].total_cmp(&b.origin[2]))
    });
    let mut bounds = Rect::NOTHING;
    for p in &parts {
        for x in [0.0, 1.0] {
            for y in [0.0, 1.0] {
                for z in [0.0, 1.0] {
                    let v = iso([
                        p.origin[0] + x * p.size[0],
                        p.origin[1] + y * p.size[1],
                        p.origin[2] + z * p.size[2],
                    ]);
                    bounds.extend_with(Pos2::new(v[0], v[1]));
                }
            }
        }
    }
    let scale = ((area.width() - 36.0) / bounds.width()).min((height - 30.0) / bounds.height());
    let screen = |p: [f64; 3]| {
        let v = iso(p);
        area.center() + Vec2::new(v[0] - bounds.center().x, v[1] - bounds.center().y) * scale
    };
    for p in &parts {
        let o = p.origin;
        let s = p.size;
        let vertices = [
            [o[0], o[1], o[2]],
            [o[0] + s[0], o[1], o[2]],
            [o[0] + s[0], o[1], o[2] + s[2]],
            [o[0], o[1], o[2] + s[2]],
            [o[0], o[1] + s[1], o[2]],
            [o[0] + s[0], o[1] + s[1], o[2]],
            [o[0] + s[0], o[1] + s[1], o[2] + s[2]],
            [o[0], o[1] + s[1], o[2] + s[2]],
        ];
        for (face, color) in [
            ([1, 5, 6, 2], Color32::from_rgb(159, 130, 82)),
            ([3, 2, 6, 7], Color32::from_rgb(218, 193, 145)),
            ([0, 1, 2, 3], Color32::from_rgb(188, 158, 108)),
        ] {
            painter.add(egui::Shape::convex_polygon(
                face.into_iter().map(|i| screen(vertices[i])).collect(),
                color,
                Stroke::new(1.0_f32, Color32::from_rgb(80, 69, 48)),
            ));
        }
    }
    // Machining contours are the same curves exported to CAM. Overlay both faces
    // in the exploded view to expose pockets rather than hiding them in a cuboid.
    for p in &parts {
        let axes = p.axes();
        for (path, face, color) in
            std::iter::once((&p.outline, MachiningFace::Front, Color32::LIGHT_BLUE)).chain(
                p.features.iter().map(|f| {
                    (
                        &f.contour,
                        f.face,
                        if f.through {
                            Color32::LIGHT_RED
                        } else {
                            Color32::YELLOW
                        },
                    )
                }),
            )
        {
            let points = path_points(path)
                .into_iter()
                .map(|v| {
                    let mut q = p.origin;
                    q[axes[0]] += v[0];
                    q[axes[1]] += v[1];
                    if face == MachiningFace::Back {
                        q[p.normal] += p.thickness;
                    }
                    screen(q)
                })
                .collect();
            painter.add(egui::Shape::line(points, Stroke::new(1.0_f32, color)));
        }
    }
    // Labels are overlaid after all faces, so a later panel cannot cover a repère.
    if height > 180.0 {
        for p in &parts {
            let o = p.origin;
            let s = p.size;
            let at = screen([o[0] + s[0] / 2.0, o[1], o[2] + s[2] / 2.0]);
            painter.text(
                at,
                egui::Align2::CENTER_CENTER,
                &p.name,
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
        }
    }
}
pub(super) fn sheet(ui: &mut egui::Ui, f: &Furniture, s: &Sheet) {
    let [w, h] = f.setup.sheet_size;
    let width = ui.available_width().max(100.0);
    let height = (width * h as f32 / w as f32).clamp(120.0, 320.0);
    let (area, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let scale = ((width - 10.0) / w as f32).min((height - 10.0) / h as f32);
    let origin = area.left_bottom() + Vec2::new(5.0, -5.0);
    let map = |x: f64, y: f64| origin + Vec2::new(x as f32 * scale, -y as f32 * scale);
    let painter = ui.painter_at(area);
    painter.rect_filled(
        Rect::from_two_pos(map(0.0, 0.0), map(w, h)),
        2.0,
        Color32::from_rgb(207, 190, 153),
    );
    let m = f.setup.margin;
    painter.rect_stroke(
        Rect::from_two_pos(map(m, m), map(w - m, h - m)),
        0.0,
        Stroke::new(1.0_f32, Color32::DARK_GRAY),
    );
    for p in &s.placements {
        let r = Rect::from_two_pos(map(p.x, p.y), map(p.x + p.width, p.y + p.height));
        painter.rect_filled(r, 2.0, Color32::from_rgb(61, 111, 89));
        painter.rect_stroke(r, 2.0, Stroke::new(1.0_f32, Color32::WHITE));
        for (path, color) in std::iter::once((&p.panel.outline, Color32::WHITE)).chain(
            p.panel.features.iter().map(|f| {
                (
                    &f.contour,
                    if f.through {
                        Color32::LIGHT_RED
                    } else {
                        Color32::YELLOW
                    },
                )
            }),
        ) {
            let points = path_points(&placed_contour(p, path, MachiningFace::Front, w))
                .into_iter()
                .map(|v| map(v[0], v[1]))
                .collect();
            painter.add(egui::Shape::line(points, Stroke::new(1.0_f32, color)));
        }
        painter.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            format!("{}\n{} × {}", p.panel.name, p.panel.width, p.panel.height),
            egui::FontId::proportional(12.0),
            Color32::WHITE,
        );
    }
}
