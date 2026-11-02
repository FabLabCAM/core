//! DXF ASCII : lecture de LINE, LWPOLYLINE (bulges), POLYLINE/VERTEX, CIRCLE, ARC, POINT ;
//! écriture en LWPOLYLINE, CIRCLE, ARC, POINT. Les autres entités sont signalées, pas perdues en silence.

use fablabcam_geom::{self as geom, PlineSource};
use fablabcam_model::Entity;

#[derive(Debug, Default)]
pub struct DxfImport {
    pub shapes: Vec<(String, Entity)>,
    /// Entités ignorées, au format « SPLINE (ligne 120) ».
    pub ignored: Vec<String>,
    /// Remarques (conversion d'unités…).
    pub notes: Vec<String>,
}

/// Valeur de `$INSUNITS` dans l'en-tête (0 = sans unité).
fn insunits(text: &str) -> Option<i64> {
    let l: Vec<&str> = text.lines().map(str::trim).collect();
    let i = l.iter().position(|v| *v == "$INSUNITS")?;
    l.get(i + 1..).and_then(|rest| rest.chunks(2).find(|c| c.first() == Some(&"70")).and_then(|c| c.get(1)?.parse().ok()))
}

/// Facteur vers le millimètre et nom de l'unité d'un code `$INSUNITS`.
fn unit_scale(code: i64) -> Option<(f64, &'static str)> {
    match code {
        1 => Some((25.4, "pouces")),
        2 => Some((304.8, "pieds")),
        4 => Some((1.0, "mm")),
        5 => Some((10.0, "cm")),
        6 => Some((1000.0, "m")),
        8 => Some((25.4e-6, "micro-pouces")),
        9 => Some((0.0254, "millièmes de pouce")),
        _ => None,
    }
}

struct Rec {
    kind: String,
    line: usize,
    codes: Vec<(i32, String)>,
}

impl Rec {
    fn get(&self, code: i32) -> Option<&str> {
        self.codes.iter().find(|(c, _)| *c == code).map(|(_, v)| v.as_str())
    }
    fn num(&self, code: i32) -> Option<f64> {
        self.get(code).and_then(|v| v.trim().parse().ok())
    }
    fn layer(&self) -> String {
        self.get(8).unwrap_or("0").to_string()
    }
}

fn records(text: &str) -> Result<Vec<Rec>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    let mut in_entities = false;
    let mut seen_entities = false;
    let mut out: Vec<Rec> = Vec::new();
    let mut prev_section_name = false;
    while i < lines.len() {
        let code_s = lines[i].trim();
        if code_s.is_empty() && i + 1 >= lines.len() {
            break;
        }
        let code: i32 = code_s.parse().map_err(|_| format!("ligne {} : code de groupe DXF invalide « {code_s} »", i + 1))?;
        let value = lines.get(i + 1).ok_or_else(|| format!("ligne {} : valeur manquante après le code {code}", i + 1))?.trim();
        let line_no = i + 1;
        i += 2;
        if code == 0 {
            match value {
                "SECTION" => prev_section_name = true,
                "ENDSEC" => in_entities = false,
                "EOF" => break,
                k if in_entities => out.push(Rec { kind: k.to_string(), line: line_no, codes: Vec::new() }),
                _ => {}
            }
            continue;
        }
        if prev_section_name && code == 2 {
            prev_section_name = false;
            in_entities = value == "ENTITIES";
            seen_entities |= in_entities;
            continue;
        }
        if in_entities {
            if let Some(r) = out.last_mut() {
                r.codes.push((code, value.to_string()));
            }
        }
    }
    if !seen_entities {
        return Err("section ENTITIES introuvable : ce n'est pas un DXF exploitable".into());
    }
    Ok(out)
}

/// Ajoute une polyligne de points (courbe approchée) ; une courbe fermée ne répète pas son premier point.
fn push_points(out: &mut DxfImport, layer: String, mut pts: Vec<(f64, f64)>, closed: bool) {
    if closed && pts.len() > 2 && (pts[0].0 - pts[pts.len() - 1].0).hypot(pts[0].1 - pts[pts.len() - 1].1) < 1e-9 {
        pts.pop();
    }
    if pts.len() >= 2 {
        let v: Vec<(f64, f64, f64)> = pts.into_iter().map(|(x, y)| (x, y, 0.0)).collect();
        out.shapes.push((layer, Entity::Polyline(geom::pline_from(&v, closed))));
    }
}

