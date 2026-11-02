//! Scène 2D du backplot : primitives en coordonnées monde (mm), indépendantes du moteur de rendu.
//! Le même résultat est dessiné par l'export SVG et par la fenêtre egui.

use crate::{Backplot, Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorBy {
    /// Dégradé selon la profondeur (violet = fond, jaune = surface).
    Z,
    /// Une couleur par type de mouvement (droite, arc, plongée).
    Type,
}

#[derive(Debug, Clone)]
pub struct ViewOptions {
    pub show_rapids: bool,
    pub show_arrows: bool,
    pub show_plunges: bool,
    pub color_by: ColorBy,
    /// N'afficher que les coupes dont l'altitude recoupe `[bas, haut]`.
    pub z_range: Option<(f64, f64)>,
    /// Animation : n'afficher que les `upto` premiers millimètres du trajet.
    pub upto: Option<f64>,
    /// Distance entre deux flèches (mm monde) ; à adapter au zoom pour une densité constante à l'écran.
    pub arrow_spacing_mm: f64,
    /// Flèche de corde maximale pour découper les arcs (mm).
    pub max_err_mm: f64,
}

impl Default for ViewOptions {
    fn default() -> Self {
        ViewOptions { show_rapids: true, show_arrows: true, show_plunges: true, color_by: ColorBy::Z, z_range: None, upto: None, arrow_spacing_mm: 10.0, max_err_mm: 0.02 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Prim {
    Line { pts: Vec<[f64; 2]>, rgb: [u8; 3], width: f32, dashed: bool },
    /// Flèche de sens de parcours : taille constante à l'écran.
    Arrow { pos: [f64; 2], dir: [f64; 2], rgb: [u8; 3] },
    /// Point : rayon en pixels.
    Dot { pos: [f64; 2], rgb: [u8; 3], radius: f32, ring: bool },
}

pub const RAPID_RGB: [u8; 3] = [150, 150, 150];
pub const ARROW_RGB: [u8; 3] = [25, 25, 25];
pub const PLUNGE_RGB: [u8; 3] = [235, 120, 20];
pub const START_RGB: [u8; 3] = [30, 160, 60];
pub const END_RGB: [u8; 3] = [200, 30, 30];
const FEED_RGB: [u8; 3] = [30, 90, 200];
const ARC_RGB: [u8; 3] = [20, 150, 110];

/// Dégradé de profondeur (inspiré de viridis) : `zmin` → violet foncé, `zmax` → jaune.
pub fn color_for_z(z: f64, zmin: f64, zmax: f64) -> [u8; 3] {
    const STOPS: [[f64; 3]; 5] = [[68.0, 1.0, 84.0], [59.0, 82.0, 139.0], [33.0, 145.0, 140.0], [94.0, 201.0, 98.0], [253.0, 231.0, 37.0]];
    let t = if zmax - zmin > 1e-12 { ((z - zmin) / (zmax - zmin)).clamp(0.0, 1.0) } else { 1.0 };
    let x = t * 4.0;
    let i = (x.floor() as usize).min(3);
    let f = x - i as f64;
    let mut out = [0u8; 3];
    for (k, o) in out.iter_mut().enumerate() {
        *o = (STOPS[i][k] + (STOPS[i + 1][k] - STOPS[i][k]) * f).round() as u8;
    }
    out
}

fn xy(p: [f64; 3]) -> [f64; 2] {
    [p[0], p[1]]
}

/// Construit la scène. Les coupes sont découpées pour que chaque primitive ait une altitude (ou
/// une couleur) homogène ; une rampe ou une hélice devient donc un dégradé.
pub fn build_scene(bp: &Backplot, o: &ViewOptions) -> Vec<Prim> {
    let mut prims = Vec::new();
    let Some((lo, hi)) = bp.bounds() else { return prims };
    let cum = bp.cumulative();
    // échelle des couleurs = altitudes de coupe, pas la hauteur de dégagement
    let (zmin, zmax) = bp.cut_z_range().unwrap_or((lo[2], hi[2]));
    let limit = o.upto.unwrap_or(f64::INFINITY);
    let mut plunge_seen: Vec<(i64, i64)> = Vec::new();
    let mut arrows = 0usize;
    let mut next_arrow = o.arrow_spacing_mm * 0.5;
    let mut first_cut: Option<[f64; 2]> = None;
    let mut last_cut: Option<[f64; 2]> = None;
    // polyligne en cours (coupes consécutives de même couleur)
    let mut cur: Option<(Vec<[f64; 2]>, [u8; 3])> = None;
    let flush = |cur: &mut Option<(Vec<[f64; 2]>, [u8; 3])>, prims: &mut Vec<Prim>| {
        if let Some((pts, rgb)) = cur.take() {
            if pts.len() >= 2 {
                prims.push(Prim::Line { pts, rgb, width: 2.0, dashed: false });
            }
        }
    };

    for (i, s) in bp.segs.iter().enumerate() {
        if cum[i] >= limit {
            break;
        }
        let t_end = if cum[i] + s.len > limit && s.len > 0.0 { (limit - cum[i]) / s.len } else { 1.0 };
        match s.kind {
            Kind::Rapid => {
                flush(&mut cur, &mut prims);
                let (a, b) = (s.point_at(0.0), s.point_at(t_end));
                if o.show_rapids && (a[0] - b[0]).hypot(a[1] - b[1]) > 1e-9 {
                    prims.push(Prim::Line { pts: vec![xy(a), xy(b)], rgb: RAPID_RGB, width: 1.0, dashed: true });
                }
            }
            Kind::Plunge => {
                flush(&mut cur, &mut prims);
                let key = ((s.to[0] * 100.0).round() as i64, (s.to[1] * 100.0).round() as i64);
                let in_range = o.z_range.map_or(true, |(zl, zh)| s.from[2].min(s.to[2]) <= zh && s.from[2].max(s.to[2]) >= zl);
                if o.show_plunges && in_range && !plunge_seen.contains(&key) {
                    plunge_seen.push(key);
                    prims.push(Prim::Dot { pos: [s.to[0], s.to[1]], rgb: PLUNGE_RGB, radius: 3.0, ring: false });
                }
            }
            Kind::Feed | Kind::Arc => {
                let (zlo, zhi) = (s.from[2].min(s.to[2]), s.from[2].max(s.to[2]));
                if let Some((a, b)) = o.z_range {
                    if zlo > b || zhi < a {
                        flush(&mut cur, &mut prims);
                        continue;
                    }
                }
                let sub = if t_end < 1.0 { partial(s, t_end) } else { s.clone() };
                if first_cut.is_none() {
                    first_cut = Some(xy(s.from));
                }
                last_cut = Some(xy(sub.to));
                let pts = sub.polyline(o.max_err_mm);
                for w in pts.windows(2) {
                    let zm = (w[0][2] + w[1][2]) / 2.0;
                    let rgb = match o.color_by {
                        ColorBy::Z => color_for_z(zm, zmin, zmax),
                        ColorBy::Type => if s.kind == Kind::Arc { ARC_RGB } else { FEED_RGB },
                    };
                    match &mut cur {
                        Some((v, c)) if *c == rgb => v.push(xy(w[1])),
                        _ => {
                            flush(&mut cur, &mut prims);
                            cur = Some((vec![xy(w[0]), xy(w[1])], rgb));
                        }
                    }
                }
                if o.show_arrows && s.len > 1e-9 {
                    let end = cum[i] + s.len * t_end;
                    while next_arrow < end && arrows < 5000 {
                        if next_arrow >= cum[i] {
                            let t = (next_arrow - cum[i]) / s.len;
                            if let Some(d) = s.direction_at(t) {
                                prims.push(Prim::Arrow { pos: xy(s.point_at(t)), dir: d, rgb: ARROW_RGB });
                                arrows += 1;
                            }
                        }
                        next_arrow += o.arrow_spacing_mm;
                    }
                    if next_arrow < cum[i] {
                        next_arrow = cum[i] + o.arrow_spacing_mm * 0.5;
                    }
                }
            }
        }
    }
    flush(&mut cur, &mut prims);
    if let Some(p) = first_cut {
        prims.push(Prim::Dot { pos: p, rgb: START_RGB, radius: 5.0, ring: false });
    }
    if let (Some(p), None) = (last_cut, o.upto) {
        prims.push(Prim::Dot { pos: p, rgb: END_RGB, radius: 5.0, ring: false });
    }
    if let Some(d) = o.upto {
        if let Some((pt, _, _)) = bp.position_at(d) {
            prims.push(Prim::Dot { pos: [pt[0], pt[1]], rgb: [255, 60, 0], radius: 7.0, ring: true });
        }
    }
    prims
}

/// Segment tronqué à la fraction `t` de sa longueur.
fn partial(s: &crate::Seg, t: f64) -> crate::Seg {
    let mut p = s.clone();
    p.to = s.point_at(t);
    p.len = s.len * t;
    p
}
