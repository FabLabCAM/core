//! Vue 3D sans moteur graphique : caméra orbitale (perspective), maillage ombré de la surface
//! simulée et projection du trajet. Les mêmes formules sont reprises en JavaScript côté web.

use crate::sim::Heightmap;

/// Caméra orbitale : `yaw` autour de Z (rad), `pitch` au-dessus de l'horizon (rad), Z toujours vertical.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera3 {
    pub yaw: f64,
    pub pitch: f64,
    pub dist: f64,
    pub target: [f64; 3],
}

impl Camera3 {
    pub fn fit(lo: [f64; 3], hi: [f64; 3]) -> Camera3 {
        let target = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let size = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt().max(1.0);
        Camera3 { yaw: -0.6, pitch: 0.7, dist: size * 1.05, target }
    }

    pub fn orbit(&mut self, dyaw: f64, dpitch: f64) {
        self.yaw += dyaw;
        self.pitch = (self.pitch + dpitch).clamp(0.05, std::f64::consts::FRAC_PI_2 - 0.02);
    }

    pub fn zoom(&mut self, f: f64) {
        self.dist = (self.dist * f).clamp(1.0, 1e5);
    }

    /// Point monde → `(x, y, profondeur)` : x à droite, y vers le haut, repère centré sur la cible,
    /// perspective de focale `focal` (pixels). La profondeur croît en s'éloignant de la caméra.
    pub fn project(&self, p: [f64; 3], focal: f64) -> [f64; 3] {
        let q = [p[0] - self.target[0], p[1] - self.target[1], p[2] - self.target[2]];
        let (cy, sy) = (self.yaw.cos(), self.yaw.sin());
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        // rotation autour de Z puis inclinaison
        let x1 = q[0] * cy - q[1] * sy;
        let y1 = q[0] * sy + q[1] * cy;
        let right = x1;
        let up = q[2] * cp + y1 * sp;
        let depth = self.dist + (y1 * cp - q[2] * sp);
        let k = focal / depth.max(1e-3);
        [right * k, up * k, depth]
    }
}

pub type Tri = ([[f64; 3]; 3], [u8; 3]);

fn shade(n: [f64; 3], z: f64, zmin: f64, zmax: f64) -> [u8; 3] {
    let l = [-0.4_f64, -0.5, 0.77];
    let nl = (n[0] * l[0] + n[1] * l[1] + n[2] * l[2]).clamp(0.0, 1.0);
    let t = if zmax - zmin > 1e-9 { ((z - zmin) / (zmax - zmin)).clamp(0.0, 1.0) } else { 1.0 };
    // bois clair en surface, plus sombre au fond
    let base = [150.0 + 70.0 * t, 105.0 + 55.0 * t, 60.0 + 40.0 * t];
    let k = 0.45 + 0.65 * nl;
    [(base[0] * k).min(255.0) as u8, (base[1] * k).min(255.0) as u8, (base[2] * k).min(255.0) as u8]
}

/// Surface ombrée de la grille, sous-échantillonnée à `max_cells` mailles au plus par côté.
pub fn surface_triangles(m: &Heightmap, max_cells: usize) -> Vec<Tri> {
    let stride = (m.nx.max(m.ny) / max_cells.max(2)).max(1);
    let (gx, gy) = (m.nx.div_ceil(stride), m.ny.div_ceil(stride));
    let (zmin, zmax) = m.z_range();
    let vert = |i: usize, j: usize| -> [f64; 3] {
        let (ix, iy) = ((i * stride).min(m.nx - 1), (j * stride).min(m.ny - 1));
        [m.min[0] + (ix as f64 + 0.5) * m.res, m.min[1] + (iy as f64 + 0.5) * m.res, m.at(ix, iy)]
    };
    let mut out = Vec::with_capacity(gx * gy * 2);
    for j in 0..gy.saturating_sub(1) {
        for i in 0..gx.saturating_sub(1) {
            let (a, b, c, d) = (vert(i, j), vert(i + 1, j), vert(i + 1, j + 1), vert(i, j + 1));
            for t in [[a, b, c], [a, c, d]] {
                let (u, v) = ([t[1][0] - t[0][0], t[1][1] - t[0][1], t[1][2] - t[0][2]], [t[2][0] - t[0][0], t[2][1] - t[0][1], t[2][2] - t[0][2]]);
                let mut n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
                n = [n[0] / l, n[1] / l, n[2] / l];
                let z = (t[0][2] + t[1][2] + t[2][2]) / 3.0;
                out.push((t, shade(n, z, zmin, zmax)));
            }
        }
    }
    out
}

/// Triangles triés du fond vers l'avant (algorithme du peintre) avec leurs sommets projetés.
pub fn sorted_projected(cam: &Camera3, tris: &[Tri], focal: f64) -> Vec<([[f64; 2]; 3], [u8; 3])> {
    let mut v: Vec<(f64, [[f64; 2]; 3], [u8; 3])> = tris
        .iter()
        .map(|(t, c)| {
            let p = [cam.project(t[0], focal), cam.project(t[1], focal), cam.project(t[2], focal)];
            ((p[0][2] + p[1][2] + p[2][2]) / 3.0, [[p[0][0], p[0][1]], [p[1][0], p[1][1]], [p[2][0], p[2][1]]], *c)
        })
        .collect();
    v.sort_by(|a, b| b.0.total_cmp(&a.0));
    v.into_iter().map(|(_, p, c)| (p, c)).collect()
}
