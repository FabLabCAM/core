use crate::{furniture, persist, Project};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct Workspace {
    pub document_id: String,
    pub revision: u64,
    pub project: Project,
    pub script: String,
    pub furniture: Option<furniture::Document>,
}

impl Workspace {
    pub fn fingerprint(&self) -> String {
        let mut hash = Sha256::new();
        for bytes in [
            self.document_id.as_bytes(),
            &self.revision.to_le_bytes(),
            persist::save(&self.project).as_bytes(),
            self.script.as_bytes(),
            self.furniture.as_ref().and_then(|d| d.to_json().ok()).unwrap_or_default().as_bytes(),
        ] {
            hash.update(bytes.len().to_le_bytes());
            hash.update(bytes);
        }
        format!("{:x}", hash.finalize())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ContextOptions {
    pub drawing: bool,
    pub selected_only: bool,
    pub script: bool,
    pub furniture: bool,
    pub tools: bool,
    pub diagnostics: bool,
    pub history: bool,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self { drawing: true, selected_only: true, script: false, furniture: false, tools: true, diagnostics: true, history: true }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextMeta {
    pub page: String,
    pub selection: Vec<u32>,
    pub cell: Option<String>,
    pub diagnostics: Vec<String>,
    pub simulation: Value,
    pub machine: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub token: String,
    pub workspace_token: String,
    pub page: String,
    pub revision: u64,
    pub data: Value,
}

impl ContextSnapshot {
    pub fn capture(w: &Workspace, meta: &ContextMeta, options: &ContextOptions) -> Result<Self, String> {
        let mut data = json!({"page":meta.page,"document":w.document_id,
            "revision":w.revision,"units":"mm","selection":meta.selection});
        if options.drawing {
            data["drawing"] = Value::Array(
                w.project
                    .shapes
                    .iter()
                    .filter(|s| !options.selected_only || meta.selection.contains(&s.id))
                    .take(2000)
                    .map(|s| {
                        json!({"id":s.id,"layer":s.layer,
                    "geometry":geometry(&s.entity),"bounds_mm":crate::shape::bounds(&s.entity)})
                    })
                    .collect(),
            );
            data["drawing_total"] = json!(w.project.shapes.len());
            data["drawing_scope"] =
                json!(if options.selected_only { "selection uniquement, 2000 maximum" } else { "2000 premières formes maximum" });
        }
        if options.script {
            data["script"] = json!(w.script);
        }
        if options.furniture {
            data["furniture"] = serde_json::to_value(&w.furniture).map_err(|e| e.to_string())?;
            data["selected_cell"] = json!(meta.cell);
            if let Some(doc) = &w.furniture {
                data["furniture_structure"] = match furniture::structure(doc.dimensions()) {
                    Ok(s) => {
                        json!({"connections":furniture::connections(&s.panels),"panels":s.panels.iter().map(|p|json!({"id":p.id,"name":p.name,"size":p.size})).collect::<Vec<_>>(),
                        "cells":s.cells.iter().map(|c|json!({"id":c.id,"size":c.size,"leaf":c.leaf})).collect::<Vec<_>>() })
                    }
                    Err(e) => json!({"error":e}),
                };
            }
        }
        if options.tools {
            data["tools"] = json!(w
                .project
                .tools
                .iter()
                .map(|t| json!({"name":t.name,"kind":t.kind.key(),
                "diameter":t.diameter,"feed":t.feed,"plunge":t.plunge,"spindle":t.spindle}))
                .collect::<Vec<_>>());
            data["operations"] = json!(w
                .project
                .ops
                .iter()
                .map(|o| json!({"id":o.id,"name":o.name,
                "kind":o.kind.key(),"shapes":o.shapes,"tool":o.tool,"depth":o.depth,
                "step_down":o.step_down,"step_over":o.step_over,"enabled":o.enabled}))
                .collect::<Vec<_>>());
            data["machine"] = meta.machine.clone();
        }
        if options.diagnostics {
            data["diagnostics"] = json!(meta.diagnostics);
            data["simulation"] = meta.simulation.clone();
        }
        let encoded = serde_json::to_vec(&data).map_err(|e| e.to_string())?;
        if encoded.len() > 128_000 {
            return Err("Contexte supérieur à 128 Ko : réduisez les éléments transmis".into());
        }
        let workspace_token = w.fingerprint();
        let token = format!("{:x}", Sha256::digest([workspace_token.as_bytes(), &encoded].concat()));
        Ok(Self { token, workspace_token, page: meta.page.clone(), revision: w.revision, data })
    }
}

// Units are explicit: a model must not interpret a radius as a diameter or a grid as millimetres.
fn geometry(entity: &fablabcam_model::Entity) -> Value {
    use fablabcam_geom::PlineSource;
    use fablabcam_model::Entity;
    match entity {
        Entity::Circle { cx, cy, r } => json!({"type":"circle","center_mm":[cx,cy],"radius_mm":r,"diameter_mm":2.0*r}),
        Entity::Arc { cx, cy, r, start_deg, sweep_deg } => {
            json!({"type":"arc","center_mm":[cx,cy],"radius_mm":r,"diameter_mm":2.0*r,"start_deg":start_deg,"sweep_deg":sweep_deg})
        }
        Entity::Point { x, y } => json!({"type":"point","position_mm":[x,y]}),
        Entity::Rect { x, y, w, h, r, rot_deg } => {
            json!({"type":"rectangle","origin_mm":[x,y],"size_mm":[w,h],"corner_radius_mm":r,"rotation_deg":rot_deg})
        }
        Entity::Polyline(p) => {
            json!({"type":"polyline","closed":p.is_closed(),"vertices":(0..p.vertex_count()).map(|i|{let v=p.at(i);json!({"position_mm":[v.x,v.y],"bulge":v.bulge})}).collect::<Vec<_>>()})
        }
    }
}
