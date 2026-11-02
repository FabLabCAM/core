//! SVG : formes de base et chemins (M L H V C S Q A Z, relatifs ou absolus) en entrée ; chemins et
//! cercles en sortie. L'axe Y du SVG est inversé : `y_doc = C − y_svg`, avec `C = vb_y + vb_h`
//! (ou l'attribut `data-doc-ymax` écrit par notre export).

use fablabcam_geom::{self as geom, PlineSource};
use fablabcam_model::Entity;

#[derive(Debug, Default)]
pub struct SvgImport {
    pub shapes: Vec<(String, Entity)>,
    pub ignored: Vec<String>,
}

fn len(s: Option<&str>) -> f64 {
    let s = s.unwrap_or("0").trim();
    let end = s.find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))).unwrap_or(s.len());
    s[..end].parse().unwrap_or(0.0)
}

fn numbers(s: &str) -> Vec<f64> {
    s.split(|c: char| c == ',' || c.is_whitespace()).filter(|t| !t.is_empty()).filter_map(|t| t.parse().ok()).collect()
}

type V = (f64, f64, f64);

struct Sub {
    pts: Vec<V>,
    closed: bool,
}

fn flatten_cubic(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), out: &mut Vec<V>) {
    let n = 24;
    for i in 1..=n {
        let t = i as f64 / n as f64;
        let u = 1.0 - t;
        let x = u * u * u * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t * t * t * p3.0;
        let y = u * u * u * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t * t * t * p3.1;
        out.push((x, y, 0.0));
    }
}

/// Arc elliptique SVG → points (ou bulge si circulaire). Coordonnées SVG (Y vers le bas).
#[allow(clippy::too_many_arguments)]
fn arc_to(cur: (f64, f64), rx: f64, ry: f64, phi_deg: f64, large: bool, sweep: bool, end: (f64, f64), pts: &mut Vec<V>) {
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
    let co = if den.abs() < 1e-18 { 0.0 } else { (num / den).max(0.0).sqrt() * if large == sweep { -1.0 } else { 1.0 } };
    let (cxp, cyp) = (co * rx * y1 / ry, -co * ry * x1 / rx);
    let (cx, cy) = (cp * cxp - sp * cyp + (cur.0 + end.0) / 2.0, sp * cxp + cp * cyp + (cur.1 + end.1) / 2.0);
    let ang = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let th1 = ang(1.0, 0.0, (x1 - cxp) / rx, (y1 - cyp) / ry);
    let mut dth = ang((x1 - cxp) / rx, (y1 - cyp) / ry, (-x1 - cxp) / rx, (-y1 - cyp) / ry);
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
            pts.push((cx + rx * t.cos() * cp - ry * t.sin() * sp, cy + rx * t.cos() * sp + ry * t.sin() * cp, 0.0));
        }
    }
}

