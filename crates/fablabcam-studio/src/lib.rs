//! Cœur d'édition de FabLabCAM, sans interface : formes 2D, historique, outils, opérations,
//! recalcul des trajectoires, G-code, import/export DXF et SVG, simulation de matière.
//!
//! L'interface egui et la page web ne font qu'afficher et déclencher ces fonctions.

pub mod gcode_store;
pub mod machine_profiles;
pub mod cad;
pub mod cbnative;
pub mod curves;
pub mod dxf;
pub mod edit;
pub mod feeds;
pub mod history;
pub mod persist;
pub mod project;
pub mod script;
pub mod shape;
pub mod sim;
pub mod studio;
pub mod svgio;
pub mod text;
pub mod view3d;
pub mod web;

pub use history::Editor;
pub use project::{CamSettings, TabShape, TabTarget, LeadChoice, OpKind, OpSide, Operation, Project, Shape, Stock, Tool, ToolKind};
pub use studio::Studio;
