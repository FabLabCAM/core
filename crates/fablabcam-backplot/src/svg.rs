//! Export SVG du backplot : vue de dessus colorée par profondeur, flèches de sens, légende,
//! statistiques et profil de profondeur le long du trajet.

use crate::scene::{build_scene, color_for_z, Prim, ViewOptions};
use crate::Backplot;
use std::fmt::Write;

#[derive(Debug, Clone)]
pub struct SvgOptions {
    pub width: f64,
    pub height: f64,
    pub title: String,
    /// Vitesse des déplacements rapides, pour l'estimation de durée (mm/min).
    pub rapid_feed: f64,
}

impl Default for SvgOptions {
    fn default() -> Self {
        SvgOptions { width: 1100.0, height: 820.0, title: "Backplot".into(), rapid_feed: 5000.0 }
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn rgb(c: [u8; 3]) -> String {
    format!("rgb({},{},{})", c[0], c[1], c[2])
}

pub fn to_svg(bp: &Backplot, view: &ViewOptions, o: &SvgOptions) -> String {
    let (w, h) = (o.width, o.height);
    let profile_h = 150.0;
    let side_w = 250.0;
    let (px, py, pw, ph) = (20.0, 50.0, w - side_w - 40.0, h - profile_h - 80.0); // zone de tracé
    let mut s = String::new();
    let _ = write!(s, r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="Helvetica, Arial, sans-serif">"#);
    let _ = write!(s, r##"<rect width="{w}" height="{h}" fill="#ffffff"/>"##);
    let _ = write!(s, r##"<text x="20" y="30" font-size="20" font-weight="bold" fill="#222">{}</text>"##, esc(&o.title));

    let Some((lo, hi)) = bp.bounds() else {
        s.push_str(r##"<text x="20" y="80" font-size="14" fill="#a00">Aucun mouvement dans ce programme.</text></svg>"##);
        return s;
    };
    let (bw, bh) = ((hi[0] - lo[0]).max(1e-6), (hi[1] - lo[1]).max(1e-6));
    let scale = ((pw - 40.0) / bw).min((ph - 40.0) / bh);
    let (ox, oy) = (px + (pw - bw * scale) / 2.0, py + (ph - bh * scale) / 2.0);
    let sx = |x: f64| ox + (x - lo[0]) * scale;
    let sy = |y: f64| oy + (hi[1] - y) * scale; // Y vers le haut

    let _ = write!(s, r##"<rect x="{px}" y="{py}" width="{pw}" height="{ph}" fill="#fafafa" stroke="#ccc"/>"##);
    // axes X=0 / Y=0 et repère d'échelle
    if lo[0] <= 0.0 && hi[0] >= 0.0 {
        let _ = write!(s, r##"<line x1="{x}" y1="{py}" x2="{x}" y2="{}" stroke="#e0e0e0"/>"##, py + ph, x = sx(0.0));
    }
    if lo[1] <= 0.0 && hi[1] >= 0.0 {
        let _ = write!(s, r##"<line x1="{px}" y1="{y}" x2="{}" y2="{y}" stroke="#e0e0e0"/>"##, px + pw, y = sy(0.0));
    }

    let mut view = view.clone();
    view.arrow_spacing_mm = 36.0 / scale; // une flèche tous les ~36 px
    view.max_err_mm = 0.4 / scale; // arcs lisses à l'écran
    for p in build_scene(bp, &view) {
        match p {
            Prim::Line { pts, rgb: c, width, dashed } => {
                let pts: Vec<String> = pts.iter().map(|q| format!("{:.2},{:.2}", sx(q[0]), sy(q[1]))).collect();
                let dash = if dashed { r#" stroke-dasharray="4 3""# } else { "" };
                let _ = write!(s, r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{width}" stroke-linejoin="round" stroke-linecap="round"{dash}/>"#, pts.join(" "), rgb(c));
            }
            Prim::Arrow { pos, dir, rgb: c } => {
                let (x, y) = (sx(pos[0]), sy(pos[1]));
                let d = (dir[0], -dir[1]); // Y inversé à l'écran
                let n = (-d.1, d.0);
                let (l, b) = (6.0, 3.2);
                let _ = write!(s, r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" fill-opacity="0.85"/>"#, x + d.0 * l, y + d.1 * l, x - d.0 * l * 0.6 + n.0 * b, y - d.1 * l * 0.6 + n.1 * b, x - d.0 * l * 0.6 - n.0 * b, y - d.1 * l * 0.6 - n.1 * b, rgb(c));
            }
            Prim::Dot { pos, rgb: c, radius, ring } => {
                let (x, y) = (sx(pos[0]), sy(pos[1]));
                if ring {
                    let _ = write!(s, r#"<circle cx="{x:.2}" cy="{y:.2}" r="{radius}" fill="none" stroke="{}" stroke-width="2.5"/>"#, rgb(c));
                } else {
                    let _ = write!(s, r##"<circle cx="{x:.2}" cy="{y:.2}" r="{radius}" fill="{}" stroke="#fff" stroke-width="1"/>"##, rgb(c));
                }
            }
        }
    }
    // échelle graphique (10 mm, 50 mm…)
    let nice = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0].iter().copied().rfind(|n| n * scale <= 160.0).unwrap_or(1.0);
    let _ = write!(s, r##"<g stroke="#444" stroke-width="2"><line x1="{x0}" y1="{y}" x2="{x1}" y2="{y}"/></g><text x="{x0}" y="{}" font-size="11" fill="#444">{nice} mm</text>"##, py + ph - 10.0, x0 = px + 12.0, x1 = px + 12.0 + nice * scale, y = py + ph - 22.0);

    // panneau latéral : dégradé de profondeur et statistiques
    let st = bp.stats(o.rapid_feed);
    let (cz_lo, cz_hi) = st.cut_z.unwrap_or((st.z_min, st.z_max));
    let (lx, ly) = (w - side_w, py);
    let _ = write!(s, r##"<defs><linearGradient id="zg" x1="0" y1="0" x2="0" y2="1">"##);
    for k in 0..=8 {
        let t = k as f64 / 8.0; // 0 = haut
        let z = cz_hi - (cz_hi - cz_lo) * t;
        let _ = write!(s, r#"<stop offset="{}%" stop-color="{}"/>"#, (t * 100.0) as i32, rgb(color_for_z(z, cz_lo, cz_hi)));
    }
    s.push_str("</linearGradient></defs>");
    let _ = write!(s, r##"<text x="{lx}" y="{}" font-size="13" font-weight="bold" fill="#222">Profondeur Z</text>"##, ly + 4.0);
    let _ = write!(s, r##"<rect x="{lx}" y="{}" width="22" height="170" fill="url(#zg)" stroke="#999"/>"##, ly + 14.0);
    let _ = write!(s, r##"<text x="{}" y="{}" font-size="12" fill="#222">{:.2} (haut des coupes)</text><text x="{}" y="{}" font-size="12" fill="#222">{:.2} (fond)</text>"##, lx + 30.0, ly + 28.0, cz_hi, lx + 30.0, ly + 184.0, cz_lo);
    let mut y = ly + 215.0;
    let mut line = |s: &mut String, txt: String| {
        let _ = write!(s, r##"<text x="{lx}" y="{y}" font-size="12" fill="#333">{}</text>"##, esc(&txt));
        y += 18.0;
    };
    line(&mut s, format!("{} mouvements", st.segments));
    line(&mut s, format!("Coupe : {:.1} mm", st.cut_len));
    line(&mut s, format!("Rapides : {:.1} mm", st.rapid_len));
    line(&mut s, format!("Durée estimée : {}", fmt_time(st.time_s)));
    line(&mut s, format!("{} niveau(x) de coupe", st.cut_levels.len()));
    if !st.cut_levels.is_empty() {
        let lv: Vec<String> = st.cut_levels.iter().take(8).map(|z| format!("{z:.2}")).collect();
        line(&mut s, format!("Z : {}{}", lv.join(", "), if st.cut_levels.len() > 8 { "…" } else { "" }));
    }
    y += 8.0;
    // légende des symboles
    let legend = [(crate::scene::START_RGB, "départ de coupe"), (crate::scene::END_RGB, "fin de coupe"), (crate::scene::PLUNGE_RGB, "plongée")];
    for (c, t) in legend {
        let _ = write!(s, r##"<circle cx="{}" cy="{}" r="5" fill="{}"/><text x="{}" y="{}" font-size="12" fill="#333">{t}</text>"##, lx + 6.0, y - 4.0, rgb(c), lx + 18.0, y);
        y += 18.0;
    }
    let _ = write!(s, r##"<line x1="{lx}" y1="{}" x2="{}" y2="{}" stroke="#999" stroke-dasharray="4 3"/><text x="{}" y="{}" font-size="12" fill="#333">rapide</text>"##, y - 4.0, lx + 14.0, y - 4.0, lx + 20.0, y);
    y += 18.0;
    let _ = write!(s, r##"<polygon points="{},{} {},{} {},{}" fill="#191919"/><text x="{}" y="{}" font-size="12" fill="#333">sens de parcours</text>"##, lx + 12.0, y - 4.0, lx + 2.0, y - 9.0, lx + 2.0, y + 1.0, lx + 20.0, y);
    for wmsg in bp.warnings.iter().take(4) {
        y += 22.0;
        let _ = write!(s, r##"<text x="{lx}" y="{y}" font-size="11" fill="#b00">⚠ {}</text>"##, esc(&wmsg.chars().take(38).collect::<String>()));
    }

    // profil de profondeur le long du trajet
    let (cx, cy, cw, ch) = (px, h - profile_h - 10.0 + 10.0, w - 40.0, profile_h - 30.0);
    let _ = write!(s, r##"<text x="{cx}" y="{}" font-size="13" font-weight="bold" fill="#222">Profil de profondeur (Z en fonction de la distance parcourue)</text>"##, cy - 6.0);
    let _ = write!(s, r##"<rect x="{cx}" y="{cy}" width="{cw}" height="{ch}" fill="#fafafa" stroke="#ccc"/>"##);
    let total = bp.total_len().max(1e-9);
    let zr = (st.z_max - st.z_min).max(1e-9);
    let prof: Vec<String> = bp.z_profile().iter().map(|(d, z)| format!("{:.1},{:.1}", cx + d / total * cw, cy + ch - 6.0 - (z - st.z_min) / zr * (ch - 12.0))).collect();
    let _ = write!(s, r##"<polyline points="{}" fill="none" stroke="#2b5fb4" stroke-width="1.2"/>"##, prof.join(" "));
    let _ = write!(s, r##"<text x="{}" y="{}" font-size="10" fill="#666">{:.1}</text><text x="{}" y="{}" font-size="10" fill="#666">{:.1}</text><text x="{}" y="{}" font-size="10" fill="#666">{:.0} mm</text>"##, cx + 3.0, cy + 11.0, st.z_max, cx + 3.0, cy + ch - 2.0, st.z_min, cx + cw - 48.0, cy + ch + 12.0, total);
    s.push_str("</svg>");
    s
}

fn fmt_time(secs: f64) -> String {
    let t = secs.round() as u64;
    if t >= 3600 {
        format!("{} h {:02} min", t / 3600, t % 3600 / 60)
    } else if t >= 60 {
        format!("{} min {:02} s", t / 60, t % 60)
    } else {
        format!("{t} s")
    }
}
