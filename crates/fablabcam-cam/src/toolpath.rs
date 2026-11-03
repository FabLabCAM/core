//! Génération de trajectoires : profil, poche (avec îlots), perçage, tabs et mouvements d'approche.
//!
//! Les règles de ponts, approches et liaisons sont isolées pour être testées sur des contours simples.

use crate::params::{CutOrdering, Lead, LeadKind, MillingDirection, Params, TabMethod, TabStyle};
use fablabcam_geom::{self as geom, Pline, PlineSource, PlineSourceMut, Segment};

#[derive(Debug, Clone, PartialEq)]
pub enum Move {
    Comment(String),
    Rapid { x: Option<f64>, y: Option<f64>, z: Option<f64> },
    Feed { x: Option<f64>, y: Option<f64>, z: Option<f64>, f: f64 },
    /// Arc dans le plan XY ; `i`, `j` relatifs au départ ; `z` renseigné = arc hélicoïdal.
    Arc { ccw: bool, x: f64, y: f64, z: Option<f64>, i: f64, j: f64, f: f64 },
    /// Perçage : `peck` = profondeur de picage (None = cycle simple G81) ; `dwell` = temporisation au fond
    /// en secondes (G82, sans picage seulement).
    Drill { x: f64, y: f64, z: f64, retract: f64, f: f64, peck: Option<f64>, dwell: f64 },
}

#[derive(Debug, Clone, Default)]
pub struct Toolpath {
    pub moves: Vec<Move>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Inside,
    Outside,
    /// Centre de l'outil sur le contour.
    On,
}

#[derive(Debug, PartialEq)]
pub enum CamError {
    /// Contour ouvert alors qu'une forme fermée est requise.
    OpenContour,
    /// Profondeur cible au-dessus ou égale à la surface du brut.
    NoDepth,
    /// L'outil est trop gros : le décalage ne laisse aucun contour.
    ToolTooLarge,
    EmptyInput,
    /// Le rayon du lead-in/out tangent sort de la zone dégagée.
    LeadDoesNotFit,
    /// `RoughingClearance` négatif de plus d'un rayon d'outil : l'axe de l'outil serait de l'autre côté de la paroi.
    ClearanceTooNegative,
}

impl std::fmt::Display for CamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CamError::OpenContour => "contour ouvert : une forme fermée est requise",
            CamError::NoDepth => "profondeur cible au-dessus de la surface du brut",
            CamError::ToolTooLarge => "outil trop gros pour cette forme",
            CamError::EmptyInput => "aucune géométrie à usiner",
            CamError::LeadDoesNotFit => "le rayon du lead-in/out tangent ne tient pas dans la zone dégagée",
            CamError::ClearanceTooNegative => "la surcoupe (RoughingClearance négatif) dépasse le rayon de l'outil",
        })
    }
}

impl std::error::Error for CamError {}

const EPS: f64 = 1e-9;

fn rapid_z(z: f64) -> Move {
    Move::Rapid { x: None, y: None, z: Some(z) }
}

/// La profondeur peut égaler la surface du brut : une passe à la surface (le logiciel de référence usine ainsi un
/// profil dont la profondeur cible est restée à 0).
fn check(p: &Params) -> Result<(), CamError> {
    if p.target_depth > p.stock_surface + EPS {
        return Err(CamError::NoDepth);
    }
    Ok(())
}

/// Profils et poches : l'axe de l'outil ne peut pas passer de l'autre côté de la paroi.
fn check_walls(p: &Params) -> Result<(), CamError> {
    check(p)?;
    if p.wall_offset() < 0.0 {
        return Err(CamError::ClearanceTooNegative);
    }
    Ok(())
}

// ───────────────────────── Marcheur de contour ─────────────────────────

/// Parcourt un contour en suivant un profil d'altitude linéaire par morceaux le long de l'abscisse
/// curviligne `s` (qui peut dépasser le périmètre : plusieurs tours). Les arcs deviennent des arcs
/// hélicoïdaux quand Z varie ; deux points de même `s` produisent une descente/montée verticale.
struct Walker {
    segs: Vec<Segment>,
    starts: Vec<f64>,
    perimeter: f64,
}

impl Walker {
    fn new(c: &Pline) -> Walker {
        let segs: Vec<Segment> = geom::segments(c).into_iter().filter(|s| s.length() > EPS).collect();
        let mut starts = Vec::with_capacity(segs.len());
        let mut acc = 0.0;
        for s in &segs {
            starts.push(acc);
            acc += s.length();
        }
        Walker { segs, starts, perimeter: acc }
    }