pub fn parse_dxf(text: &str) -> Result<DxfImport, String> {
    let recs = records(text)?;
    let mut out = DxfImport::default();
    let mut k = 0;
    while k < recs.len() {
        let r = &recs[k];
        k += 1;
        let need = |c: i32| r.num(c).ok_or_else(|| format!("ligne {} : {} sans code {c}", r.line, r.kind));
        match r.kind.as_str() {
            "LINE" => {
                let (a, b) = ((need(10)?, need(20)?), (need(11)?, need(21)?));
                if let Ok(e) = crate::shape::line(a, b) {
                    out.shapes.push((r.layer(), e));
                }
            }
            "CIRCLE" => {
                if let Ok(e) = crate::shape::circle((need(10)?, need(20)?), need(40)?) {
                    out.shapes.push((r.layer(), e));
                }
            }
            "ARC" => {
                let (s, e) = (need(50)?, need(51)?);
                let mut sweep = (e - s) % 360.0;
                if sweep <= 0.0 {
                    sweep += 360.0;
                }
                if let Ok(a) = crate::shape::arc((need(10)?, need(20)?), need(40)?, s, sweep) {
                    out.shapes.push((r.layer(), a));
                }
            }
            "POINT" => out.shapes.push((r.layer(), Entity::Point { x: need(10)?, y: need(20)? })),
            "LWPOLYLINE" => {
                let mut v: Vec<(f64, f64, f64)> = Vec::new();
                for (c, val) in &r.codes {
                    if !matches!(c, 10 | 20 | 42) {
                        continue;
                    }
                    let n: f64 = val.parse().map_err(|_| format!("ligne {} : nombre invalide « {val} »", r.line))?;
                    match c {
                        10 => v.push((n, 0.0, 0.0)),
                        20 => {
                            if let Some(l) = v.last_mut() { l.1 = n }
                        }
                        42 => {
                            if let Some(l) = v.last_mut() { l.2 = n }
                        }
                        _ => {}
                    }
                }
                let closed = r.num(70).is_some_and(|f| (f as i64) & 1 == 1);
                if v.len() >= 2 {
                    out.shapes.push((r.layer(), Entity::Polyline(geom::pline_from(&v, closed))));
                }
            }
            "POLYLINE" => {
                let closed = r.num(70).is_some_and(|f| (f as i64) & 1 == 1);
                let mut v = Vec::new();
                while k < recs.len() && recs[k].kind == "VERTEX" {
                    let q = &recs[k];
                    v.push((q.num(10).unwrap_or(0.0), q.num(20).unwrap_or(0.0), q.num(42).unwrap_or(0.0)));
                    k += 1;
                }
                if k < recs.len() && recs[k].kind == "SEQEND" {
                    k += 1;
                }
                if v.len() >= 2 {
                    out.shapes.push((r.layer(), Entity::Polyline(geom::pline_from(&v, closed))));
                }
            }
            "ELLIPSE" => {
                let c = (need(10)?, need(20)?);
                let major = (need(11)?, need(21)?);
                let (t0, t1) = (r.num(41).unwrap_or(0.0), r.num(42).unwrap_or(std::f64::consts::TAU));
                let full = crate::curves::is_full(t0, t1);
                let pts = crate::curves::ellipse(c, major, need(40)?, t0, t1);
                push_points(&mut out, r.layer(), pts, full);
            }
            "SPLINE" => {
                let flags = r.num(70).unwrap_or(0.0) as i64;
                let degree = r.num(71).unwrap_or(3.0) as usize;
                let all = |code: i32| -> Result<Vec<f64>, String> {
                    r.codes.iter().filter(|(c, _)| *c == code).map(|(_, v)| v.trim().parse::<f64>().map_err(|_| format!("ligne {} : nombre invalide « {v} »", r.line))).collect()
                };
                let (knots, xs, ys, ws) = (all(40)?, all(10)?, all(20)?, all(41)?);
                let ctrl: Vec<(f64, f64, f64)> = xs.iter().zip(&ys).enumerate().map(|(i, (x, y))| (*x, *y, ws.get(i).copied().unwrap_or(1.0))).collect();
                let pts = if ctrl.is_empty() {
                    // seulement des points d'ajustement : la courbe passe par eux, approchée par la polyligne
                    let (fx, fy) = (all(11)?, all(21)?);
                    out.ignored.push(format!("SPLINE (ligne {}) : sans points de contrôle, approchée par ses points d'ajustement", r.line));
                    fx.into_iter().zip(fy).collect()
                } else {
                    crate::curves::bspline(degree, &knots, &ctrl).map_err(|e| format!("ligne {} : {e}", r.line))?
                };
                push_points(&mut out, r.layer(), pts, flags & 1 == 1);
            }
            "VERTEX" | "SEQEND" => {}
            other => out.ignored.push(format!("{other} (ligne {})", r.line)),
        }
    }
    // le dessin interne est en mm : conversion selon l'unité déclarée dans l'en-tête
    match insunits(text).map(|c| (c, unit_scale(c))) {
        Some((_, Some((k, name)))) if (k - 1.0).abs() > 1e-12 => {
            let t = crate::cad::Similarity::scale((0.0, 0.0), k).map_err(|e| e.to_string())?;
            for (_, e) in &mut out.shapes {
                *e = crate::cad::transform(e, &t);
            }
            out.notes.push(format!("DXF en {name} : converti en mm (× {k})"));
        }
        Some((c, None)) if c != 0 => out.notes.push(format!("unité DXF inconnue ($INSUNITS = {c}) : valeurs lues comme des mm")),
        _ => {}
    }
    Ok(out)
}

