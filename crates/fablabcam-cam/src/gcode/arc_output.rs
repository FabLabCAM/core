//! Decisions use the coordinates the controller will read, after formatting.
//! Keep valid circles; linearize arcs whose rounded center or endpoints collapse.
use crate::toolpath::Move;

pub(crate) fn degenerates(
    start: (f64, f64), end: (f64, f64), offset: (f64, f64),
    absolute_center: bool, round: impl Fn(f64) -> f64,
) -> bool {
    let a = (round(start.0), round(start.1));
    let b = (round(end.0), round(end.1));
    let c = if absolute_center {
        (round(start.0 + offset.0), round(start.1 + offset.1))
    } else {
        (a.0 + round(offset.0), a.1 + round(offset.1))
    };
    let distance = |p: (f64, f64), q: (f64, f64)| (p.0 - q.0).hypot(p.1 - q.1);
    distance(a, c) < 1e-9 || distance(b, c) < 1e-9
        || (distance(a, b) < 1e-9 && distance(start, end) >= 1e-9)
}

/// Sweep of the original arc, before any decimal rounding.
pub(crate) fn sweep(start: (f64, f64), end: (f64, f64), center: (f64, f64), ccw: bool) -> f64 {
    let a0 = (start.1 - center.1).atan2(start.0 - center.0);
    let a1 = (end.1 - center.1).atan2(end.0 - center.0);
    let d = if ccw { (a1 - a0).rem_euclid(std::f64::consts::TAU) } else { (a0 - a1).rem_euclid(std::f64::consts::TAU) };
    if d < 1e-9 { std::f64::consts::TAU } else { d }
}

/// Reuse the CBPP chord-error subdivision for both exporters. The last point,
/// feed and helical Z are preserved. Tolerance is in the path's output units;
/// subsequent decimal rounding adds at most half a grid step per coordinate.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lines(
    start: (f64, f64, f64), end: (f64, f64, Option<f64>), center: (f64, f64),
    radius: f64, sweep: f64, ccw: bool, feed: f64, tolerance: f64,
) -> Vec<Move> {
    let tol = tolerance.max(1e-9);
    let step = if tol >= radius { std::f64::consts::PI } else { 2. * (1. - tol / radius).acos() };
    let n = ((sweep / step).ceil() as usize).clamp(1, 100_000);
    let a0 = (start.1 - center.1).atan2(start.0 - center.0);
    let dir = if ccw { 1. } else { -1. };
    (1..=n).map(|k| {
        let t = k as f64 / n as f64;
        let (x, y) = if k == n { (end.0, end.1) } else {
            (center.0 + radius * (a0 + dir * sweep * t).cos(), center.1 + radius * (a0 + dir * sweep * t).sin())
        };
        Move::Feed { x: Some(x), y: Some(y), z: end.2.map(|z| start.2 + (z - start.2) * t), f: feed }
    }).collect()
}