fn parse_path(d: &str) -> Vec<Sub> {
    let mut toks: Vec<(char, Vec<f64>)> = Vec::new();
    let b: Vec<char> = d.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_alphabetic() {
            let c = b[i];
            i += 1;
            let mut s = String::new();
            while i < b.len() && !b[i].is_ascii_alphabetic() {
                s.push(b[i]);
                i += 1;
            }
            // « 10-5 » et « 1.5.5 » : les signes et seconds points séparent les nombres
            let mut nums = Vec::new();
            let mut cur = String::new();
            let mut dot = false;
            for ch in s.chars() {
                let split = matches!(ch, ',' | ' ' | '\t' | '\n' | '\r') || (ch == '-' && !cur.is_empty() && !cur.ends_with(['e', 'E'])) || (ch == '.' && dot);
                if split {
                    if let Ok(v) = cur.parse::<f64>() { nums.push(v) }
                    cur.clear();
                    dot = false;
                }
                if matches!(ch, ',' | ' ' | '\t' | '\n' | '\r') {
                    continue;
                }
                if ch == '.' { dot = true }
                cur.push(ch);
            }
            if let Ok(v) = cur.parse::<f64>() { nums.push(v) }
            toks.push((c, nums));
        } else {
            i += 1;
        }
    }
    let mut subs: Vec<Sub> = Vec::new();
    let (mut cur, mut start) = ((0.0, 0.0), (0.0, 0.0));
    let mut last_ctrl: Option<(f64, f64)> = None;
    let mut pts: Vec<V> = Vec::new();
    let mut closed = false;
    let flush = |pts: &mut Vec<V>, closed: &mut bool, subs: &mut Vec<Sub>| {
        if pts.len() >= 2 {
            subs.push(Sub { pts: std::mem::take(pts), closed: *closed });
        }
        pts.clear();
        *closed = false;
    };
    for (c, a) in toks {
        let rel = c.is_ascii_lowercase();
        let abs = |p: (f64, f64), cur: (f64, f64)| if rel { (cur.0 + p.0, cur.1 + p.1) } else { p };
        match c.to_ascii_uppercase() {
            'M' => {
                for (k, ch) in a.chunks(2).enumerate().filter(|(_, ch)| ch.len() == 2) {
                    let p = abs((ch[0], ch[1]), cur);
                    if k == 0 {
                        flush(&mut pts, &mut closed, &mut subs);
                        pts.push((p.0, p.1, 0.0));
                        start = p;
                    } else {
                        pts.push((p.0, p.1, 0.0));
                    }
                    cur = p;
                }
                last_ctrl = None;
            }
            'L' => {
                for ch in a.chunks(2).filter(|ch| ch.len() == 2) {
                    cur = abs((ch[0], ch[1]), cur);
                    pts.push((cur.0, cur.1, 0.0));
                }
                last_ctrl = None;
            }
            'H' => {
                for v in a {
                    cur.0 = if rel { cur.0 + v } else { v };
                    pts.push((cur.0, cur.1, 0.0));
                }
                last_ctrl = None;
            }
            'V' => {
                for v in a {
                    cur.1 = if rel { cur.1 + v } else { v };
                    pts.push((cur.0, cur.1, 0.0));
                }
                last_ctrl = None;
            }
            'C' | 'S' => {
                let step = if c.eq_ignore_ascii_case(&'C') { 6 } else { 4 };
                for ch in a.chunks(step).filter(|ch| ch.len() == step) {
                    let (p1, p2, p3) = if step == 6 {
                        (abs((ch[0], ch[1]), cur), abs((ch[2], ch[3]), cur), abs((ch[4], ch[5]), cur))
                    } else {
                        let r = last_ctrl.map_or(cur, |l| (2.0 * cur.0 - l.0, 2.0 * cur.1 - l.1));
                        (r, abs((ch[0], ch[1]), cur), abs((ch[2], ch[3]), cur))
                    };
                    flatten_cubic(cur, p1, p2, p3, &mut pts);
                    last_ctrl = Some(p2);
                    cur = p3;
                }
            }
            'Q' => {
                for ch in a.chunks(4).filter(|ch| ch.len() == 4) {
                    let (q, p3) = (abs((ch[0], ch[1]), cur), abs((ch[2], ch[3]), cur));
                    let c1 = (cur.0 + 2.0 / 3.0 * (q.0 - cur.0), cur.1 + 2.0 / 3.0 * (q.1 - cur.1));
                    let c2 = (p3.0 + 2.0 / 3.0 * (q.0 - p3.0), p3.1 + 2.0 / 3.0 * (q.1 - p3.1));
                    flatten_cubic(cur, c1, c2, p3, &mut pts);
                    cur = p3;
                }
                last_ctrl = None;
            }
            'A' => {
                for ch in a.chunks(7).filter(|ch| ch.len() == 7) {
                    let end = abs((ch[5], ch[6]), cur);
                    arc_to(cur, ch[0], ch[1], ch[2], ch[3] != 0.0, ch[4] != 0.0, end, &mut pts);
                    cur = end;
                }
                last_ctrl = None;
            }
            'Z' => {
                closed = true;
                // un dernier point égal au premier est redondant dans une polyligne fermée
                if pts.len() > 1 {
                    let (f, l) = (pts[0], *pts.last().unwrap());
                    if (f.0 - l.0).hypot(f.1 - l.1) < 1e-9 {
                        let b = l.2;
                        pts.pop();
                        if let Some(x) = pts.last_mut() { x.2 = b }
                    }
                }
                flush(&mut pts, &mut closed, &mut subs);
                closed = false;
                cur = start;
                pts.push((cur.0, cur.1, 0.0));
                last_ctrl = None;
            }
            _ => {}
        }
    }
    flush(&mut pts, &mut closed, &mut subs);
    subs
}

