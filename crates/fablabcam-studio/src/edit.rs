//! Modification d'objets existants : propriétés numériques d'un rectangle, d'un cercle, d'un arc et édition
//! des sommets d'une polyligne. Fonctions pures `entité → entité` ; `Project::edit_shape` les applique de façon
//! annulable et les deux interfaces n'appellent que celles-ci.

use crate::shape::EditError;
use fablabcam_geom::{self as geom, Pline, PlineSource};
use fablabcam_model::{rect_pline, Entity};

const EPS: f64 = 1e-9;

fn bad(m: &str) -> EditError {
    EditError::InvalidParameter(m.to_string())
}

fn vertices(p: &Pline) -> Vec<(f64, f64, f64)> {
    (0..p.vertex_count()).map(|i| {
        let v = p.at(i);
        (v.x, v.y, v.bulge)
    }).collect()
}

fn poly(v: &[(f64, f64, f64)], closed: bool) -> Entity {
    Entity::Polyline(geom::pline_from(v, closed))
}

type Vertices = Vec<(f64, f64, f64)>;

fn polyline_of(e: &Entity) -> Result<(&Pline, Vertices), EditError> {
    match e {
        Entity::Polyline(p) => Ok((p, vertices(p))),
        _ => Err(bad("cette action ne s'applique qu'à une polyligne")),
    }
}

/// Reconnaît un rectangle (4 sommets, droit, sens trigonométrique, sans arc) parmi les polylignes : les
/// fichiers anciens et les import DXF/SVG les décrivent ainsi.
pub fn recognize_rect(e: &Entity) -> Option<Entity> {
    let Entity::Polyline(p) = e else { return None };
    if !p.is_closed() || p.vertex_count() != 4 {
        return None;
    }
    let v = vertices(p);
    if v.iter().any(|q| q.2.abs() > 1e-12) {
        return None;
    }
    let e1 = (v[1].0 - v[0].0, v[1].1 - v[0].1);
    let e2 = (v[3].0 - v[0].0, v[3].1 - v[0].1);
    let (w, h) = (e1.0.hypot(e1.1), e2.0.hypot(e2.1));
    let scale = w.max(h);
    let tol = 1e-7 * scale.max(1.0);
    let dot = e1.0 * e2.0 + e1.1 * e2.1;
    let cross = e1.0 * e2.1 - e1.1 * e2.0;
    // le 3e sommet ferme le parallélogramme, l'angle est droit, le sens est trigonométrique
    let closes = (v[2].0 - v[1].0 - e2.0).abs() < tol && (v[2].1 - v[1].1 - e2.1).abs() < tol;
    (w > EPS && h > EPS && closes && dot.abs() < tol * scale && cross > 0.0).then(|| Entity::Rect { x: v[0].0, y: v[0].1, w, h, r: 0.0, rot_deg: e1.1.atan2(e1.0).to_degrees() })
}

/// Rectangle de l'entité, qu'elle soit déjà paramétrique ou une polyligne reconnue.
pub fn as_rect(e: &Entity) -> Option<Entity> {
    match e {
        Entity::Rect { .. } => Some(e.clone()),
        _ => recognize_rect(e),
    }
}

/// Nouveaux paramètres d'un rectangle : origine, taille, rayon de coin et rotation autour de l'origine.
pub fn set_rect(e: &Entity, x: f64, y: f64, w: f64, h: f64, r: f64, rot_deg: f64) -> Result<Entity, EditError> {
    if as_rect(e).is_none() {
        return Err(bad("la forme n'est pas un rectangle"));
    }
    if ![x, y, w, h, r, rot_deg].iter().all(|v| v.is_finite()) {
        return Err(bad("valeur non numérique"));
    }
    if w < EPS || h < EPS {
        return Err(bad("la largeur et la hauteur doivent être > 0"));
    }
    if r < 0.0 {
        return Err(bad("le rayon de coin ne peut pas être négatif"));
    }
    if r > w.min(h) / 2.0 + EPS {
        return Err(bad("le rayon de coin ne peut pas dépasser la moitié du plus petit côté"));
    }
    Ok(Entity::Rect { x, y, w, h, r, rot_deg })
}

