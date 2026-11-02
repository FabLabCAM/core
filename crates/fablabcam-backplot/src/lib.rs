//! Backplot : relit un programme G-code et en reconstruit le trajet de l'outil en 3D
//! (rapides, coupes, plongées, arcs et hélices), avec statistiques, couleurs par profondeur,
//! flèches de sens de parcours, export SVG et page HTML interactive.
//!
//! Le backplot est volontairement construit à partir du **G-code**, pas des mouvements internes :
//! on voit ce que la machine exécutera, y compris pour un programme écrit par un autre logiciel.

pub mod gcode;
pub mod scene;
pub mod svg;
pub mod web;

pub use gcode::parse_gcode;
pub use scene::{build_scene, color_for_z, ColorBy, Prim, ViewOptions};

/// Type de mouvement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Déplacement rapide (G0).
    Rapid,
    /// Coupe en ligne droite avec déplacement horizontal (G1).
    Feed,
    /// Descente ou remontée verticale à l'avance de travail (G1 sur Z seul).
    Plunge,
    /// Arc ou hélice (G2/G3).
    Arc,
}

/// Un mouvement élémentaire ; `from` et `to` sont `[x, y, z]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Seg {
    pub kind: Kind,
    pub from: [f64; 3],
    pub to: [f64; 3],
    /// Centre de l'arc (XY), pour `Kind::Arc`.
    pub center: Option<[f64; 2]>,
    pub ccw: bool,
    pub feed: f64,
    /// Numéro de la ligne de G-code (à partir de 1).
    pub line: usize,
    /// Longueur 3D (arc de cercle ou hélice compris).
    pub len: f64,
}

impl Seg {
    /// Angle balayé (radians, signé : positif = anti-horaire) ; 0 pour un segment droit.
    pub fn sweep(&self) -> f64 {
        let Some(c) = self.center else { return 0.0 };
        let a0 = (self.from[1] - c[1]).atan2(self.from[0] - c[0]);
        let a1 = (self.to[1] - c[1]).atan2(self.to[0] - c[0]);
        let mut d = a1 - a0;
        if self.ccw {
            if d <= 1e-9 {
                d += std::f64::consts::TAU; // départ = arrivée → tour complet
            }
        } else if d >= -1e-9 {
            d -= std::f64::consts::TAU;
        }
        d
    }

    /// Point à la fraction `t` (0..=1) de la longueur (hélice : Z linéaire).
    pub fn point_at(&self, t: f64) -> [f64; 3] {
        let z = self.from[2] + (self.to[2] - self.from[2]) * t;
        match self.center {
            Some(c) => {
                let r = (self.from[0] - c[0]).hypot(self.from[1] - c[1]);
                let a = (self.from[1] - c[1]).atan2(self.from[0] - c[0]) + self.sweep() * t;
                [c[0] + r * a.cos(), c[1] + r * a.sin(), z]
            }
            None => [self.from[0] + (self.to[0] - self.from[0]) * t, self.from[1] + (self.to[1] - self.from[1]) * t, z],
        }
    }

    /// Direction XY unitaire au point `t` (None pour un déplacement vertical).
    pub fn direction_at(&self, t: f64) -> Option<[f64; 2]> {
        match self.center {
            Some(c) => {
                let p = self.point_at(t);
                let (rx, ry) = (p[0] - c[0], p[1] - c[1]);
                let l = rx.hypot(ry).max(1e-12);
                let k = if self.ccw { 1.0 } else { -1.0 };
                Some([-k * ry / l, k * rx / l])
            }
            None => {
                let (dx, dy) = (self.to[0] - self.from[0], self.to[1] - self.from[1]);
                let l = dx.hypot(dy);
                (l > 1e-12).then_some([dx / l, dy / l])
            }
        }
    }

    /// Points échantillonnés pour le dessin : un arc est découpé pour que la flèche de corde
    /// reste sous `max_err` (mm).
    pub fn polyline(&self, max_err: f64) -> Vec<[f64; 3]> {
        let n = match self.center {
            Some(c) => {
                let r = (self.from[0] - c[0]).hypot(self.from[1] - c[1]);
                let sweep = self.sweep().abs();
                if r < 1e-9 {
                    1
                } else {
                    let step = 2.0 * (1.0 - (max_err / r).min(1.0)).acos().max(1e-3);
                    ((sweep / step).ceil() as usize).clamp(2, 720)
                }
            }
            None => 1,
        };
        (0..=n).map(|k| self.point_at(k as f64 / n as f64)).collect()
    }
}