pub fn parse_svg(text: &str) -> Result<SvgImport, String> {
    let doc = roxmltree::Document::parse(text).map_err(|e| format!("SVG invalide : {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg" {
        return Err("l'élément racine n'est pas <svg>".into());
    }
    let vb: Vec<f64> = root.attribute("viewBox").map(numbers).unwrap_or_default();
    let c = root
        .attribute("data-doc-ymax")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or_else(|| if vb.len() == 4 { vb[1] + vb[3] } else { len(root.attribute("height")) });
    let mut out = SvgImport::default();
    let push_poly = |out: &mut SvgImport, pts: Vec<V>, closed: bool| {
        let v: Vec<V> = pts.iter().map(|p| (p.0, c - p.1, -p.2)).collect();
        // doublons consécutifs : le moteur géométrique n'en veut pas
        let mut w: Vec<V> = Vec::new();
        for p in v {
            if w.last().map_or(true, |l| (l.0 - p.0).hypot(l.1 - p.1) > 1e-9) {
                w.push(p);
            } else if let Some(l) = w.last_mut() {
                l.2 = p.2.max(l.2);
            }
        }
        if w.len() >= 2 && (!closed || w.len() >= 3) {
            out.shapes.push(("0".into(), Entity::Polyline(geom::pline_from(&w, closed))));
        }
    };
    for n in root.descendants().filter(|n| n.is_element()) {
        let tag = n.tag_name().name();
        let a = |k: &str| n.attribute(k);
        if n.attribute("transform").is_some() && tag != "svg" {
            out.ignored.push(format!("transform de <{tag}> ignoré"));
        }
        match tag {
            "line" => push_poly(&mut out, vec![(len(a("x1")), len(a("y1")), 0.0), (len(a("x2")), len(a("y2")), 0.0)], false),
            "polyline" | "polygon" => {
                let nums = numbers(a("points").unwrap_or(""));
                let pts: Vec<V> = nums.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1], 0.0)).collect();
                push_poly(&mut out, pts, tag == "polygon");
            }
            "rect" => {
                let (x, y, w, h) = (len(a("x")), len(a("y")), len(a("width")), len(a("height")));
                if a("rx").is_some() || a("ry").is_some() {
                    out.ignored.push("coins arrondis de <rect> ignorés".into());
                }
                push_poly(&mut out, vec![(x, y, 0.0), (x + w, y, 0.0), (x + w, y + h, 0.0), (x, y + h, 0.0)], true);
            }
            "circle" => {
                if let Ok(e) = crate::shape::circle((len(a("cx")), c - len(a("cy"))), len(a("r"))) {
                    out.shapes.push(("0".into(), e));
                }
            }
            "path" => {
                for s in parse_path(a("d").unwrap_or("")) {
                    push_poly(&mut out, s.pts, s.closed);
                }
            }
            "ellipse" => {
                let (rx, ry) = (len(a("rx")), len(a("ry")));
                if rx > 0.0 && ry > 0.0 {
                    // coordonnées SVG brutes : `push_poly` retourne l'axe Y comme pour les autres formes
                    let pts = crate::curves::ellipse((len(a("cx")), len(a("cy"))), (rx, 0.0), ry / rx, 0.0, std::f64::consts::TAU);
                    let n = pts.len() - 1; // le dernier point répète le premier
                    push_poly(&mut out, pts.into_iter().take(n).map(|(x, y)| (x, y, 0.0)).collect(), true);
                }
            }
            "text" | "image" | "use" => out.ignored.push(format!("<{tag}> ignoré")),
            _ => {}
        }
    }
    Ok(out)
}

fn f(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" { "0".into() } else { s.into() }
}

pub fn write_svg(shapes: &[(String, Entity)]) -> String {
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for (_, e) in shapes {
        if let Some((a, b)) = crate::shape::bounds(e) {
            for k in 0..2 {
                lo[k] = lo[k].min(a[k]);
                hi[k] = hi[k].max(b[k]);
            }
        }
    }
    if lo[0] > hi[0] {
        lo = [0.0, 0.0];
        hi = [1.0, 1.0];
    }
    let c = hi[1];
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} 0 {} {}\" data-doc-ymax=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"0.2\">\n",
        f(lo[0]), f(hi[0] - lo[0]), f(hi[1] - lo[1]), f(c)
    );
    let path_of = |p: &geom::Pline| {
        let n = p.vertex_count();
        let mut d = String::new();
        let segs = if p.is_closed() { n } else { n - 1 };
        for i in 0..n {
            let v = p.at(i);
            if i == 0 {
                d += &format!("M{} {}", f(v.x), f(c - v.y));
            }
            if i < segs {
                let w = p.at((i + 1) % n);
                if v.bulge.abs() > 1e-12 {
                    let ch = (w.x - v.x).hypot(w.y - v.y);
                    let r = ch * (1.0 + v.bulge * v.bulge) / (4.0 * v.bulge.abs());
                    d += &format!(" A{} {} 0 {} {} {} {}", f(r), f(r), u8::from(v.bulge.abs() > 1.0), u8::from(v.bulge < 0.0), f(w.x), f(c - w.y));
                } else if !(p.is_closed() && i + 1 == n) {
                    d += &format!(" L{} {}", f(w.x), f(c - w.y));
                }
            }
        }
        if p.is_closed() {
            d += " Z";
        }
        d
    };
    for (_, e) in shapes {
        match e {
            Entity::Circle { cx, cy, r } => s += &format!("<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>\n", f(*cx), f(c - cy), f(*r)),
            Entity::Point { .. } => {}
            other => {
                if let Some(p) = other.to_pline() {
                    s += &format!("<path d=\"{}\"/>\n", path_of(&p));
                }
            }
        }
    }
    s += "</svg>\n";
    s
}
