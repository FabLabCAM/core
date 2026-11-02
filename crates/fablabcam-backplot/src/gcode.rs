//! Lecture d'un programme G-code (dialecte Mach3/GRBL : G0 G1 G2 G3 G4 G17 G20 G21 G80 G81 G82 G83 G90 G91).
//!
//! Les codes inconnus sont ignorés et signalés dans `Backplot::warnings` : un backplot qui se tait
//! sur ce qu'il n'a pas compris serait trompeur.

use crate::{Backplot, Kind, Seg};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Motion {
    Rapid,
    Linear,
    ArcCw,
    ArcCcw,
    Drill,
    Peck,
}

#[derive(Default, Debug)]
struct Words {
    g: Vec<f64>,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    i: Option<f64>,
    j: Option<f64>,
    k: Option<f64>,
    r: Option<f64>,
    q: Option<f64>,
    f: Option<f64>,
    other_motion_word: Option<char>,
}

/// Retire commentaires `( … )` et `; …`, met en majuscules.
fn clean(line: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for ch in line.chars() {
        match ch {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            ';' if depth == 0 => break,
            c if depth == 0 => out.push(c.to_ascii_uppercase()),
            _ => {}
        }
    }
    out
}

fn words(line: &str) -> Words {
    let mut w = Words::default();
    let b: Vec<char> = line.chars().collect();
    let mut k = 0;
    while k < b.len() {
        let c = b[k];
        k += 1;
        if !c.is_ascii_alphabetic() {
            continue;
        }
        let mut num = String::new();
        while k < b.len() && (b[k].is_ascii_digit() || matches!(b[k], '.' | '-' | '+')) {
            num.push(b[k]);
            k += 1;
        }
        let Ok(v) = num.parse::<f64>() else { continue };
        match c {
            'G' => w.g.push(v),
            'X' => w.x = Some(v),
            'Y' => w.y = Some(v),
            'Z' => w.z = Some(v),
            'I' => w.i = Some(v),
            'J' => w.j = Some(v),
            'K' => w.k = Some(v),
            'R' => w.r = Some(v),
            'Q' => w.q = Some(v),
            'F' => w.f = Some(v),
            'A' | 'B' | 'C' | 'U' | 'V' | 'W' => w.other_motion_word = Some(c),
            _ => {}
        }
    }
    w
}

struct State {
    pos: Option<[f64; 3]>,
    /// Faux tant qu'aucun mot X/Y n'a donné la position horizontale de la machine.
    xy_known: bool,
    motion: Motion,
    feed: f64,
    scale: f64,
    relative: bool,
    retract: f64,
    drill_z: f64,
    peck: f64,
    /// Plan des arcs : 17 (XY), 18 (ZX), 19 (YZ).
    plane: u8,
}

