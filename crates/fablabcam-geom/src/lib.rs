//! Géométrie 2D pour FabLabCAM : polylignes avec arcs (bulge), décalage parallèle, cercles.
//!
//! Le décalage s'appuie sur `cavalier_contours` (Rust pur). Convention de ce crate :
//! `inset` réduit une forme fermée, `outset` l'agrandit, quel que soit son sens de rotation.

use cavalier_contours::core::math::Vector2;
use cavalier_contours::polyline::{BooleanOp, PlineOffsetOptions, PlineOrientation};

pub use cavalier_contours::polyline::{PlineSource, PlineSourceMut, PlineVertex, Polyline};

pub type Pline = Polyline<f64>;

/// Distance en dessous de laquelle deux points sont considérés égaux (mm).
pub const EPS: f64 = 1e-9;

/// Construit une polyligne à partir de sommets `(x, y, bulge)`.
pub fn pline_from(points: &[(f64, f64, f64)], closed: bool) -> Pline {
    let mut p = if closed { Pline::new_closed() } else { Pline::new() };
    for &(x, y, b) in points {
        p.add(x, y, b);
    }
    p
}

/// Rectangle fermé orienté dans le sens trigonométrique.
pub fn rectangle(x: f64, y: f64, w: f64, h: f64) -> Pline {
    pline_from(&[(x, y, 0.0), (x + w, y, 0.0), (x + w, y + h, 0.0), (x, y + h, 0.0)], true)
}

/// Cercle fermé (deux demi-arcs), sens trigonométrique.
pub fn circle(cx: f64, cy: f64, r: f64) -> Pline {
    pline_from(&[(cx - r, cy, 1.0), (cx + r, cy, 1.0)], true)
}

/// Arc défini par centre, rayon, angle de départ et balayage (degrés, positif = anti-horaire).
pub fn arc(cx: f64, cy: f64, r: f64, start_deg: f64, sweep_deg: f64) -> Pline {
    let a0 = start_deg.to_radians();
    let a1 = (start_deg + sweep_deg).to_radians();
    let bulge = (sweep_deg.to_radians() / 4.0).tan();
    pline_from(
        &[(cx + r * a0.cos(), cy + r * a0.sin(), bulge), (cx + r * a1.cos(), cy + r * a1.sin(), 0.0)],
        false,
    )
}

/// Copie orientée dans le sens trigonométrique (aire positive).
pub fn ccw(p: &Pline) -> Pline {
    let mut q = p.clone();
    if p.is_closed() && p.orientation() == PlineOrientation::Clockwise {
        q.invert_direction_mut();
    }
    q
}

/// Réduit la forme fermée de `distance` (> 0). Peut renvoyer plusieurs contours, ou aucun.
pub fn inset(p: &Pline, distance: f64) -> Vec<Pline> {
    if !p.is_closed() {
        return Vec::new();
    }
    offset_without_redundant_vertices(&ccw(p), distance)
}

/// Agrandit la forme fermée de `distance` (> 0).
pub fn outset(p: &Pline, distance: f64) -> Vec<Pline> {
    if !p.is_closed() {
        return Vec::new();
    }
    offset_without_redundant_vertices(&ccw(p), -distance)
}

/// ARCHITECTURE NOTE — DXF contours can repeat the closing vertex and contain
/// aligned microsegments. Normalize a copy with the kernel's tolerance before
/// offsetting; otherwise cavalier_contours 0.3 can build NaN bounds (#24).
/// Shared with the normalization prepared for issue #23.
fn offset_without_redundant_vertices(p: &Pline, distance: f64) -> Vec<Pline> {
    let options = PlineOffsetOptions::default();
    // remove_redundant alone can collapse a short contour starting with a
    // repeated position in 0.3. Remove repeats first, retaining outgoing bulges.
    let distinct = p.remove_repeat_pos(options.pos_equal_eps);
    let contour = distinct.as_ref().unwrap_or(p);
    let cleaned = contour.remove_redundant(options.pos_equal_eps);
    cleaned.as_ref().unwrap_or(contour).parallel_offset_opt(distance, &options)
}

/// Aire absolue d'une forme fermée.
pub fn area(p: &Pline) -> f64 {
    p.area().abs()
}

/// Segment d'une polyligne prêt à être parcouru.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub bulge: f64,
}

impl Segment {
    pub fn is_arc(&self) -> bool {
        self.bulge.abs() > EPS
    }

