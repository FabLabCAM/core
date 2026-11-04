use super::{Dimensions, Template};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Panel {
    pub id: String,
    pub name: String,
    pub width: f64,
    pub height: f64,
    pub thickness: f64,
    /// Assembled cuboid: X = width, Y = depth, Z = height, all in millimetres.
    pub origin: [f64; 3],
    pub size: [f64; 3],
    pub normal: usize,
    pub assembly: String,
    pub uv_min: [f64; 2],
    pub outline: super::Contour,
    pub features: Vec<super::MachiningFeature>,
}
pub fn panels(template: &Template, d: &Dimensions) -> Result<Vec<Panel>, String> {
    panels_with_cutter(template, d, 6.0)
}
pub fn panels_with_cutter(
    template: &Template,
    d: &Dimensions,
    cutter: f64,
) -> Result<Vec<Panel>, String> {
    template.check()?;
    template.check_dimensions(d)?;
    let mut parts = super::structure(d)?.panels;
    super::joints::machine(&mut parts, d, cutter)?;
    Ok(parts)
}

pub(super) fn legacy_panels(d: &Dimensions) -> Vec<Panel> {
    let (w, h, t) = (d.width, d.height, d.thickness);
    let depth = d.depth - if d.back { t } else { 0.0 };
    let mut result = Vec::new();
    let mut add = |id: &str, name: &str, width, height, origin, size| {
        let normal = match id {
            "C1" | "C2" => 0,
            "DOS" => 1,
            _ => 2,
        };
        let _ = (width, height);
        result.push(Panel::rectangular(id, name, origin, size, normal))
    };
    add(
        "C1",
        "Côté gauche",
        h,
        depth,
        [0.0, 0.0, 0.0],
        [t, depth, h],
    );
    add(
        "C2",
        "Côté droit",
        h,
        depth,
        [w - t, 0.0, 0.0],
        [t, depth, h],
    );
    add(
        "SUP",
        "Dessus",
        w - 2.0 * t,
        depth,
        [t, 0.0, h - t],
        [w - 2.0 * t, depth, t],
    );
    add(
        "INF",
        "Dessous",
        w - 2.0 * t,
        depth,
        [t, 0.0, 0.0],
        [w - 2.0 * t, depth, t],
    );
    let gap = (h - (d.shelves + 2) as f64 * t) / (d.shelves + 1) as f64;
    for i in 1..=d.shelves {
        add(
            &format!("T{i:02}"),
            &format!("Tablette {i}"),
            w - 2.0 * t,
            depth,
            [t, 0.0, i as f64 * (t + gap)],
            [w - 2.0 * t, depth, t],
        );
    }
    if d.back {
        add("DOS", "Dos rapporté", w, h, [0.0, depth, 0.0], [w, t, h]);
    }
    result
}
