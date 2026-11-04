use super::{panels_with_cutter, Furniture, Panel};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Placement {
    pub panel: Panel,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub rotated: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct Sheet {
    pub placements: Vec<Placement>,
    pub occupied_ratio: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct Layout {
    pub sheets: Vec<Sheet>,
}
#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Deterministic first-fit decreasing with guillotine free rectangles.
/// Returns no partial result: an unplaceable panel prevents a complete dossier.
pub fn place(job: &Furniture) -> Result<Layout, String> {
    job.check()?;
    let setup = &job.setup;
    let mut parts = panels_with_cutter(&job.template, &job.dimensions, job.setup.cutter)?;
    parts.sort_by(|a, b| {
        (b.width * b.height)
            .total_cmp(&(a.width * a.height))
            .then(a.id.cmp(&b.id))
    });
    let [w, h] = setup.sheet_size;
    let m = setup.margin;
    let empty = Rect {
        x: m,
        y: m,
        w: w - 2.0 * m,
        h: h - 2.0 * m,
    };
    let mut free: Vec<Vec<Rect>> = Vec::new();
    let mut sheets: Vec<Sheet> = Vec::new();
    for panel in parts {
        let fits = |r: Rect, rot: bool| {
            let (pw, ph) = if rot {
                (panel.height, panel.width)
            } else {
                (panel.width, panel.height)
            };
            pw <= r.w + 1e-8 && ph <= r.h + 1e-8
        };
        if !fits(empty, false) && !(setup.rotate && fits(empty, true)) {
            return Err(format!(
                "Pièce {} ({} × {} mm) non plaçable dans la zone utile {} × {} mm",
                panel.id, panel.width, panel.height, empty.w, empty.h
            ));
        }
        let mut selected = None;
        for (si, rects) in free.iter().enumerate() {
            if (sheets[si].placements[0].panel.thickness - panel.thickness).abs() > 1e-8 {
                continue;
            }
            for (ri, &r) in rects.iter().enumerate() {
                for rot in [false, true]
                    .into_iter()
                    .take(if setup.rotate { 2 } else { 1 })
                {
                    if fits(r, rot) {
                        selected = Some((si, ri, rot));
                        break;
                    }
                }
                if selected.is_some() {
                    break;
                }
            }
            if selected.is_some() {
                break;
            }
        }
        let (si, ri, rot) = selected.unwrap_or_else(|| {
            free.push(vec![empty]);
            sheets.push(Sheet {
                placements: Vec::new(),
                occupied_ratio: 0.0,
            });
            (free.len() - 1, 0, !fits(empty, false))
        });
        let r = free[si].remove(ri);
        let (pw, ph) = if rot {
            (panel.height, panel.width)
        } else {
            (panel.width, panel.height)
        };
        let gap = setup.spacing;
        // Disjoint strips: no overlapping free rectangles or hidden double placements.
        if r.w > pw + gap {
            free[si].push(Rect {
                x: r.x + pw + gap,
                y: r.y,
                w: r.w - pw - gap,
                h: ph,
            });
        }
        if r.h > ph + gap {
            free[si].push(Rect {
                x: r.x,
                y: r.y + ph + gap,
                w: r.w,
                h: r.h - ph - gap,
            });
        }
        let silhouette = super::panel_entity(&panel)
            .to_pline()
            .expect("panel outline");
        let holes = panel
            .features
            .iter()
            .filter(|f| f.through && !f.in_outline)
            .map(|f| {
                let p = fablabcam_geom::pline_from(
                    &f.contour
                        .iter()
                        .map(|v| (v[0], v[1], v[2]))
                        .collect::<Vec<_>>(),
                    true,
                );
                fablabcam_geom::area(&p)
            })
            .sum::<f64>();
        sheets[si].occupied_ratio += (fablabcam_geom::area(&silhouette) - holes).max(0.0) / (w * h);
        sheets[si].placements.push(Placement {
            panel,
            x: r.x,
            y: r.y,
            width: pw,
            height: ph,
            rotated: rot,
        });
    }
    Ok(Layout { sheets })
}