    fn walk(&self, zpts: &[(f64, f64)], rapid: &[(f64, f64)], p: &Params, f: f64, cur_z: &mut f64, out: &mut Vec<Move>) {
        let total = zpts.last().map_or(0.0, |z| z.0);
        let mut stops: Vec<f64> = zpts.iter().map(|z| z.0).collect();
        stops.extend(rapid.iter().flat_map(|r| [r.0, r.1]));
        let loops = (total / self.perimeter).ceil() as usize;
        for k in 0..=loops {
            for st in &self.starts {
                stops.push(st + k as f64 * self.perimeter);
            }
        }
        stops.retain(|s| *s >= -EPS && *s <= total + EPS);
        stops.sort_by(f64::total_cmp);
        stops.dedup_by(|a, b| (*a - *b).abs() < EPS);

        for w in stops.windows(2) {
            let (a, b) = (w[0], w[1]);
            if b - a < EPS {
                continue;
            }
            let mid = (a + b) / 2.0;
            let k = (mid / self.perimeter).floor();
            let local = mid - k * self.perimeter;
            let idx = self.starts.partition_point(|s| *s <= local).saturating_sub(1);
            let seg = self.segs[idx];
            let base = k * self.perimeter + self.starts[idx];
            let len = seg.length();
            let sub = seg.sub((a - base) / len, (b - base) / len);

            let Some(win) = zpts.windows(2).find(|z| z[0].0 <= a + EPS && z[1].0 >= b - EPS && z[1].0 > z[0].0 + EPS) else { continue };
            let zat = |s: f64| win[0].1 + (win[1].1 - win[0].1) * (s - win[0].0) / (win[1].0 - win[0].0);
            let (za, zb) = (zat(a), zat(b));
            // saut vertical (début de passe, bord de tab carrée…)
            if (za - *cur_z).abs() > EPS {
                out.push(Move::Feed { x: None, y: None, z: Some(za), f: p.plunge_feedrate });
                *cur_z = za;
            }
            let zmove = ((zb - *cur_z).abs() > EPS).then_some(zb);
            if rapid.iter().any(|r| mid > r.0 && mid < r.1) {
                out.push(Move::Rapid { x: Some(sub.to.0), y: Some(sub.to.1), z: zmove });
            } else if let (true, Some(c)) = (sub.is_arc(), sub.arc_center()) {
                out.push(Move::Arc { ccw: sub.is_ccw(), x: sub.to.0, y: sub.to.1, z: zmove, i: c.0 - sub.from.0, j: c.1 - sub.from.1, f });
            } else {
                out.push(Move::Feed { x: Some(sub.to.0), y: Some(sub.to.1), z: zmove, f });
            }
            *cur_z = zb;
        }
    }
}