/// Statistiques d'un trajet.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats {
    pub segments: usize,
    pub rapid_len: f64,
    pub cut_len: f64,
    /// Durée estimée (s) : coupes à leur avance, rapides à `rapid_feed`.
    pub time_s: f64,
    /// Altitudes distinctes (arrondies au µm) auxquelles l'outil coupe, de haut en bas.
    pub cut_levels: Vec<f64>,
    pub z_min: f64,
    pub z_max: f64,
    /// Altitudes extrêmes de coupe (échelle des couleurs).
    pub cut_z: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Default)]
pub struct Backplot {
    pub segs: Vec<Seg>,
    /// Anomalies rencontrées à la lecture (codes ignorés, mode incrémental, etc.).
    pub warnings: Vec<String>,
    /// Lignes du programme, pour afficher la ligne courante.
    pub source: Vec<String>,
}

impl Backplot {
    pub fn total_len(&self) -> f64 {
        self.segs.iter().map(|s| s.len).sum()
    }

    /// Longueur cumulée au début de chaque segment (même taille que `segs`).
    pub fn cumulative(&self) -> Vec<f64> {
        let mut acc = 0.0;
        self.segs
            .iter()
            .map(|s| {
                let a = acc;
                acc += s.len;
                a
            })
            .collect()
    }

    /// Boîte englobante `(min, max)` de tous les mouvements.
    pub fn bounds(&self) -> Option<([f64; 3], [f64; 3])> {
        let mut it = self.segs.iter().flat_map(|s| s.polyline(0.05));
        let first = it.next()?;
        let (mut lo, mut hi) = (first, first);
        for p in it {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        Some((lo, hi))
    }

    /// Altitudes extrêmes auxquelles l'outil **coupe** (hors déplacements rapides et hors hauteur de
    /// dégagement) : c'est l'échelle des couleurs. Une plongée ne compte que par son point bas.
    pub fn cut_z_range(&self) -> Option<(f64, f64)> {
        let mut r: Option<(f64, f64)> = None;
        for s in &self.segs {
            let zs = match s.kind {
                Kind::Rapid => continue,
                Kind::Plunge => vec![s.from[2].min(s.to[2])],
                Kind::Feed | Kind::Arc => vec![s.from[2], s.to[2]],
            };
            for z in zs {
                r = Some(r.map_or((z, z), |(lo, hi)| (lo.min(z), hi.max(z))));
            }
        }
        r
    }

    pub fn stats(&self, rapid_feed: f64) -> Stats {
        let mut st = Stats { segments: self.segs.len(), z_min: f64::INFINITY, z_max: f64::NEG_INFINITY, ..Default::default() };
        let mut levels: Vec<i64> = Vec::new();
        for s in &self.segs {
            for z in [s.from[2], s.to[2]] {
                st.z_min = st.z_min.min(z);
                st.z_max = st.z_max.max(z);
            }
            match s.kind {
                Kind::Rapid => {
                    st.rapid_len += s.len;
                    st.time_s += s.len / rapid_feed.max(1.0) * 60.0;
                }
                _ => {
                    st.cut_len += s.len;
                    st.time_s += s.len / s.feed.max(1.0) * 60.0;
                    if s.kind != Kind::Plunge && (s.from[2] - s.to[2]).abs() < 1e-9 {
                        levels.push((s.from[2] * 1000.0).round() as i64);
                    }
                }
            }
        }
        levels.sort_unstable_by(|a, b| b.cmp(a));
        levels.dedup();
        st.cut_levels = levels.into_iter().map(|l| l as f64 / 1000.0).collect();
        st.cut_z = self.cut_z_range();
        if self.segs.is_empty() {
            st.z_min = 0.0;
            st.z_max = 0.0;
        }
        st
    }

    /// Position de l'outil après avoir parcouru `dist` mm : `(point, indice du segment, direction XY)`.
    pub fn position_at(&self, dist: f64) -> Option<([f64; 3], usize, Option<[f64; 2]>)> {
        if self.segs.is_empty() {
            return None;
        }
        let cum = self.cumulative();
        let i = cum.partition_point(|c| *c <= dist).saturating_sub(1).min(self.segs.len() - 1);
        let s = &self.segs[i];
        let t = if s.len > 1e-12 { ((dist - cum[i]) / s.len).clamp(0.0, 1.0) } else { 1.0 };
        Some((s.point_at(t), i, s.direction_at(t)))
    }

    /// Profil de profondeur : `(distance cumulée, z)` à chaque extrémité de segment.
    pub fn z_profile(&self) -> Vec<(f64, f64)> {
        let mut out = Vec::with_capacity(self.segs.len() * 2);
        let mut acc = 0.0;
        for s in &self.segs {
            out.push((acc, s.from[2]));
            acc += s.len;
            out.push((acc, s.to[2]));
        }
        out
    }
}