    /// Centre de l'arc (None pour un segment droit).
    pub fn arc_center(&self) -> Option<(f64, f64)> {
        if !self.is_arc() {
            return None;
        }
        let (dx, dy) = (self.to.0 - self.from.0, self.to.1 - self.from.1);
        let chord = dx.hypot(dy);
        if chord < EPS {
            return None;
        }
        let theta = 4.0 * self.bulge.atan();
        let h = (chord / 2.0) / (theta / 2.0).tan();
        let (mx, my) = ((self.from.0 + self.to.0) / 2.0, (self.from.1 + self.to.1) / 2.0);
        Some((mx - dy / chord * h, my + dx / chord * h))
    }

    /// Vrai si l'arc est parcouru dans le sens trigonométrique.
    pub fn is_ccw(&self) -> bool {
        self.bulge > 0.0
    }

    /// Angle total balayé par l'arc (radians, signé) ; 0 pour un segment droit.
    pub fn sweep(&self) -> f64 {
        4.0 * self.bulge.atan()
    }

    pub fn length(&self) -> f64 {
        match self.arc_center() {
            Some(c) => (self.from.0 - c.0).hypot(self.from.1 - c.1) * self.sweep().abs(),
            None => (self.to.0 - self.from.0).hypot(self.to.1 - self.from.1),
        }
    }

    /// Point à la fraction `t` (0..=1) du segment, mesurée sur l'angle pour un arc.
    pub fn point_at(&self, t: f64) -> (f64, f64) {
        match self.arc_center() {
            Some(c) => {
                let r = (self.from.0 - c.0).hypot(self.from.1 - c.1);
                let a = (self.from.1 - c.1).atan2(self.from.0 - c.0) + t * self.sweep();
                (c.0 + r * a.cos(), c.1 + r * a.sin())
            }
            None => (self.from.0 + t * (self.to.0 - self.from.0), self.from.1 + t * (self.to.1 - self.from.1)),
        }
    }

    /// Direction unitaire de déplacement à la fraction `t`.
    pub fn tangent_at(&self, t: f64) -> (f64, f64) {
        match self.arc_center() {
            Some(c) => {
                let a = (self.from.1 - c.1).atan2(self.from.0 - c.0) + t * self.sweep();
                let k = self.sweep().signum();
                (-k * a.sin(), k * a.cos())
            }
            None => {
                let (dx, dy) = (self.to.0 - self.from.0, self.to.1 - self.from.1);
                let l = dx.hypot(dy).max(EPS);
                (dx / l, dy / l)
            }
        }
    }

    /// Sous-segment entre les fractions `t0` et `t1`.
    pub fn sub(&self, t0: f64, t1: f64) -> Segment {
        Segment {
            from: self.point_at(t0),
            to: self.point_at(t1),
            bulge: if self.is_arc() { (self.sweep() * (t1 - t0) / 4.0).tan() } else { 0.0 },
        }
    }
}

impl Segment {
    /// Fraction (0..=1) du segment correspondant à la projection de `pt` sur celui-ci.
    pub fn param_of(&self, pt: (f64, f64)) -> f64 {
        match self.arc_center() {
            Some(c) => {
                let a0 = (self.from.1 - c.1).atan2(self.from.0 - c.0);
                let a = (pt.1 - c.1).atan2(pt.0 - c.0);
                let sweep = self.sweep();
                let mut delta = a - a0;
                // ramener dans le sens de parcours de l'arc
                if sweep > 0.0 {
                    delta = delta.rem_euclid(std::f64::consts::TAU);
                } else {
                    delta = -((-delta).rem_euclid(std::f64::consts::TAU));
                }
                (delta / sweep).clamp(0.0, 1.0)
            }
            None => {
                let (dx, dy) = (self.to.0 - self.from.0, self.to.1 - self.from.1);
                let l2 = (dx * dx + dy * dy).max(EPS);
                (((pt.0 - self.from.0) * dx + (pt.1 - self.from.1) * dy) / l2).clamp(0.0, 1.0)
            }
        }
    }
}

/// Point le plus proche de `pt` sur la polyligne : (point, distance, indice du segment).
pub fn closest_point(p: &Pline, pt: (f64, f64)) -> Option<((f64, f64), f64, usize)> {
    p.closest_point(Vector2::new(pt.0, pt.1), 1e-9).map(|r| ((r.seg_point.x, r.seg_point.y), r.distance, r.seg_start_index))
}

/// Copie parcourue en sens inverse.
pub fn reversed(p: &Pline) -> Pline {
    let mut q = p.clone();
    q.invert_direction_mut();
    q
}