/// Lit un programme G-code et reconstruit son trajet.
pub fn parse_gcode(text: &str) -> Backplot {
    let mut bp = Backplot { source: text.lines().map(str::to_string).collect(), ..Default::default() };
    let mut st = State { pos: None, xy_known: false, motion: Motion::Rapid, feed: 0.0, scale: 1.0, relative: false, retract: 0.0, drill_z: 0.0, peck: 0.0, plane: 17 };
    let mut warned: Vec<String> = Vec::new();
    let mut warn = |bp: &mut Backplot, msg: String| {
        if !warned.contains(&msg) {
            warned.push(msg.clone());
            bp.warnings.push(msg);
        }
    };

    for (n, raw) in text.lines().enumerate() {
        let line_no = n + 1;
        let w = words(&clean(raw));
        let mut canned_cancel = false;
        for &g in &w.g {
            match g as i32 {
                0 => st.motion = Motion::Rapid,
                1 => st.motion = Motion::Linear,
                2 => st.motion = Motion::ArcCw,
                3 => st.motion = Motion::ArcCcw,
                80 => canned_cancel = true,
                // G82 : comme G81, la temporisation au fond ne change pas le tracé
                81 | 82 => st.motion = Motion::Drill,
                83 => st.motion = Motion::Peck,
                17 => st.plane = 17,
                18 => st.plane = 18,
                19 => st.plane = 19,
                20 => st.scale = 25.4,
                21 => st.scale = 1.0,
                90 => st.relative = false,
                91 => st.relative = true,
                // codes sans effet sur la géométrie
                4 | 40 | 41 | 42 | 43 | 49 | 54..=59 | 61 | 64 | 94 | 95 | 98 | 99 | 28 | 53 => {}
                other => warn(&mut bp, format!("ligne {line_no} : G{other} ignoré")),
            }
        }
        if let Some(c) = w.other_motion_word {
            warn(&mut bp, format!("ligne {line_no} : axe {c} ignoré (seuls X, Y, Z sont tracés)"));
        }
        if let Some(f) = w.f {
            st.feed = f * st.scale;
        }
        if canned_cancel {
            st.motion = Motion::Rapid;
            continue;
        }
        let has_pos = w.x.is_some() || w.y.is_some() || w.z.is_some();
        let sc = st.scale;
        // les mots de position de cette ligne, absolus
        let resolve = |cur: Option<[f64; 3]>| -> [f64; 3] {
            let base = cur.unwrap_or([0.0; 3]);
            let ax = |v: Option<f64>, b: f64| match v {
                Some(v) if st.relative => b + v * sc,
                Some(v) => v * sc,
                None => b,
            };
            [ax(w.x, base[0]), ax(w.y, base[1]), ax(w.z, base[2])]
        };
        match st.motion {
            Motion::Rapid | Motion::Linear if has_pos => {
                let to = resolve(st.pos);
                match st.pos {
                    None => {} // première position : point de départ, rien à tracer
                    // premier déplacement horizontal : la position XY de départ est inconnue, on n'invente pas de trajet
                    Some(_) if !st.xy_known && (w.x.is_some() || w.y.is_some()) => {}
                    Some(from) => {
                        let horizontal = (to[0] - from[0]).hypot(to[1] - from[1]) > 1e-9;
                        let kind = match (st.motion, horizontal) {
                            (Motion::Rapid, _) => Kind::Rapid,
                            (_, true) => Kind::Feed,
                            (_, false) => Kind::Plunge,
                        };
                        push(&mut bp, kind, from, to, None, false, st.feed, line_no);
                    }
                }
                st.pos = Some(to);
                st.xy_known |= w.x.is_some() || w.y.is_some();
            }
            Motion::ArcCw | Motion::ArcCcw if st.plane != 17 && (has_pos || w.i.is_some() || w.j.is_some() || w.k.is_some()) => {
                let to = resolve(st.pos);
                let from = st.pos.unwrap_or(to);
                // axes du plan (abscisse, ordonnée) et décalages du centre : G18 = (Z, X) avec K, I ;
                // G19 = (Y, Z) avec J, K ; G3 tourne dans le sens trigonométrique de ce repère
                let (u, v, du, dv) = if st.plane == 18 { (2, 0, w.k, w.i) } else { (1, 2, w.j, w.k) };
                if du.is_none() && dv.is_none() {
                    warn(&mut bp, format!("ligne {line_no} : arc G{} sans centre (I/J/K), tracé en ligne droite", st.plane));
                    let kind = if (to[0] - from[0]).hypot(to[1] - from[1]) > 1e-9 { Kind::Feed } else { Kind::Plunge };
                    push(&mut bp, kind, from, to, None, false, st.feed, line_no);
                } else {
                    let c = (from[u] + du.unwrap_or(0.0) * sc, from[v] + dv.unwrap_or(0.0) * sc);
                    plane_arc(&mut bp, from, to, (u, v), c, st.motion == Motion::ArcCcw, st.feed, line_no);
                }
                st.pos = Some(to);
            }
            Motion::ArcCw | Motion::ArcCcw if has_pos || w.i.is_some() || w.j.is_some() => {
                let to = resolve(st.pos);
                let from = st.pos.unwrap_or(to);
                if w.r.is_some() && w.i.is_none() && w.j.is_none() {
                    warn(&mut bp, format!("ligne {line_no} : arc au format rayon (R) non géré, tracé en ligne droite"));
                    push(&mut bp, Kind::Feed, from, to, None, false, st.feed, line_no);
                } else if w.i.is_none() && w.j.is_none() {
                    warn(&mut bp, format!("ligne {line_no} : arc sans I/J, tracé en ligne droite"));
                    push(&mut bp, Kind::Feed, from, to, None, false, st.feed, line_no);
                } else {
                    let c = [from[0] + w.i.unwrap_or(0.0) * sc, from[1] + w.j.unwrap_or(0.0) * sc];
                    let (r0, r1) = ((from[0] - c[0]).hypot(from[1] - c[1]), (to[0] - c[0]).hypot(to[1] - c[1]));
                    if (r0 - r1).abs() > 0.01 {
                        warn(&mut bp, format!("ligne {line_no} : arc dont les rayons de départ et d'arrivée diffèrent de {:.3} mm", (r0 - r1).abs()));
                    }
                    push(&mut bp, Kind::Arc, from, to, Some(c), st.motion == Motion::ArcCcw, st.feed, line_no);
                }
                st.pos = Some(to);
            }
            Motion::Drill | Motion::Peck if has_pos || w.r.is_some() => {
                let cur = st.pos.unwrap_or([0.0; 3]);
                if let Some(z) = w.z {
                    st.drill_z = z * sc;
                }
                if let Some(r) = w.r {
                    st.retract = r * sc;
                }
                if let Some(q) = w.q {
                    st.peck = q * sc;
                }
                let x = w.x.map_or(cur[0], |v| if st.relative { cur[0] + v * sc } else { v * sc });
                let y = w.y.map_or(cur[1], |v| if st.relative { cur[1] + v * sc } else { v * sc });
                drill_cycle(&mut bp, &st, cur, [x, y], line_no);
                st.pos = Some([x, y, cur[2].max(st.retract)]);
                st.xy_known = true;
            }
            _ => {}
        }
    }
    bp
}