// ───────────────────────────── Tabs ─────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
struct Tab {
    center: f64,
    half_top: f64,
    half_base: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapeKind {
    Outer,
    Inner,
    Unknown,
}

/// Positions des tabs sur un contour (abscisses curvilignes), selon les règles de la doc le logiciel de référence :
/// `round(périmètre / distance)` borné par [min, max] ; distance 0 → minimum ; périmètre sous le
/// seuil → aucune. La largeur est celle de la partie la plus étroite, augmentée du diamètre d'outil.
/// Demi-largeurs (au sommet, à la base) de la zone où le centre de l'outil franchit une tab.
/// Triangle : la matière laissée est un triangle de base `width` et de hauteur `height` ; le centre
/// d'une fraise plate reste à plat sur un diamètre au sommet et suit la pente des flancs (observé
/// sur le G-code du logiciel de référence : 1,5 mm de rampe pour 3 mm de haut, 6 mm à plat avec une fraise de 6).
/// Carré : un bloc de largeur `width`, franchi verticalement.
fn tab_size(p: &Params) -> (f64, f64) {
    let r = p.tool_diameter / 2.0;
    match p.tabs.style {
        TabStyle::Triangle => (r, r + p.tabs.width / 2.0),
        TabStyle::Square | TabStyle::Skip => (r + p.tabs.width / 2.0, r + p.tabs.width / 2.0),
    }
}

/// Positions des tabs automatiques le long d'une forme de longueur `len` (observé sur le logiciel de référence) :
/// n = arrondi(len / distance) ; si n est dans [min ; max], une tab tous les `distance` à partir de
/// `distance` / 2, sinon n est ramené dans l'intervalle et les tabs sont réparties uniformément.
fn auto_tab_positions(len: f64, t: &crate::params::Tabs) -> Vec<f64> {
    let (lo, hi) = (t.min, t.max.max(t.min));
    let ideal = if t.distance > 0.0 { (len / t.distance + 0.5).floor() as usize } else { lo };
    let (n, step) = if t.distance > 0.0 && (lo..=hi).contains(&ideal) { (ideal, t.distance) } else { (ideal.clamp(lo, hi), len / ideal.clamp(lo, hi).max(1) as f64) };
    (0..n).map(|i| (i as f64 + 0.5) * step).filter(|s| *s <= len + 1e-6).collect()
}

/// Point situé à l'abscisse curviligne `s` d'une polyligne.
fn point_along(c: &Pline, s: f64) -> Option<(f64, f64)> {
    let w = Walker::new(c);
    let idx = w.starts.partition_point(|st| *st <= s).saturating_sub(1);
    let seg = w.segs.get(idx)?;
    let t = ((s - w.starts[idx]) / seg.length()).clamp(0.0, 1.0);
    Some(if t <= 0.0 { seg.from } else { seg.sub(0.0, t).to })
}

/// Abscisse curviligne, sur le contour `c`, du point de `c` le plus proche de `pt`.
fn path_param(c: &Pline, w: &Walker, pt: (f64, f64)) -> Option<f64> {
    let (_, _, idx) = geom::closest_point(c, pt)?;
    // les segments de longueur nulle sont absents du Walker : on retrouve l'index par le départ
    let seg = *geom::segments(c).get(idx)?;
    let k = w.segs.iter().position(|s| s.from == seg.from && s.to == seg.to)?;
    Some(w.starts[k] + seg.param_of(pt) * w.segs[k].length())
}

/// Tabs d'un contour d'usinage `c`. Les positions automatiques sont calculées sur la forme dessinée
/// `shape` (si elle est connue) à partir de son premier sommet, puis reportées sur `c` (le logiciel de référence).
/// Sur un contour fermé, une tab qui chevauche le point de départ est écartée ; sur un contour
/// ouvert, une tab qui dépasse une extrémité est coupée.
fn tab_spans(c: &Pline, p: &Params, kind: ShapeKind, shape: Option<&Pline>) -> Vec<Tab> {
    let t = &p.tabs;
    let allowed = !matches!(
        (t.method, kind),
        (TabMethod::None, _) | (TabMethod::AutomaticOuter, ShapeKind::Inner) | (TabMethod::AutomaticInner, ShapeKind::Outer)
    );
    let src = shape.unwrap_or(c);
    let len = geom::perimeter(src);
    if !allowed || len < t.size_threshold || t.height <= 0.0 {
        return Vec::new();
    }
    let (half_top, half_base) = tab_size(p);
    let w = Walker::new(c);
    let perim = w.perimeter;
    let points: Vec<(f64, f64)> = if t.method == TabMethod::Manual {
        t.positions.clone()
    } else {
        auto_tab_positions(len, t).into_iter().filter_map(|s| point_along(src, s)).collect()
    };
    let mut centers: Vec<f64> = points.iter().filter_map(|&pt| path_param(c, &w, pt)).collect();
    centers.sort_by(f64::total_cmp);
    let closed = c.is_closed();
    let mut out: Vec<Tab> = Vec::new();
    for center in centers {
        // tabs manuelles : celle qui chevauche la précédente est écartée ; automatiques : les tabs
        // qui se chevauchent sont gardées et usinées en enveloppe (le logiciel de référence)
        let free = t.method != TabMethod::Manual || out.last().is_none_or(|l| center - half_base >= l.center + l.half_base - EPS);
        let inside = !closed || (center - half_base >= 0.0 && center + half_base <= perim);
        if free && inside {
            out.push(Tab { center, half_top, half_base });
        }
    }
    out
}

/// Profil d'altitude d'une passe à l'altitude `z` sur un contour de longueur `perim` : points
/// (abscisse curviligne, z) et intervalles franchis en rapide (tabs `Skip`).
type PassProfile = (Vec<(f64, f64)>, Vec<(f64, f64)>);

fn pass_profile(perim: f64, z: f64, tabs: &[Tab], p: &Params) -> PassProfile {
    let (target, h) = (p.target_depth, p.tabs.height);
    let zr = z - target;
    let top = target + h;
    if tabs.is_empty() || zr >= h - EPS {
        return (vec![(0.0, z), (perim, z)], Vec::new());
    }
    let triangle = p.tabs.style == TabStyle::Triangle;
    // tabs insérées une à une dans le profil (comme le logiciel de référence) : les sommets de la nouvelle tab sont
    // relevés au profil existant, et les points existants sous la tab sont relevés à son tracé ;
    // des tabs qui se chevauchent forment ainsi un seul relief sans jamais entamer l'une d'elles
    let mut prof: Vec<(f64, f64)> = Vec::new();
    for t in tabs {
        // demi-largeur à l'altitude `z` : pente du triangle, verticale pour une tab carrée
        let side = if triangle { t.half_top + (t.half_base - t.half_top) * (1.0 - zr / h) } else { t.half_top };
        let tv = [(t.center - side, z), (t.center - t.half_top, top), (t.center + t.half_top, top), (t.center + side, z)];
        if prof.is_empty() {
            prof = vec![(tv[0].0.min(0.0), z), (tv[3].0.max(perim), z)];
        }
        if let Some(last) = prof.last_mut() {
            last.0 = last.0.max(tv[3].0 + 1.0);
        }
        let (lo, hi) = (tv[0].0, tv[3].0);
        let raised: Vec<(f64, f64)> = tv.iter().map(|&(s, v)| (s, v.max(z_at(&prof, s)))).collect();
        let mut inner: Vec<(f64, f64)> = prof.iter().filter(|q| q.0 > lo + EPS && q.0 < hi - EPS).map(|&(s, v)| (s, v.max(z_at(&raised, s)))).collect();
        inner.retain(|q| !raised.iter().any(|r| (r.0 - q.0).abs() < EPS));
        let mut mid: Vec<(f64, f64)> = raised.into_iter().chain(inner).collect();
        mid.sort_by(|a, b| a.0.total_cmp(&b.0)); // tri stable : les bords verticaux gardent leur ordre
        let mut next: Vec<(f64, f64)> = prof.iter().copied().filter(|q| q.0 < lo - EPS).collect();
        next.extend(mid);
        next.extend(prof.iter().copied().filter(|q| q.0 > hi + EPS));
        prof = next;
    }
    let rapid = if p.tabs.style == TabStyle::Skip { tabs.iter().map(|t| ((t.center - t.half_top).max(0.0), (t.center + t.half_top).min(perim))).collect() } else { Vec::new() };
    (clip_profile(&prof, perim), rapid)
}

/// Altitude d'un profil (abscisse, z) en `s` ; sur un bord vertical, la plus haute.
fn z_at(pts: &[(f64, f64)], s: f64) -> f64 {
    let mut best: Option<f64> = None;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a.0 <= s + EPS && b.0 >= s - EPS {
            let v = if b.0 - a.0 < EPS { a.1.max(b.1) } else { a.1 + (b.1 - a.1) * ((s - a.0) / (b.0 - a.0)).clamp(0.0, 1.0) };
            best = Some(best.map_or(v, |x: f64| x.max(v)));
        }
    }
    best.unwrap_or_else(|| pts.first().map_or(0.0, |q| q.1))
}

/// Restreint un profil d'altitude à [0 ; perim] (tabs coupées aux extrémités d'un contour ouvert).
fn clip_profile(pts: &[(f64, f64)], perim: f64) -> Vec<(f64, f64)> {
    let mut out = vec![(0.0, z_at(pts, 0.0))];
    out.extend(pts.iter().copied().filter(|q| q.0 > EPS && q.0 < perim - EPS));
    out.push((perim, z_at(pts, perim)));
    out
}

