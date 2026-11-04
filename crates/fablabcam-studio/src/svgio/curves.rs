use super::V;

pub(super) fn flatten_cubic(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    out: &mut Vec<V>,
) {
    let n = 24;
    for i in 1..=n {
        let t = i as f64 / n as f64;
        let u = 1.0 - t;
        let x =
            u * u * u * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t * t * t * p3.0;
        let y =
            u * u * u * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t * t * t * p3.1;
        out.push((x, y, 0.0));
    }
}

/// Arc elliptique SVG → points (ou bulge si circulaire). Coordonnées SVG (Y vers le bas).
#[allow(clippy::too_many_arguments)]
pub(super) fn arc_to(
    cur: (f64, f64),
    rx: f64,
    ry: f64,
    phi_deg: f64,
    large: bool,
    sweep: bool,
    end: (f64, f64),
    pts: &mut Vec<V>,
) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-12 || ry < 1e-12 || (cur.0 - end.0).hypot(cur.1 - end.1) < 1e-12 {
        pts.push((end.0, end.1, 0.0));
        return;
    }
    let phi = phi_deg.to_radians();
    let (cp, sp) = (phi.cos(), phi.sin());
    let (dx, dy) = ((cur.0 - end.0) / 2.0, (cur.1 - end.1) / 2.0);
    let (x1, y1) = (cp * dx + sp * dy, -sp * dx + cp * dy);
    let lam = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lam > 1.0 {
        rx *= lam.sqrt();
        ry *= lam.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let co = if den.abs() < 1e-18 {
        0.0
    } else {
        (num / den).max(0.0).sqrt() * if large == sweep { -1.0 } else { 1.0 }
    };
    let (cxp, cyp) = (co * rx * y1 / ry, -co * ry * x1 / rx);
    let (cx, cy) = (
        cp * cxp - sp * cyp + (cur.0 + end.0) / 2.0,
        sp * cxp + cp * cyp + (cur.1 + end.1) / 2.0,
    );
    let ang = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let th1 = ang(1.0, 0.0, (x1 - cxp) / rx, (y1 - cyp) / ry);
    let mut dth = ang(
        (x1 - cxp) / rx,
        (y1 - cyp) / ry,
        (-x1 - cxp) / rx,
        (-y1 - cyp) / ry,
    );
    if !sweep && dth > 0.0 {
        dth -= std::f64::consts::TAU;
    } else if sweep && dth < 0.0 {
        dth += std::f64::consts::TAU;
    }
    if (rx - ry).abs() < 1e-9 * rx.max(1.0) {
        if let Some(l) = pts.last_mut() {
            l.2 = (dth / 4.0).tan(); // bulge en repère SVG ; l'inversion de Y est faite à la fin
        }
        pts.push((end.0, end.1, 0.0));
    } else {
        let n = ((dth.abs() / 0.1).ceil() as usize).max(4);
        for i in 1..=n {
            let t = th1 + dth * i as f64 / n as f64;
            pts.push((
                cx + rx * t.cos() * cp - ry * t.sin() * sp,
                cy + rx * t.cos() * sp + ry * t.sin() * cp,
                0.0,
            ));
        }
    }
}
