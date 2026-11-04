use super::*;
use crate::furniture::{geometry::legacy_panels, Panel};
#[derive(Debug, Clone)]
pub struct CellBounds {
    pub id: String,
    pub origin: [f64; 3],
    pub size: [f64; 3],
    pub leaf: bool,
}
#[derive(Debug, Clone)]
pub struct Structure {
    pub panels: Vec<Panel>,
    pub cells: Vec<CellBounds>,
}

pub fn structure(d: &Dimensions) -> Result<Structure, String> {
    let mut basic = d.clone();
    basic.composition = None;
    basic.shelves = 0;
    let mut result = Structure {
        panels: legacy_panels(&basic),
        cells: Vec::new(),
    };
    let Some(c) = &d.composition else {
        result.panels = legacy_panels(d);
        return Ok(result);
    };
    c.check()?;
    let (shell, origin, size) = super::shell::build(d, c);
    result.panels = shell;
    let t = d.thickness;
    if size.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return Err("Dimensions intérieures impossibles".into());
    }
    resolve(&c.root, origin, size, t, &mut result)?;
    for p in &mut result.panels {
        if let Some(name) = c.panel_names.get(&p.id) {
            p.name = name.clone();
        }
    }
    if let Some(i) = result.panels.iter().position(|p| p.id == "DOS") {
        let back = result.panels.remove(i);
        result.panels.push(back);
    }
    let ids = result
        .panels
        .iter()
        .map(|p| p.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    if ids.len() != result.panels.len() {
        return Err("Repère de panneau en double".into());
    }
    Ok(result)
}
fn resolve(
    c: &Cell,
    o: [f64; 3],
    size: [f64; 3],
    t: f64,
    out: &mut Structure,
) -> Result<(), String> {
    out.cells.push(CellBounds {
        id: c.id.clone(),
        origin: o,
        size,
        leaf: c.division.is_none(),
    });
    if let Some(bin) = &c.bin {
        bins(c, bin, o, size, out)?;
    }
    let Some(d) = &c.division else {
        return Ok(());
    };
    let axis = d.axis.coordinate();
    let offsets = d.offsets(size[axis], t);
    let mut start = 0.0;
    for (i, child) in d.children.iter().enumerate() {
        let end = offsets.get(i).copied().unwrap_or(size[axis]);
        if end - start <= 0.01 || !end.is_finite() {
            return Err(format!(
                "{} : panneaux superposés ou case sans espace, revoir les positions",
                c.name
            ));
        }
        let mut co = o;
        let mut cs = size;
        co[axis] += start;
        cs[axis] = end - start;
        resolve(child, co, cs, t, out)?;
        if let Some(divider) = d.dividers.get(i) {
            let mut po = o;
            let mut ps = size;
            po[axis] += end;
            ps[axis] = divider.thickness.unwrap_or(t);
            ps[1] = divider.depth.unwrap_or(size[1]);
            if ps[1] > size[1] || ps[1] <= 0.0 {
                return Err(format!("{} : profondeur hors de la case", divider.name));
            }
            out.panels
                .push(panel(&divider.id, &divider.name, po, ps, axis));
        }
        start = end + d.dividers.get(i).map_or(t, |p| p.thickness.unwrap_or(t));
    }
    Ok(())
}
pub(crate) fn panel(
    id: &str,
    name: &str,
    origin: [f64; 3],
    size: [f64; 3],
    normal: usize,
) -> Panel {
    Panel::rectangular(id, name, origin, size, normal)
}
fn bins(c: &Cell, b: &Bin, o: [f64; 3], s: [f64; 3], out: &mut Structure) -> Result<(), String> {
    let w = s[0] - 2.0 * b.side_gap;
    let h = s[2] - b.top_gap;
    let depth = b.depth;
    let t = b.thickness;
    if w <= 2.0 * t || h <= t || depth + b.back_gap > s[1] {
        return Err(format!(
            "{} / {} : bac trop grand ou jeux incompatibles",
            c.name, b.name
        ));
    }
    let x = o[0] + b.side_gap;
    let y = o[1];
    let z = o[2];
    let bottom = if b.bottom_covers_sides { t } else { 0.0 };
    let wall_z = z + bottom;
    let wall_h = h - bottom;
    for (suffix, label, origin, size, normal) in [
        ("L", "gauche", [x, y, wall_z], [t, depth, wall_h], 0),
        ("R", "droit", [x + w - t, y, wall_z], [t, depth, wall_h], 0),
        (
            "B",
            "fond",
            [x + t - bottom, y + t - bottom, z],
            [w - 2.0 * (t - bottom), depth - 2.0 * (t - bottom), t],
            2,
        ),
        (
            "F",
            "façade",
            [x + t, y, wall_z],
            [w - 2.0 * t, t, wall_h],
            1,
        ),
        (
            "A",
            "arrière",
            [x + t, y + depth - t, wall_z],
            [w - 2.0 * t, t, wall_h],
            1,
        ),
    ] {
        let mut p = panel(
            &format!("{}-{suffix}", b.id),
            &format!("{} — {label}", b.name),
            origin,
            size,
            normal,
        );
        p.assembly = b.id.clone();
        out.panels.push(p);
    }
    Ok(())
}