// ───────────────────────── Mouvements d'approche ─────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Waste {
    Left,
    Right,
}

/// Arc tangent d'approche ou de sortie : (segment, centre).
fn tangent_arc(start: (f64, f64), tangent: (f64, f64), waste: Waste, r: f64, entering: bool) -> Segment {
    let n = match waste {
        Waste::Left => (-tangent.1, tangent.0),
        Waste::Right => (tangent.1, -tangent.0),
    };
    let c = (start.0 + n.0 * r, start.1 + n.1 * r);
    let dir = if waste == Waste::Left { 1.0 } else { -1.0 }; // centre à gauche = rotation anti-horaire
    let a_s = (start.1 - c.1).atan2(start.0 - c.0);
    let a_o = a_s + if entering { -dir } else { dir } * std::f64::consts::FRAC_PI_2;
    let other = (c.0 + r * a_o.cos(), c.1 + r * a_o.sin());
    let bulge = (dir * std::f64::consts::FRAC_PI_8).tan();
    if entering {
        Segment { from: other, to: start, bulge }
    } else {
        Segment { from: start, to: other, bulge }
    }
}

struct Ctx<'a> {
    p: &'a Params,
    waste: Option<Waste>,
    kind: ShapeKind,
    /// (forme d'origine, la zone dégagée est-elle l'intérieur ?) pour vérifier que les arcs tiennent.
    fit: Option<(&'a Pline, bool)>,
}

impl Ctx<'_> {
    fn lead_feed(&self, l: &Lead) -> f64 {
        if l.feedrate > 0.0 {
            l.feedrate
        } else {
            self.p.cut_feedrate
        }
    }

    fn arc_fits(&self, arc: &Segment) -> bool {
        match self.fit {
            None => true,
            Some((shape, free_inside)) => [arc.from, arc.point_at(0.5), arc.to].iter().all(|&pt| geom::contains_point(shape, pt) == free_inside),
        }
    }
}

/// Comment un contour est entamé et quitté.
#[derive(Debug, Clone, Copy, Default)]
struct Entry {
    /// L'outil est déjà à cette position, à la profondeur du niveau : on s'y relie par une
    /// transition horizontale au lieu de retirer/plonger.
    link_from: Option<(f64, f64)>,
    /// Retrait au plan de dégagement en fin de contour (faux si on enchaîne par une transition).
    keep_down: bool,
}

/// Oriente un contour selon `MillingDirection` : la paroi usinée est du côté opposé au côté dégagé.
/// Broche horaire : en avalant (Climb) la paroi est à droite du sens de parcours (contour extérieur
/// parcouru en sens horaire, comme `G41` + `M3`) ; en conventionnel elle est à gauche (sens
/// trigonométrique autour d'un contour extérieur, observé sur le G-code du logiciel de référence).
fn orient(path: Pline, waste: Option<Waste>, p: &Params) -> (Pline, Option<Waste>) {
    let Some(w) = waste else { return (path, None) };
    let want_wall_left = match p.milling {
        MillingDirection::Mixed => return (path, waste),
        MillingDirection::Climb => !p.spindle_cw,
        MillingDirection::Conventional => p.spindle_cw,
    };
    let wall_left = w == Waste::Right;
    if wall_left == want_wall_left {
        (path, waste)
    } else {
        let flipped = if w == Waste::Left { Waste::Right } else { Waste::Left };
        (geom::reversed(&path), Some(flipped))
    }
}

/// Surcoupe de coin (« dog-bone ») : à chaque coin vif du trajet dont le virage est tourné vers
/// la zone dégagée, l'outil va jusqu'à ce que son cercle atteigne le coin de la forme, puis revient.
/// Pour un virage d'angle α, le coin est à `r / sin((π − α)/2)` du sommet du trajet.
fn add_overcut(c: &Pline, r: f64, waste: Waste) -> Pline {
    let n = c.vertex_count();
    if n < 3 || !c.is_closed() {
        return c.clone();
    }
    let mut out = Pline::new_closed();
    for i in 0..n {
        let (prev, v, next) = (c.at((i + n - 1) % n), c.at(i), c.at((i + 1) % n));
        let sharp = prev.bulge.abs() < EPS && v.bulge.abs() < EPS;
        let unit = |ax: f64, ay: f64| {
            let l = ax.hypot(ay).max(EPS);
            (ax / l, ay / l)
        };
        let (d1, d2) = (unit(v.x - prev.x, v.y - prev.y), unit(next.x - v.x, next.y - v.y));
        let cross = d1.0 * d2.1 - d1.1 * d2.0;
        let turn = cross.atan2(d1.0 * d2.0 + d1.1 * d2.1);
        let toward_waste = (waste == Waste::Left && cross > 0.0) || (waste == Waste::Right && cross < 0.0);
        let angle = turn.abs();
        if sharp && toward_waste && angle > 20f64.to_radians() && angle < 150f64.to_radians() {
            let dir = unit(d1.0 - d2.0, d1.1 - d2.1); // bissectrice extérieure du virage
            let m = r / ((std::f64::consts::PI - angle) / 2.0).sin() - r;
            out.add(v.x, v.y, 0.0);
            out.add(v.x + dir.0 * m, v.y + dir.1 * m, 0.0);
            out.add(v.x, v.y, v.bulge);
        } else {
            out.add(v.x, v.y, v.bulge);
        }
    }
    out
}

// ───────────────────────── Découpe d'un contour fermé ─────────────────────────