#[allow(clippy::too_many_arguments)]
fn push(bp: &mut Backplot, kind: Kind, from: [f64; 3], to: [f64; 3], center: Option<[f64; 2]>, ccw: bool, feed: f64, line: usize) {
    let mut s = Seg { kind, from, to, center, ccw, feed, line, len: 0.0 };
    let dz = to[2] - from[2];
    s.len = match center {
        Some(c) => {
            let r = (from[0] - c[0]).hypot(from[1] - c[1]);
            (r * s.sweep().abs()).hypot(dz)
        }
        None => (to[0] - from[0]).hypot(to[1] - from[1]).hypot(dz),
    };
    if s.len > 1e-12 {
        bp.segs.push(s);
    }
}

/// Arc dans le plan ZX (G18) ou YZ (G19), découpé en segments à 0,01 mm près ; le troisième axe
/// varie linéairement (hélice). `axes` = indices (abscisse, ordonnée), `c` = centre dans ce plan.
#[allow(clippy::too_many_arguments)]
fn plane_arc(bp: &mut Backplot, from: [f64; 3], to: [f64; 3], axes: (usize, usize), c: (f64, f64), ccw: bool, feed: f64, line: usize) {
    let (u, v) = axes;
    let w = 3 - u - v; // axe normal au plan
    let r = (from[u] - c.0).hypot(from[v] - c.1);
    let a0 = (from[v] - c.1).atan2(from[u] - c.0);
    let a1 = (to[v] - c.1).atan2(to[u] - c.0);
    let mut sweep = a1 - a0;
    if ccw && sweep <= 1e-9 {
        sweep += std::f64::consts::TAU;
    } else if !ccw && sweep >= -1e-9 {
        sweep -= std::f64::consts::TAU;
    }
    let step = if r > 0.01 { 2.0 * (1.0 - 0.01 / r).acos() } else { sweep.abs() };
    let n = ((sweep.abs() / step.max(1e-3)).ceil() as usize).clamp(1, 720);
    let mut prev = from;
    for k in 1..=n {
        let t = k as f64 / n as f64;
        let mut p = [0.0; 3];
        if k == n {
            p = to;
        } else {
            let a = a0 + sweep * t;
            p[u] = c.0 + r * a.cos();
            p[v] = c.1 + r * a.sin();
            p[w] = from[w] + (to[w] - from[w]) * t;
        }
        let kind = if (p[0] - prev[0]).hypot(p[1] - prev[1]) > 1e-9 { Kind::Feed } else { Kind::Plunge };
        push(bp, kind, prev, p, None, false, feed, line);
        prev = p;
    }
}

/// G81 (perçage simple) et G83 (picage) : approche, plongée(s) à l'avance, remontée au plan R.
fn drill_cycle(bp: &mut Backplot, st: &State, cur: [f64; 3], xy: [f64; 2], line: usize) {
    let (r, z) = (st.retract, st.drill_z);
    let mut at = cur;
    let go = |bp: &mut Backplot, kind: Kind, to: [f64; 3], at: &mut [f64; 3]| {
        push(bp, kind, *at, to, None, false, st.feed, line);
        *at = to;
    };
    if at[2] < r {
        go(bp, Kind::Rapid, [at[0], at[1], r], &mut at);
    }
    if st.xy_known {
        go(bp, Kind::Rapid, [xy[0], xy[1], at[2]], &mut at);
    } else {
        at = [xy[0], xy[1], at[2]]; // position XY de départ inconnue : pas de trajet inventé
    }
    if at[2] > r {
        go(bp, Kind::Rapid, [xy[0], xy[1], r], &mut at);
    }
    let depths: Vec<f64> = if st.peck > 0.0 && st.motion == Motion::Peck {
        let mut v = Vec::new();
        let mut d = r - st.peck;
        while d > z + 1e-9 {
            v.push(d);
            d -= st.peck;
        }
        v.push(z);
        v
    } else {
        vec![z]
    };
    let mut prev = r;
    for d in depths {
        if prev < r {
            go(bp, Kind::Rapid, [xy[0], xy[1], (prev + 0.5).min(r)], &mut at); // redescente rapide près du fond
        }
        go(bp, Kind::Plunge, [xy[0], xy[1], d], &mut at);
        go(bp, Kind::Rapid, [xy[0], xy[1], r], &mut at);
        prev = d;
    }
}
