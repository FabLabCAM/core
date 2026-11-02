//! Texte : contours des lettres d'une police TrueType / OpenType, en polylignes fermées (les trous des lettres
//! comme « o » sont des contours séparés, reconnus comme îlots par la poche).
//!
//! Les courbes de la police sont approchées par des segments à `curves::TOL` près environ.

use fablabcam_geom as geom;
use fablabcam_model::Entity;
use ttf_parser::OutlineBuilder;

/// Polices essayées quand aucune n'est indiquée.
pub const DEFAULT_FONTS: &[&str] = &[
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
    "/Library/Fonts/Arial.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
];

/// Première police par défaut présente sur la machine.
pub fn default_font() -> Option<&'static str> {
    DEFAULT_FONTS.iter().copied().find(|p| std::path::Path::new(p).is_file())
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// Hauteur des majuscules (mm).
    pub height: f64,
    /// Espacement supplémentaire entre lettres (mm).
    pub spacing: f64,
    /// Interligne, en multiple de la hauteur de ligne de la police.
    pub line_spacing: f64,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle { height: 10.0, spacing: 0.0, line_spacing: 1.0 }
    }
}

struct Outline {
    scale: f64,
    origin: (f64, f64),
    tol: f64,
    cur: Vec<(f64, f64)>,
    last: (f64, f64),
    contours: Vec<Vec<(f64, f64)>>,
}

impl Outline {
    fn pt(&self, x: f32, y: f32) -> (f64, f64) {
        (self.origin.0 + x as f64 * self.scale, self.origin.1 + y as f64 * self.scale)
    }

    /// Nombre de segments d'une courbe dont le polygone de contrôle mesure `len` mm.
    fn steps(&self, len: f64) -> usize {
        ((len / self.tol).sqrt().ceil() as usize).clamp(2, 64)
    }
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

impl OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        self.last = self.pt(x, y);
        self.cur.push(self.last);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.last = self.pt(x, y);
        self.cur.push(self.last);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (p0, c, p) = (self.last, self.pt(x1, y1), self.pt(x, y));
        let n = self.steps(dist(p0, c) + dist(c, p));
        for k in 1..=n {
            let t = k as f64 / n as f64;
            let u = 1.0 - t;
            self.cur.push((u * u * p0.0 + 2.0 * u * t * c.0 + t * t * p.0, u * u * p0.1 + 2.0 * u * t * c.1 + t * t * p.1));
        }
        self.last = p;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (p0, c1, c2, p) = (self.last, self.pt(x1, y1), self.pt(x2, y2), self.pt(x, y));
        let n = self.steps(dist(p0, c1) + dist(c1, c2) + dist(c2, p));
        for k in 1..=n {
            let t = k as f64 / n as f64;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.cur.push((a * p0.0 + b * c1.0 + c * c2.0 + d * p.0, a * p0.1 + b * c1.1 + c * c2.1 + d * p.1));
        }
        self.last = p;
    }

    fn close(&mut self) {
        let mut c = std::mem::take(&mut self.cur);
        c.dedup_by(|a, b| dist(*a, *b) < 1e-9);
        if c.len() > 2 && dist(c[0], c[c.len() - 1]) < 1e-9 {
            c.pop();
        }
        if c.len() >= 3 {
            self.contours.push(c);
        }
    }
}

/// Contours du texte `text` (plusieurs lignes possibles), ligne de base de la première ligne en `origin`.
/// Renvoie aussi les caractères absents de la police (non dessinés).
pub fn outlines(font: &[u8], text: &str, origin: (f64, f64), style: &TextStyle) -> Result<(Vec<Entity>, String), String> {
    if !(style.height.is_finite() && style.height > 0.0) {
        return Err("la hauteur du texte doit être > 0".into());
    }
    let face = ttf_parser::Face::parse(font, 0).map_err(|e| format!("police illisible : {e}"))?;
    let cap = face.capital_height().filter(|h| *h > 0).map(f64::from).or_else(|| {
        // Certaines polices n'indiquent pas leur hauteur de capitale (ex. DejaVu).
        // Mesurer H évite que la hauteur usinée dépende d'une estimation du cadratin.
        let glyph = face.glyph_index('H')?;
        let bounds = face.glyph_bounding_box(glyph)?;
        let height = f64::from(bounds.y_max) - f64::from(bounds.y_min);
        (height > 0.0).then_some(height)
    }).unwrap_or_else(|| face.units_per_em() as f64 * 0.7);
    let scale = style.height / cap;
    let line_h = (face.ascender() as f64 - face.descender() as f64 + face.line_gap() as f64) * scale * style.line_spacing;
    let mut shapes = Vec::new();
    let mut missing = Vec::new();
    for (row, line) in text.lines().enumerate() {
        let mut x = origin.0;
        let y = origin.1 - row as f64 * line_h;
        for ch in line.chars() {
            let Some(g) = face.glyph_index(ch) else {
                missing.push(ch);
                continue;
            };
            let mut o = Outline { scale, origin: (x, y), tol: crate::curves::TOL, cur: Vec::new(), last: (0.0, 0.0), contours: Vec::new() };
            face.outline_glyph(g, &mut o);
            o.close();
            for c in o.contours {
                let v: Vec<(f64, f64, f64)> = c.into_iter().map(|(x, y)| (x, y, 0.0)).collect();
                shapes.push(Entity::Polyline(geom::pline_from(&v, true)));
            }
            x += face.glyph_hor_advance(g).unwrap_or(0) as f64 * scale + style.spacing;
        }
    }
    let missing: String = missing.into_iter().filter(|c| !c.is_whitespace()).collect();
    if shapes.is_empty() {
        return Err(if missing.is_empty() { "texte vide".into() } else { format!("caractères absents de la police : {missing}") });
    }
    Ok((shapes, missing))
}