/// Usine un contour fermé sur les niveaux donnés (`levels`, de haut en bas), en partant de
/// l'altitude `z_prev` : approche, descente (plongée, spirale ou arc tangent), passes plates avec
/// tabs, sortie. Se termine toujours en retrait au plan de dégagement.
fn cut_closed(c: &Pline, levels: &[f64], mut z_prev: f64, ctx: &Ctx, entry: Entry, out: &mut Vec<Move>) -> Result<(), CamError> {
    let p = ctx.p;
    let w = Walker::new(c);
    let (Some(first), true) = (w.segs.first(), !levels.is_empty()) else { return Ok(()) };
    let start = first.from;
    let tangent = first.tangent_at(0.0);
    let perim = w.perimeter;
    let tabs = tab_spans(c, p, ctx.kind, ctx.fit.map(|f| f.0));
    let target = p.target_depth;

    let lead_in = &p.lead_in;
    let tangent_in = lead_in.kind == LeadKind::Tangent && lead_in.tangent_radius > 0.0 && ctx.waste.is_some();
    let tangent_out = p.lead_out.kind == LeadKind::Tangent && p.lead_out.tangent_radius > 0.0 && ctx.waste.is_some();

    let arc_in = match (tangent_in && entry.link_from.is_none(), ctx.waste) {
        (true, Some(wst)) => Some(tangent_arc(start, tangent, wst, lead_in.tangent_radius, true)),
        _ => None,
    };
    let arc_out = match (tangent_out, ctx.waste) {
        (true, Some(wst)) => Some(tangent_arc(start, tangent, wst, p.lead_out.tangent_radius, false)),
        _ => None,
    };
    if arc_in.iter().chain(arc_out.iter()).any(|a| !ctx.arc_fits(a)) {
        return Err(CamError::LeadDoesNotFit);
    }
    let approach = arc_in.map_or(start, |a| a.from);

    // retrait, déplacement au-dessus du départ, descente rapide jusqu'à `RAPID_DOWN` au-dessus du
    // niveau déjà usiné (rien si le niveau suivant n'est pas plus bas), le reste en plongée
    let reposition = |out: &mut Vec<Move>, z_prev: f64, z_next: f64| -> f64 {
        if !at_clearance(out, p) {
            out.push(rapid_z(p.clearance_plane));
        }
        out.push(Move::Rapid { x: Some(approach.0), y: Some(approach.1), z: None });
        rapid_down(p, z_prev, z_next, out)
    };
    let mut cur_z;
    if let Some(from) = entry.link_from {
        // transition horizontale à la profondeur courante, sans retrait ni plongée
        cur_z = levels[0];
        z_prev = levels[0];
        if (from.0 - start.0).hypot(from.1 - start.1) > EPS {
            let f = if p.stepover_feedrate > 0.0 { p.stepover_feedrate } else { p.cut_feedrate };
            out.push(Move::Feed { x: Some(start.0), y: Some(start.1), z: None, f });
        }
    } else {
        cur_z = reposition(out, z_prev, levels[0]);
    }

    for (idx, &z) in levels.iter().enumerate() {
        let last = idx + 1 == levels.len();
        let tabs_here = !tabs.is_empty() && z - target < p.tabs.height - EPS;
        let descent = (z_prev - z).max(0.0);
        let mut ramped = false;

        if let Some(arc) = arc_in {
            // arc tangent ; avec un angle de spirale il descend en hélice, le reste de la descente
            // se fait en plongée verticale au début de l'arc
            let dz = if lead_in.spiral_angle > 0.0 { (arc.length() * lead_in.spiral_angle.to_radians().tan()).min(descent) } else { 0.0 };
            let z_start = z + dz;
            if (cur_z - z_start).abs() > EPS {
                out.push(Move::Feed { x: None, y: None, z: Some(z_start), f: p.plunge_feedrate });
            }
            let ctr = arc.arc_center().unwrap_or(arc.from);
            out.push(Move::Arc { ccw: arc.is_ccw(), x: start.0, y: start.1, z: (dz > 0.0).then_some(z), i: ctr.0 - arc.from.0, j: ctr.1 - arc.from.1, f: ctx.lead_feed(lead_in) });
            cur_z = z;
        } else if lead_in.kind == LeadKind::Spiral && !tabs_here && descent > EPS {
            // rampe le long du contour : angle 0 = un incrément de profondeur par tour
            let slope = if lead_in.spiral_angle > 0.0 { lead_in.spiral_angle.to_radians().tan() } else { descent / perim };
            let length = descent / slope;
            let zpts = if length < perim - EPS {
                vec![(0.0, z_prev), (length, z), (perim, z)]
            } else {
                vec![(0.0, z_prev), ((length / perim).ceil() * perim, z)]
            };
            w.walk(&zpts, &[], p, ctx.lead_feed(lead_in), &mut cur_z, out);
            ramped = true;
        }

        // la rampe remplace la passe de son niveau, sauf au niveau final où une passe plate
        // garantit un fond de coupe plan
        if !ramped || last {
            let (zpts, rapid) = pass_profile(perim, z, &tabs, p);
            w.walk(&zpts, &rapid, p, p.cut_feedrate, &mut cur_z, out);
        }

        if let Some(arc) = arc_out {
            let ctr = arc.arc_center().unwrap_or(arc.from);
            out.push(Move::Arc { ccw: arc.is_ccw(), x: arc.to.0, y: arc.to.1, z: None, i: ctr.0 - arc.from.0, j: ctr.1 - arc.from.1, f: ctx.lead_feed(&p.lead_out) });
        }
        if !last && arc_in.is_some() {
            cur_z = reposition(out, z, levels[idx + 1]); // nouvelle approche tangente à chaque niveau
        }
        z_prev = z;
    }
    if !entry.keep_down {
        out.push(rapid_z(p.clearance_plane));
    }
    Ok(())
}

