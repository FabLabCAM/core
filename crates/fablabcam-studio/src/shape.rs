//! Formes 2D : constructeurs validés, boîte englobante, translation, distance au pointeur.

use fablabcam_geom::{self as geom, Pline, PlineSource};
use fablabcam_model::Entity;

#[derive(Debug, Clone, PartialEq)]
pub enum EditError {
    /// Forme dégénérée (rayon nul, côté nul, moins de deux points…).
    Degenerate(&'static str),
    UnknownShape(u32),
    UnknownTool(String),
    UnknownOperation(u32),
    DuplicateName(String),
    ToolInUse { tool: String, operation: String },
    InvalidParameter(String),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::Degenerate(m) => write!(f, "forme dégénérée : {m}"),
            EditError::UnknownShape(i) => write!(f, "forme {i} introuvable"),
            EditError::UnknownTool(n) => write!(f, "outil « {n} » introuvable"),
            EditError::UnknownOperation(i) => write!(f, "opération {i} introuvable"),
            EditError::DuplicateName(n) => write!(f, "le nom « {n} » existe déjà"),
            EditError::ToolInUse { tool, operation } => write!(f, "l'outil « {tool} » est utilisé par l'opération « {operation} »"),
            EditError::InvalidParameter(m) => write!(f, "paramètre invalide : {m}"),
        }
    }
}

impl std::error::Error for EditError {}

/// Formes avec leur calque, telles qu'échangées avec DXF/SVG.
pub type LayeredShapes = Vec<(String, Entity)>;

const EPS: f64 = 1e-9;

pub fn line(a: (f64, f64), b: (f64, f64)) -> Result<Entity, EditError> {
    if (a.0 - b.0).hypot(a.1 - b.1) < EPS {
        return Err(EditError::Degenerate("ligne de longueur nulle"));
    }
    Ok(Entity::Polyline(geom::pline_from(&[(a.0, a.1, 0.0), (b.0, b.1, 0.0)], false)))
}

pub fn polyline(pts: &[(f64, f64)], closed: bool) -> Result<Entity, EditError> {
    let mut v: Vec<(f64, f64, f64)> = Vec::new();
    for &(x, y) in pts {
        if v.last().is_none_or(|l| (l.0 - x).hypot(l.1 - y) > EPS) {
            v.push((x, y, 0.0));
        }
    }
    if v.len() < 2 || (closed && v.len() < 3) {
        return Err(EditError::Degenerate("polyligne trop courte"));
    }
    Ok(Entity::Polyline(geom::pline_from(&v, closed)))
}

pub fn rectangle(a: (f64, f64), b: (f64, f64)) -> Result<Entity, EditError> {
    let (w, h) = ((b.0 - a.0).abs(), (b.1 - a.1).abs());
    if w < EPS || h < EPS {
        return Err(EditError::Degenerate("rectangle de largeur ou hauteur nulle"));
    }
    Ok(Entity::Rect { x: a.0.min(b.0), y: a.1.min(b.1), w, h, r: 0.0, rot_deg: 0.0 })
}

pub fn circle(c: (f64, f64), r: f64) -> Result<Entity, EditError> {
    if r.is_nan() || r < EPS {
        return Err(EditError::Degenerate("cercle de rayon nul"));
    }
    Ok(Entity::Circle { cx: c.0, cy: c.1, r })
}

pub fn arc(c: (f64, f64), r: f64, start_deg: f64, sweep_deg: f64) -> Result<Entity, EditError> {
    if r.is_nan() || r < EPS || sweep_deg.abs() < 1e-6 {
        return Err(EditError::Degenerate("arc de rayon ou d'angle nul"));
    }
    Ok(Entity::Arc { cx: c.0, cy: c.1, r, start_deg, sweep_deg })
}

/// Boîte englobante `(min, max)`.
pub fn bounds(e: &Entity) -> Option<([f64; 2], [f64; 2])> {
    match e {
        Entity::Point { x, y } => Some(([*x, *y], [*x, *y])),
        _ => {
            let p = e.to_pline()?;
            let b = p.extents()?;
            Some(([b.min_x, b.min_y], [b.max_x, b.max_y]))
        }
    }
}

pub fn translate(e: &Entity, dx: f64, dy: f64) -> Entity {
    match e {
        Entity::Circle { cx, cy, r } => Entity::Circle { cx: cx + dx, cy: cy + dy, r: *r },
        Entity::Arc { cx, cy, r, start_deg, sweep_deg } => Entity::Arc { cx: cx + dx, cy: cy + dy, r: *r, start_deg: *start_deg, sweep_deg: *sweep_deg },
        Entity::Point { x, y } => Entity::Point { x: x + dx, y: y + dy },
        Entity::Rect { x, y, w, h, r, rot_deg } => Entity::Rect { x: x + dx, y: y + dy, w: *w, h: *h, r: *r, rot_deg: *rot_deg },
        Entity::Polyline(p) => {
            let v: Vec<(f64, f64, f64)> = (0..p.vertex_count()).map(|i| { let q = p.at(i); (q.x + dx, q.y + dy, q.bulge) }).collect();
            Entity::Polyline(geom::pline_from(&v, p.is_closed()))
        }
    }
}

/// Distance du point `pt` au contour de la forme.
pub fn distance(e: &Entity, pt: (f64, f64)) -> f64 {
    match e {
        Entity::Point { x, y } => (x - pt.0).hypot(y - pt.1),
        _ => e.to_pline().and_then(|p| geom::closest_point(&p, pt)).map_or(f64::INFINITY, |(_, d, _)| d),
    }
}

/// Contour fermé exploitable en poche/profil ?
pub fn is_closed(e: &Entity) -> bool {
    match e {
        Entity::Circle { .. } => true,
        Entity::Polyline(p) => p.is_closed(),
        _ => false,
    }
}

pub fn pline(e: &Entity) -> Option<Pline> {
    e.to_pline()
}

/// Points échantillonnés du contour (arcs découpés en ≥ 24 pas par tour) pour l'affichage.
pub fn sample(e: &Entity) -> Vec<[f64; 2]> {
    let Some(p) = e.to_pline() else {
        return match e {
            Entity::Point { x, y } => vec![[*x, *y]],
            _ => Vec::new(),
        };
    };
    let mut out: Vec<[f64; 2]> = Vec::new();
    for s in geom::segments(&p) {
        let n = if s.is_arc() { ((s.sweep().abs() / std::f64::consts::TAU * 48.0).ceil() as usize).max(4) } else { 1 };
        if out.is_empty() {
            out.push([s.from.0, s.from.1]);
        }
        for k in 1..=n {
            let q = s.point_at(k as f64 / n as f64);
            out.push([q.0, q.1]);
        }
    }
    out
}
