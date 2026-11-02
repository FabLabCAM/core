//! Simulation d'enlèvement de matière sur une grille de hauteurs (heightmap).
//!
//! Chaque mouvement du G-code est échantillonné plus finement que la maille et l'outil est
//! « tamponné » à chaque point : fraise droite et foret = disque plat, fraise boule = calotte,
//! fraise en V = cône à 90°. Les rapides ne retirent rien mais sont contrôlés (collision).

use crate::project::{OpKind, Stock, ToolKind};
use crate::studio::Studio;
use fablabcam_backplot::{Backplot, Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GougeKind {
    /// L'outil descend sous la profondeur programmée de l'opération.
    BelowFloor,
    /// L'outil entame un pont de maintien laissé par une opération précédente.
    IntoTab,
    /// Un déplacement rapide traverse de la matière.
    RapidInMaterial,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Gouge {
    pub kind: GougeKind,
    pub pos: [f64; 3],
    pub op: String,
}

#[derive(Debug, Clone)]
pub struct Heightmap {
    pub min: [f64; 2],
    pub res: f64,
    pub nx: usize,
    pub ny: usize,
    pub top: f64,
    pub bottom: f64,
    pub h: Vec<f32>,
}

impl Heightmap {
    pub fn new(stock: &Stock, res: f64) -> Heightmap {
        let res = res.max(0.01);
        let nx = (((stock.max[0] - stock.min[0]) / res).ceil() as usize).clamp(1, 4000);
        let ny = (((stock.max[1] - stock.min[1]) / res).ceil() as usize).clamp(1, 4000);
        Heightmap { min: stock.min, res, nx, ny, top: stock.top, bottom: stock.bottom, h: vec![stock.top as f32; nx * ny] }
    }

    pub fn at(&self, ix: usize, iy: usize) -> f64 {
        self.h[iy * self.nx + ix] as f64
    }

    /// Hauteur à la position monde (la cellule contenant le point ; `top` hors du brut).
    pub fn height_at(&self, x: f64, y: f64) -> f64 {
        let (fx, fy) = ((x - self.min[0]) / self.res, (y - self.min[1]) / self.res);
        if fx < 0.0 || fy < 0.0 || fx >= self.nx as f64 || fy >= self.ny as f64 {
            return self.top;
        }
        self.at(fx as usize, fy as usize)
    }

    pub fn z_range(&self) -> (f64, f64) {
        self.h.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(*v as f64), b.max(*v as f64)))
    }

    /// Volume retiré (mm³).
    pub fn removed_volume(&self) -> f64 {
        self.h.iter().map(|v| (self.top - *v as f64).max(0.0)).sum::<f64>() * self.res * self.res
    }
}

pub struct Sim {
    pub map: Heightmap,
    protect: Vec<f32>,
    pub gouges: Vec<Gouge>,
}

/// Outil vu par la simulation : rayon et forme de la pointe.
#[derive(Debug, Clone, Copy)]
pub struct Cutter {
    pub r: f64,
    pub kind: ToolKind,
    /// Angle total du V en degrés (fraise en V seulement).
    pub vee: f64,
}

impl Sim {
    pub fn new(stock: &Stock, res: f64) -> Sim {
        let map = Heightmap::new(stock, res);
        let n = map.h.len();
        Sim { map, protect: vec![f32::NEG_INFINITY; n], gouges: Vec::new() }
    }

    /// Cellules sous l'outil en `(x, y)` ; appelle `f(indice, hauteur_de_l_outil_en_cette_cellule)`.
    fn footprint(&self, x: f64, y: f64, z: f64, t: Cutter, mut f: impl FnMut(usize, f64)) {
        let m = &self.map;
        let (cx, cy) = ((x - m.min[0]) / m.res, (y - m.min[1]) / m.res);
        let rc = t.r / m.res;
        let (x0, x1) = (((cx - rc).floor().max(0.0)) as usize, ((cx + rc).ceil().min(m.nx as f64 - 1.0)).max(0.0) as usize);
        let (y0, y1) = (((cy - rc).floor().max(0.0)) as usize, ((cy + rc).ceil().min(m.ny as f64 - 1.0)).max(0.0) as usize);
        if cx + rc < 0.0 || cy + rc < 0.0 || cx - rc > m.nx as f64 || cy - rc > m.ny as f64 {
            return;
        }
        for iy in y0..=y1 {
            for ix in x0..=x1 {
                let (dx, dy) = (m.min[0] + (ix as f64 + 0.5) * m.res - x, m.min[1] + (iy as f64 + 0.5) * m.res - y);
                let d2 = dx * dx + dy * dy;
                if d2 > t.r * t.r {
                    continue;
                }
                let zc = match t.kind {
                    ToolKind::BallNose => z + t.r - (t.r * t.r - d2).max(0.0).sqrt(),
                    ToolKind::VBit => z + d2.sqrt() / (t.vee.to_radians() / 2.0).tan().max(1e-6),
                    _ => z,
                };
                f(iy * m.nx + ix, zc);
            }
        }
    }

    fn stamp(&mut self, x: f64, y: f64, z: f64, t: Cutter, floor: f64, op: &str) {
        if z < floor - 1e-6 && !self.gouges.iter().any(|g| g.kind == GougeKind::BelowFloor && g.op == op) {
            self.gouges.push(Gouge { kind: GougeKind::BelowFloor, pos: [x, y, z], op: op.into() });
        }
        let mut hit_tab: Option<[f64; 3]> = None;
        let mut writes: Vec<(usize, f32)> = Vec::new();
        self.footprint(x, y, z, t, |i, zc| {
            if (zc as f32) < self.map.h[i] {
                if self.protect[i] > zc as f32 + 1e-3 && hit_tab.is_none() {
                    hit_tab = Some([x, y, zc]);
                }
                writes.push((i, zc as f32));
            }
        });
        for (i, v) in writes {
            self.map.h[i] = v;
        }
        if let Some(p) = hit_tab {
            if !self.gouges.iter().any(|g| g.kind == GougeKind::IntoTab && g.op == op) {
                self.gouges.push(Gouge { kind: GougeKind::IntoTab, pos: p, op: op.into() });
            }
        }
    }