/// Le dernier mouvement émis (commentaires ignorés) est-il déjà un retrait au plan de dégagement ?
/// Un trajet vide ne l'est pas : une opération ne suppose jamais la machine déjà en hauteur.
fn at_clearance(out: &[Move], p: &Params) -> bool {
    out.iter().rev().find(|m| !matches!(m, Move::Comment(_))).is_some_and(|m| *m == rapid_z(p.clearance_plane))
}

/// Marge de la descente rapide au-dessus de la matière déjà usinée (le logiciel de référence : `G0 Z1.0` au-dessus
/// d'une surface à 0, puis 1 mm au-dessus de la passe précédente).
const RAPID_DOWN: f64 = 1.0;

/// Descente rapide avant une passe au niveau `z_next`, la matière étant usinée jusqu'à `z_prev` ;
/// renvoie l'altitude atteinte (le plan de dégagement si aucune descente rapide n'est utile).
fn rapid_down(p: &Params, z_prev: f64, z_next: f64, out: &mut Vec<Move>) -> f64 {
    let z = (z_prev + RAPID_DOWN).min(p.clearance_plane);
    if z_next < z_prev - EPS && z < p.clearance_plane - EPS {
        out.push(rapid_z(z));
        return z;
    }
    p.clearance_plane
}

/// Contour ouvert : à chaque passe, retrait, retour au début et parcours dans le même sens (tabs
/// coupées aux extrémités, sans lead moves).
fn cut_open(c: &Pline, levels: &[f64], p: &Params, kind: ShapeKind, shape: Option<&Pline>, out: &mut Vec<Move>) {
    let w = Walker::new(c);
    let Some(first) = w.segs.first() else { return };
    let start = first.from;
    let tabs = tab_spans(c, p, kind, shape);
    let mut z_prev = p.stock_surface;
    for &z in levels {
        if !at_clearance(out, p) {
            out.push(rapid_z(p.clearance_plane));
        }
        out.push(Move::Rapid { x: Some(start.0), y: Some(start.1), z: None });
        let mut cur_z = rapid_down(p, z_prev, z, out);
        let (zpts, rapid) = pass_profile(w.perimeter, z, &tabs, p);
        w.walk(&zpts, &rapid, p, p.cut_feedrate, &mut cur_z, out);
        z_prev = z;
    }
    out.push(rapid_z(p.clearance_plane));
}

// ───────────────────────────── Opérations ─────────────────────────────

/// Profil d'une forme (intérieur, extérieur ou sur le trait), avec tabs, lead moves, sens de
/// fraisage et surcoupe de coin du style.
pub fn profile(shape: &Pline, side: Side, p: &Params) -> Result<Toolpath, CamError> {
    check_walls(p)?;
    let r = p.wall_offset();
    let contours: Vec<Pline> = match side {
        Side::On => vec![shape.clone()],
        // forme ouverte (observé sur le logiciel de référence) : décalage à droite du sens de tracé pour « extérieur »,
        // à gauche pour « intérieur », parcouru dans le sens du tracé en fraisage conventionnel
        _ if !shape.is_closed() => {
            let off = geom::offset_open(shape, if side == Side::Outside { -r } else { r });
            // la forme est à gauche du sens de parcours : fraisage conventionnel avec une broche horaire
            let reverse = match p.milling {
                MillingDirection::Mixed => false,
                MillingDirection::Climb => p.spindle_cw,
                MillingDirection::Conventional => !p.spindle_cw,
            };
            off.into_iter().map(|c| if reverse { geom::reversed(&c) } else { c }).collect()
        }
        Side::Outside => geom::outset(shape, r),
        Side::Inside => geom::inset(shape, r),
    };
    if contours.is_empty() {
        return Err(CamError::ToolTooLarge);
    }
    // le contour décalé est parcouru dans le sens trigonométrique : la matière retirée est à droite
    // pour un profil extérieur, à gauche pour un profil intérieur
    let base_waste = match side {
        Side::Outside => Some(Waste::Right),
        Side::Inside => Some(Waste::Left),
        Side::On => None,
    };
    let kind = match side {
        Side::Outside => ShapeKind::Outer,
        Side::Inside => ShapeKind::Inner,
        Side::On => ShapeKind::Unknown,
    };
    let fit = match side {
        Side::Outside => Some((shape, false)),
        Side::Inside => Some((shape, true)),
        Side::On => None,
    };
    let levels = p.depth_levels();
    let mut moves = vec![Move::Comment(format!("Profil {:?}", side))];
    for c in &contours {
        if c.is_closed() {
            let (path, waste) = orient(c.clone(), base_waste, p);
            let path = match side {
                Side::On => path,
                _ => offset_profile_start(&path, shape, r),
            };
            let path = match (p.corner_overcut, waste) {
                (true, Some(w)) => add_overcut(&path, p.tool_radius(), w),
                _ => path,
            };
            let ctx = Ctx { p, waste, kind, fit };
            cut_closed(&path, &levels, p.stock_surface, &ctx, Entry::default(), &mut moves)?;
        } else {
            cut_open(c, &levels, p, kind, Some(shape), &mut moves);
        }
    }
    Ok(Toolpath { moves })
}

