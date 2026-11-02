//! Moteur CAM de FabLabCAM : styles, paramètres, trajectoires et G-code.

pub mod gcode;
pub mod params;
pub mod post;
pub mod style;
pub mod toolpath;

pub use gcode::{emit, Post};
pub use post::{render as render_post, ArcCenter, ArcOutput, LineNumbering, Mop, PostDef, PostOutput, Program, ToolInfo};
pub use params::{CutOrdering, Lead, LeadKind, MillingDirection, Params, TabMethod, TabStyle, Tabs};
pub use style::{CamStyle, StyleLibrary};
pub use toolpath::{drill, scaled, pocket, pocket_region, pocket_shapes, profile, CamError, Move, Side, Toolpath};