pub fn set_circle(e: &Entity, cx: f64, cy: f64, r: f64) -> Result<Entity, EditError> {
    if !matches!(e, Entity::Circle { .. }) {
        return Err(bad("la forme n'est pas un cercle"));
    }
    if ![cx, cy, r].iter().all(|v| v.is_finite()) || r < EPS {
        return Err(bad("le rayon doit être > 0"));
    }
    Ok(Entity::Circle { cx, cy, r })
}

pub fn set_arc(e: &Entity, cx: f64, cy: f64, r: f64, start_deg: f64, sweep_deg: f64) -> Result<Entity, EditError> {
    if !matches!(e, Entity::Arc { .. }) {
        return Err(bad("la forme n'est pas un arc"));
    }
    if ![cx, cy, r, start_deg, sweep_deg].iter().all(|v| v.is_finite()) || r < EPS || sweep_deg.abs() < 1e-6 {
        return Err(bad("rayon et angle de l'arc doivent être non nuls"));
    }
    Ok(Entity::Arc { cx, cy, r, start_deg, sweep_deg })
}

/// Sommet `i` d'une polyligne déplacé en `pt` (les extrémités d'une ligne sont ses sommets 0 et 1).
pub fn set_vertex(e: &Entity, i: usize, pt: (f64, f64)) -> Result<Entity, EditError> {
    let (p, mut v) = polyline_of(e)?;
    if !(pt.0.is_finite() && pt.1.is_finite()) {
        return Err(bad("coordonnée non numérique"));
    }
    let b = v.get(i).ok_or_else(|| bad("sommet inexistant"))?.2;
    v[i] = (pt.0, pt.1, b);
    Ok(poly(&v, p.is_closed()))
}

/// Insère un sommet sur le segment `seg` (de `seg` au suivant), à la fraction `t` ∈ ]0 ; 1[ ; l'arc éventuel du
/// segment est coupé en deux arcs de même cercle, donc la forme ne change pas.
pub fn insert_vertex(e: &Entity, seg: usize, t: f64) -> Result<Entity, EditError> {
    let (p, v) = polyline_of(e)?;
    let segs = geom::segments(p);
    let s = segs.get(seg).ok_or_else(|| bad("segment inexistant"))?;
    if !(t > 1e-6 && t < 1.0 - 1e-6) {
        return Err(bad("la position d'insertion doit être strictement à l'intérieur du segment"));
    }
    let q = s.point_at(t);
    let theta = 4.0 * v[seg].2.atan();
    let (b1, b2) = (((t * theta) / 4.0).tan(), (((1.0 - t) * theta) / 4.0).tan());
    let mut out = v.clone();
    out[seg].2 = b1;
    out.insert(seg + 1, (q.0, q.1, b2));
    Ok(poly(&out, p.is_closed()))
}

/// Supprime le sommet `i` ; le segment qui le remplace est droit. Une polyligne garde au moins 2 sommets
/// (3 si elle est fermée).
pub fn remove_vertex(e: &Entity, i: usize) -> Result<Entity, EditError> {
    let (p, mut v) = polyline_of(e)?;
    let min = if p.is_closed() { 3 } else { 2 };
    if v.len() <= min {
        return Err(bad(&format!("une polyligne {} garde au moins {min} sommets", if p.is_closed() { "fermée" } else { "ouverte" })));
    }
    if i >= v.len() {
        return Err(bad("sommet inexistant"));
    }
    v.remove(i);
    let before = (i + v.len() - 1) % v.len();
    if p.is_closed() || i > 0 {
        v[before].2 = 0.0;
    }
    Ok(poly(&v, p.is_closed()))
}

