use super::{affine::Affine, len, numbers, path::parse_path, SvgImport, V};
use fablabcam_geom::{self as geom, PlineSource};

pub(super) fn import(
    n: roxmltree::Node<'_, '_>,
    t: Affine,
    c: f64,
    out: &mut SvgImport,
) -> Result<(), String> {
    let a = |key| n.attribute(key);
    let l = |key| len(a(key));
    let mut poly = |pts: Vec<V>, closed| -> Result<(), String> {
        if let Some(e) = t.polyline(pts, closed, c)? {
            out.shapes.push(("0".into(), e));
        }
        Ok(())
    };
    match n.tag_name().name() {
        "line" => poly(
            vec![(l("x1")?, l("y1")?, 0.0), (l("x2")?, l("y2")?, 0.0)],
            false,
        )?,
        "polyline" | "polygon" => {
            let values = numbers(a("points").unwrap_or(""))?;
            if values.len() % 2 != 0 {
                return Err("nombre impair de coordonnées SVG".into());
            }
            poly(
                values
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| (p[0], p[1], 0.0))
                    .collect(),
                n.tag_name().name() == "polygon",
            )?;
        }
        "rect" => {
            let (x, y, w, h) = (l("x")?, l("y")?, l("width")?, l("height")?);
            if w <= 0.0 || h <= 0.0 {
                return Ok(());
            }
            poly(
                vec![
                    (x, y, 0.0),
                    (x + w, y, 0.0),
                    (x + w, y + h, 0.0),
                    (x, y + h, 0.0),
                ],
                true,
            )?;
            if a("rx").is_some() || a("ry").is_some() {
                out.ignored.push("coins arrondis de <rect> ignorés".into());
            }
        }
        "path" => {
            for sub in parse_path(a("d").unwrap_or(""))? {
                poly(sub.pts, sub.closed)?;
            }
        }
        "circle" => {
            let (cx, cy, r) = (l("cx")?, l("cy")?, l("r")?);
            if r <= 0.0 {
                return Ok(());
            }
            if let Some(scale) = t.similarity_scale() {
                let (cx, cy) = t.point((cx, cy));
                let e = crate::shape::circle((cx, c - cy), r * scale).map_err(|e| e.to_string())?;
                out.shapes.push(("0".into(), e));
            } else {
                let p = geom::circle(cx, cy, r);
                poly(
                    p.iter_vertexes().map(|v| (v.x, v.y, v.bulge)).collect(),
                    true,
                )?;
            }
        }
        "ellipse" => {
            let (rx, ry) = (l("rx")?, l("ry")?);
            if rx <= 0.0 || ry <= 0.0 {
                return Ok(());
            }
            let pts = crate::curves::ellipse(
                (l("cx")?, l("cy")?),
                (rx, 0.0),
                ry / rx,
                0.0,
                std::f64::consts::TAU,
            );
            let n = pts.len() - 1;
            poly(
                pts.into_iter().take(n).map(|p| (p.0, p.1, 0.0)).collect(),
                true,
            )?;
        }
        _ => unreachable!(),
    }
    Ok(())
}