fn n(v: f64) -> String {
    let s = format!("{v:.9}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" { "0".into() } else { s.into() }
}

pub fn write_dxf(shapes: &[(String, Entity)]) -> String {
    let mut s = String::from("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n");
    for (layer, e) in shapes {
        // DXF ne connaît pas le rectangle paramétrique : il sort comme polyligne
        let flat;
        let e = match e.to_pline() {
            Some(p) if matches!(e, Entity::Rect { .. }) => {
                flat = Entity::Polyline(p);
                &flat
            }
            _ => e,
        };
        match e {
            Entity::Polyline(p) => {
                s += &format!("0\nLWPOLYLINE\n8\n{layer}\n90\n{}\n70\n{}\n", p.vertex_count(), u8::from(p.is_closed()));
                for i in 0..p.vertex_count() {
                    let v = p.at(i);
                    s += &format!("10\n{}\n20\n{}\n", n(v.x), n(v.y));
                    if v.bulge.abs() > 1e-12 {
                        s += &format!("42\n{}\n", n(v.bulge));
                    }
                }
            }
            Entity::Circle { cx, cy, r } => s += &format!("0\nCIRCLE\n8\n{layer}\n10\n{}\n20\n{}\n40\n{}\n", n(*cx), n(*cy), n(*r)),
            Entity::Arc { cx, cy, r, start_deg, sweep_deg } => {
                // DXF ne connaît que le sens anti-horaire : un balayage négatif échange début et fin
                let (a, b) = if *sweep_deg >= 0.0 { (*start_deg, start_deg + sweep_deg) } else { (start_deg + sweep_deg, *start_deg) };
                s += &format!("0\nARC\n8\n{layer}\n10\n{}\n20\n{}\n40\n{}\n50\n{}\n51\n{}\n", n(*cx), n(*cy), n(*r), n(a.rem_euclid(360.0)), n(b.rem_euclid(360.0)));
            }
            Entity::Point { x, y } => s += &format!("0\nPOINT\n8\n{layer}\n10\n{}\n20\n{}\n", n(*x), n(*y)),
            Entity::Rect { .. } => {}
        }
    }
    s += "0\nENDSEC\n0\nEOF\n";
    s
}
