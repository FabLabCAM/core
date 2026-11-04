//! Parametric furniture use cases. No GUI or machine control lives here.
//! Projects embed their template and machine: library updates cannot change a job.
mod cam_plan;
mod composition;
mod data;
mod document;
mod fabrication;
mod geometry;
mod joinery;
mod joints;
mod layout;
mod library;
mod manual;
mod setup;

pub use cam_plan::{placed_contour, CamRequirement};
pub use composition::*;
pub use data::*;
pub use document::{Document, Draft};
pub use fabrication::{
    cam_project, cam_project_for_face, fabrication_files, fabrication_zip, ExportOptions,
};
pub use geometry::{panels, panels_with_cutter, Panel};
pub use joinery::*;
pub use joints::{
    connections, machining_preview, panel_entity, path_points, Connection, Contour, MachiningFace,
    MachiningFeature,
};
pub use layout::{place, Layout, Placement, Sheet};
pub use library::{export_file, read_document, Library};
pub use setup::*;

/// Shared by native and CLI exports: a furniture CAM project never bypasses its
/// attached machine just because it was saved/reopened as a CB document.
pub fn check_cam_export(studio: &crate::Studio, code: &str) -> Result<(), String> {
    use fablabcam_cam::gcode::machine::{validate, Severity, Status};
    cam_plan::check_requirements(studio.project())?;
    let Some(profile) = &studio.project().machine_profile else {
        return Ok(());
    };
    let enabled = studio
        .project()
        .ops
        .iter()
        .filter(|o| o.enabled)
        .collect::<Vec<_>>();
    if enabled.is_empty()
        || enabled.iter().any(|o| {
            studio
                .computed
                .get(&o.id)
                .is_none_or(|c| c.error.is_some() || c.path.is_none())
        })
    {
        return Err("Préparez des opérations FAO valides avant l'export de cette dalle".into());
    }
    let report = validate(code, profile);
    if report.status != Status::Valid {
        let messages = report
            .diagnostics
            .iter()
            .filter(|d| d.severity != Severity::Info)
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
            .join(" ; ");
        return Err(format!(
            "Export bloqué pour {} ({:?}) : {messages}",
            profile.name, report.status
        ));
    }
    Ok(())
}
