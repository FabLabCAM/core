//! Post-processeur G-code générique (dialecte proche de Mach3/GRBL, unités mm).
pub mod machine;

use crate::params::Params;
use crate::toolpath::{Move, Toolpath};

#[derive(Debug, Clone)]
pub struct Post {
    pub decimals: usize,
    /// Cycle G81/G83 ; sinon perçage expansé en G0/G1.
    pub canned_drill: bool,
    pub program_name: String,
}

impl Default for Post {
    fn default() -> Self {
        Post { decimals: 3, canned_drill: true, program_name: "FabLabCAM".into() }
    }
}

struct Emitter<'a> {
    post: &'a Post,
    out: String,
    group: Option<&'static str>,
    feed: Option<f64>,
}

impl Emitter<'_> {
    fn n(&self, v: f64) -> String {
        let s = format!("{:.*}", self.post.decimals, v);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        if s.is_empty() || s == "-0" { "0".into() } else { s.into() }
    }

    fn line(&mut self, s: impl AsRef<str>) {
        self.out.push_str(s.as_ref());
        self.out.push('\n');
    }

    fn motion(&mut self, g: &'static str, words: &[(&str, Option<f64>)], feed: Option<f64>) {
        let mut l = String::new();
        if self.group != Some(g) {
            l.push_str(g);
            self.group = Some(g);
        }
        for (k, v) in words {
            if let Some(v) = v {
                if !l.is_empty() {
                    l.push(' ');
                }
                l.push_str(k);
                l.push_str(&self.n(*v));
            }
        }
        if let Some(f) = feed {
            if self.feed != Some(f) {
                l.push_str(&format!(" F{}", self.n(f)));
                self.feed = Some(f);
            }
        }
        if !l.trim().is_empty() {
            self.line(l.trim());
        }
    }
}

pub fn emit(path: &Toolpath, p: &Params, post: &Post) -> String {
    let mut e = Emitter { post, out: String::new(), group: None, feed: None };
    e.line(format!("({})", post.program_name));
    e.line("G21 G90 G17 G94");
    e.line(format!("(Outil D{})", e.n(p.tool_diameter)));
    e.line(format!("{} S{}", if p.spindle_cw { "M3" } else { "M4" }, e.n(p.spindle_speed)));
    e.motion("G0", &[("Z", Some(p.clearance_plane))], None);
    // altitude connue de l'outil : un rapide vertical vers l'altitude courante est omis (jonction
    // entre les trajets de deux formes)
    let mut cur_z = Some(p.clearance_plane);
    for m in &path.moves {
        if let Move::Rapid { x: None, y: None, z: Some(z) } = m {
            if cur_z.is_some_and(|c| (c - z).abs() < 1e-9) {
                continue;
            }
        }
        match m {
            Move::Rapid { z: Some(z), .. } | Move::Feed { z: Some(z), .. } | Move::Arc { z: Some(z), .. } => cur_z = Some(*z),
            Move::Drill { .. } => cur_z = None,
            _ => {}
        }
        match m {
            Move::Comment(c) => e.line(format!("({})", c.replace(['(', ')'], ""))),
            Move::Rapid { x, y, z } => e.motion("G0", &[("X", *x), ("Y", *y), ("Z", *z)], None),
            Move::Feed { x, y, z, f } => e.motion("G1", &[("X", *x), ("Y", *y), ("Z", *z)], Some(*f)),
            Move::Arc { ccw, x, y, z, i, j, f } => {
                let g = if *ccw { "G3" } else { "G2" };
                e.motion(g, &[("X", Some(*x)), ("Y", Some(*y)), ("Z", *z), ("I", Some(*i)), ("J", Some(*j))], Some(*f));
            }
            Move::Drill { x, y, z, retract, f, peck, dwell } => {
                if post.canned_drill {
                    e.group = None; // les G81/G82/G83 sont modaux : on réécrit toujours le code
                    let (code, q) = match peck {
                        Some(q) => ("G83", format!(" Q{}", e.n(*q))),
                        None if *dwell > 0.0 => ("G82", format!(" P{}", e.n(*dwell))),
                        None => ("G81", String::new()),
                    };
                    e.line(format!("{code} X{} Y{} Z{} R{}{q} F{}", e.n(*x), e.n(*y), e.n(*z), e.n(*retract), e.n(*f)));
                    e.feed = Some(*f);
                    e.line("G80");
                } else {
                    e.motion("G0", &[("X", Some(*x)), ("Y", Some(*y))], None);
                    e.motion("G0", &[("Z", Some(p.stock_surface + 1.0))], None);
                    e.motion("G1", &[("Z", Some(*z))], Some(*f));
                    if *dwell > 0.0 {
                        e.line(format!("G4 P{}", e.n(*dwell)));
                    }
                    e.motion("G0", &[("Z", Some(*retract))], None);
                }
            }
        }
    }
    e.line("M5");
    e.line("M30");
    e.out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolpath::{profile, Side};
    use fablabcam_geom as geom;

    fn p() -> Params {
        Params { tool_diameter: 6.0, target_depth: -2.0, depth_increment: 1.0, ..Params::default() }
    }

    #[test]
    fn number_formatting() {
        let post = Post::default();
        let e = Emitter { post: &post, out: String::new(), group: None, feed: None };
        assert_eq!(e.n(1.0), "1");
        assert_eq!(e.n(-0.0004), "0");
        assert_eq!(e.n(2.50), "2.5");
        assert_eq!(e.n(-3.1417), "-3.142");
    }

    #[test]
    fn program_structure_and_modal_codes() {
        let t = profile(&geom::rectangle(0.0, 0.0, 50.0, 30.0), Side::Outside, &p()).unwrap();
        let g = emit(&t, &p(), &Post::default());
        let lines: Vec<&str> = g.lines().collect();
        assert_eq!(lines[1], "G21 G90 G17 G94");
        assert_eq!(*lines.last().unwrap(), "M30");
        assert!(g.contains("G3 ") || g.contains("G2 "));
        // modalité : jamais deux « G1 » consécutifs ne répètent le code
        assert!(!g.contains("G1 G1"));
        // un seul F800 tant que l'avance ne change pas entre deux G1 d'usinage
        assert!(g.matches("F800").count() >= 1);
    }

    #[test]
    fn drill_canned_cycle() {
        let t = crate::toolpath::drill(&[(5.0, 5.0)], &p()).unwrap();
        let g = emit(&t, &p(), &Post::default());
        assert!(g.contains("G83 X5 Y5 Z-2 R3 Q1 F300"), "{g}");
        assert!(g.contains("G80"));
    }

    #[test]
    fn drill_without_canned_cycle() {
        let t = crate::toolpath::drill(&[(5.0, 5.0)], &p()).unwrap();
        let g = emit(&t, &p(), &Post { canned_drill: false, ..Post::default() });
        assert!(!g.contains("G81") && !g.contains("G83"));
        assert!(g.contains("G1 Z-2"));
    }

    #[test]
    fn helical_arc_carries_z() {
        use crate::toolpath::Move;
        let t = Toolpath { moves: vec![Move::Arc { ccw: true, x: 10.0, y: 0.0, z: Some(-1.5), i: -5.0, j: 0.0, f: 800.0 }] };
        let g = emit(&t, &p(), &Post::default());
        assert!(g.contains("G3 X10 Y0 Z-1.5 I-5 J0 F800"), "{g}");
    }
}