/// Polyligne construite à partir de segments consécutifs.
pub fn pline_from_segments(segs: &[Segment], closed: bool) -> Pline {
    let mut p = if closed { Pline::new_closed() } else { Pline::new() };
    for s in segs {
        p.add(s.from.0, s.from.1, s.bulge);
    }
    if !closed {
        if let Some(l) = segs.last() {
            p.add(l.to.0, l.to.1, 0.0);
        }
    }
    p
}

/// Même contour fermé, mais commençant au point le plus proche de `pt`.
pub fn rotate_start(c: &Pline, pt: (f64, f64)) -> Pline {
    let segs = segments(c);
    let Some((cp, _, idx)) = closest_point(c, pt).filter(|_| c.is_closed() && !segs.is_empty()) else { return c.clone() };
    let t = segs[idx].param_of(cp);
    let mut out = vec![segs[idx].sub(t, 1.0)];
    out.extend_from_slice(&segs[idx + 1..]);
    out.extend_from_slice(&segs[..idx]);
    out.push(segs[idx].sub(0.0, t));
    out.retain(|s| s.length() > 1e-9);
    pline_from_segments(&out, true)
}

/// Longueur d'une polyligne (arcs compris, segment de fermeture inclus si fermée).
pub fn perimeter(p: &Pline) -> f64 {
    segments(p).iter().map(Segment::length).sum()
}

/// Le point est-il à l'intérieur de la forme fermée ?
pub fn contains_point(shape: &Pline, pt: (f64, f64)) -> bool {
    shape.is_closed() && shape.winding_number(Vector2::new(pt.0, pt.1)) != 0
}

/// `inner` est-il entièrement dans `outer` (sans croisement) ?
pub fn is_inside(outer: &Pline, inner: &Pline) -> bool {
    let Some(pt) = start_point(inner) else { return false };
    let hits = outer.find_intersects(inner);
    contains_point(outer, pt) && hits.basic_intersects.is_empty() && hits.overlapping_intersects.is_empty()
}

/// Résultat d'une opération booléenne : contours positifs et trous.
#[derive(Debug, Default, Clone)]
pub struct Region {
    pub outlines: Vec<Pline>,
    pub holes: Vec<Pline>,
}

fn bool_op(a: &Pline, b: &Pline, op: BooleanOp) -> Region {
    let r = a.boolean(b, op);
    Region {
        outlines: r.pos_plines.into_iter().map(|p| p.pline).collect(),
        holes: r.neg_plines.into_iter().map(|p| p.pline).collect(),
    }
}

/// `a` privée de `b`.
pub fn difference(a: &Pline, b: &Pline) -> Region {
    bool_op(a, b, BooleanOp::Not)
}

/// Partie commune de `a` et `b`.
pub fn intersection(a: &Pline, b: &Pline) -> Region {
    bool_op(a, b, BooleanOp::And)
}

/// Décalages d'une polyligne ouverte ; `distance` > 0 du côté gauche du sens de parcours.
pub fn offset_open(p: &Pline, distance: f64) -> Vec<Pline> {
    if p.is_closed() {
        return Vec::new();
    }
    offset_without_redundant_vertices(p, distance)
}

/// Union de `a` et `b`. Si les formes sont disjointes, les deux sont conservées.
pub fn union(a: &Pline, b: &Pline) -> Region {
    let r = a.boolean(b, BooleanOp::Or);
    if matches!(r.result_info, cavalier_contours::polyline::BooleanResultInfo::Disjoint) {
        return Region { outlines: vec![a.clone(), b.clone()], holes: Vec::new() };
    }
    Region {
        outlines: r.pos_plines.into_iter().map(|p| p.pline).collect(),
        holes: r.neg_plines.into_iter().map(|p| p.pline).collect(),
    }
}

/// Union d'un ensemble de formes (les trous créés par l'union sont ignorés).
pub fn union_all(shapes: &[Pline]) -> Vec<Pline> {
    let mut acc: Vec<Pline> = Vec::new();
    for s in shapes {
        let mut cur = s.clone();
        let mut i = 0;
        while i < acc.len() {
            let r = union(&acc[i], &cur);
            if r.outlines.len() == 1 && (r.outlines[0].vertex_count() > 0) && !is_disjoint(&acc[i], &cur) {
                cur = r.outlines.into_iter().next().unwrap();
                acc.remove(i);
                i = 0;
            } else {
                i += 1;
            }
        }
        acc.push(cur);
    }
    acc
}

fn is_disjoint(a: &Pline, b: &Pline) -> bool {
    matches!(a.boolean(b, BooleanOp::Or).result_info, cavalier_contours::polyline::BooleanResultInfo::Disjoint)
}

