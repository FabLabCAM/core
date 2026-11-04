//! Orthogonal contact inference and complementary machining in panel-local axes.
mod contacts;
mod contours;
mod generate;
use super::{CornerRelief, Dimensions, JointKind, JointSettings, Panel};
pub use contacts::connections;
pub use contours::{panel_entity, path_points};
pub(super) use generate::machine;
use serde::{Deserialize, Serialize};

pub type Contour = Vec<[f64; 3]>;
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MachiningFace {
    Front,
    Back,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MachiningFeature {
    pub joint: String,
    pub contour: Contour,
    pub depth: f64,
    pub face: MachiningFace,
    pub through: bool,
    pub in_outline: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Connection {
    pub id: String,
    pub male: String,
    pub female: String,
    /// The male end meets a female broad face; its axis is the female normal.
    pub axis: usize,
    pub high_end: bool,
    pub female_face: MachiningFace,
    pub span_axis: usize,
    pub span: [f64; 2],
}
impl Panel {
    pub fn axes(&self) -> [usize; 2] {
        match self.normal {
            0 => [2, 1],
            1 => [0, 2],
            _ => [0, 1],
        }
    }
    pub fn rectangular(
        id: &str,
        name: &str,
        origin: [f64; 3],
        size: [f64; 3],
        normal: usize,
    ) -> Self {
        let axes = match normal {
            0 => [2, 1],
            1 => [0, 2],
            _ => [0, 1],
        };
        let w = size[axes[0]];
        let h = size[axes[1]];
        Self {
            id: id.into(),
            name: name.into(),
            origin,
            size,
            normal,
            width: w,
            height: h,
            thickness: size[normal],
            assembly: "frame".into(),
            uv_min: [0.0, 0.0],
            outline: contours::rectangle([0.0, 0.0], [w, h]),
            features: Vec::new(),
        }
    }
}

/// Same machining used by fabrication, exposed for the UI's selected-pair preview.
pub fn machining_preview(
    mut parts: Vec<Panel>,
    composition: &super::Composition,
    cutter: f64,
) -> Result<Vec<Panel>, String> {
    let d = Dimensions {
        composition: Some(Box::new(composition.clone())),
        ..Dimensions::default()
    };
    machine(&mut parts, &d, cutter)?;
    Ok(parts)
}
