use fablabcam_backplot::{Backplot, Kind};

/// Caméra 2D : `center` = point monde au centre du canevas, `scale` = pixels par mm. Y vers le haut.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub center: [f64; 2],
    pub scale: f64,
}

impl Camera {
    /// Cadre la boîte `lo..hi` dans un canevas `w × h` avec une marge en pixels.
    pub fn fit(lo: [f64; 2], hi: [f64; 2], w: f64, h: f64, margin: f64) -> Camera {
        let (bw, bh) = ((hi[0] - lo[0]).max(1e-6), (hi[1] - lo[1]).max(1e-6));
        let scale = ((w - 2.0 * margin).max(10.0) / bw).min((h - 2.0 * margin).max(10.0) / bh);
        Camera { center: [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0], scale }
    }

    /// Monde → pixels, relativement au coin haut-gauche du canevas.
    pub fn to_screen(&self, p: [f64; 2], size: (f64, f64)) -> (f64, f64) {
        (size.0 / 2.0 + (p[0] - self.center[0]) * self.scale, size.1 / 2.0 - (p[1] - self.center[1]) * self.scale)
    }

    pub fn to_world(&self, s: (f64, f64), size: (f64, f64)) -> [f64; 2] {
        [self.center[0] + (s.0 - size.0 / 2.0) / self.scale, self.center[1] - (s.1 - size.1 / 2.0) / self.scale]
    }

    /// Zoom d'un facteur `f` en gardant fixe le point monde sous le curseur.
    pub fn zoom_at(&mut self, f: f64, cursor: (f64, f64), size: (f64, f64)) {
        let before = self.to_world(cursor, size);
        self.scale = (self.scale * f).clamp(0.01, 5000.0);
        let after = self.to_world(cursor, size);
        self.center[0] += before[0] - after[0];
        self.center[1] += before[1] - after[1];
    }

    /// Déplacement de la vue d'un glissement de souris en pixels.
    pub fn pan_px(&mut self, dx: f64, dy: f64) {
        self.center[0] -= dx / self.scale;
        self.center[1] += dy / self.scale;
    }
}

