use fablabcam_geom::{self as geom, PlineSource};
use fablabcam_model::Entity;

fn f(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".into()
    } else {
        s.into()
    }
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
                    d += &format!(
                        " A{} {} 0 {} {} {} {}",
                        f(r),
                        f(r),
                        u8::from(v.bulge.abs() > 1.0),
                        u8::from(v.bulge < 0.0),
                        f(w.x),
                        f(c - w.y)
                    );
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
            Entity::Circle { cx, cy, r } => {
                s += &format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>\n",
                    f(*cx),
                    f(c - cy),
                    f(*r)
                )
            }
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
