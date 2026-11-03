use super::{
    data::{identity, text, version},
    new_id, Dimensions, Template, SCHEMA_VERSION,
};
use fablabcam_cam::gcode::machine::Profile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    pub material: String,
    /// Snapshot, not a lookup by display name. Unknown physical limits stay unknown.
    pub machine: Profile,
    /// Working rectangle declared for this job in mm; not a qualification of the machine.
    pub working_size: [f64; 2],
    pub sheet_size: [f64; 2],
    pub margin: f64,
    pub cutter: f64,
    pub spacing: f64,
    pub rotate: bool,
}
impl Default for Setup {
    fn default() -> Self {
        Self {
            material: "OSB".into(),
            machine: Profile::default(),
            working_size: [1500.0, 800.0],
            sheet_size: [1500.0, 800.0],
            margin: 10.0,
            cutter: 6.0,
            spacing: 8.0,
            rotate: true,
        }
    }
}
impl Setup {
    pub fn check(&self) -> Result<(), String> {
        self.machine.check()?;
        text(&self.material, "Matériau")?;
        for value in self.working_size.into_iter().chain(self.sheet_size).chain([
            self.margin,
            self.cutter,
            self.spacing,
        ]) {
            if !value.is_finite() || value <= 0.0 || value > 100_000.0 {
                return Err("CNC et dalle : valeurs finies positives ≤ 100 000 mm requises".into());
            }
        }
        // A contour cutter sweeps one diameter beyond the finished edge, not only a radius.
        if self.margin < self.cutter || self.spacing < self.cutter {
            return Err(
                "Marge et espacement doivent être au moins égaux au diamètre de fraise".into(),
            );
        }
        for (i, axis) in ['X', 'Y'].into_iter().enumerate() {
            if self.sheet_size[i] > self.working_size[i] {
                return Err(format!("Dalle trop grande pour la course utile {axis}"));
            }
            if self.sheet_size[i] <= 2.0 * self.margin {
                return Err("La marge ne laisse aucune surface utile".into());
            }
            if let Some(a) = self.machine.axes.get(&axis) {
                if let (Some(min), Some(max)) = (a.min, a.max) {
                    if self.working_size[i] > max - min {
                        return Err(format!(
                            "Course utile {axis} supérieure aux limites du profil machine"
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Furniture {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub template: Template,
    pub dimensions: Dimensions,
    pub setup: Setup,
}
impl Furniture {
    pub fn from_template(template: Template) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: new_id(),
            name: template.name.clone(),
            dimensions: template.defaults.clone(),
            template,
            setup: Setup::default(),
        }
    }
    /// A native project may be saved with a non-placeable setup, but never invalid furniture.
    pub fn check(&self) -> Result<(), String> {
        version(self.schema_version)?;
        identity(&self.id, &self.name)?;
        self.template.check()?;
        self.template.check_dimensions(&self.dimensions)?;
        self.setup.check()
    }
}
