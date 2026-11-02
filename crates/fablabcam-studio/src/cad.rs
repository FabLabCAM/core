//! Opérations de dessin : transformations (similitudes), décalage, joindre, éclater, booléens, congés.
//!
//! Fonctions pures sur les entités ; `Project` les applique à la sélection (annulable).

use crate::shape::EditError;
use fablabcam_geom::{self as geom, Pline, PlineSource};
use fablabcam_model::Entity;

const EPS: f64 = 1e-9;

fn bad(m: &str) -> EditError {
    EditError::InvalidParameter(m.to_string())
}

/// Similitude du plan `p' = M p + t` (déplacement, rotation, échelle uniforme, miroir) : elle garde les cercles
/// et les arcs, ce que ne ferait pas une échelle différente en X et en Y.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Similarity {
    m: [f64; 4],
    t: [f64; 2],
}

impl Similarity {
    pub fn translation(dx: f64, dy: f64) -> Similarity {
        Similarity { m: [1.0, 0.0, 0.0, 1.0], t: [dx, dy] }
    }

    fn about(center: (f64, f64), m: [f64; 4]) -> Similarity {
        let t = [center.0 - (m[0] * center.0 + m[1] * center.1), center.1 - (m[2] * center.0 + m[3] * center.1)];
        Similarity { m, t }
    }

    /// Rotation de `deg` degrés (sens trigonométrique) autour de `center`.
    pub fn rotation(center: (f64, f64), deg: f64) -> Similarity {
        let (s, c) = deg.to_radians().sin_cos();
        Self::about(center, [c, -s, s, c])
    }

    /// Échelle uniforme de facteur `k` (> 0) autour de `center`.
    pub fn scale(center: (f64, f64), k: f64) -> Result<Similarity, EditError> {
        if !(k.is_finite() && k > EPS) {
            return Err(bad("le facteur d'échelle doit être > 0"));
        }
        Ok(Self::about(center, [k, 0.0, 0.0, k]))
    }

    /// Symétrie par rapport à la droite passant par `center`, d'angle `axis_deg` (0 = horizontale).
    pub fn mirror(center: (f64, f64), axis_deg: f64) -> Similarity {
        let (s, c) = (2.0 * axis_deg.to_radians()).sin_cos();
        Self::about(center, [c, s, s, -c])
    }

    pub fn apply(&self, p: (f64, f64)) -> (f64, f64) {
        (self.m[0] * p.0 + self.m[1] * p.1 + self.t[0], self.m[2] * p.0 + self.m[3] * p.1 + self.t[1])
    }

    fn det(&self) -> f64 {
        self.m[0] * self.m[3] - self.m[1] * self.m[2]
    }

    fn factor(&self) -> f64 {
        self.det().abs().sqrt()
    }

    /// Rotation de la partie linéaire, en degrés (sans objet pour un miroir).
    fn angle(&self) -> f64 {
        self.m[2].atan2(self.m[0]).to_degrees()
    }

    fn reflects(&self) -> bool {
        self.det() < 0.0
    }
}

fn vertices(p: &Pline) -> Vec<(f64, f64, f64)> {
    (0..p.vertex_count()).map(|i| {
        let v = p.at(i);
        (v.x, v.y, v.bulge)
    }).collect()
}

fn poly(v: &[(f64, f64, f64)], closed: bool) -> Entity {
    Entity::Polyline(geom::pline_from(v, closed))
}