/// Courbure (bulge = tan(balayage / 4)) du segment qui part du sommet `i`.
pub fn set_bulge(e: &Entity, i: usize, bulge: f64) -> Result<Entity, EditError> {
    let (p, mut v) = polyline_of(e)?;
    if !bulge.is_finite() {
        return Err(bad("bulge non numérique"));
    }
    if i >= v.len() || (!p.is_closed() && i + 1 == v.len()) {
        return Err(bad("ce sommet ne commence aucun segment"));
    }
    v[i].2 = bulge;
    Ok(poly(&v, p.is_closed()))
}

/// Ferme ou ouvre la polyligne (fermer demande 3 sommets au moins).
pub fn set_closed(e: &Entity, closed: bool) -> Result<Entity, EditError> {
    let (p, mut v) = polyline_of(e)?;
    if closed && v.len() < 3 {
        return Err(bad("fermer demande au moins 3 sommets"));
    }
    if !closed && p.is_closed() {
        // le dernier sommet d'une polyligne ouverte ne porte pas d'arc de retour
        if let Some(l) = v.last_mut() {
            l.2 = 0.0;
        }
    }
    Ok(poly(&v, closed))
}

/// Inverse l'ordre des sommets (change le côté d'un décalage de polyligne ouverte).
pub fn reverse(e: &Entity) -> Result<Entity, EditError> {
    let (p, _) = polyline_of(e)?;
    Ok(Entity::Polyline(geom::reversed(p)))
}

/// Premier sommet d'une polyligne fermée (point de départ de l'usinage).
pub fn set_start(e: &Entity, i: usize) -> Result<Entity, EditError> {
    let (p, mut v) = polyline_of(e)?;
    if !p.is_closed() {
        return Err(bad("le point de départ ne se change que sur une polyligne fermée"));
    }
    if i >= v.len() {
        return Err(bad("sommet inexistant"));
    }
    v.rotate_left(i);
    Ok(poly(&v, true))
}

/// Retire les sommets en double (consécutifs et confondus).
pub fn clean(e: &Entity) -> Result<Entity, EditError> {
    let (p, v) = polyline_of(e)?;
    let mut out: Vec<(f64, f64, f64)> = Vec::new();
    for q in v {
        match out.last_mut() {
            Some(l) if (l.0 - q.0).hypot(l.1 - q.1) < 1e-9 => l.2 = q.2,
            _ => out.push(q),
        }
    }
    if p.is_closed() && out.len() > 1 {
        let (f, l) = (out[0], out[out.len() - 1]);
        if (f.0 - l.0).hypot(f.1 - l.1) < 1e-9 {
            out.pop();
        }
    }
    let min = if p.is_closed() { 3 } else { 2 };
    if out.len() < min {
        return Err(EditError::Degenerate("il ne reste pas assez de sommets distincts"));
    }
    Ok(poly(&out, p.is_closed()))
}

/// Poignée d'édition à la souris d'une forme sélectionnée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    /// Sommet d'une polyligne (une ligne a les sommets 0 et 1).
    Vertex(usize),
    /// Coin `k` d'un rectangle (0 = origine, puis sens trigonométrique) : l'angle opposé reste fixe.
    Corner(usize),
    /// Centre d'un cercle ou d'un arc : déplace la forme.
    Center,
    /// Point du cercle : change le rayon.
    Radius,
    /// Début d'un arc : change rayon et angle de départ, la fin reste en place.
    Start,
    /// Fin d'un arc : change le balayage.
    End,
}

fn rot((x, y): (f64, f64), deg: f64) -> (f64, f64) {
    let (s, c) = deg.to_radians().sin_cos();
    (c * x - s * y, s * x + c * y)
}

fn arc_point(cx: f64, cy: f64, r: f64, deg: f64) -> (f64, f64) {
    (cx + r * deg.to_radians().cos(), cy + r * deg.to_radians().sin())
}

