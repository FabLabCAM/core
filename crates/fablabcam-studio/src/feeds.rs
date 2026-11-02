//! Calculateur de vitesses et d'avances (formules usuelles de l'usinage) :
//! `N = Vc × 1000 / (π × D)` (tr/min) et `Vf = N × Z × fz` (mm/min).
//!
//! Les matières proposées donnent des **ordres de grandeur** pour une fraise carbure ; les valeurs du fabricant
//! de l'outil priment.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    pub name: &'static str,
    /// Vitesse de coupe (m/min).
    pub vc: f64,
    /// Avance par dent, en fraction du diamètre de l'outil.
    pub fz_per_d: f64,
}

/// Valeurs indicatives (fraise carbure, usinage à sec).
pub const MATERIALS: [Material; 5] = [
    Material { name: "Bois tendre", vc: 500.0, fz_per_d: 0.02 },
    Material { name: "Bois dur / MDF", vc: 400.0, fz_per_d: 0.015 },
    Material { name: "Plastique (acrylique, POM)", vc: 250.0, fz_per_d: 0.015 },
    Material { name: "Aluminium", vc: 200.0, fz_per_d: 0.006 },
    Material { name: "Laiton", vc: 150.0, fz_per_d: 0.005 },
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cutting {
    /// Vitesse de broche (tr/min), bornée par la vitesse maximale de la machine.
    pub rpm: f64,
    /// Avance de coupe (mm/min).
    pub feed: f64,
    /// Avance de plongée (mm/min), un tiers de l'avance de coupe.
    pub plunge: f64,
    /// Vitesse de coupe réellement obtenue (m/min), plus faible si la broche plafonne.
    pub vc: f64,
    pub limited: bool,
}

/// Vitesses et avances pour un outil de diamètre `d` (mm) à `flutes` dents.
pub fn compute(d: f64, flutes: u32, vc: f64, fz: f64, max_rpm: f64) -> Result<Cutting, String> {
    if !(d > 0.0 && vc > 0.0 && fz > 0.0 && max_rpm > 0.0) || flutes == 0 {
        return Err("diamètre, dents, vitesse de coupe, avance par dent et broche maxi doivent être > 0".into());
    }
    let ideal = vc * 1000.0 / (std::f64::consts::PI * d);
    let rpm = ideal.min(max_rpm).round();
    let feed = (rpm * flutes as f64 * fz).round();
    Ok(Cutting { rpm, feed, plunge: (feed / 3.0).round(), vc: rpm * std::f64::consts::PI * d / 1000.0, limited: ideal > max_rpm })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formules() {
        // Ø6, 2 dents, Vc 300 m/min, fz 0,05 mm : N = 15 915, Vf = 1 592
        let c = compute(6.0, 2, 300.0, 0.05, 30_000.0).unwrap();
        assert_eq!((c.rpm, c.feed, c.plunge), (15_915.0, 1_592.0, 531.0));
        assert!(!c.limited && (c.vc - 300.0).abs() < 0.1);
        // broche plafonnée : l'avance suit la vitesse réelle
        let c = compute(3.0, 2, 500.0, 0.05, 24_000.0).unwrap();
        assert_eq!((c.rpm, c.feed), (24_000.0, 2_400.0));
        assert!(c.limited && c.vc < 500.0);
        assert!(compute(0.0, 2, 300.0, 0.05, 1.0).is_err() && compute(6.0, 0, 300.0, 0.05, 1.0).is_err());
    }
}
