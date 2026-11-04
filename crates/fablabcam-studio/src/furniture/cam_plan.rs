//! Requirements travel with CB files so a partial set of operations cannot pass as complete.
use super::{Contour, MachiningFace, Placement};
use crate::{OpKind, OpSide, Project};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CamRequirement {
    pub layer: String,
    pub depth: f64,
    pub pocket: bool,
    pub inside: bool,
}
impl CamRequirement {
    pub fn check(&self) -> Result<(), String> {
        if self.layer.is_empty()
            || self.layer.len() > 400
            || !self.depth.is_finite()
            || self.depth <= 0.0
        {
            return Err("Opération attendue invalide".into());
        }
        Ok(())
    }
}
pub fn placed_contour(
    p: &Placement,
    path: &Contour,
    face: MachiningFace,
    sheet_width: f64,
) -> Contour {
    path.iter()
        .map(|v| {
            let x = v[0] - p.panel.uv_min[0];
            let y = v[1] - p.panel.uv_min[1];
            let (x, y) = if p.rotated {
                (p.panel.height - y, x)
            } else {
                (x, y)
            };
            let x = x + p.x;
            let y = y + p.y;
            if face == MachiningFace::Back {
                [sheet_width - x, y, -v[2]]
            } else {
                [x, y, v[2]]
            }
        })
        .collect()
}
pub(super) fn check_requirements(project: &Project) -> Result<(), String> {
    for requirement in &project.furniture_requirements {
        requirement.check()?;
        let shapes = project
            .shapes
            .iter()
            .filter(|s| s.layer == requirement.layer)
            .collect::<Vec<_>>();
        if shapes.is_empty() {
            return Err(format!(
                "Géométrie de fabrication absente : {}",
                requirement.layer
            ));
        }
        for shape in shapes {
            let valid = project.ops.iter().any(|o| {
                o.enabled
                    && o.shapes.contains(&shape.id)
                    && if requirement.pocket {
                        o.kind == OpKind::Pocket
                            && (o.depth + requirement.depth).abs() < 1e-6
                            && o.stock_to_leave.abs() < 1e-6
                    } else {
                        o.kind == OpKind::Profile
                            && o.side
                                == if requirement.inside {
                                    OpSide::Inside
                                } else {
                                    OpSide::Outside
                                }
                            && o.depth <= -requirement.depth + 1e-6
                            && o.stock_to_leave.abs() < 1e-6
                    }
            });
            if !valid {
                return Err(format!(
                    "Préparer l’opération {} : {} à {:.2} mm",
                    requirement.layer,
                    if requirement.pocket {
                        "poche"
                    } else {
                        "profil"
                    },
                    requirement.depth
                ));
            }
        }
    }
    Ok(())
}
