//! A shallow native tool for small local models, translated into the existing Plan.
//! This is a proposal transport, never an execution path or a text-to-tool repair.
use crate::assistant::{plan::Step, Action, ContextSnapshot, Plan};
use genai::chat::Tool;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DivisionProposal {
    /// Exact identifier of the target cell from the transmitted context.
    cell_id: String,
    /// side_by_side: equal widths; stacked: equal heights.
    layout: Layout,
    /// Number of resulting cells, not the number of dividers.
    count: usize,
}

// Name the arrangement instead of asking a small model to infer a boolean axis.
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Layout {
    SideBySide,
    Stacked,
}

pub(super) fn tool(context: &ContextSnapshot) -> Option<Tool> {
    let cells = context.data.pointer("/furniture_structure/cells")?.as_array()?;
    let ids: Vec<_> = cells.iter().filter(|c| c["leaf"] == true).filter_map(|c| c["id"].as_str()).collect();
    if ids.is_empty() {
        return None;
    }
    let mut schema = serde_json::to_value(schemars::schema_for!(DivisionProposal)).expect("static schema");
    // Inline this enum so the small model sees both choices at the argument site.
    schema["properties"]["layout"] = serde_json::to_value(schemars::schema_for!(Layout)).expect("static schema");
    schema.as_object_mut().expect("object schema").remove("$defs");
    schema["properties"]["cell_id"]["enum"] = json!(ids);
    if let Some(selected) = context.data["selected_cell"].as_str().filter(|id| ids.contains(id)) {
        schema["properties"]["cell_id"]["description"] = json!(format!("Pour la case sélectionnée, utiliser exactement {selected}. Un autre identifiant ne convient que si la demande vise explicitement une autre case."));
    }
    Some(Tool::new("divide_cell")
        .with_description("Propose de diviser une seule case du meuble en parts égales. Ne modifie rien : aperçu et validation humaine obligatoires. layout=side_by_side donne des cases de même largeur côte à côte ; layout=stacked donne des cases de même hauteur superposées. count est le nombre de cases, pas de séparateurs. Tous les paramètres sont obligatoires. Si une information manque, utiliser submit_plan avec missing.")
        .with_schema(schema).with_strict(false))
}

pub(super) fn plan(arguments: Value) -> Result<Plan, String> {
    let p: DivisionProposal = serde_json::from_value(arguments).map_err(|_| "Arguments de division invalides")?;
    Ok(Plan {
        title: "Diviser la case".into(),
        assumptions: vec![],
        missing: vec![],
        steps: vec![Step {
            explanation: format!(
                "Proposer {} cases {} égales dans {}",
                p.count,
                if matches!(p.layout, Layout::SideBySide) { "côte à côte" } else { "superposées" },
                p.cell_id
            ),
            action: Action::DivideCell { cell_id: p.cell_id, vertical: matches!(p.layout, Layout::SideBySide), count: p.count },
        }],
    })
}
