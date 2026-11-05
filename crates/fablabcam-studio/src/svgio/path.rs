use super::{
    curves::{arc_to, flatten_cubic},
    V,
};
use svgtypes::{PathParser, PathSegment as S};

pub(super) struct Sub {
    pub pts: Vec<V>,
    pub closed: bool,
}

fn flush(pts: &mut Vec<V>, closed: bool, subs: &mut Vec<Sub>) {
    if closed && pts.len() > 1 {
        let (f, l) = (pts[0], *pts.last().unwrap());
        if (f.0 - l.0).hypot(f.1 - l.1) < 1e-9 {
            // The previous vertex owns the closing arc. Removing its endpoint
            // must not overwrite that bulge with the endpoint's zero bulge.
            pts.pop();
        }
    }
    if pts.len() >= 2 {
        subs.push(Sub {
            pts: std::mem::take(pts),
            closed,
        });
    }
    pts.clear();
}

/// Parse the complete path before exposing any of its contours to the caller.
pub(super) fn parse_path(d: &str) -> Result<Vec<Sub>, String> {
    let mut subs = Vec::new();
    let mut pts = Vec::new();
    let (mut cur, mut start) = ((0.0, 0.0), (0.0, 0.0));
    let (mut cubic_ctrl, mut quad_ctrl) = (None, None);
    for segment in PathParser::from(d) {
        let segment = segment.map_err(|e| format!("chemin SVG invalide : {e}"))?;
        let point = |x, y| {
            if segment.is_abs() {
                (x, y)
            } else {
                (cur.0 + x, cur.1 + y)
            }
        };
        let (mut next_cubic, mut next_quad) = (None, None);
        match segment {
            S::MoveTo { x, y, .. } => {
                let p = point(x, y);
                flush(&mut pts, false, &mut subs);
                cur = p;
                start = cur;
                pts.push((cur.0, cur.1, 0.0));
            }
            S::LineTo { x, y, .. } => {
                cur = point(x, y);
                pts.push((cur.0, cur.1, 0.0));
            }
            S::HorizontalLineTo { x, .. } => {
                cur.0 = point(x, 0.0).0;
                pts.push((cur.0, cur.1, 0.0));
            }
            S::VerticalLineTo { y, .. } => {
                cur.1 = point(0.0, y).1;
                pts.push((cur.0, cur.1, 0.0));
            }
            S::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
                ..
            } => {
                let (p1, p2, end) = (point(x1, y1), point(x2, y2), point(x, y));
                flatten_cubic(cur, p1, p2, end, &mut pts);
                cur = end;
                next_cubic = Some(p2);
            }
            S::SmoothCurveTo { x2, y2, x, y, .. } => {
                let p1 = reflect(cubic_ctrl, cur);
                let (p2, end) = (point(x2, y2), point(x, y));
                flatten_cubic(cur, p1, p2, end, &mut pts);
                cur = end;
                next_cubic = Some(p2);
            }
            S::Quadratic { x1, y1, x, y, .. } => {
                let (q, end) = (point(x1, y1), point(x, y));
                quadratic(cur, q, end, &mut pts);
                cur = end;
                next_quad = Some(q);
            }
            S::SmoothQuadratic { x, y, .. } => {
                let (q, end) = (reflect(quad_ctrl, cur), point(x, y));
                quadratic(cur, q, end, &mut pts);
                cur = end;
                next_quad = Some(q);
            }
            S::EllipticalArc {
                rx,
                ry,
                x_axis_rotation,
                large_arc,
                sweep,
                x,
                y,
                ..
            } => {
                let end = point(x, y);
                arc_to(
                    cur,
                    rx,
                    ry,
                    x_axis_rotation,
                    large_arc,
                    sweep,
                    end,
                    &mut pts,
                );
                cur = end;
            }
            S::ClosePath { .. } => {
                flush(&mut pts, true, &mut subs);
                cur = start;
                pts.push((cur.0, cur.1, 0.0));
            }
        }
        cubic_ctrl = next_cubic;
        quad_ctrl = next_quad;
    }
    flush(&mut pts, false, &mut subs);
    Ok(subs)
}

fn reflect(ctrl: Option<(f64, f64)>, cur: (f64, f64)) -> (f64, f64) {
    ctrl.map_or(cur, |p| (2.0 * cur.0 - p.0, 2.0 * cur.1 - p.1))
}

fn quadratic(cur: (f64, f64), q: (f64, f64), end: (f64, f64), pts: &mut Vec<V>) {
    let c1 = (
        cur.0 + 2.0 / 3.0 * (q.0 - cur.0),
        cur.1 + 2.0 / 3.0 * (q.1 - cur.1),
    );
    let c2 = (
        end.0 + 2.0 / 3.0 * (q.0 - end.0),
        end.1 + 2.0 / 3.0 * (q.1 - end.1),
    );
    flatten_cubic(cur, c1, c2, end, pts);
}
