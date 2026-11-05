use std::f64::consts::{PI, TAU};
use super::{Block, Point, state::State};

pub(crate) struct Arc {
    pub length: f64,
    pub extremes: Vec<Point>,
    pub planar_length: f64,
}
fn sweep(start: f64, end: f64, cw: bool) -> f64 {
    let a = if cw { (start-end).rem_euclid(TAU) } else { (end-start).rem_euclid(TAU) };
    if a < 1e-12 { TAU } else { a }
}
pub(crate) fn calculate(b: &Block, s: &State, from: Point, to: Point) -> Result<Arc,String> {
    let [u,v,h] = s.plane;
    let cw = s.motion == Some(2);
    let uv_words = [['I','J','K'][u], ['I','J','K'][v]];
    if b.value(['I','J','K'][h]).is_some() { return Err("Centre d'arc hors du plan actif".into()); }
    let center = if let Some(radius) = b.value('R') {
        if b.words.iter().any(|w| "IJK".contains(w.letter)) { return Err("R et IJK sont exclusifs".into()); }
        radius_center(from,to,[u,v],radius*s.scale,cw)?
    } else {
        let mut center = [0.;2];
        if uv_words.iter().all(|c| b.value(*c).is_none()) { return Err("Arc sans centre IJK ni rayon R".into()); }
        for (i, axis) in [u,v].into_iter().enumerate() {
            center[i] = if s.arc_absolute {
                b.value(uv_words[i]).ok_or("IJK absolu : les deux coordonnées du centre sont requises")? * s.scale + s.offset.ok_or("Origine inconnue")?[axis]
            } else { from[axis] + b.value(uv_words[i]).unwrap_or(0.)*s.scale };
        }
        center
    };
    let radius = (from[u]-center[0]).hypot(from[v]-center[1]);
    let end_radius = (to[u]-center[0]).hypot(to[v]-center[1]);
    // Mill manual 10.7.3.2: 0.0002 inch in G20, 0.002 mm in G21.
    // Geometry is already in mm here. Never average mismatched radii.
    let tolerance_mm = if s.scale == 25.4 { 0.0002 * 25.4 } else { 0.002 };
    if radius < 1e-9 || end_radius < 1e-9 {
        return Err(format!("Rayon d'arc nul au départ ou à l'arrivée : {radius:.6} / {end_radius:.6} mm"));
    }
    if (radius-end_radius).abs() > tolerance_mm {
        return Err(format!("Rayons d'arc incohérents : {radius:.6} / {end_radius:.6} mm"));
    }
    let a0 = (from[v]-center[1]).atan2(from[u]-center[0]);
    let a1 = (to[v]-center[1]).atan2(to[u]-center[0]);
    let angle = sweep(a0,a1,cw);
    let mut extremes = vec![from,to];
    for a in [0., PI/2., PI, PI*1.5] {
        let travel = if cw { (a0-a).rem_euclid(TAU) } else { (a-a0).rem_euclid(TAU) };
        if travel <= angle + 1e-12 {
            let mut point = from;
            point[u] = center[0]+radius*a.cos(); point[v] = center[1]+radius*a.sin();
            point[h] = from[h]+(to[h]-from[h])*travel/angle;
            extremes.push(point);
        }
    }
    Ok(Arc { length: (radius*angle).hypot(to[h]-from[h]), planar_length: radius*angle, extremes })
}
fn radius_center(from: Point, to: Point, [u,v]: [usize;2], r: f64, cw: bool) -> Result<[f64;2],String> {
    let (dx,dy) = (to[u]-from[u],to[v]-from[v]);
    let chord = dx.hypot(dy);
    if chord < 1e-9 || chord > 2.*r.abs()+1e-9 { return Err("Arc R impossible (cercle complet ou corde > diamètre)".into()); }
    let height = (r*r-chord*chord/4.).max(0.).sqrt();
    for sign in [-1.,1.] {
        let c = [(from[u]+to[u])/2.-sign*dy*height/chord, (from[v]+to[v])/2.+sign*dx*height/chord];
        let a0 = (from[v]-c[1]).atan2(from[u]-c[0]);
        let a1 = (to[v]-c[1]).atan2(to[u]-c[0]);
        let a = sweep(a0,a1,cw);
        if (r >= 0. && a <= PI+1e-12) || (r < 0. && a >= PI-1e-12) { return Ok(c); }
    }
    Err("Centre d'arc indéterminé".into())
}
