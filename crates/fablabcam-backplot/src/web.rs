//! Export HTML autonome du backplot : une page unique (canvas + JavaScript, sans dépendance) qui
//! reprend les fonctions du viewer egui — pan/zoom, couleur par profondeur, flèches de sens,
//! filtre en Z, animation, profil de profondeur, survol avec ligne de G-code.
//!
//! Les mouvements viennent de [`parse_gcode`](crate::parse_gcode) : la page ne relit pas le G-code,
//! elle dessine exactement ce que le backplot Rust a reconstruit.

use crate::{Backplot, Kind};
use std::fmt::Write;

const TEMPLATE: &str = include_str!("web_viewer.html");

fn num(v: f64) -> String {
    if v.is_finite() {
        let s = format!("{v:.4}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        if s.is_empty() || s == "-0" { "0".into() } else { s.into() }
    } else {
        "0".into()
    }
}

fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => {}
            '\t' => o.push_str("\\t"),
            '<' => o.push_str("\\u003c"), // évite « </script> » dans la page
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Données du backplot au format JSON (segments, programme, avertissements, échelle des couleurs).
pub fn to_json(bp: &Backplot, title: &str) -> String {
    let mut o = String::new();
    let _ = write!(o, "{{\"title\":{},\"segs\":[", json_str(title));
    for (i, s) in bp.segs.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let k = match s.kind {
            Kind::Rapid => 0,
            Kind::Feed => 1,
            Kind::Plunge => 2,
            Kind::Arc => 3,
        };
        let c = s.center.map_or("null".to_string(), |c| format!("[{},{}]", num(c[0]), num(c[1])));
        let _ = write!(
            o,
            "[{k},{},{},{},{},{},{},{c},{},{},{},{}]",
            num(s.from[0]),
            num(s.from[1]),
            num(s.from[2]),
            num(s.to[0]),
            num(s.to[1]),
            num(s.to[2]),
            u8::from(s.ccw),
            num(s.feed),
            s.line,
            num(s.len),
        );
    }
    o.push_str("],\"source\":[");
    for (i, l) in bp.source.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        o.push_str(&json_str(l));
    }
    o.push_str("],\"warnings\":[");
    for (i, w) in bp.warnings.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        o.push_str(&json_str(w));
    }
    o.push_str("]}");
    o
}

/// Page HTML complète, utilisable hors ligne.
pub fn to_html(bp: &Backplot, title: &str) -> String {
    let esc = title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    TEMPLATE.replace("__TITLE__", &esc).replace("__DATA__", &to_json(bp, title))
}