    /// Joue un trajet avec l'outil donné, jusqu'à `limit` mm de longueur ; renvoie la longueur jouée.
    /// `floor` : profondeur programmée. `tabs` : les cellules restées au-dessus du fond après
    /// l'opération deviennent des ponts protégés pour les opérations suivantes.
    pub fn run(&mut self, bp: &Backplot, t: Cutter, floor: f64, tabs: bool, op: &str, limit: f64) -> f64 {
        let step = (self.map.res * 0.5).max(0.02);
        let mut done = 0.0;
        let mut samples: Vec<[f64; 3]> = Vec::new();
        for s in &bp.segs {
            if done >= limit {
                break;
            }
            let part = if done + s.len > limit && s.len > 0.0 { (limit - done) / s.len } else { 1.0 };
            let n = ((s.len * part / step).ceil() as usize).max(1);
            for k in 0..=n {
                let p = s.point_at(part * k as f64 / n as f64);
                match s.kind {
                    Kind::Rapid => {
                        if p[2] < self.map.height_at(p[0], p[1]) - 1e-3 && !self.gouges.iter().any(|g| g.kind == GougeKind::RapidInMaterial && g.op == op) {
                            self.gouges.push(Gouge { kind: GougeKind::RapidInMaterial, pos: p, op: op.into() });
                        }
                    }
                    _ => {
                        self.stamp(p[0], p[1], p[2], t, floor, op);
                        if tabs && s.kind != Kind::Plunge {
                            samples.push(p);
                        }
                    }
                }
            }
            done += s.len * part;
        }
        if tabs && done >= bp.total_len() - 1e-9 {
            // ponts : cellules du couloir de coupe restées au-dessus du fond
            let mut mark: Vec<usize> = Vec::new();
            let stride = (samples.len() / 4000).max(1);
            for p in samples.iter().step_by(stride) {
                self.footprint(p[0], p[1], p[2], t, |i, _| mark.push(i));
            }
            for i in mark {
                if self.map.h[i] as f64 > floor + 0.01 {
                    self.protect[i] = self.protect[i].max(self.map.h[i]);
                }
            }
        }
        done
    }
}

/// Brut par défaut : bornes des trajectoires + marge, de Z=0 au point le plus bas.
pub fn default_stock(studio: &Studio) -> Option<Stock> {
    studio.project().stock.or_else(|| auto_stock(studio))
}

/// Brut calculé d'après les trajectoires, en ignorant un éventuel brut défini à la main.
pub fn auto_stock(studio: &Studio) -> Option<Stock> {
    let p = studio.project();
    let bp = studio.backplot();
    let (lo, hi) = bp.bounds()?;
    let margin = p.tools.iter().map(|t| t.diameter / 2.0).fold(0.0, f64::max) + 2.0;
    let enabled = || p.ops.iter().filter(|o| o.enabled);
    // dessus = la plus haute surface déclarée par les opérations (0 par défaut)
    let top = enabled().map(|o| o.cam.surface).fold(f64::NEG_INFINITY, f64::max);
    let top = if top.is_finite() { top } else { 0.0 };
    let bottom = enabled().map(|o| o.depth).fold(top, f64::min);
    Some(Stock { min: [lo[0] - margin, lo[1] - margin], max: [hi[0] + margin, hi[1] + margin], top, bottom })
}

/// Trajet de chaque opération active (mêmes options de G-code que la simulation : perçages expansés).
pub fn op_backplots(studio: &Studio) -> Vec<(u32, Backplot)> {
    studio
        .project()
        .ops
        .iter()
        .filter(|o| o.enabled)
        .filter_map(|o| {
            let c = studio.computed.get(&o.id)?;
            let (path, params) = (c.path.as_ref()?, c.params.as_ref()?);
            let g = fablabcam_cam::emit(path, params, &fablabcam_cam::Post { canned_drill: false, ..Default::default() });
            Some((o.id, fablabcam_backplot::parse_gcode(&g)))
        })
        .collect()
}

pub struct SimOutput {
    pub map: Heightmap,
    pub gouges: Vec<Gouge>,
    pub total_len: f64,
}

/// Simule les opérations actives dans l'ordre ; `frac` ∈ [0,1] = part du trajet total jouée.
pub fn simulate(studio: &Studio, res: f64, frac: f64) -> Option<SimOutput> {
    let stock = default_stock(studio)?;
    let p = studio.project();
    let mut sim = Sim::new(&stock, res);
    let ops: Vec<_> = op_backplots(studio).into_iter().filter_map(|(id, bp)| Some((p.op(id)?, bp))).collect();
    let total: f64 = ops.iter().map(|(_, b)| b.total_len()).sum();
    let mut left = frac.clamp(0.0, 1.0) * total;
    for (o, bp) in &ops {
        if left <= 0.0 {
            break;
        }
        let tool = p.tool(&o.tool)?;
        let kind = if o.kind == OpKind::Drill { ToolKind::Drill } else { tool.kind };
        left -= sim.run(bp, Cutter { r: tool.diameter / 2.0, kind, vee: tool.vee_angle }, o.depth, o.tabs && o.kind == OpKind::Profile, &o.name, left);
    }
    Some(SimOutput { map: sim.map, gouges: sim.gouges, total_len: total })
}