/// Départ d'un profil décalé (observé sur le logiciel de référence) : le premier sommet de la forme, décalé de `r`
/// perpendiculairement à son premier côté, du côté du contour d'usinage.
fn offset_profile_start(path: &Pline, shape: &Pline, r: f64) -> Pline {
    let Some(first) = geom::segments(shape).into_iter().find(|s| s.length() > EPS) else { return path.clone() };
    let (tx, ty) = first.tangent_at(0.0);
    let n = tx.hypot(ty);
    if n < EPS || r <= 0.0 {
        return path.clone();
    }
    let (nx, ny) = (-ty / n * r, tx / n * r);
    let v = first.from;
    let dist = |pt: (f64, f64)| geom::closest_point(path, pt).map_or(f64::INFINITY, |(q, _, _)| (q.0 - pt.0).hypot(q.1 - pt.1));
    let (a, b) = ((v.0 + nx, v.1 + ny), (v.0 - nx, v.1 - ny));
    geom::rotate_start(path, if dist(a) <= dist(b) { a } else { b })
}

struct Ring {
    path: Pline,
    hole: bool,
}

/// Anneaux d'une poche : à chaque pas, contour décalé vers l'intérieur privé des îlots décalés
/// vers l'extérieur (rayon `d0` + k × pas). Les trous (îlots) sont des anneaux à part entière.
/// `single` : un seul décalage (anneau de finition).
fn pocket_rings(outer: &Pline, islands: &[Pline], p: &Params, d0: f64, single: bool) -> Vec<Ring> {
    let step = (p.step_over * p.tool_diameter).max(1e-6);
    let live = |v: Vec<Pline>| -> Vec<Pline> { v.into_iter().filter(|r| geom::area(r) > 1e-4).collect() };
    let mut rings = Vec::new();
    let mut d = d0;
    let mut last_area = f64::INFINITY;
    for _ in 0..10_000 {
        let mut regions = live(geom::inset(outer, d));
        let grown: Vec<Pline> = islands.iter().flat_map(|i| geom::outset(i, d)).collect();
        let mut holes: Vec<Pline> = Vec::new();
        for isl in geom::union_all(&grown) {
            let mut next = Vec::new();
            for r in regions {
                let res = geom::difference(&r, &isl);
                next.extend(res.outlines);
                holes.extend(res.holes);
            }
            regions = next;
        }
        let regions = live(regions);
        if regions.is_empty() {
            break;
        }
        // l'aire dégagée doit décroître strictement, sinon le décalage ne progresse plus
        let total: f64 = regions.iter().map(geom::area).sum::<f64>() - holes.iter().map(geom::area).sum::<f64>();
        if total >= last_area - 1e-9 {
            break;
        }
        last_area = total;
        rings.extend(regions.into_iter().map(|path| Ring { path, hole: false }));
        rings.extend(holes.into_iter().map(|path| Ring { path, hole: true }));
        if single {
            break;
        }
        d += step;
    }
    rings
}

/// Zone où le centre de l'outil peut se déplacer librement (tolérance de 1 µm sur les bords).
struct FreeZone {
    free: Vec<Pline>,
    keepout: Vec<Pline>,
}

impl FreeZone {
    fn new(outer: &Pline, islands: &[Pline], r: f64) -> FreeZone {
        let r = (r - 1e-3).max(0.0);
        let grown: Vec<Pline> = islands.iter().flat_map(|i| geom::outset(i, r)).collect();
        FreeZone { free: geom::inset(outer, r), keepout: geom::union_all(&grown) }
    }

    /// Le segment a→b reste-t-il dans la zone libre (aucun gougeage d'îlot ni de paroi) ?
    fn segment_is_clear(&self, a: (f64, f64), b: (f64, f64)) -> bool {
        (0..=8).all(|k| {
            let t = k as f64 / 8.0;
            let pt = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            self.free.iter().any(|f| geom::contains_point(f, pt)) && !self.keepout.iter().any(|f| geom::contains_point(f, pt))
        })
    }
}

fn ring_ctx<'a>(r: &Ring, p: &'a Params) -> (Pline, Ctx<'a>) {
    // anneau extérieur anti-horaire : zone dégagée à gauche ; un anneau d'îlot est inversé
    let waste = Some(if (r.path.area() > 0.0) != r.hole { Waste::Left } else { Waste::Right });
    let (path, waste) = orient(r.path.clone(), waste, p);
    (path, Ctx { p, waste, kind: ShapeKind::Unknown, fit: None })
}

