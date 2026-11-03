use fablabcam_cam::gcode::machine::Profile;

#[derive(Debug)]
pub struct Configuration {
    pub error: Option<String>,
    pub missing: Vec<String>,
    pub cautions: Vec<String>,
}
impl Configuration {
    pub fn label(&self) -> &'static str {
        if self.error.is_some() {
            "Erreur de configuration"
        } else if !self.missing.is_empty() {
            "À configurer"
        } else {
            "Champs renseignés"
        }
    }
}

/// Completeness of configuration, never a machine/program safety certificate.
pub fn configuration(p: &Profile) -> Configuration {
    let mut missing = Vec::new();
    let mut cautions = Vec::new();
    for axis in ['X', 'Y', 'Z'] {
        match p.axes.get(&axis) {
            Some(a) => {
                if a.min.is_none() || a.max.is_none() {
                    missing.push(format!("Limites de l'axe {axis}"));
                }
                if a.max_feed.is_none() {
                    missing.push(format!("Avance maximale de l'axe {axis}"));
                }
                if a.min.is_some() && a.min == a.max {
                    cautions.push(format!("L'axe {axis} a une course nulle"));
                }
            }
            None => missing.push(format!("Axe {axis}")),
        }
    }
    for (v, label) in [
        (p.max_feed_mm_min, "Avance globale"),
        (p.rapid_mm_min, "Vitesse rapide"),
        (p.spindle.min_rpm, "Vitesse minimale de broche"),
        (p.spindle.max_rpm, "Vitesse maximale de broche"),
        (p.safety.safe_z_mm, "Z de sécurité"),
    ] {
        if v.is_none() {
            missing.push(label.into());
        }
    }
    if p.initial_position_mm.is_none() {
        missing.push("Position initiale machine".into());
    }
    if !p.work_offsets_mm.contains_key(&p.defaults.work_offset) {
        missing.push(format!("Origine pièce {}", p.defaults.work_offset));
    }
    if let Some(z) = p.safety.safe_z_mm {
        if p.axes
            .get(&'Z')
            .is_some_and(|a| a.min.is_some_and(|v| z < v) || a.max.is_some_and(|v| z > v))
        {
            cautions.push("Le Z de sécurité est hors des limites Z configurées".into());
        }
    }
    if p.axes.keys().any(|a| !"XYZ".contains(*a)) {
        cautions.push("La cinématique des axes rotatifs n'est pas analysée".into());
    }
    Configuration {
        error: p.check().err(),
        missing,
        cautions,
    }
}
