use super::V;
use fablabcam_geom::{self as geom, PlineSource};
use fablabcam_model::Entity;
use svgtypes::Transform;

#[derive(Clone, Copy, Default)]
pub(super) struct Affine(Transform);

impl Affine {
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        let t = value
            .unwrap_or("")
            .parse::<Transform>()
            .map_err(|e| format!("transformation SVG invalide : {e}"))?;
        Self(t).finite()
    }

    fn finite(self) -> Result<Self, String> {
        let t = self.0;
        if [t.a, t.b, t.c, t.d, t.e, t.f].iter().all(|n| n.is_finite()) {
            Ok(self)
        } else {
            Err("transformation SVG non finie".into())
        }
    }

    /// Parent × local: local coordinates enter the parent frame before the Y flip.
    pub fn compose(self, local: Self) -> Result<Self, String> {
        let (p, l) = (self.0, local.0);
        Self(Transform::new(
            p.a * l.a + p.c * l.b,
            p.b * l.a + p.d * l.b,
            p.a * l.c + p.c * l.d,
            p.b * l.c + p.d * l.d,
            p.a * l.e + p.c * l.f + p.e,
            p.b * l.e + p.d * l.f + p.f,
        ))
        .finite()
    }

    pub fn point(self, p: (f64, f64)) -> (f64, f64) {
        let t = self.0;
        (t.a * p.0 + t.c * p.1 + t.e, t.b * p.0 + t.d * p.1 + t.f)
    }

    fn determinant(self) -> f64 {
        self.0.a * self.0.d - self.0.b * self.0.c
    }

    pub fn similarity_scale(self) -> Option<f64> {
        let t = self.0;
        let (x, y) = (t.a.hypot(t.b), t.c.hypot(t.d));
        let norm = x.max(y);
        if norm > 0.0
            && (x - y).abs() <= norm * 1e-10
            && ((t.a / norm) * (t.c / norm) + (t.b / norm) * (t.d / norm)).abs() <= 1e-10
        {
            Some(x)
        } else {
            None
        }
    }

    pub fn polyline(self, pts: Vec<V>, closed: bool, ymax: f64) -> Result<Option<Entity>, String> {
        let pts = if self.similarity_scale().is_none() {
            self.flatten_arcs(pts, closed)?
        } else {
            pts
        };
        let sign = if self.determinant() < 0.0 { 1.0 } else { -1.0 };
        let mut vertices: Vec<V> = Vec::new();
        for (x, y, b) in pts {
            let (x, y) = self.point((x, y));
            let v = (x, ymax - y, sign * b);
            if ![v.0, v.1, v.2].iter().all(|n| n.is_finite()) {
                return Err("géométrie SVG non finie après transformation".into());
            }
            if let Some(last) = vertices
                .last_mut()
                .filter(|p| (p.0 - x).hypot(p.1 - v.1) < 1e-9)
            {
                last.2 = v.2;
            } else {
                vertices.push(v);
            }
        }
        // Two circular arcs can form a closed circle with only two vertices.
        let valid = vertices.len() >= 2
            && (!closed || vertices.len() >= 3 || vertices.iter().any(|p| p.2 != 0.0));
        Ok(valid.then(|| Entity::Polyline(geom::pline_from(&vertices, closed))))
    }

    fn flatten_arcs(self, pts: Vec<V>, closed: bool) -> Result<Vec<V>, String> {
        if pts.len() < 2 || pts.iter().all(|p| p.2 == 0.0) {
            return Ok(pts);
        }
        let p = geom::pline_from(&pts, closed);
        let mut out = Vec::new();
        // Frobenius norm bounds the stretch in every direction. Choose the
        // sagitta in document units, not the untransformed SVG coordinate frame.
        let t = self.0;
        let stretch = t.a.hypot(t.b).hypot(t.c.hypot(t.d));
        for s in geom::segments(&p) {
            let n = if let Some(c) = s.arc_center() {
                let r = (s.from.0 - c.0).hypot(s.from.1 - c.1) * stretch;
                let step = if r <= crate::curves::TOL {
                    std::f64::consts::PI
                } else {
                    2.0 * (1.0 - crate::curves::TOL / r).acos()
                };
                let n = (s.sweep().abs() / step).ceil().max(1.0);
                if !n.is_finite() || n > 100_000.0 {
                    return Err("arc SVG trop grand pour la tolérance d'import".into());
                }
                n as usize
            } else {
                1
            };
            for i in 0..n {
                let (x, y) = if i == 0 {
                    s.from
                } else {
                    s.point_at(i as f64 / n as f64)
                };
                out.push((x, y, 0.0));
            }
        }
        if !closed {
            let last = p.at(p.vertex_count() - 1);
            out.push((last.x, last.y, 0.0));
        }
        Ok(out)
    }
}
