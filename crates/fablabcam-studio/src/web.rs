//! Page HTML « studio » autonome : dessin, sélection, outils, opérations, trajectoires et 3D,
//! navigables dans un navigateur sans réseau. Seule la navigation est interactive côté web :
//! l'édition se fait dans l'application native.

use crate::shape;
use crate::studio::Studio;
use fablabcam_model::Entity;
use fablabcam_geom::PlineSource;
use std::fmt::Write;

const TEMPLATE: &str = include_str!("studio.html");

pub fn num(v: f64) -> String {
    if v.is_finite() {
        let s = format!("{v:.4}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        if s.is_empty() || s == "-0" { "0".into() } else { s.into() }
    } else {
        "0".into()
    }
}

pub fn jstr(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => {}
            '\t' => o.push_str("\\t"),
            '<' => o.push_str("\\u003c"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn describe(e: &Entity) -> String {
    match e {
        Entity::Circle { r, .. } => format!("cercle r={}", num(*r)),
        Entity::Arc { r, sweep_deg, .. } => format!("arc r={} balayage {}°", num(*r), num(*sweep_deg)),
        Entity::Point { .. } => "point".into(),
        Entity::Rect { w, h, r, .. } => format!("rectangle {} × {}{}", num(*w), num(*h), if *r > 0.0 { format!(", coins r={}", num(*r)) } else { String::new() }),
        Entity::Polyline(p) => format!("{} ({} sommets)", if p.is_closed() { "polyligne fermée" } else { "polyligne ouverte" }, p.vertex_count()),
    }
}

/// Données du studio en JSON ; `selection` = identifiants des formes sélectionnées.
pub fn to_json(s: &Studio, selection: &[u32], extra: &str) -> String {
    let p = s.project();
    let mut o = String::from("{\"shapes\":[");
    for (i, sh) in p.shapes.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let pts: Vec<String> = shape::sample(&sh.entity).iter().map(|q| format!("[{},{}]", num(q[0]), num(q[1]))).collect();
        let (lo, hi) = shape::bounds(&sh.entity).unwrap_or(([0.0; 2], [0.0; 2]));
        let _ = write!(
            o,
            "{{\"id\":{},\"layer\":{},\"info\":{},\"box\":[{},{},{},{}],\"closed\":{},\"pts\":[{}]}}",
            sh.id,
            jstr(&sh.layer),
            jstr(&describe(&sh.entity)),
            num(lo[0]), num(lo[1]), num(hi[0]), num(hi[1]),
            shape::is_closed(&sh.entity),
            pts.join(",")
        );
    }
    let _ = write!(o, "],\"selection\":[{}]", selection.iter().map(u32::to_string).collect::<Vec<_>>().join(","));
    o.push_str(",\"tools\":[");
    for (i, t) in p.tools.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let _ = write!(
            o,
            "{{\"name\":{},\"n\":{},\"kind\":{},\"d\":{},\"flutes\":{},\"feed\":{},\"plunge\":{},\"spindle\":{},\"info\":{}}}",
            jstr(&t.name), t.index, jstr(t.kind.label()), num(t.diameter), t.flutes, num(t.feed), num(t.plunge), num(t.spindle), jstr(&tool_info(t))
        );
    }
    o.push_str("],\"ops\":[");
    for (i, op) in p.ops.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let c = s.computed.get(&op.id);
        let _ = write!(
            o,
            "{{\"id\":{},\"name\":{},\"kind\":{},\"side\":{},\"tool\":{},\"depth\":{},\"stepDown\":{},\"stepOver\":{},\"stock\":{},\"tabPoints\":{},\"cam\":{},\"climb\":{},\"tabs\":{},\"lead\":{},\"enabled\":{},\"shapes\":[{}],\"error\":{},\"moves\":{}}}",
            op.id, jstr(&op.name), jstr(op.kind.label()), jstr(op.side.label()), jstr(&op.tool), num(op.depth), num(op.step_down), num(op.step_over), num(op.stock_to_leave), op.tab_points.len(), jstr(&cam_info(op)), op.climb, op.tabs, jstr(op.lead.label()), op.enabled,
            op.shapes.iter().map(u32::to_string).collect::<Vec<_>>().join(","),
            c.and_then(|c| c.error.as_deref()).map_or("null".to_string(), jstr),
            c.and_then(|c| c.path.as_ref()).map_or(0, |p| p.moves.len()),
        );
    }
    o.push_str("],\"bp\":");
    o.push_str(&fablabcam_backplot::web::to_json(&s.backplot(), "studio"));
    o.push_str(extra);
    o.push('}');
    o
}

/// Page complète. `extra` : fragment JSON supplémentaire commençant par une virgule.
pub fn to_html(s: &Studio, selection: &[u32], title: &str, extra: &str) -> String {
    let esc = title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    TEMPLATE.replace("__TITLE__", &esc).replace("__DATA__", &to_json(s, selection, extra))
}

