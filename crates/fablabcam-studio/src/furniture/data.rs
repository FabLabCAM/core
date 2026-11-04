use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Construction {
    Cabinet,
    Shelving,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Dimensions {
    pub width: f64,
    pub depth: f64,
    pub height: f64,
    pub thickness: f64,
    pub back: bool,
    pub shelves: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition: Option<Box<super::Composition>>,
}
impl Default for Dimensions {
    fn default() -> Self {
        Self {
            width: 600.0,
            depth: 600.0,
            height: 800.0,
            thickness: 20.0,
            back: true,
            shelves: 1,
            composition: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub width: Range,
    pub depth: Range,
    pub height: Range,
    pub thickness: Range,
    pub max_shelves: u32,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            width: Range {
                min: 100.0,
                max: 5000.0,
            },
            depth: Range {
                min: 100.0,
                max: 5000.0,
            },
            height: Range {
                min: 100.0,
                max: 5000.0,
            },
            thickness: Range {
                min: 3.0,
                max: 80.0,
            },
            max_shelves: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub revision: u32,
    pub construction: Construction,
    pub defaults: Dimensions,
    pub limits: Limits,
}
impl Template {
    pub fn cabinet() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: "caisson-osb".into(),
            name: "Caisson OSB FabLab".into(),
            revision: 1,
            construction: Construction::Cabinet,
            defaults: Dimensions::default(),
            limits: Limits::default(),
        }
    }
    pub fn shelving() -> Self {
        Self {
            id: "etagere-ouverte".into(),
            name: "Étagère ouverte".into(),
            construction: Construction::Shelving,
            defaults: Dimensions {
                back: false,
                shelves: 3,
                height: 1200.0,
                ..Dimensions::default()
            },
            ..Self::cabinet()
        }
    }
    pub fn check(&self) -> Result<(), String> {
        version(self.schema_version)?;
        identity(&self.id, &self.name)?;
        if self.revision == 0 {
            return Err("La révision du modèle doit être positive".into());
        }
        self.check_dimensions(&self.defaults)
    }
    pub fn check_dimensions(&self, d: &Dimensions) -> Result<(), String> {
        for (name, value, range) in [
            ("Largeur", d.width, &self.limits.width),
            ("Profondeur", d.depth, &self.limits.depth),
            ("Hauteur", d.height, &self.limits.height),
            ("Épaisseur", d.thickness, &self.limits.thickness),
        ] {
            if !range.min.is_finite()
                || !range.max.is_finite()
                || range.min <= 0.0
                || range.max < range.min
                || range.max > 100_000.0
            {
                return Err(format!(
                    "{name} : limites incohérentes (0 < min ≤ max ≤ 100 000 mm)"
                ));
            }
            if !value.is_finite() || value < range.min || value > range.max {
                return Err(format!(
                    "{name} : valeur attendue entre {} et {} mm",
                    range.min, range.max
                ));
            }
        }
        if self.limits.max_shelves > 100 || d.shelves > self.limits.max_shelves {
            return Err("Nombre de tablettes hors limites (maximum absolu : 100)".into());
        }
        let shelves = if d.composition.is_some() {
            0
        } else {
            d.shelves
        };
        if d.width <= 2.0 * d.thickness
            || d.depth <= d.thickness
            || d.height <= (2 + shelves) as f64 * d.thickness
        {
            return Err(
                "Dimensions impossibles : les panneaux ne laissent aucun espace intérieur".into(),
            );
        }
        if let Some(c) = &d.composition {
            let scene = super::structure(d)?;
            let links = super::connections(&scene.panels);
            if c.joints.keys().any(|id| !links.iter().any(|l| &l.id == id)) {
                return Err("Liaison sans pièces correspondantes".into());
            }
            if c.panel_names
                .keys()
                .any(|id| id != "DOS" && !scene.panels.iter().any(|p| &p.id == id))
            {
                return Err("Nom de panneau sans élément correspondant".into());
            }
        }
        Ok(())
    }
}

pub(super) fn version(v: u32) -> Result<(), String> {
    if v != 1 && v != SCHEMA_VERSION {
        return Err(format!(
            "Version de format {v} non prise en charge (attendue : {SCHEMA_VERSION})"
        ));
    }
    Ok(())
}
pub(super) fn text(s: &str, label: &str) -> Result<(), String> {
    if s.trim().is_empty() || s.len() > 200 || s.chars().any(char::is_control) {
        return Err(format!(
            "{label} : texte non vide, sans contrôle, limité à 200 octets"
        ));
    }
    Ok(())
}
pub(super) fn identity(id: &str, name: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 80
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
    {
        return Err("Identifiant invalide : lettres ASCII, chiffres, - et _ uniquement".into());
    }
    text(name, "Nom")
}
pub fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "f-{now:x}-{:x}-{:x}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}
