use super::context::Workspace;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Ask,
    Plan,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    /// Short title identifying the requested scope.
    pub title: String,
    /// Explicit assumptions, never invented object identifiers or cutting parameters.
    pub assumptions: Vec<String>,
    /// Blocking missing inputs only. MUST be [] when all requested parameters are known.
    pub missing: Vec<String>,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// Concise explanation of the effect, not a request for permission.
    pub explanation: String,
    pub action: Action,
}

impl Action {
    pub fn label(&self) -> &'static str {
        match self {
            Self::SetCircleRadius { .. } => "Modifier le rayon",
            Self::MoveShapes { .. } => "Déplacer les formes",
            Self::ReplaceScript { .. } => "Remplacer le texte Rhai",
            Self::RunScript { .. } => "Exécuter le script",
            Self::CreateCabinet { .. } => "Créer le caisson",
            Self::ResizeFurniture { .. } => "Redimensionner le meuble",
            Self::DivideCell { .. } => "Diviser la case",
            Self::AddBin { .. } => "Ajouter un bac",
            Self::SetJoint { .. } => "Régler l'assemblage",
            Self::DuplicateTool { .. } => "Dupliquer l'outil",
            Self::CreateTool { .. } => "Créer l'outil",
            Self::UpdateTool { .. } => "Modifier l'outil",
            Self::CreateOperation { .. } => "Créer l'opération",
            Self::UpdateOperation { .. } => "Modifier l'opération",
            Self::PrepareExport { .. } => "Préparer un export",
        }
    }
}

/// All lengths are millimetres; cut feeds mm/min and spindle rpm. No implicit defaults.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    SetCircleRadius {
        ids: Vec<u32>,
        radius: f64,
    },
    MoveShapes {
        ids: Vec<u32>,
        dx: f64,
        dy: f64,
    },
    ReplaceScript {
        code: String,
    },
    RunScript {
        code: String,
    },
    CreateCabinet {
        name: String,
        width: f64,
        depth: f64,
        height: f64,
        thickness: f64,
        shelves: u32,
        back: bool,
    },
    ResizeFurniture {
        width: f64,
        depth: f64,
        height: f64,
    },
    DivideCell {
        cell_id: String,
        vertical: bool,
        count: usize,
    },
    AddBin {
        cell_id: String,
        depth: f64,
        thickness: f64,
        side_gap: f64,
        top_gap: f64,
        back_gap: f64,
    },
    SetJoint {
        connection_id: String,
        settings: crate::furniture::JointSettings,
    },
    DuplicateTool {
        source: String,
        name: String,
        diameter: f64,
    },
    CreateTool {
        name: String,
        kind: String,
        diameter: f64,
        feed: f64,
        plunge: f64,
        spindle: f64,
    },
    UpdateTool {
        name: String,
        diameter: f64,
        feed: f64,
        plunge: f64,
        spindle: f64,
    },
    CreateOperation {
        name: String,
        kind: String,
        ids: Vec<u32>,
        tool: String,
        depth: f64,
        step_down: f64,
        step_over: f64,
    },
    UpdateOperation {
        id: u32,
        tool: String,
        depth: f64,
        step_down: f64,
        step_over: f64,
    },
    /// Prepare an export through the native UI; no path is supplied by the model.
    PrepareExport {
        format: ExportFormat,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Cb,
    Svg,
    Dxf,
    Gcode,
    FurnitureJson,
    FurnitureZip,
}

#[derive(Debug, Clone)]
pub struct PreparedPlan {
    pub plan: Plan,
    pub before: Workspace,
    pub after: Workspace,
    pub results: Vec<String>,
    pub exports: Vec<ExportFormat>,
    pub context_token: String,
    pub before_project: crate::studio::PreparedProject,
    pub after_project: crate::studio::PreparedProject,
}

impl Plan {
    pub fn check(&self) -> Result<(), String> {
        if self.title.trim().is_empty() || self.title.len() > 512 {
            return Err("Le plan doit avoir un titre de 1 à 512 octets".into());
        }
        if self.steps.is_empty() || self.steps.len() > 12 {
            return Err("Un lot contient entre 1 et 12 étapes".into());
        }
        if !self.missing.is_empty() {
            return Err(format!("Informations à compléter : {}", self.missing.join(" ; ")));
        }
        if self.steps.len() != 1 && self.steps.iter().any(|s| matches!(s.action, Action::PrepareExport { .. })) {
            return Err("Préparez chaque export dans un lot distinct, après les modifications du projet".into());
        }
        if serde_json::to_vec(self).map_err(|_| "Plan non sérialisable")?.len() > 128_000 {
            return Err("Plan trop volumineux (128 Ko maximum)".into());
        }
        Ok(())
    }
}
