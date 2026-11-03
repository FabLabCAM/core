use super::profile::*;

pub fn valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 80 && s.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
}
fn positive(v: Option<f64>) -> bool { v.is_none_or(|v| v.is_finite() && v > 0.) }
fn finite(v: Option<f64>) -> bool { v.is_none_or(f64::is_finite) }
fn range(a: Option<f64>, b: Option<f64>) -> bool {
    finite(a) && finite(b) && a.zip(b).is_none_or(|(a,b)| a <= b)
}
impl Profile {
    pub fn check(&self) -> Result<(), String> {
        let require = |ok, message: &str| if ok { Ok(()) } else { Err(message.to_owned()) };
        require(self.schema_version == 1, "schema_version doit être 1")?;
        require(valid_id(&self.id) && !self.name.trim().is_empty(), "id ou nom de profil invalide")?;
        require(!self.axes.is_empty(), "axes manquants")?;
        for (letter, axis) in &self.axes {
            require("XYZABC".contains(*letter), "axe inconnu")?;
            require(range(axis.min, axis.max) && positive(axis.max_feed), "limites d'axe invalides")?;
            require(!"XYZ".contains(*letter) || axis.kind == AxisKind::Linear, "XYZ doivent être linéaires")?;
        }
        require(positive(self.max_feed_mm_min) && positive(self.rapid_mm_min) && positive(self.acceleration_mm_s2), "vitesse/accélération invalide")?;
        require(range(self.spindle.min_rpm, self.spindle.max_rpm) && self.spindle.min_rpm.is_none_or(|v| v >= 0.) && self.spindle.max_rpm.is_none_or(|v| v >= 0.), "limites de broche invalides")?;
        require(self.spindle.directions.iter().all(|s| ["cw", "ccw"].contains(&s.as_str())), "sens de broche inconnu")?;
        for p in self.initial_position_mm.iter().chain(self.work_offsets_mm.values()).chain(self.safety.park_position_mm.iter()).chain(self.safety.tool_change_position_mm.iter()) {
            require(p.iter().all(|v| v.is_finite()), "position non finie")?;
        }
        require(self.work_offsets_mm.keys().all(|k| ["G54","G55","G56","G57","G58","G59"].contains(&k.as_str())), "origine de travail inconnue")?;
        let d = &self.defaults;
        require(["G20","G21"].contains(&d.units.as_str()) && ["G17","G18","G19"].contains(&d.plane.as_str()) && ["G90","G91"].contains(&d.distance.as_str()) && ["G90.1","G91.1"].contains(&d.arc_distance.as_str()) && ["G93","G94","G95"].contains(&d.feed_mode.as_str()), "mode par défaut inconnu")?;
        require(["G54","G55","G56","G57","G58","G59"].contains(&d.work_offset.as_str()), "origine par défaut inconnue")?;
        for (prefix, list) in [('G', &self.gcodes), ('M', &self.mcodes)] {
            for code in list.allow.iter().chain(&list.deny) {
                let n = code.get(1..).and_then(|v| v.parse::<f64>().ok());
                require(code.starts_with(prefix) && n.is_some_and(|n| n.is_finite() && n >= 0. && format!("{prefix}{n}") == *code), "codes attendus sous forme canonique, ex. G1")?;
            }
        }
        require(self.format.max_decimals <= 12 && self.format.max_line_length > 0, "format numérique ou longueur invalide")?;
        require(self.format.comments.iter().all(|s| ["()", ";", "//", "%"].contains(&s.as_str())), "syntaxe de commentaire inconnue")?;
        require(self.format.header.as_ref().is_none_or(|s| s == "%"), "en-tête pris en charge : % ou null")?;
        require(!self.format.end_codes.is_empty() && self.format.end_codes.iter().all(|c| ["M2","M30"].contains(&c.as_str()) && self.mcodes.permits(c)), "terminaison attendue M2/M30 autorisée")?;
        require(self.safety.required_preamble.iter().all(|c| self.gcodes.permits(c)), "préambule non autorisé par le profil")?;
        require(finite(self.safety.safe_z_mm) && (!self.safety.forbid_rapid_below_safe_z || self.safety.safe_z_mm.is_some()), "Z de sécurité manquant ou invalide")?;
        require(self.tools.positions.is_none_or(|v| v > 0) && self.tools.table.values().all(|t| t.diameter_mm.is_finite() && t.diameter_mm > 0. && t.length_mm.is_finite() && t.length_mm >= 0.), "table d'outils invalide")?;
        require(self.tools.coolant.iter().all(|s| ["M7","M8","M9"].contains(&s.as_str())), "code de refroidissement inconnu")
    }
}