/// Mouvement de coupe le plus proche d'un point monde : `(indice, distance, t sur le segment)`.
/// Les rapides ne sont pas considérés : c'est la matière usinée qu'on veut interroger.
pub fn nearest_cut(bp: &Backplot, world: [f64; 2], max_dist: f64) -> Option<(usize, f64, f64)> {
    let mut best: Option<(usize, f64, f64)> = None;
    for (i, s) in bp.segs.iter().enumerate() {
        if s.kind == Kind::Rapid || s.kind == Kind::Plunge {
            continue;
        }
        // rejet rapide par boîte englobante grossière (arcs : rayon)
        let r = s.center.map_or(0.0, |c| (s.from[0] - c[0]).hypot(s.from[1] - c[1]));
        let (cx, cy, rad) = match s.center {
            Some(c) => (c[0], c[1], r + max_dist),
            None => ((s.from[0] + s.to[0]) / 2.0, (s.from[1] + s.to[1]) / 2.0, (s.from[0] - s.to[0]).hypot(s.from[1] - s.to[1]) / 2.0 + max_dist),
        };
        if (world[0] - cx).hypot(world[1] - cy) > rad {
            continue;
        }
        let pts = s.polyline(0.05);
        let n = pts.len() - 1;
        for k in 0..n {
            let (a, b) = (pts[k], pts[k + 1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l2 = dx * dx + dy * dy;
            let u = if l2 > 1e-18 { (((world[0] - a[0]) * dx + (world[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let d = (world[0] - (a[0] + u * dx)).hypot(world[1] - (a[1] + u * dy));
            if d <= max_dist && best.is_none_or(|(_, bd, _)| d < bd) {
                best = Some((i, d, (k as f64 + u) / n as f64));
            }
        }
    }
    best
}

/// Lecture animée du trajet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Player {
    /// Distance parcourue (mm).
    pub pos: f64,
    pub playing: bool,
    /// Vitesse de lecture (mm/s de trajet).
    pub speed: f64,
}

impl Default for Player {
    fn default() -> Self {
        Player { pos: 0.0, playing: false, speed: 200.0 }
    }
}

impl Player {
    /// Avance de `dt` secondes ; s'arrête à la fin du trajet.
    pub fn advance(&mut self, dt: f64, total: f64) {
        if self.playing {
            self.pos += self.speed * dt;
            if self.pos >= total {
                self.pos = total;
                self.playing = false;
            }
        }
        self.pos = self.pos.clamp(0.0, total.max(0.0));
    }

    /// Pas à pas : avancer = début du segment suivant (ou fin du trajet) ; reculer = début du segment
    /// courant, ou du précédent si on y est déjà.
    pub fn step_segments(&mut self, bp: &Backplot, delta: isize) {
        let cum = bp.cumulative();
        if cum.is_empty() {
            return;
        }
        let n = cum.len() as isize;
        let cur = cum.partition_point(|c| *c <= self.pos + 1e-9).saturating_sub(1) as isize;
        self.pos = if delta > 0 {
            let target = cur + delta;
            if target >= n { bp.total_len() } else { cum[target as usize] }
        } else {
            let target = if delta == -1 && self.pos > cum[cur as usize] + 1e-9 { cur } else { cur + delta };
            cum[target.clamp(0, n - 1) as usize]
        };
        self.playing = false;
    }
}

/// Bornes `[bas, haut]` (avec un peu de marge) isolant le niveau `levels[idx]`.
pub fn solo_range(levels: &[f64], idx: usize) -> Option<(f64, f64)> {
    levels.get(idx).map(|z| (z - 1e-3, z + 1e-3))
}

/// Cadre de la boîte XY du tracé (None si le programme est vide).
pub fn bounds_xy(bp: &Backplot) -> Option<([f64; 2], [f64; 2])> {
    bp.bounds().map(|(lo, hi)| ([lo[0], lo[1]], [hi[0], hi[1]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fablabcam_backplot::parse_gcode;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn fit_centers_and_scales_to_the_tightest_axis() {
        let cam = Camera::fit([0.0, 0.0], [100.0, 50.0], 1000.0, 400.0, 20.0);
        assert_eq!(cam.center, [50.0, 25.0]);
        // 960/100 = 9.6 horizontalement, 360/50 = 7.2 verticalement
        assert!(close(cam.scale, 7.2));
        // les coins tiennent dans le canevas
        let (x, y) = cam.to_screen([0.0, 0.0], (1000.0, 400.0));
        assert!(x >= 20.0 && y <= 380.0 + 1e-9);
    }

    #[test]
    fn screen_world_round_trip_and_y_is_up() {
        let cam = Camera { center: [10.0, 20.0], scale: 4.0 };
        let size = (800.0, 600.0);
        let s = cam.to_screen([12.0, 25.0], size);
        assert!(close(s.0, 408.0) && close(s.1, 280.0)); // y monde ↑ = y écran ↓
        let w = cam.to_world(s, size);
        assert!(close(w[0], 12.0) && close(w[1], 25.0));
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor_fixed() {
        let mut cam = Camera { center: [0.0, 0.0], scale: 2.0 };
        let size = (800.0, 600.0);
        let cursor = (600.0, 150.0);
        let before = cam.to_world(cursor, size);
        cam.zoom_at(1.5, cursor, size);
        assert!(close(cam.scale, 3.0));
        let after = cam.to_world(cursor, size);
        assert!(close(before[0], after[0]) && close(before[1], after[1]));
        cam.zoom_at(1e9, cursor, size);
        assert!(cam.scale <= 5000.0);
    }

    #[test]
    fn pan_moves_the_scene_with_the_mouse() {
        let mut cam = Camera { center: [0.0, 0.0], scale: 2.0 };
        let size = (100.0, 100.0);
        let p = [5.0, 5.0];
        let before = cam.to_screen(p, size);
        cam.pan_px(10.0, -6.0);
        let after = cam.to_screen(p, size);
        assert!(close(after.0 - before.0, 10.0) && close(after.1 - before.1, -6.0));
    }

    #[test]
    fn nearest_cut_finds_the_segment_under_the_cursor() {
        let bp = parse_gcode("G0 X0 Y0 Z3\nG1 Z-1 F100\nG1 X100\nG1 Y50\nG3 X0 Y50 I-50 J0\n");
        // près du premier côté (y ≈ 0) : segment 1 (la plongée est indice 0)
        let (i, d, t) = nearest_cut(&bp, [40.0, 0.5], 2.0).unwrap();
        assert_eq!(bp.segs[i].to, [100.0, 0.0, -1.0]);
        assert!(close(d, 0.5) && close(t, 0.4), "{d} {t}");
        // sur l'arc de centre (50,50) rayon 50 : point (50, 100)
        let (i, d, _) = nearest_cut(&bp, [50.0, 99.0], 2.0).unwrap();
        assert_eq!(bp.segs[i].kind, Kind::Arc);
        assert!((d - 1.0).abs() < 0.06, "{d}"); // l'arc est échantillonné à 0,05 mm près
        assert!(nearest_cut(&bp, [40.0, 20.0], 2.0).is_none());
    }

    #[test]
    fn nearest_cut_ignores_rapids_and_plunges() {
        let bp = parse_gcode("G0 X0 Y0 Z3\nG0 X50 Y0\nG1 Z-1 F100\n");
        assert!(nearest_cut(&bp, [25.0, 0.0], 5.0).is_none());
        assert!(nearest_cut(&bp, [50.0, 0.0], 5.0).is_none());
    }

    #[test]
    fn player_advances_stops_at_the_end_and_steps_by_segment() {
        let bp = parse_gcode("G0 X0 Y0 Z0\nG1 X10 F100\nG1 Y10\nG1 X0\n");
        let total = bp.total_len();
        let mut p = Player { pos: 0.0, playing: true, speed: 10.0 };
        p.advance(1.0, total);
        assert!(close(p.pos, 10.0) && p.playing);
        p.advance(100.0, total);
        assert!(close(p.pos, 30.0) && !p.playing, "{p:?}");
        // pause : ne bouge pas
        let mut q = Player { pos: 5.0, playing: false, speed: 10.0 };
        q.advance(5.0, total);
        assert!(close(q.pos, 5.0));
        // pas à pas
        let mut s = Player::default();
        s.step_segments(&bp, 1);
        assert!(close(s.pos, 10.0));
        s.step_segments(&bp, 1);
        assert!(close(s.pos, 20.0));
        s.step_segments(&bp, -1);
        assert!(close(s.pos, 10.0)); // début du segment précédent
        s.step_segments(&bp, 99);
        assert!(close(s.pos, 30.0));
    }

    #[test]
    fn solo_range_isolates_one_level() {
        let levels = [-1.0, -2.0, -3.0];
        let (lo, hi) = solo_range(&levels, 1).unwrap();
        assert!(lo < -2.0 && hi > -2.0 && hi < -1.9 && lo > -2.1);
        assert!(solo_range(&levels, 9).is_none());
    }

    #[test]
    fn bounds_of_an_empty_program_are_none() {
        assert!(bounds_xy(&parse_gcode("(rien)")).is_none());
        let (lo, hi) = bounds_xy(&parse_gcode("G0 X1 Y2 Z0\nG1 X5 Y9 F10\n")).unwrap();
        assert_eq!((lo, hi), ([1.0, 2.0], [5.0, 9.0]));
    }
}
