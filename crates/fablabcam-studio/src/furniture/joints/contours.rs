use super::*;
use fablabcam_geom::{self as geom, Pline, PlineSource};
use fablabcam_model::Entity;
pub(super) fn rectangle(a: [f64; 2], b: [f64; 2]) -> Contour {
    vec![
        [a[0], a[1], 0.0],
        [b[0], a[1], 0.0],
        [b[0], b[1], 0.0],
        [a[0], b[1], 0.0],
    ]
}
pub(super) fn pline(c: &Contour) -> Pline {
    geom::pline_from(
        &c.iter().map(|p| (p[0], p[1], p[2])).collect::<Vec<_>>(),
        true,
    )
}
pub(super) fn contour(p: &Pline) -> Contour {
    (0..p.vertex_count())
        .map(|i| {
            let v = p.at(i);
            [v.x, v.y, v.bulge]
        })
        .collect()
}
pub(super) fn union(a: &Contour, b: &Contour) -> Result<Contour, String> {
    let r = geom::union(&pline(a), &pline(b));
    if r.outlines.len() != 1 || !r.holes.is_empty() {
        return Err("Assemblage : contour disjoint ou trou inattendu".into());
    }
    Ok(contour(&r.outlines[0]))
}
pub fn panel_entity(p: &Panel) -> Entity {
    Entity::Polyline(pline(&p.outline))
}
pub fn path_points(c: &Contour) -> Vec<[f64; 2]> {
    // Subdivide arcs only for display; exports retain exact bulges.
    crate::shape::sample(&Entity::Polyline(pline(c)))
}
pub(super) fn relieved(
    mut slot: Contour,
    radius: f64,
    relief: CornerRelief,
) -> Result<Contour, String> {
    if relief == CornerRelief::None {
        return Ok(slot);
    }
    let corners = slot.clone();
    let min = [corners[0][0], corners[0][1]];
    for p in corners {
        let sx = if p[0] == min[0] { 1.0 } else { -1.0 };
        let sy = if p[1] == min[1] { 1.0 } else { -1.0 };
        let c = if relief == CornerRelief::Dogbone {
            [
                p[0] + sx * radius / 2.0_f64.sqrt(),
                p[1] + sy * radius / 2.0_f64.sqrt(),
            ]
        } else {
            [p[0] + sx * radius, p[1]]
        };
        let circle = crate::shape::circle((c[0], c[1]), radius)
            .map_err(|e| e.to_string())?
            .to_pline()
            .ok_or("Cercle invalide")?;
        slot = union(&slot, &contour(&circle))?;
    }
    Ok(slot)
}
/// Clear the concave shoulders left by a round cutter around a protruding tab.
pub(super) fn shoulders(
    outline: &Contour,
    axis: usize,
    end: f64,
    span: [f64; 2],
    direction: f64,
    radius: f64,
    relief: CornerRelief,
) -> Result<Contour, String> {
    let mut result = outline.clone();
    if relief == CornerRelief::None {
        return Ok(result);
    }
    for (edge, sign) in [(span[0], -1.0), (span[1], 1.0)] {
        let mut center = [0.0; 2];
        let diagonal = if relief == CornerRelief::Dogbone {
            radius / 2.0_f64.sqrt()
        } else {
            radius
        };
        center[axis] = end + direction * diagonal;
        center[1 - axis] = edge
            + if relief == CornerRelief::Dogbone {
                sign * diagonal
            } else {
                0.0
            };
        let circle = crate::shape::circle((center[0], center[1]), radius)
            .map_err(|e| e.to_string())?
            .to_pline()
            .ok_or("Cercle invalide")?;
        let cut = geom::difference(&pline(&result), &circle);
        if cut.outlines.len() != 1 || !cut.holes.is_empty() {
            return Err(
                "Dégagement trop grand : augmenter la largeur du tenon ou réduire la fraise".into(),
            );
        }
        result = contour(&cut.outlines[0]);
    }
    Ok(result)
}
pub(super) fn finish(p: &mut Panel) -> Result<(), String> {
    let mut bounds = pline(&p.outline)
        .extents()
        .ok_or("Contour de panneau vide")?;
    for f in &p.features {
        if let Some(b) = pline(&f.contour).extents() {
            bounds.min_x = bounds.min_x.min(b.min_x);
            bounds.min_y = bounds.min_y.min(b.min_y);
            bounds.max_x = bounds.max_x.max(b.max_x);
            bounds.max_y = bounds.max_y.max(b.max_y);
        }
    }
    p.uv_min = [bounds.min_x, bounds.min_y];
    p.width = bounds.max_x - bounds.min_x;
    p.height = bounds.max_y - bounds.min_y;
    if p.width <= 0.0 || p.height <= 0.0 {
        return Err(format!("{} : panneau dégénéré", p.id));
    }
    Ok(())
}