/// Poignées d'une forme et leur position.
pub fn handles(e: &Entity) -> Vec<(Handle, (f64, f64))> {
    match e {
        Entity::Polyline(p) => vertices(p).into_iter().enumerate().map(|(i, (x, y, _))| (Handle::Vertex(i), (x, y))).collect(),
        Entity::Rect { x, y, w, h, rot_deg, .. } => [(0.0, 0.0), (*w, 0.0), (*w, *h), (0.0, *h)]
            .iter()
            .enumerate()
            .map(|(k, l)| {
                let d = rot(*l, *rot_deg);
                (Handle::Corner(k), (x + d.0, y + d.1))
            })
            .collect(),
        Entity::Circle { cx, cy, r } => vec![(Handle::Center, (*cx, *cy)), (Handle::Radius, (cx + r, *cy))],
        Entity::Arc { cx, cy, r, start_deg, sweep_deg } => vec![
            (Handle::Center, (*cx, *cy)),
            (Handle::Start, arc_point(*cx, *cy, *r, *start_deg)),
            (Handle::End, arc_point(*cx, *cy, *r, start_deg + sweep_deg)),
        ],
        Entity::Point { .. } => Vec::new(),
    }
}

/// Forme obtenue en glissant la poignée `h` jusqu'en `to` ; `None` si la poignée ne s'applique pas ou si le
/// résultat serait dégénéré.
pub fn drag_handle(e: &Entity, h: Handle, to: (f64, f64)) -> Option<Entity> {
    match (e, h) {
        (Entity::Polyline(_), Handle::Vertex(i)) => set_vertex(e, i, to).ok(),
        (Entity::Rect { x, y, w, h: rh, r, rot_deg }, Handle::Corner(k)) => {
            let local = [(0.0, 0.0), (*w, 0.0), (*w, *rh), (0.0, *rh)];
            let opposite = local[(k + 2) % 4];
            let p = rot((to.0 - x, to.1 - y), -rot_deg);
            let (lo, hi) = ((p.0.min(opposite.0), p.1.min(opposite.1)), (p.0.max(opposite.0), p.1.max(opposite.1)));
            let (nw, nh) = (hi.0 - lo.0, hi.1 - lo.1);
            if nw < EPS || nh < EPS {
                return None;
            }
            let o = rot(lo, *rot_deg);
            Some(Entity::Rect { x: x + o.0, y: y + o.1, w: nw, h: nh, r: r.min(nw.min(nh) / 2.0), rot_deg: *rot_deg })
        }
        (Entity::Circle { r, .. }, Handle::Center) => Some(Entity::Circle { cx: to.0, cy: to.1, r: *r }),
        (Entity::Circle { cx, cy, .. }, Handle::Radius) => {
            let r = (to.0 - cx).hypot(to.1 - cy);
            (r >= EPS).then_some(Entity::Circle { cx: *cx, cy: *cy, r })
        }
        (Entity::Arc { r, start_deg, sweep_deg, .. }, Handle::Center) => Some(Entity::Arc { cx: to.0, cy: to.1, r: *r, start_deg: *start_deg, sweep_deg: *sweep_deg }),
        (Entity::Arc { cx, cy, start_deg, sweep_deg, .. }, Handle::Start) => {
            let r = (to.0 - cx).hypot(to.1 - cy);
            let end = start_deg + sweep_deg;
            let new_start = (to.1 - cy).atan2(to.0 - cx).to_degrees();
            // la fin reste en place : on garde le sens et on recompose le balayage dans ]0 ; 360]
            let raw = if *sweep_deg >= 0.0 { (end - new_start).rem_euclid(360.0) } else { -(new_start - end).rem_euclid(360.0) };
            (r >= EPS && raw.abs() > 1e-6).then_some(Entity::Arc { cx: *cx, cy: *cy, r, start_deg: new_start, sweep_deg: raw })
        }
        (Entity::Arc { cx, cy, r, start_deg, sweep_deg }, Handle::End) => {
            let end = (to.1 - cy).atan2(to.0 - cx).to_degrees();
            let raw = if *sweep_deg >= 0.0 { (end - start_deg).rem_euclid(360.0) } else { -(start_deg - end).rem_euclid(360.0) };
            (raw.abs() > 1e-6).then_some(Entity::Arc { cx: *cx, cy: *cy, r: *r, start_deg: *start_deg, sweep_deg: raw })
        }
        _ => None,
    }
}

