use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JointKind {
    Butt,
    Tabs,
    Box,
    Dado,
    Rabbet,
    HalfLap,
    MortiseTenon,
    TongueGroove,
    Dovetail,
    HalfDovetail,
    Bridle,
    Dowel,
    Mitre,
    Biscuit,
    Pocket,
    Scarf,
}
impl JointKind {
    pub const ALL: [Self; 16] = [
        Self::Butt,
        Self::Tabs,
        Self::Box,
        Self::Dado,
        Self::Rabbet,
        Self::HalfLap,
        Self::MortiseTenon,
        Self::TongueGroove,
        Self::Dovetail,
        Self::HalfDovetail,
        Self::Bridle,
        Self::Dowel,
        Self::Mitre,
        Self::Biscuit,
        Self::Pocket,
        Self::Scarf,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Butt => "Bord à bord",
            Self::Tabs => "Tenons plats traversants",
            Self::Box => "Queues droites",
            Self::Dado => "Rainure pour tablette",
            Self::Rabbet => "Feuillure",
            Self::HalfLap => "Mi-bois",
            Self::MortiseTenon => "Tenon-mortaise",
            Self::TongueGroove => "Rainure-languette",
            Self::Dovetail => "Queue d’aronde",
            Self::HalfDovetail => "Demi-queue d’aronde",
            Self::Bridle => "Enfourchement",
            Self::Dowel => "Tourillons",
            Self::Mitre => "Onglet",
            Self::Biscuit => "Lamelles",
            Self::Pocket => "Vissage oblique",
            Self::Scarf => "Enture en sifflet",
        }
    }
    pub fn supported(self) -> bool {
        matches!(
            self,
            Self::Butt
                | Self::Tabs
                | Self::Box
                | Self::Dado
                | Self::Rabbet
                | Self::HalfLap
                | Self::MortiseTenon
                | Self::TongueGroove
        )
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CornerRelief {
    None,
    Dogbone,
    Tbone,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct JointSettings {
    pub kind: JointKind,
    /// TOTAL female enlargement, not clearance per side.
    pub clearance: f64,
    pub engagement: f64,
    pub width: f64,
    pub count: u32,
    pub blind: bool,
    pub relief: CornerRelief,
}
impl Default for JointSettings {
    fn default() -> Self {
        Self {
            kind: JointKind::Butt,
            clearance: 0.2,
            engagement: 8.0,
            width: 20.0,
            count: 3,
            blind: false,
            relief: CornerRelief::Dogbone,
        }
    }
}
impl JointSettings {
    pub fn check(&self) -> Result<(), String> {
        if [self.clearance, self.engagement, self.width]
            .iter()
            .any(|v| !v.is_finite())
            || !(0.0..=5.0).contains(&self.clearance)
            || !(0.1..=80.0).contains(&self.engagement)
            || !(1.0..=5000.0).contains(&self.width)
            || !(1..=100).contains(&self.count)
        {
            return Err("Assemblage : jeu total 0–5 mm, engagement 0,1–80 mm, largeur 1–5000 mm et 1–100 tenons requis".into());
        }
        Ok(())
    }
}