/// Segments d'une polyligne, en incluant le segment de fermeture si elle est fermée.
pub fn segments(p: &Pline) -> Vec<Segment> {
    let n = p.vertex_count();
    let count = if p.is_closed() { n } else { n.saturating_sub(1) };
    (0..count)
        .map(|i| {
            let a = p.at(i);
            let b = p.at((i + 1) % n);
            Segment { from: (a.x, a.y), to: (b.x, b.y), bulge: a.bulge }
        })
        .collect()
}

/// Point de départ de la polyligne.
pub fn start_point(p: &Pline) -> Option<(f64, f64)> {
    (p.vertex_count() > 0).then(|| {
        let v = p.at(0);
        (v.x, v.y)
    })
}

/// Regroupe des formes fermées en (contour, îlots) par imbrication : profondeur paire = contour,
/// profondeur impaire = îlot du contour direct qui le contient.
pub fn nest(shapes: &[Pline]) -> Vec<(Pline, Vec<Pline>)> {
    let closed: Vec<&Pline> = shapes.iter().filter(|s| s.is_closed()).collect();
    let parents: Vec<Option<usize>> = (0..closed.len())
        .map(|i| {
            // parent direct = le plus petit contenant
            (0..closed.len())
                .filter(|&j| j != i && is_inside(closed[j], closed[i]))
                .min_by(|&a, &b| area(closed[a]).total_cmp(&area(closed[b])))
        })
        .collect();
    let depth = |mut i: usize| {
        let mut d = 0;
        while let Some(p) = parents[i] {
            d += 1;
            i = p;
        }
        d
    };
    (0..closed.len())
        .filter(|&i| depth(i) % 2 == 0)
        .map(|i| {
            let islands = (0..closed.len()).filter(|&j| parents[j] == Some(i)).map(|j| closed[j].clone()).collect();
            (closed[i].clone(), islands)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_area_and_orientation() {
        let r = rectangle(0.0, 0.0, 100.0, 50.0);
        assert!((area(&r) - 5000.0).abs() < 1e-9);
        let mut cw = r.clone();
        cw.invert_direction_mut();
        assert!(ccw(&cw).area() > 0.0);
    }

    #[test]
    fn circle_area() {
        let c = circle(0.0, 0.0, 10.0);
        assert!((area(&c) - std::f64::consts::PI * 100.0).abs() < 1e-6);
    }

    #[test]
    fn outset_rectangle_has_rounded_corners() {
        let out = outset(&rectangle(0.0, 0.0, 100.0, 50.0), 3.0);
        assert_eq!(out.len(), 1);
        let arcs = segments(&out[0]).iter().filter(|s| s.is_arc()).count();
        assert_eq!(arcs, 4);
        // aire = 100*50 + 2*3*(100+50) + pi*9
        let expected = 5000.0 + 6.0 * 150.0 + std::f64::consts::PI * 9.0;
        assert!((area(&out[0]) - expected).abs() < 1e-6);
    }

    #[test]
    fn inset_rectangle_keeps_square_corners() {
        let inn = inset(&rectangle(0.0, 0.0, 100.0, 50.0), 3.0);
        assert_eq!(inn.len(), 1);
        assert!((area(&inn[0]) - 94.0 * 44.0).abs() < 1e-6);
    }

    #[test]
    fn inset_too_large_gives_nothing() {
        assert!(inset(&rectangle(0.0, 0.0, 10.0, 10.0), 6.0).is_empty());
    }

    #[test]
    fn arc_center_of_semicircle() {
        let s = Segment { from: (-5.0, 0.0), to: (5.0, 0.0), bulge: 1.0 };
        let c = s.arc_center().unwrap();
        assert!(c.0.abs() < 1e-9 && c.1.abs() < 1e-9);
        assert!(s.is_ccw());
    }

    #[test]
    fn arc_center_quarter_circle() {
        // quart de cercle centré en (0,0), de (10,0) à (0,10), sens trigonométrique
        let b = (std::f64::consts::FRAC_PI_2 / 4.0).tan();
        let s = Segment { from: (10.0, 0.0), to: (0.0, 10.0), bulge: b };
        let c = s.arc_center().unwrap();
        assert!(c.0.abs() < 1e-9 && c.1.abs() < 1e-9, "{c:?}");
    }

    #[test]
    fn segment_length_point_and_sub() {
        let line = Segment { from: (0.0, 0.0), to: (3.0, 4.0), bulge: 0.0 };
        assert!((line.length() - 5.0).abs() < 1e-12);
        assert_eq!(line.point_at(0.5), (1.5, 2.0));
        let semi = Segment { from: (-5.0, 0.0), to: (5.0, 0.0), bulge: 1.0 };
        assert!((semi.length() - 5.0 * std::f64::consts::PI).abs() < 1e-9);
        let top = semi.point_at(0.5);
        assert!(top.0.abs() < 1e-9 && (top.1 + 5.0).abs() < 1e-9, "{top:?}"); // CCW depuis (-5,0) passe par (0,-5)
        let quarter = semi.sub(0.0, 0.5);
        assert!((quarter.length() - 2.5 * std::f64::consts::PI).abs() < 1e-9);
        let t = semi.tangent_at(0.0);
        assert!(t.0.abs() < 1e-9 && (t.1 + 1.0).abs() < 1e-9, "{t:?}");
    }

    #[test]
    fn perimeter_of_circle_and_rectangle() {
        assert!((perimeter(&circle(0.0, 0.0, 10.0)) - 2.0 * std::f64::consts::PI * 10.0).abs() < 1e-9);
        assert!((perimeter(&rectangle(0.0, 0.0, 10.0, 5.0)) - 30.0).abs() < 1e-9);
    }

    #[test]
    fn containment_and_difference() {
        let outer = rectangle(0.0, 0.0, 100.0, 100.0);
        let island = circle(50.0, 50.0, 10.0);
        assert!(is_inside(&outer, &island));
        assert!(!is_inside(&island, &outer));
        assert!(!is_inside(&outer, &circle(100.0, 50.0, 10.0))); // à cheval
        let r = difference(&outer, &island);
        assert_eq!((r.outlines.len(), r.holes.len()), (1, 1));
    }

    #[test]
    fn union_merges_overlaps_and_keeps_disjoint() {
        let a = circle(0.0, 0.0, 10.0);
        let b = circle(15.0, 0.0, 10.0);
        let far = circle(100.0, 0.0, 5.0);
        let u = union_all(&[a, b, far]);
        assert_eq!(u.len(), 2);
    }

    #[test]
    fn nesting_detects_islands() {
        let shapes = vec![rectangle(0.0, 0.0, 100.0, 100.0), circle(30.0, 50.0, 5.0), circle(70.0, 50.0, 5.0), circle(70.0, 50.0, 2.0)];
        let n = nest(&shapes);
        // contour 0 (2 îlots) + le petit cercle de profondeur 2 (contour sans îlot)
        assert_eq!(n.len(), 2);
        let main = n.iter().find(|(o, _)| area(o) > 9000.0).unwrap();
        assert_eq!(main.1.len(), 2);
    }

    #[test]
    fn closest_point_and_param() {
        let r = rectangle(0.0, 0.0, 10.0, 10.0);
        let (pt, d, i) = closest_point(&r, (5.0, -2.0)).unwrap();
        assert_eq!((pt, i), ((5.0, 0.0), 0));
        assert!((d - 2.0).abs() < 1e-9);
        let seg = segments(&r)[0];
        assert!((seg.param_of((2.5, 0.0)) - 0.25).abs() < 1e-12);
        let semi = Segment { from: (-5.0, 0.0), to: (5.0, 0.0), bulge: 1.0 };
        assert!((semi.param_of(semi.point_at(0.3)) - 0.3).abs() < 1e-9);
    }

    #[test]
    fn rotate_start_keeps_the_shape() {
        let c = circle(0.0, 0.0, 10.0);
        let r = rotate_start(&c, (0.0, 12.0)); // le point le plus proche est (0, 10)
        assert!((area(&r) - area(&c)).abs() < 1e-6);
        assert!((perimeter(&r) - perimeter(&c)).abs() < 1e-9);
        let (sx, sy) = start_point(&r).unwrap();
        assert!(sx.abs() < 1e-9 && (sy - 10.0).abs() < 1e-9, "({sx},{sy})");
        let rect = rectangle(0.0, 0.0, 10.0, 6.0);
        let r2 = rotate_start(&rect, (5.0, 9.0));
        assert_eq!(start_point(&r2).unwrap(), (5.0, 6.0));
        assert!((perimeter(&r2) - 32.0).abs() < 1e-9);
        assert!((area(&r2) - 60.0).abs() < 1e-9);
    }

    #[test]
    fn reversed_flips_orientation() {
        let r = rectangle(0.0, 0.0, 10.0, 10.0);
        assert!(r.area() > 0.0 && reversed(&r).area() < 0.0);
    }
}
