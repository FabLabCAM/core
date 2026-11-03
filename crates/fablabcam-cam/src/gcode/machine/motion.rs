use super::{arc, state::State, Block, Point, Profile, Report};

pub(crate) fn execute(b: &Block, p: &Profile, r: &mut Report, s: &mut State) {
    if s.poisoned { r.statistics.complete = false; return; }
    let has_axis = b.words.iter().any(|w| "XYZ".contains(w.letter));
    let has_center = b.words.iter().any(|w| "IJKR".contains(w.letter));
    if b.has("G4") {
        if has_axis || has_center { r.error("dwell", b.line, 1, None, "G4 ne doit pas porter de déplacement"); s.poisoned = true; return; }
        match b.value('P') { Some(v) if v >= 0. => r.partial("dwell_units", b.line, b.column('P'), Some("G4".into()), "Durée G4 dépend du réglage secondes/millisecondes du contrôleur"), _ => r.error("dwell", b.line, 1, Some("G4".into()), "G4 exige P positif ou nul") }
        return;
    }
    if b.value('P').is_some() { r.partial("unmodeled_word", b.line, b.column('P'), Some("P".into()), "P sans cycle interprété"); s.poisoned = true; }
    if !has_axis && !has_center { return; }
    let Some(mode) = s.motion else { r.error("motion_mode", b.line, 1, None, "Déplacement sans mode G0/G1/G2/G3 actif"); s.poisoned = true; return; };
    if mode < 2 && has_center { r.error("arc_word", b.line, 1, None, "IJK/R sans mouvement circulaire"); s.poisoned = true; }
    if s.poisoned { r.statistics.complete = false; return; }
    let machine = b.has("G53");
    if machine && (!s.absolute || mode > 1) { r.error("machine_move", b.line, 1, Some("G53".into()), "G53 exige G90 et un mouvement linéaire G0/G1"); s.poisoned = true; return; }
    let offset = if machine { [0.;3] } else {
        match s.offset { Some(v) => v, None => { r.partial("work_offset", b.line, 1, None, "Origine active non configurée en coordonnées machine"); s.poisoned = true; return; } }
    };
    let from = s.position;
    if from.is_none() && (!s.absolute || ['X','Y','Z'].iter().any(|c| b.value(*c).is_none())) {
        r.partial("initial_position", b.line, 1, None, "Position initiale inconnue : fournir XYZ absolus ou configurer initial_position_mm"); return;
    }
    let mut to = from.unwrap_or([0.;3]);
    for (i,c) in ['X','Y','Z'].into_iter().enumerate() {
        if let Some(v) = b.value(c) { to[i] = if s.absolute { v*s.scale+offset[i] } else { to[i]+v*s.scale }; }
    }
    if !to.iter().all(|v| v.is_finite()) { r.error("overflow", b.line, 1, None, "Coordonnée hors capacité numérique"); s.poisoned = true; return; }
    let Some(from) = from else {
        bounds(to,b,p,r); s.position = Some(to);
        r.partial("initial_position", b.line, 1, None, "Extrémité connue, premier trajet impossible à mesurer"); return;
    };
    let (length, extremes, axis_lengths) = if mode >= 2 {
        match arc::calculate(b,s,from,to) {
            Ok(a) => {
                let [u,v,h] = s.plane;
                if !p.safety.allow_helical_arcs && (to[h]-from[h]).abs() > 1e-9 { r.error("helix", b.line, 1, None, "Arcs hélicoïdaux interdits"); }
                // The planar component bounds the instantaneous speed of either planar axis.
                let mut axes = [0.;3]; axes[u] = a.planar_length; axes[v] = a.planar_length; axes[h] = (to[h]-from[h]).abs();
                (a.length,a.extremes,axes)
            }
            Err(e) => { r.error("arc", b.line, 1, None, e); s.poisoned = true; return; }
        }
    } else {
        let axes = std::array::from_fn::<_,3,_>(|i| (to[i]-from[i]).abs());
        (axes[0].hypot(axes[1]).hypot(axes[2]), vec![from,to], axes)
    };
    if !length.is_finite() { r.error("overflow", b.line, 1, None, "Longueur hors capacité numérique"); s.poisoned = true; return; }
    for point in &extremes { bounds(*point,b,p,r); }
    if mode == 0 && p.safety.forbid_rapid_below_safe_z && extremes.iter().any(|v| v[2] < p.safety.safe_z_mm.unwrap_or(0.)) {
        r.error("rapid_below_safe_z", b.line, 1, Some("G0".into()), "Trajet rapide sous le Z de sécurité (coordonnées machine)");
    }
    timing(b,p,r,s,length,axis_lengths);
    s.position = Some(to);
}
fn bounds(point: Point, b: &Block, p: &Profile, r: &mut Report) {
    let lo = r.statistics.bounds_min_mm.get_or_insert(point);
    let hi = r.statistics.bounds_max_mm.get_or_insert(point);
    for (i,c) in ['X','Y','Z'].into_iter().enumerate() {
        lo[i] = lo[i].min(point[i]); hi[i] = hi[i].max(point[i]);
        if let Some(a) = p.axes.get(&c) {
            if a.min.is_some_and(|v| point[i] < v-1e-8) || a.max.is_some_and(|v| point[i] > v+1e-8) {
                r.diagnostics.push(super::Diagnostic::new(super::Severity::Error,"travel",b.line,b.column(c),Some(c.to_string()),format!("{c} hors course : {:.6} mm",point[i])));
            }
        }
    }
}
fn timing(b: &Block, p: &Profile, r: &mut Report, s: &State, length: f64, axes: Point) {
    let rapid = s.motion == Some(0);
    if rapid { r.statistics.rapid_length_mm += length; } else { r.statistics.cutting_length_mm += length; }
    if length <= 1e-12 { return; }
    let feed = if rapid { p.rapid_mm_min } else {
        match s.feed_mode.as_str() {
            "G93" => b.value('F').map(|v| v*length),
            "G95" => if s.spindle.is_some() && s.speed > 0. { s.feed.map(|f| f*s.speed) } else { None },
            _ => s.feed,
        }
    };
    let Some(feed) = feed.filter(|f| *f > 0. && f.is_finite()) else {
        if rapid { r.partial("rapid_speed", b.line, 1, None, "Vitesse rapide inconnue : estimation de durée partielle"); }
        else { r.error("feed_missing", b.line, b.column('F'), None, "Avance positive requise ; G93 exige F à chaque bloc, G95 une broche active"); r.statistics.complete = false; }
        return;
    };
    if !rapid && p.max_feed_mm_min.is_some_and(|max| feed > max+1e-8) { r.error("feed_limit", b.line, b.column('F'), Some("F".into()), format!("Avance {feed:.3} mm/min supérieure au maximum")); }
    for (i,c) in ['X','Y','Z'].into_iter().enumerate() {
        if p.axes.get(&c).and_then(|a| a.max_feed).is_some_and(|max| feed*axes[i]/length > max+1e-8) {
            r.error("axis_feed_limit", b.line, b.column(c), Some(c.to_string()), "Vitesse d'axe supérieure au maximum");
        }
    }
    r.statistics.estimated_seconds += length/feed*60.;
}