/// Image d'une forme par une similitude ; un miroir inverse le sens des arcs.
pub fn transform(e: &Entity, s: &Similarity) -> Entity {
    let k = s.factor();
    match e {
        Entity::Point { x, y } => {
            let (x, y) = s.apply((*x, *y));
            Entity::Point { x, y }
        }
        Entity::Circle { cx, cy, r } => {
            let (cx, cy) = s.apply((*cx, *cy));
            Entity::Circle { cx, cy, r: r * k }
        }
        Entity::Arc { cx, cy, r, start_deg, sweep_deg } => {
            let c = s.apply((*cx, *cy));
            let a = start_deg.to_radians();
            let p0 = s.apply((cx + r * a.cos(), cy + r * a.sin()));
            let start = (p0.1 - c.1).atan2(p0.0 - c.0).to_degrees();
            Entity::Arc { cx: c.0, cy: c.1, r: r * k, start_deg: start, sweep_deg: if s.reflects() { -sweep_deg } else { *sweep_deg } }
        }
        // un rectangle reste un rectangle sauf s'il est retourné par un miroir
        Entity::Rect { x, y, w, h, r, rot_deg } if !s.reflects() => {
            let o = s.apply((*x, *y));
            Entity::Rect { x: o.0, y: o.1, w: w * k, h: h * k, r: r * k, rot_deg: rot_deg + s.angle() }
        }
        Entity::Rect { .. } => match e.to_pline() {
            Some(p) => transform(&Entity::Polyline(p), s),
            None => e.clone(),
        },
        Entity::Polyline(p) => {
            let sign = if s.reflects() { -1.0 } else { 1.0 };
            let v: Vec<(f64, f64, f64)> = vertices(p).into_iter().map(|(x, y, b)| {
                let (x, y) = s.apply((x, y));
                (x, y, b * sign)
            }).collect();
            poly(&v, p.is_closed())
        }
    }
}

fn to_entities(pl: Vec<Pline>) -> Vec<Entity> {
    pl.into_iter().filter(|p| p.vertex_count() >= 2).map(Entity::Polyline).collect()
}

/// Décalage de `d` mm : vers l'extérieur pour `d` > 0 sur une forme fermée, à gauche du sens de tracé sur une
/// forme ouverte.
pub fn offset(e: &Entity, d: f64) -> Result<Vec<Entity>, EditError> {
    if !d.is_finite() || d.abs() < EPS {
        return Err(bad("la distance de décalage doit être non nulle"));
    }
    if let Entity::Circle { cx, cy, r } = e {
        return if r + d > EPS { Ok(vec![Entity::Circle { cx: *cx, cy: *cy, r: r + d }]) } else { Err(EditError::Degenerate("décalage plus grand que le rayon")) };
    }
    let Some(p) = e.to_pline() else { return Err(bad("un point ne se décale pas")) };
    let out = if p.is_closed() {
        if d > 0.0 { geom::outset(&p, d) } else { geom::inset(&p, -d) }
    } else {
        geom::offset_open(&p, d)
    };
    let out = to_entities(out);
    if out.is_empty() {
        return Err(EditError::Degenerate("le décalage ne laisse aucun contour"));
    }
    Ok(out)
}

fn close_to(a: (f64, f64, f64), b: (f64, f64, f64), tol: f64) -> bool {
    (a.0 - b.0).hypot(a.1 - b.1) <= tol
}

/// Concatène `a` puis `b`, `b` commençant là où `a` finit.
fn concat(a: &[(f64, f64, f64)], b: &[(f64, f64, f64)]) -> Vec<(f64, f64, f64)> {
    let mut v = a[..a.len() - 1].to_vec();
    let last = a[a.len() - 1];
    v.push((last.0, last.1, b[0].2));
    v.extend_from_slice(&b[1..]);
    v
}

fn reversed(v: &[(f64, f64, f64)]) -> Vec<(f64, f64, f64)> {
    vertices(&geom::reversed(&geom::pline_from(v, false)))
}

/// Décalage double d'une forme ouverte (« Open Offset » du logiciel de référence) : contour fermé, à `d` de part et d'autre du
/// tracé et arrondi aux deux bouts. C'est la manière de dessiner une rainure à partir de sa ligne centrale.
pub fn open_offset(e: &Entity, d: f64) -> Result<Entity, EditError> {
    if !(d.is_finite() && d > EPS) {
        return Err(bad("la demi-largeur doit être > 0"));
    }
    let p = match e.to_pline() {
        Some(p) if !p.is_closed() && p.vertex_count() >= 2 && !matches!(e, Entity::Circle { .. }) => p,
        _ => return Err(bad("le décalage double demande une forme ouverte (ligne, polyligne, arc)")),
    };
    let (left, right) = (geom::offset_open(&p, d), geom::offset_open(&p, -d));
    let (Some(l), Some(r), 1, 1) = (left.first(), right.first(), left.len(), right.len()) else {
        return Err(bad("le décalage se recoupe : simplifiez le tracé ou décalez par morceaux"));
    };
    let mut v = vertices(l);
    let mut back = vertices(&geom::reversed(r));
    // chaque bout est un demi-cercle parcouru dans le sens horaire (bulge −1)
    if let Some(x) = v.last_mut() {
        x.2 = -1.0;
    }
    if let Some(x) = back.last_mut() {
        x.2 = -1.0;
    }
    v.extend(back);
    Ok(Entity::Polyline(geom::ccw(&geom::pline_from(&v, true))))
}