/// Remplace tous les sommets (tableau de coordonnées éditable) en gardant l'état ouvert/fermé.
pub fn set_vertices(e: &Entity, v: &[(f64, f64, f64)]) -> Result<Entity, EditError> {
    let (p, _) = polyline_of(e)?;
    let min = if p.is_closed() { 3 } else { 2 };
    if v.len() < min {
        return Err(bad(&format!("il faut au moins {min} sommets")));
    }
    if v.iter().any(|q| !(q.0.is_finite() && q.1.is_finite() && q.2.is_finite())) {
        return Err(bad("coordonnée non numérique"));
    }
    Ok(poly(v, p.is_closed()))
}

/// Contour d'un rectangle paramétrique, utile aux tests et à l'affichage.
pub fn rect_outline(e: &Entity) -> Option<Pline> {
    match e {
        Entity::Rect { x, y, w, h, r, rot_deg } => Some(rect_pline(*x, *y, *w, *h, *r, *rot_deg)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape;

    fn rect() -> Entity {
        shape::rectangle((10.0, 5.0), (50.0, 25.0)).unwrap()
    }

    fn area(e: &Entity) -> f64 {
        geom::area(&e.to_pline().unwrap()).abs()
    }

    #[test]
    fn largeur_changee_origine_conservee() {
        let r = set_rect(&rect(), 10.0, 5.0, 60.0, 20.0, 0.0, 0.0).unwrap();
        assert_eq!(r, Entity::Rect { x: 10.0, y: 5.0, w: 60.0, h: 20.0, r: 0.0, rot_deg: 0.0 });
        assert!((area(&r) - 1200.0).abs() < 1e-9);
    }

    #[test]
    fn rayon_de_coin_arrondit_quatre_coins() {
        let r = set_rect(&rect(), 10.0, 5.0, 40.0, 20.0, 5.0, 0.0).unwrap();
        let p = r.to_pline().unwrap();
        assert_eq!(p.vertex_count(), 8);
        let arcs = geom::segments(&p).iter().filter(|s| s.is_arc()).count();
        assert_eq!(arcs, 4);
        // aire = l·h − (4 − π) r²
        assert!((area(&r) - (800.0 - (4.0 - std::f64::consts::PI) * 25.0)).abs() < 1e-6);
    }

    #[test]
    fn rayon_egal_a_la_demi_hauteur_donne_une_gelule() {
        let r = set_rect(&rect(), 0.0, 0.0, 40.0, 20.0, 10.0, 0.0).unwrap();
        let p = r.to_pline().unwrap();
        assert!(geom::segments(&p).iter().all(|s| s.length() > 1e-9), "pas de segment nul");
        assert!((area(&r) - (20.0 * 20.0 + std::f64::consts::PI * 100.0)).abs() < 1e-6);
    }

    #[test]
    fn valeurs_invalides_refusees_et_forme_inchangee() {
        let e = rect();
        for (w, h, r) in [(0.0, 10.0, 0.0), (-3.0, 10.0, 0.0), (20.0, 10.0, 6.0), (20.0, 10.0, -1.0), (f64::NAN, 10.0, 0.0)] {
            assert!(set_rect(&e, 0.0, 0.0, w, h, r, 0.0).is_err(), "{w} {h} {r}");
        }
        assert!(set_rect(&Entity::Circle { cx: 0.0, cy: 0.0, r: 1.0 }, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0).is_err());
    }

    #[test]
    fn rotation_autour_du_coin_d_origine() {
        let r = set_rect(&rect(), 0.0, 0.0, 10.0, 4.0, 0.0, 90.0).unwrap();
        let p = r.to_pline().unwrap();
        let v = vertices(&p);
        assert!((v[1].0 - 0.0).abs() < 1e-9 && (v[1].1 - 10.0).abs() < 1e-9, "{v:?}");
        assert!((v[3].0 + 4.0).abs() < 1e-9 && v[3].1.abs() < 1e-9);
    }

    #[test]
    fn une_polyligne_rectangulaire_est_reconnue() {
        let pl = poly(&[(0.0, 0.0, 0.0), (30.0, 0.0, 0.0), (30.0, 10.0, 0.0), (0.0, 10.0, 0.0)], true);
        assert_eq!(recognize_rect(&pl), Some(Entity::Rect { x: 0.0, y: 0.0, w: 30.0, h: 10.0, r: 0.0, rot_deg: 0.0 }));
        let biais = poly(&[(0.0, 0.0, 0.0), (30.0, 0.0, 0.0), (35.0, 10.0, 0.0), (0.0, 10.0, 0.0)], true);
        assert!(recognize_rect(&biais).is_none());
        let horaire = poly(&[(0.0, 0.0, 0.0), (0.0, 10.0, 0.0), (30.0, 10.0, 0.0), (30.0, 0.0, 0.0)], true);
        assert!(recognize_rect(&horaire).is_none());
        // une polyligne reconnue devient modifiable
        let m = set_rect(&pl, 0.0, 0.0, 50.0, 10.0, 0.0, 0.0).unwrap();
        assert!(matches!(m, Entity::Rect { w, .. } if (w - 50.0).abs() < 1e-12));
    }

    #[test]
    fn cercle_et_arc() {
        let c = Entity::Circle { cx: 1.0, cy: 2.0, r: 3.0 };
        assert_eq!(set_circle(&c, 1.0, 2.0, 7.0).unwrap(), Entity::Circle { cx: 1.0, cy: 2.0, r: 7.0 });
        assert!(set_circle(&c, 1.0, 2.0, 0.0).is_err());
        let a = shape::arc((0.0, 0.0), 5.0, 0.0, 90.0).unwrap();
        assert!(matches!(set_arc(&a, 1.0, 1.0, 4.0, 10.0, -45.0).unwrap(), Entity::Arc { r, .. } if r == 4.0));
        assert!(set_arc(&a, 0.0, 0.0, 5.0, 0.0, 0.0).is_err());
        assert!(set_arc(&c, 0.0, 0.0, 5.0, 0.0, 10.0).is_err());
    }

    #[test]
    fn extremite_de_ligne() {
        let l = shape::line((0.0, 0.0), (10.0, 0.0)).unwrap();
        let m = set_vertex(&l, 1, (10.0, 5.0)).unwrap();
        assert_eq!(vertices(match &m { Entity::Polyline(p) => p, _ => unreachable!() })[1].1, 5.0);
        assert!(set_vertex(&l, 2, (0.0, 0.0)).is_err());
    }

    #[test]
    fn insertion_au_milieu_garde_la_forme() {
        let sq = poly(&[(0.0, 0.0, 0.0), (10.0, 0.0, 0.0), (10.0, 10.0, 0.0), (0.0, 10.0, 0.0)], true);
        let e = insert_vertex(&sq, 0, 0.5).unwrap();
        let p = e.to_pline().unwrap();
        assert_eq!(p.vertex_count(), 5);
        assert!((geom::area(&p).abs() - 100.0).abs() < 1e-9);
        assert!((p.at(1).x - 5.0).abs() < 1e-12);
        assert!(insert_vertex(&sq, 0, 0.0).is_err() && insert_vertex(&sq, 9, 0.5).is_err());
    }

    #[test]
    fn insertion_sur_un_arc_coupe_l_arc_sans_le_deformer() {
        // demi-cercle de rayon 5 : sommets (−5,0) et (5,0), bulge 1
        let d = poly(&[(-5.0, 0.0, 1.0), (5.0, 0.0, 0.0)], false);
        let e = insert_vertex(&d, 0, 0.25).unwrap();
        let p = e.to_pline().unwrap();
        assert_eq!(p.vertex_count(), 3);
        for s in geom::segments(&p) {
            let c = s.arc_center().unwrap();
            assert!(c.0.abs() < 1e-9 && c.1.abs() < 1e-9, "même centre {c:?}");
        }
        let q = (p.at(1).x, p.at(1).y);
        assert!((q.0.hypot(q.1) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn suppression_limitee() {
        let tri = poly(&[(0.0, 0.0, 0.0), (10.0, 0.0, 0.0), (5.0, 8.0, 0.0)], true);
        assert!(remove_vertex(&tri, 0).is_err(), "triangle fermé : 3 sommets minimum");
        let deux = poly(&[(0.0, 0.0, 0.0), (10.0, 0.0, 0.0)], false);
        assert!(remove_vertex(&deux, 0).is_err());
        let sq = poly(&[(0.0, 0.0, 0.0), (10.0, 0.0, 0.0), (10.0, 10.0, 0.0), (0.0, 10.0, 0.0)], true);
        assert_eq!(remove_vertex(&sq, 2).unwrap().to_pline().unwrap().vertex_count(), 3);
    }

    #[test]
    fn bulge_ferme_ouvre_inverse_depart() {
        let sq = poly(&[(0.0, 0.0, 0.0), (10.0, 0.0, 0.0), (10.0, 10.0, 0.0), (0.0, 10.0, 0.0)], true);
        let b = set_bulge(&sq, 0, 0.5).unwrap().to_pline().unwrap();
        assert!((b.at(0).bulge - 0.5).abs() < 1e-12);
        let ouvert = set_closed(&sq, false).unwrap();
        assert!(!ouvert.to_pline().unwrap().is_closed());
        assert!(set_bulge(&ouvert, 3, 0.5).is_err(), "le dernier sommet d'une ouverte ne porte pas d'arc");
        assert!(set_closed(&poly(&[(0.0, 0.0, 0.0), (1.0, 0.0, 0.0)], false), true).is_err());
        let inv = reverse(&ouvert).unwrap().to_pline().unwrap();
        assert!((inv.at(0).x - 0.0).abs() < 1e-12 && (inv.at(0).y - 10.0).abs() < 1e-12);
        let dep = set_start(&sq, 2).unwrap().to_pline().unwrap();
        assert!((dep.at(0).x - 10.0).abs() < 1e-12 && (dep.at(0).y - 10.0).abs() < 1e-12);
        assert!(set_start(&ouvert, 1).is_err());
    }

    #[test]
    fn poignees_du_rectangle_et_coin_oppose_fixe() {
        let r = Entity::Rect { x: 0.0, y: 0.0, w: 40.0, h: 20.0, r: 0.0, rot_deg: 0.0 };
        let hs = handles(&r);
        assert_eq!(hs.len(), 4);
        assert_eq!(hs[2], (Handle::Corner(2), (40.0, 20.0)));
        // glisser le coin haut-droit : l'origine reste, la taille suit
        let m = drag_handle(&r, Handle::Corner(2), (60.0, 30.0)).unwrap();
        assert_eq!(m, Entity::Rect { x: 0.0, y: 0.0, w: 60.0, h: 30.0, r: 0.0, rot_deg: 0.0 });
        // glisser l'origine : le coin opposé (40 ; 20) reste en place
        let n = drag_handle(&r, Handle::Corner(0), (-10.0, -5.0)).unwrap();
        assert_eq!(n, Entity::Rect { x: -10.0, y: -5.0, w: 50.0, h: 25.0, r: 0.0, rot_deg: 0.0 });
        // traverser le coin opposé retourne le rectangle sans le dégénérer
        let t = drag_handle(&r, Handle::Corner(2), (-10.0, -5.0)).unwrap();
        assert_eq!(t, Entity::Rect { x: -10.0, y: -5.0, w: 10.0, h: 5.0, r: 0.0, rot_deg: 0.0 });
        assert!(drag_handle(&r, Handle::Corner(2), (0.0, 20.0)).is_none());
    }

    #[test]
    fn poignees_du_rectangle_tourne() {
        let r = Entity::Rect { x: 5.0, y: 5.0, w: 10.0, h: 4.0, r: 1.0, rot_deg: 90.0 };
        let p = handles(&r)[1].1;
        assert!((p.0 - 5.0).abs() < 1e-9 && (p.1 - 15.0).abs() < 1e-9);
        // coin 2 vers (1 ; 20) : opposé = origine (5 ; 5)
        let m = drag_handle(&r, Handle::Corner(2), (1.0, 20.0)).unwrap();
        let Entity::Rect { x, y, w, h, rot_deg, .. } = m else { panic!() };
        assert!((w - 15.0).abs() < 1e-9 && (h - 4.0).abs() < 1e-9 && rot_deg == 90.0);
        assert!((x - 5.0).abs() < 1e-9 && (y - 5.0).abs() < 1e-9);
    }

    #[test]
    fn poignees_cercle_et_arc() {
        let c = Entity::Circle { cx: 0.0, cy: 0.0, r: 5.0 };
        assert_eq!(drag_handle(&c, Handle::Radius, (0.0, 8.0)), Some(Entity::Circle { cx: 0.0, cy: 0.0, r: 8.0 }));
        assert_eq!(drag_handle(&c, Handle::Center, (3.0, 4.0)), Some(Entity::Circle { cx: 3.0, cy: 4.0, r: 5.0 }));
        assert!(drag_handle(&c, Handle::Radius, (0.0, 0.0)).is_none());
        let a = Entity::Arc { cx: 0.0, cy: 0.0, r: 5.0, start_deg: 0.0, sweep_deg: 90.0 };
        let hs = handles(&a);
        assert!((hs[2].1 .0).abs() < 1e-9 && (hs[2].1 .1 - 5.0).abs() < 1e-9);
        // la fin glisse à 180° : balayage 180
        let Some(Entity::Arc { sweep_deg, .. }) = drag_handle(&a, Handle::End, (-5.0, 0.0)) else { panic!() };
        assert!((sweep_deg - 180.0).abs() < 1e-9);
        // le début glisse à -90° : la fin (90°) reste, le balayage devient 180° et le rayon suit
        let Some(Entity::Arc { start_deg, sweep_deg, r, .. }) = drag_handle(&a, Handle::Start, (0.0, -7.0)) else { panic!() };
        assert!((start_deg + 90.0).abs() < 1e-9 && (sweep_deg - 180.0).abs() < 1e-9 && (r - 7.0).abs() < 1e-9);
    }

    #[test]
    fn tableau_de_sommets() {
        let l = shape::polyline(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)], false).unwrap();
        let m = set_vertices(&l, &[(0.0, 0.0, 0.0), (2.0, 0.0, 0.5), (2.0, 2.0, 0.0), (0.0, 2.0, 0.0)]).unwrap();
        assert_eq!(m.to_pline().unwrap().vertex_count(), 4);
        assert!(set_vertices(&l, &[(0.0, 0.0, 0.0)]).is_err());
    }

    #[test]
    fn nettoyage_des_doublons() {
        let d = poly(&[(0.0, 0.0, 0.0), (0.0, 0.0, 0.0), (10.0, 0.0, 0.0), (10.0, 10.0, 0.0), (0.0, 0.0, 0.0)], true);
        assert_eq!(clean(&d).unwrap().to_pline().unwrap().vertex_count(), 3);
    }
}
