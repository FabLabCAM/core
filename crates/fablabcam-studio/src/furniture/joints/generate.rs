use super::*;
use contours::{contour, finish, pline, rectangle, relieved, union};
use fablabcam_geom as geom;

pub(crate) fn machine(parts: &mut [Panel], d: &Dimensions, cutter: f64) -> Result<(), String> {
    let Some(c) = &d.composition else {
        return Ok(());
    };
    if !cutter.is_finite() || cutter <= 0.0 {
        return Err("Diamètre de fraise invalide".into());
    }
    let links = connections(parts);
    if let Some(id) = c
        .joints
        .keys()
        .find(|id| !links.iter().any(|l| &l.id == *id))
    {
        return Err(format!("Liaison sans pièces correspondantes : {id}"));
    }
    // Infer against the unmachined envelopes, then apply each complementary pair.
    for link in links {
        let settings = c.joints.get(&link.id).unwrap_or(&c.joint_default);
        if settings.kind == JointKind::Butt {
            continue;
        }
        let mi = parts.iter().position(|p| p.id == link.male).unwrap();
        let fi = parts.iter().position(|p| p.id == link.female).unwrap();
        let mut male = parts[mi].clone();
        let mut female = parts[fi].clone();
        joint(&mut male, &mut female, &link, settings, cutter)
            .map_err(|e| format!("{} ↔ {} : {e}", male.name, female.name))?;
        parts[mi] = male;
        parts[fi] = female;
    }
    for p in parts {
        finish(p)?;
    }
    Ok(())
}
fn joint(
    m: &mut Panel,
    f: &mut Panel,
    l: &Connection,
    s: &JointSettings,
    cutter: f64,
) -> Result<(), String> {
    s.check()?;
    if !s.kind.supported() {
        return Err(format!("{} : non encore pris en charge", s.kind.label()));
    }
    let continuous = matches!(
        s.kind,
        JointKind::Dado | JointKind::Rabbet | JointKind::HalfLap | JointKind::TongueGroove
    );
    let reduced = matches!(s.kind, JointKind::HalfLap | JointKind::TongueGroove);
    let forced_blind = continuous;
    let depth = if s.blind || forced_blind {
        s.engagement
    } else {
        f.thickness
    };
    if depth > f.thickness + 1e-8 || ((s.blind || forced_blind) && depth >= f.thickness - 0.01) {
        return Err("Profondeur borgne : conserver de la matière dans le panneau femelle".into());
    }
    let normal = m.normal;
    let edge = (m.origin[normal] - f.origin[normal]).abs() < 1e-7
        || (m.origin[normal] + m.thickness - f.origin[normal] - f.size[normal]).abs() < 1e-7;
    if matches!(
        s.kind,
        JointKind::Box | JointKind::Rabbet | JointKind::HalfLap
    ) && !edge
    {
        return Err(format!("{} nécessite une liaison au bord du panneau ; choisir rainure ou tenons pour une tablette intérieure",s.kind.label()));
    }
    let tongue = if s.kind == JointKind::TongueGroove {
        m.thickness / 3.0
    } else if reduced {
        m.thickness / 2.0
    } else {
        m.thickness
    };
    if tongue + s.clearance < cutter {
        return Err(format!(
            "Rainure {:.2} mm trop étroite pour la fraise {:.2} mm",
            tongue + s.clearance,
            cutter
        ));
    }
    let ranges = ranges(l, s, continuous, cutter)?;
    for span in ranges {
        let ma = m.axes();
        let axis = usize::from(ma[1] == l.axis);
        let transverse = 1 - axis;
        let end = if l.high_end { m.size[l.axis] } else { 0.0 };
        let dir = if l.high_end { 1.0 } else { -1.0 };
        let mut a = [0.0; 2];
        let mut b = [0.0; 2];
        a[axis] = (end + dir * depth).min(end) - if l.high_end { 0.001 } else { 0.0 };
        b[axis] = (end + dir * depth).max(end) + if l.high_end { 0.0 } else { 0.001 };
        a[transverse] = span[0] - m.origin[l.span_axis];
        b[transverse] = span[1] - m.origin[l.span_axis];
        m.outline = union(&m.outline, &rectangle(a, b))?;
        let shrink = if s.kind == JointKind::TongueGroove {
            (m.thickness - tongue) / 2.0
        } else {
            0.0
        };
        let female_low = m.origin[normal] + shrink;
        let fa = f.axes();
        let mut lo = [0.0; 2];
        let mut hi = [0.0; 2];
        for i in 0..2 {
            let (v, w) = if fa[i] == normal {
                (female_low, tongue)
            } else {
                (span[0], span[1] - span[0])
            };
            lo[i] = v - f.origin[fa[i]] - s.clearance / 2.0;
            hi[i] = v + w - f.origin[fa[i]] + s.clearance / 2.0;
        }
        let slot = relieved(rectangle(lo, hi), cutter / 2.0, s.relief)?;
        feature(
            f,
            l,
            slot,
            depth,
            l.female_face,
            depth >= f.thickness - 1e-8,
        )?;
        if reduced {
            // Relief pockets on the tongue itself preserve its assembled location.
            // The broad-face setup is explicit; back pockets require a separate setup.
            let cut = rectangle(a, b);
            if s.kind == JointKind::TongueGroove {
                feature(m, l, cut.clone(), shrink, MachiningFace::Front, false)?;
                feature(m, l, cut, shrink, MachiningFace::Back, false)?;
            } else {
                feature(m, l, cut, m.thickness - tongue, MachiningFace::Back, false)?;
            }
        }
    }
    Ok(())
}
fn ranges(
    l: &Connection,
    s: &JointSettings,
    continuous: bool,
    cutter: f64,
) -> Result<Vec<[f64; 2]>, String> {
    let len = l.span[1] - l.span[0];
    if continuous {
        return Ok(vec![l.span]);
    }
    let count = s.count as usize;
    let gap = (len - s.width * count as f64) / (count + 1) as f64;
    if s.width + s.clearance < cutter || gap < cutter + s.clearance {
        return Err(
            "Tenons trop larges/nombreux ou trop proches des bords pour cette fraise".into(),
        );
    }
    Ok((0..count)
        .map(|i| {
            let a = l.span[0] + gap + (s.width + gap) * i as f64;
            [a, a + s.width]
        })
        .collect())
}
fn feature(
    p: &mut Panel,
    l: &Connection,
    cut: Contour,
    depth: f64,
    face: MachiningFace,
    through: bool,
) -> Result<(), String> {
    let overlap = geom::intersection(&pline(&p.outline), &pline(&cut));
    if overlap.outlines.is_empty() {
        return Err("Usinage hors matière".into());
    }
    for other in &p.features {
        if (other.face == face || other.depth + depth > p.thickness + 1e-8)
            && !geom::intersection(&pline(&other.contour), &pline(&cut))
                .outlines
                .is_empty()
        {
            return Err(format!(
                "Usinages en collision : {} et {}",
                other.joint, l.id
            ));
        }
    }
    let mut in_outline = false;
    if through {
        let diff = geom::difference(&pline(&p.outline), &pline(&cut));
        if diff.outlines.len() != 1 {
            return Err("L’assemblage sépare le panneau en plusieurs morceaux".into());
        }
        // Edge-open slots belong to the external profile; closed slots remain features.
        if diff.holes.is_empty() {
            in_outline = true;
            p.outline = contour(&diff.outlines[0]);
        }
    }
    p.features.push(MachiningFeature {
        joint: l.id.clone(),
        contour: cut,
        depth,
        face,
        through,
        in_outline,
    });
    Ok(())
}