/// `n` points régulièrement répartis sur le contour d'une forme, à partir de son début : une forme fermée en reçoit exactement `n` (sans doublon au point de départ), une ouverte `n` dont
/// les deux extrémités.
pub fn divide(e: &Entity, n: usize) -> Result<Vec<Entity>, EditError> {
    if !(2..=10_000).contains(&n) && !(n == 1 && e.to_pline().is_some_and(|p| p.is_closed())) {
        return Err(bad("le nombre de points doit être entre 2 et 10 000"));
    }
    let Some(p) = e.to_pline() else { return Err(bad("un point ne se divise pas")) };
    let segs = geom::segments(&p);
    let total: f64 = segs.iter().map(|s| s.length()).sum();
    if total < EPS {
        return Err(EditError::Degenerate("contour de longueur nulle"));
    }
    let steps = if p.is_closed() { n } else { n - 1 };
    Ok((0..n).map(|k| {
        let mut left = total * k as f64 / steps as f64;
        let mut at = segs.last().map_or((0.0, 0.0), |s| s.to);
        for s in &segs {
            let len = s.length();
            if left <= len + 1e-9 {
                at = s.point_at(if len > 0.0 { (left / len).clamp(0.0, 1.0) } else { 0.0 });
                break;
            }
            left -= len;
        }
        Entity::Point { x: at.0, y: at.1 }
    }).collect())
}

/// Joint les formes ouvertes dont les extrémités se touchent (à `tol` près) ; une chaîne qui revient à son
/// point de départ est fermée. Renvoie une forme par chaîne.
pub fn join(items: &[Entity], tol: f64) -> Result<Vec<Entity>, EditError> {
    let mut rest: Vec<Vec<(f64, f64, f64)>> = Vec::new();
    for e in items {
        let p = e.to_pline().filter(|p| !p.is_closed() && !matches!(e, Entity::Circle { .. }));
        match p {
            Some(p) if p.vertex_count() >= 2 => rest.push(vertices(&p)),
            _ => return Err(bad("seules les formes ouvertes (lignes, polylignes, arcs) se joignent")),
        }
    }
    if rest.len() < 2 {
        return Err(bad("sélectionnez au moins deux formes ouvertes"));
    }
    let mut chains = Vec::new();
    while let Some(mut chain) = (!rest.is_empty()).then(|| rest.remove(0)) {
        loop {
            let (s, e) = (chain[0], chain[chain.len() - 1]);
            let Some(i) = rest.iter().position(|b| close_to(b[0], e, tol) || close_to(b[b.len() - 1], e, tol) || close_to(b[0], s, tol) || close_to(b[b.len() - 1], s, tol)) else { break };
            let b = rest.remove(i);
            let (bs, be) = (b[0], b[b.len() - 1]);
            chain = if close_to(bs, e, tol) {
                concat(&chain, &b)
            } else if close_to(be, e, tol) {
                concat(&chain, &reversed(&b))
            } else if close_to(be, s, tol) {
                concat(&b, &chain)
            } else {
                concat(&reversed(&b), &chain)
            };
        }
        let closed = chain.len() >= 3 && close_to(chain[0], chain[chain.len() - 1], tol);
        if closed {
            chain.pop();
        }
        chains.push(poly(&chain, closed));
    }
    Ok(chains)
}

