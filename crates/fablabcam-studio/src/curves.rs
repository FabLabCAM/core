//! Courbes approchées par des polylignes : ellipses et B-splines (rationnelles ou non), pour les imports DXF
//! (`ELLIPSE`, `SPLINE`) et SVG (`<ellipse>`). L'écart entre la courbe et la polyligne est d'environ `TOL`.

use std::f64::consts::TAU;

/// Écart visé entre courbe et polyligne (mm).
pub const TOL: f64 = 0.01;

/// Nombre de segments pour un arc de rayon `r` balayant `sweep` radians, à `TOL` près.
fn segments_for(r: f64, sweep: f64) -> usize {
    let step = if r <= TOL { std::f64::consts::PI } else { 2.0 * (1.0 - TOL / r).acos() };
    ((sweep.abs() / step).ceil() as usize).clamp(8, 4000)
}

/// Points d'une ellipse de centre `c`, de demi-grand axe `major` (vecteur), de rapport petit/grand axe `ratio`,
/// entre les paramètres `t0` et `t1` (radians, sens trigonométrique). Le petit axe est `major` tourné de +90°.
pub fn ellipse(c: (f64, f64), major: (f64, f64), ratio: f64, t0: f64, t1: f64) -> Vec<(f64, f64)> {
    let mut t1 = t1;
    while t1 <= t0 + 1e-12 {
        t1 += TAU;
    }
    let minor = (-major.1 * ratio, major.0 * ratio);
    let r = major.0.hypot(major.1);
    let n = segments_for(r, t1 - t0);
    (0..=n)
        .map(|k| {
            let t = t0 + (t1 - t0) * k as f64 / n as f64;
            let (s, co) = t.sin_cos();
            (c.0 + major.0 * co + minor.0 * s, c.1 + major.1 * co + minor.1 * s)
        })
        .collect()
}

/// Ellipse complète ?
pub fn is_full(t0: f64, t1: f64) -> bool {
    let mut d = (t1 - t0).rem_euclid(TAU);
    if d < 1e-9 {
        d = TAU;
    }
    (d - TAU).abs() < 1e-6
}

/// Point d'une B-spline de degré `p` au paramètre `u` (algorithme de de Boor, coordonnées homogènes).
fn de_boor(p: usize, knots: &[f64], ctrl: &[(f64, f64, f64)], u: f64) -> (f64, f64) {
    let n = ctrl.len();
    // intervalle k tel que knots[k] <= u < knots[k+1], borné aux intervalles utiles
    let mut k = p;
    while k + 1 < n && u >= knots[k + 1] {
        k += 1;
    }
    let mut d: Vec<(f64, f64, f64)> = (0..=p).map(|j| {
        let (x, y, w) = ctrl[j + k - p];
        (x * w, y * w, w)
    }).collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = j + k - p;
            let den = knots[i + p + 1 - r] - knots[i];
            let a = if den.abs() < 1e-15 { 0.0 } else { (u - knots[i]) / den };
            d[j] = ((1.0 - a) * d[j - 1].0 + a * d[j].0, (1.0 - a) * d[j - 1].1 + a * d[j].1, (1.0 - a) * d[j - 1].2 + a * d[j].2);
        }
    }
    let (x, y, w) = d[p];
    if w.abs() < 1e-15 { (x, y) } else { (x / w, y / w) }
}

/// Points d'une B-spline : `ctrl` = (x, y, poids). Renvoie une erreur si les nœuds ne correspondent pas.
pub fn bspline(degree: usize, knots: &[f64], ctrl: &[(f64, f64, f64)]) -> Result<Vec<(f64, f64)>, String> {
    let (n, p) = (ctrl.len(), degree);
    if p == 0 || n <= p {
        return Err(format!("spline de degré {p} avec {n} points de contrôle"));
    }
    if knots.len() != n + p + 1 {
        return Err(format!("spline : {} nœuds pour {n} points de contrôle de degré {p} (attendu {})", knots.len(), n + p + 1));
    }
    if knots.windows(2).any(|w| w[1] < w[0]) {
        return Err("spline : nœuds non croissants".into());
    }
    let (u0, u1) = (knots[p], knots[n]);
    // densité : longueur du polygone de contrôle / TOL, bornée
    let poly_len: f64 = ctrl.windows(2).map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1)).sum();
    let samples = ((poly_len / (20.0 * TOL)).sqrt().ceil() as usize * 8).clamp(16 * (n - p), 4000);
    let mut pts: Vec<(f64, f64)> = (0..=samples).map(|k| de_boor(p, knots, ctrl, u0 + (u1 - u0) * k as f64 / samples as f64)).collect();
    pts.dedup_by(|a, b| (a.0 - b.0).hypot(a.1 - b.1) < 1e-9);
    Ok(pts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ellipse_sur_ses_axes() {
        let pts = ellipse((1.0, 2.0), (4.0, 0.0), 0.5, 0.0, TAU);
        assert!(pts.iter().all(|(x, y)| (((x - 1.0) / 4.0).powi(2) + ((y - 2.0) / 2.0).powi(2) - 1.0).abs() < 1e-9));
        assert!(is_full(0.0, TAU) && !is_full(0.0, 1.0));
        let q = ellipse((0.0, 0.0), (0.0, 3.0), 1.0, 0.0, std::f64::consts::FRAC_PI_2);
        // grand axe vertical : départ en (0,3), quart de tour vers (-3,0)
        assert!((q[0].1 - 3.0).abs() < 1e-12 && (q.last().unwrap().0 + 3.0).abs() < 1e-9);
    }

    #[test]
    fn spline_de_bezier_et_cercle_rationnel() {
        // Bézier quadratique (0,0) (1,2) (2,0) : sommet en (1, 1)
        let pts = bspline(2, &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0], &[(0.0, 0.0, 1.0), (1.0, 2.0, 1.0), (2.0, 0.0, 1.0)]).unwrap();
        assert_eq!(pts[0], (0.0, 0.0));
        assert!((pts.last().unwrap().0 - 2.0).abs() < 1e-12);
        let top = pts.iter().fold(0.0f64, |m, p| m.max(p.1));
        assert!((top - 1.0).abs() < 1e-3, "{top}");
        // quart de cercle rationnel : poids √2/2 au point du milieu
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let arc = bspline(2, &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0], &[(1.0, 0.0, 1.0), (1.0, 1.0, w), (0.0, 1.0, 1.0)]).unwrap();
        assert!(arc.iter().all(|(x, y)| (x.hypot(*y) - 1.0).abs() < 1e-9));
        assert!(bspline(3, &[0.0; 4], &[(0.0, 0.0, 1.0); 4]).is_err());
    }
}