/// Poche d'un contour avec ses îlots.
///
/// * `FinishStepover` > 0 : un anneau de finition sur la paroi (décalage = rayon d'outil) est coupé
///   en dernier, l'ébauche commençant à `rayon + FinishStepover` ; avec `FinishStepoverAtTargetDepth`
///   cela n'a lieu qu'à la profondeur cible.
/// * `MaxCrossoverDistance` > 0 (ordre par niveau) : les anneaux sont enchaînés par une transition
///   horizontale quand la distance ne dépasse pas `fraction × diamètre` et que le segment ne gouge
///   ni paroi ni îlot ; sinon retrait, rapide et plongée. Pas de transition avec un lead-out tangent.
/// * Avec une finition, l'usinage se fait niveau par niveau quel que soit `CutOrdering`.
///
/// Limites connues : un résidu de moins d'un pas peut rester au centre.
pub fn pocket_region(outer: &Pline, islands: &[Pline], p: &Params) -> Result<Toolpath, CamError> {
    check_walls(p)?;
    if !outer.is_closed() {
        return Err(CamError::OpenContour);
    }
    let r = p.wall_offset();
    let standard = pocket_rings(outer, islands, p, r, false);
    if standard.is_empty() {
        return Err(CamError::ToolTooLarge);
    }
    let finishing = p.finish_stepover > 0.0;
    let (rough, finish) = if finishing {
        (pocket_rings(outer, islands, p, r + p.finish_stepover, false), pocket_rings(outer, islands, p, r, true))
    } else {
        (Vec::new(), Vec::new())
    };
    let levels = p.depth_levels();
    let mut moves = vec![Move::Comment(if islands.is_empty() { "Poche".into() } else { format!("Poche, {} {}", islands.len(), if islands.len() == 1 { "îlot" } else { "îlots" }) })];

    if !finishing && p.cut_ordering == CutOrdering::DepthFirst {
        for ring in &standard {
            let (path, ctx) = ring_ctx(ring, p);
            cut_closed(&path, &levels, p.stock_surface, &ctx, Entry::default(), &mut moves)?;
        }
        return Ok(Toolpath { moves });
    }

    let zone = FreeZone::new(outer, islands, r);
    let tangent_out = p.lead_out.kind == LeadKind::Tangent && p.lead_out.tangent_radius > 0.0;
    let crossover = p.max_crossover * p.tool_diameter;
    let mut z_prev = p.stock_surface;
    for &z in &levels {
        let sets: Vec<&Vec<Ring>> = if !finishing || (p.finish_at_target_only && (z - p.target_depth).abs() > EPS) {
            vec![&standard]
        } else {
            vec![&rough, &finish]
        };
        let mut prev_end: Option<(f64, f64)> = None;
        for set in sets {
            let mut remaining: Vec<&Ring> = set.iter().collect();
            while !remaining.is_empty() {
                // avec transitions : anneau suivant = le plus proche de la position courante
                let pick = match prev_end {
                    Some(pe) if crossover > 0.0 => (0..remaining.len())
                        .min_by(|&a, &b| {
                            let d = |i: usize| geom::closest_point(&remaining[i].path, pe).map_or(f64::INFINITY, |c| c.1);
                            d(a).total_cmp(&d(b))
                        })
                        .unwrap_or(0),
                    _ => 0,
                };
                let ring = remaining.remove(pick);
                let (mut path, ctx) = ring_ctx(ring, p);
                let mut entry = Entry { link_from: None, keep_down: true };
                if let (Some(pe), true) = (prev_end, crossover > 0.0) {
                    if let Some((cp, d, _)) = geom::closest_point(&path, pe) {
                        if d <= crossover && zone.segment_is_clear(pe, cp) {
                            path = geom::rotate_start(&path, pe);
                            entry.link_from = Some(pe);
                        }
                    }
                }
                cut_closed(&path, &[z], z_prev, &ctx, entry, &mut moves)?;
                prev_end = if tangent_out && ctx.waste.is_some() { None } else { geom::start_point(&path) };
            }
        }
        moves.push(rapid_z(p.clearance_plane));
        z_prev = z;
    }
    Ok(Toolpath { moves })
}

/// Poche d'une forme sans îlot.
pub fn pocket(shape: &Pline, p: &Params) -> Result<Toolpath, CamError> {
    pocket_region(shape, &[], p)
}

/// Poches d'un ensemble de formes : une forme contenue dans une autre est détectée comme îlot
/// (comportement documenté du logiciel de référence), une forme dans un îlot redevient un contour à creuser.
pub fn pocket_shapes(shapes: &[Pline], p: &Params) -> Result<Toolpath, CamError> {
    let regions = geom::nest(shapes);
    if regions.is_empty() {
        return Err(CamError::EmptyInput);
    }
    let mut all = Toolpath::default();
    for (outer, islands) in regions {
        all.moves.extend(pocket_region(&outer, &islands, p)?.moves);
    }
    Ok(all)
}

/// Perçage de points. Avec `depth_increment` > 0, perçage par picage.
pub fn drill(points: &[(f64, f64)], p: &Params) -> Result<Toolpath, CamError> {
    check(p)?;
    if points.is_empty() {
        return Err(CamError::EmptyInput);
    }
    let peck = (p.depth_increment > 0.0).then_some(p.depth_increment);
    let dwell = if peck.is_none() { p.drill_dwell.max(0.0) } else { 0.0 };
    let mut moves = vec![Move::Comment("Perçage".into()), rapid_z(p.clearance_plane)];
    for &(x, y) in points {
        moves.push(Move::Drill { x, y, z: p.target_depth, retract: p.clearance_plane, f: p.plunge_feedrate, peck, dwell });
    }
    moves.push(rapid_z(p.clearance_plane));
    Ok(Toolpath { moves })
}

/// Trajectoire mise à l'échelle `k` (positions, centres d'arcs, avances, profondeurs de picage) : sert à sortir
/// le G-code en pouces (`k` = 1 / 25,4). Les temporisations (secondes) ne changent pas.
pub fn scaled(tp: &Toolpath, k: f64) -> Toolpath {
    let o = |v: &Option<f64>| v.map(|x| x * k);
    let moves = tp
        .moves
        .iter()
        .map(|m| match m {
            Move::Comment(c) => Move::Comment(c.clone()),
            Move::Rapid { x, y, z } => Move::Rapid { x: o(x), y: o(y), z: o(z) },
            Move::Feed { x, y, z, f } => Move::Feed { x: o(x), y: o(y), z: o(z), f: f * k },
            Move::Arc { ccw, x, y, z, i, j, f } => Move::Arc { ccw: *ccw, x: x * k, y: y * k, z: o(z), i: i * k, j: j * k, f: f * k },
            Move::Drill { x, y, z, retract, f, peck, dwell } => Move::Drill { x: x * k, y: y * k, z: z * k, retract: retract * k, f: f * k, peck: o(peck), dwell: *dwell },
        })
        .collect();
    Toolpath { moves }
}