/// Fragment JSON `,"sim":{…}` : grille de hauteurs à 11 instants du trajet (0 %, 10 % … 100 %),
/// gouges et trajets découpés par segment avec leur longueur cumulée. None si rien à simuler.
pub fn sim_json(studio: &Studio, max_cells: usize) -> Option<String> {
    use crate::sim::{default_stock, op_backplots, simulate};
    let stock = default_stock(studio)?;
    let res = ((stock.max[0] - stock.min[0]).max(stock.max[1] - stock.min[1]) / max_cells as f64).max(0.1);
    let first = simulate(studio, res, 1.0)?;
    let m = &first.map;
    let mut o = format!(
        ",\"sim\":{{\"nx\":{},\"ny\":{},\"min\":[{},{}],\"res\":{},\"top\":{},\"bottom\":{},\"total\":{},\"frames\":[",
        m.nx, m.ny, num(m.min[0]), num(m.min[1]), num(m.res), num(m.top), num(m.bottom), num(first.total_len)
    );
    for k in 0..=10 {
        let out = if k == 10 { None } else { simulate(studio, res, k as f64 / 10.0) };
        let map = out.as_ref().map_or(&first.map, |x| &x.map);
        if k > 0 {
            o.push(',');
        }
        o.push('[');
        for (i, v) in map.h.iter().enumerate() {
            if i > 0 {
                o.push(',');
            }
            o.push_str(&num(*v as f64));
        }
        o.push(']');
    }
    o.push_str("],\"gouges\":[");
    for (i, g) in first.gouges.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let _ = write!(o, "{{\"x\":{},\"y\":{},\"z\":{},\"op\":{},\"kind\":{}}}", num(g.pos[0]), num(g.pos[1]), num(g.pos[2]), jstr(&g.op), jstr(&format!("{:?}", g.kind)));
    }
    o.push_str("],\"paths\":[");
    let mut cum = 0.0;
    let mut firstseg = true;
    for (_, bp) in op_backplots(studio) {
        for s in &bp.segs {
            if !firstseg {
                o.push(',');
            }
            firstseg = false;
            let pts: Vec<String> = if s.kind == fablabcam_backplot::Kind::Rapid { vec![s.from, s.to] } else { s.polyline(0.1).into_iter().collect() }
                .iter()
                .map(|p| format!("[{},{},{}]", num(p[0]), num(p[1]), num(p[2])))
                .collect();
            let _ = write!(o, "[{},{},{},[{}]]", u8::from(s.kind == fablabcam_backplot::Kind::Rapid), num(cum), num(s.len), pts.join(","));
            cum += s.len;
        }
    }
    o.push_str("]}");
    Some(o)
}

/// Propriétés renseignées d'un outil, en une ligne pour la table de la page web.
fn tool_info(t: &crate::project::Tool) -> String {
    let mut v: Vec<String> = Vec::new();
    for (label, x, unit) in [
        ("coupe", t.flute_length, " mm"),
        ("longueur", t.length, " mm"),
        ("queue Ø", t.shank_diameter, " mm"),
        ("V", t.vee_angle, "°"),
        ("hélice", t.helix_angle, "°"),
        ("rampe max", t.max_ramp_angle, "°"),
        ("passe axiale", t.axial_depth, " mm"),
        ("passe radiale", t.radial_depth, " mm"),
        ("charge/dent", t.tooth_load, " mm"),
    ] {
        if x != 0.0 {
            v.push(format!("{label} {}{unit}", num(x)));
        }
    }
    for (label, x) in [("matière", &t.material), ("revêtement", &t.coating), ("réf.", &t.part_code), ("note", &t.notes)] {
        if !x.is_empty() {
            v.push(format!("{label} : {x}"));
        }
    }
    v.join(" · ")
}

/// Réglages d'usinage différents du défaut, en une ligne.
fn cam_info(op: &crate::project::Operation) -> String {
    let (c, d) = (&op.cam, crate::project::CamSettings::default());
    let mut v = vec![format!("surface Z {}", num(c.surface)), format!("dégagement Z {}", num(c.clearance))];
    if c.level_first {
        v.push("niveau par niveau".into());
    }
    if c.finish_stepover > 0.0 {
        v.push(format!("finition {} mm{}", num(c.finish_stepover), if c.finish_at_target_only { " au fond" } else { "" }));
    }
    if c.final_increment > 0.0 {
        v.push(format!("dernière passe {} mm", num(c.final_increment)));
    }
    if c.corner_overcut {
        v.push("surcoupe des coins".into());
    }
    if c.max_crossover != d.max_crossover {
        v.push(format!("transition {} × Ø", num(c.max_crossover)));
    }
    if c.spindle_ccw {
        v.push("broche M4".into());
    }
    if op.tabs {
        v.push(format!("ponts {} {}×{} mm", c.tab_shape.label(), num(c.tab_width), num(c.tab_height)));
    }
    v.join(" · ")
}