/// Décompose une polyligne en segments (lignes et arcs) ; un cercle devient deux demi-arcs.
pub fn explode(e: &Entity) -> Vec<Entity> {
    if let Entity::Rect { .. } = e {
        return e.to_pline().map_or_else(|| vec![e.clone()], |p| explode(&Entity::Polyline(p)));
    }
    match e {
        Entity::Circle { cx, cy, r } => vec![
            Entity::Arc { cx: *cx, cy: *cy, r: *r, start_deg: 0.0, sweep_deg: 180.0 },
            Entity::Arc { cx: *cx, cy: *cy, r: *r, start_deg: 180.0, sweep_deg: 180.0 },
        ],
        Entity::Polyline(p) => {
            let segs = geom::segments(p);
            if segs.len() <= 1 && !p.is_closed() {
                return vec![e.clone()];
            }
            segs.iter().filter(|s| s.length() > EPS).map(|s| poly(&[(s.from.0, s.from.1, s.bulge), (s.to.0, s.to.1, 0.0)], false)).collect()
        }
        _ => vec![e.clone()],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    Difference,
    Intersection,
}

/// Opération booléenne entre deux formes fermées ; renvoie les contours résultants (trous compris).
pub fn boolean(a: &Entity, b: &Entity, op: BoolOp) -> Result<Vec<Entity>, EditError> {
    let closed = |e: &Entity| e.to_pline().filter(|p| p.is_closed()).ok_or_else(|| bad("les opérations booléennes demandent deux formes fermées"));
    let (pa, pb) = (geom::ccw(&closed(a)?), geom::ccw(&closed(b)?));
    let r = match op {
        BoolOp::Union => geom::union(&pa, &pb),
        BoolOp::Difference => geom::difference(&pa, &pb),
        BoolOp::Intersection => geom::intersection(&pa, &pb),
    };
    let mut out = to_entities(r.outlines);
    out.extend(to_entities(r.holes));
    if out.is_empty() {
        return Err(EditError::Degenerate("le résultat est vide (formes disjointes ?)"));
    }
    Ok(out)
}

/// Arrondit d'un rayon `r` les coins entre deux segments droits d'une polyligne. Les coins où l'arc ne tient pas
/// (segments trop courts) sont laissés tels quels. Renvoie la forme et le nombre de coins arrondis.
pub fn fillet(e: &Entity, r: f64) -> Result<(Entity, usize), EditError> {
    if !(r.is_finite() && r > EPS) {
        return Err(bad("le rayon de congé doit être > 0"));
    }
    // un rectangle aux coins vifs reste un rectangle : le congé devient son rayon de coin
    if let Entity::Rect { x, y, w, h, r: 0.0, rot_deg } = e {
        if r <= w.min(*h) / 2.0 + EPS {
            return Ok((Entity::Rect { x: *x, y: *y, w: *w, h: *h, r, rot_deg: *rot_deg }, 4));
        }
    }
    let flat;
    let e = if matches!(e, Entity::Rect { .. }) {
        flat = e.to_pline().map(Entity::Polyline).unwrap_or_else(|| e.clone());
        &flat
    } else {
        e
    };
    let Entity::Polyline(p) = e else { return Err(bad("les congés s'appliquent aux polylignes")) };
    let v = vertices(p);
    let n = v.len();
    let closed = p.is_closed();
    if n < 3 {
        return Ok((e.clone(), 0));
    }
    let mut out: Vec<(f64, f64, f64)> = Vec::new();
    let mut count = 0;
    for i in 0..n {
        let interior = closed || (i > 0 && i + 1 < n);
        let (prev, next) = (v[(i + n - 1) % n], v[(i + 1) % n]);
        let cur = v[i];
        // les deux segments adjacents doivent être droits
        if !interior || prev.2.abs() > EPS || cur.2.abs() > EPS {
            out.push(cur);
            continue;
        }
        let (l1, l2) = ((cur.0 - prev.0).hypot(cur.1 - prev.1), (next.0 - cur.0).hypot(next.1 - cur.1));
        if l1 < EPS || l2 < EPS {
            out.push(cur);
            continue;
        }
        let u1 = ((cur.0 - prev.0) / l1, (cur.1 - prev.1) / l1);
        let u2 = ((next.0 - cur.0) / l2, (next.1 - cur.1) / l2);
        let phi = (u1.0 * u2.1 - u1.1 * u2.0).atan2(u1.0 * u2.0 + u1.1 * u2.1);
        let t = r * (phi.abs() / 2.0).tan();
        // un segment est partagé par deux coins : chacun n'en prend que la moitié
        if phi.abs() < 1e-6 || t > l1 / 2.0 + EPS || t > l2 / 2.0 + EPS {
            out.push(cur);
            continue;
        }
        out.push((cur.0 - u1.0 * t, cur.1 - u1.1 * t, (phi / 4.0).tan()));
        out.push((cur.0 + u2.0 * t, cur.1 + u2.1 * t, 0.0));
        count += 1;
    }
    Ok((poly(&out, closed), count))
}

/// Polyligne dont le sommet `i` est déplacé en `pt` ; `None` pour une autre forme ou un indice invalide.
pub fn move_vertex(e: &Entity, i: usize, pt: (f64, f64)) -> Option<Entity> {
    let Entity::Polyline(p) = e else { return None };
    let mut v = vertices(p);
    let b = v.get(i)?.2;
    v[i] = (pt.0, pt.1, b);
    Some(poly(&v, p.is_closed()))
}

/// Sommets d'une polyligne (pour les poignées d'édition).
pub fn vertex_points(e: &Entity) -> Vec<(f64, f64)> {
    match e {
        Entity::Polyline(p) => vertices(p).into_iter().map(|(x, y, _)| (x, y)).collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decalage_double_d_une_ligne_est_une_rainure() {
        let l = Entity::Polyline(geom::pline_from(&[(0.0, 0.0, 0.0), (10.0, 0.0, 0.0)], false));
        let r = open_offset(&l, 2.0).unwrap();
        let p = r.to_pline().unwrap();
        assert!(p.is_closed());
        assert!((geom::area(&p) - (10.0 * 4.0 + std::f64::consts::PI * 4.0)).abs() < 1e-6, "{}", geom::area(&p));
        assert!(open_offset(&l, 0.0).is_err());
        assert!(open_offset(&Entity::Circle { cx: 0.0, cy: 0.0, r: 1.0 }, 1.0).is_err());
    }

    #[test]
    fn decalage_double_d_une_polyligne_coudee() {
        let l = Entity::Polyline(geom::pline_from(&[(0.0, 0.0, 0.0), (20.0, 0.0, 0.0), (20.0, 20.0, 0.0)], false));
        let r = open_offset(&l, 1.0).unwrap().to_pline().unwrap();
        assert!(r.is_closed() && geom::area(&r) > 0.0);
        // longueur 40 × largeur 2, deux bouts ronds, coin : aire approchée
        assert!((geom::area(&r) - (40.0 * 2.0 + std::f64::consts::PI)).abs() < 1.0);
    }

    #[test]
    fn division_en_points_reguliers() {
        let c = Entity::Circle { cx: 0.0, cy: 0.0, r: 10.0 };
        let pts = divide(&c, 6).unwrap();
        assert_eq!(pts.len(), 6);
        for q in &pts {
            let Entity::Point { x, y } = q else { panic!() };
            assert!((x.hypot(*y) - 10.0).abs() < 1e-9);
        }
        let l = Entity::Polyline(geom::pline_from(&[(0.0, 0.0, 0.0), (10.0, 0.0, 0.0)], false));
        let m = divide(&l, 5).unwrap();
        assert_eq!(m.first(), Some(&Entity::Point { x: 0.0, y: 0.0 }));
        assert_eq!(m.last(), Some(&Entity::Point { x: 10.0, y: 0.0 }));
        assert_eq!(m[2], Entity::Point { x: 5.0, y: 0.0 });
        assert!(divide(&l, 1).is_err() && divide(&Entity::Point { x: 0.0, y: 0.0 }, 3).is_err());
    }

    #[test]
    fn similitudes_composees() {
        let r = Similarity::rotation((1.0, 1.0), 90.0);
        let p = r.apply((2.0, 1.0));
        assert!((p.0 - 1.0).abs() < 1e-12 && (p.1 - 2.0).abs() < 1e-12);
        let m = Similarity::mirror((0.0, 0.0), 90.0); // axe vertical
        let q = m.apply((3.0, 4.0));
        assert!((q.0 + 3.0).abs() < 1e-12 && (q.1 - 4.0).abs() < 1e-12);
        assert!(m.reflects() && !r.reflects());
        assert!((Similarity::scale((0.0, 0.0), 2.5).unwrap().factor() - 2.5).abs() < 1e-12);
    }
}
