//! Preparation is transactional. No provider or preparation worker receives a live Studio.
mod furniture;
use super::{
    plan::{Action, Mode, Plan, PreparedPlan},
    ContextSnapshot, Workspace,
};
use crate::{shape, Tool, ToolKind};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub fn prepare(
    w: &Workspace,
    context: &ContextSnapshot,
    plan: Plan,
    mode: Mode,
    cancelled: Arc<AtomicBool>,
) -> Result<PreparedPlan, String> {
    if mode == Mode::Ask {
        return Err("Ask est en lecture seule : préparation d'actions refusée".into());
    }
    plan.check()?;
    if mode == Mode::Plan && plan.steps.iter().any(|s| matches!(s.action, Action::RunScript { .. })) {
        return Err("Plan n'exécute aucun script. Relisez le code puis passez en Agent pour préparer son exécution".into());
    }
    if context.workspace_token != w.fingerprint() {
        return Err("Contexte obsolète : actualisez puis préparez à nouveau".into());
    }
    let mut after = w.clone();
    let mut results = Vec::new();
    let mut exports = Vec::new();
    for step in &plan.steps {
        if cancelled.load(Ordering::Relaxed) {
            return Err("Préparation arrêtée, aucune modification appliquée".into());
        }
        if let Action::PrepareExport { format } = step.action {
            if matches!(format, super::plan::ExportFormat::FurnitureJson | super::plan::ExportFormat::FurnitureZip) && w.furniture.is_none()
            {
                return Err("Aucun meuble ouvert à exporter".into());
            }
            exports.push(format);
            results.push("Export préparé ; destination et éventuel écrasement à confirmer dans le dialogue natif".into());
            continue;
        }
        results.push(apply(&mut after, &step.action, cancelled.clone())?);
        if after.project.shapes.len() > 10_000 || after.project.ops.len() > 200 {
            return Err("Limite du lot : 10 000 formes et 200 opérations".into());
        }
    }
    let before_project = crate::studio::PreparedProject::new(w.project.clone());
    let after_project = crate::studio::PreparedProject::new(after.project.clone());
    results.extend(after_project.diagnostics().into_iter().map(|d| format!("Diagnostic à résoudre : {d}")));
    if cancelled.load(Ordering::Relaxed) {
        return Err("Préparation arrêtée".into());
    }
    Ok(PreparedPlan {
        plan,
        before: w.clone(),
        after,
        results,
        exports,
        context_token: context.token.clone(),
        before_project,
        after_project,
    })
}

pub fn authorize(
    prepared: &PreparedPlan,
    current: &Workspace,
    context: &ContextSnapshot,
    mode: Mode,
    cancelled: bool,
) -> Result<(), String> {
    if mode != Mode::Agent {
        return Err("Passez explicitement en Agent avant d'appliquer".into());
    }
    if cancelled {
        return Err("Lot arrêté".into());
    }
    if prepared.before.fingerprint() != current.fingerprint() || prepared.context_token != context.token {
        return Err("Document, sélection ou contexte modifié : préparez et vérifiez un nouvel aperçu".into());
    }
    Ok(())
}

fn apply(w: &mut Workspace, a: &Action, cancelled: Arc<AtomicBool>) -> Result<String, String> {
    let edit_error = |e: shape::EditError| e.to_string();
    match a {
        Action::ReplaceScript { code } => {
            crate::script::validate(code)?;
            w.script = code.clone();
            Ok("Texte Rhai remplacé ; dessin inchangé, exécution séparée".into())
        }
        Action::RunScript { code } => {
            let output = crate::script::run_controlled(&w.project, code, cancelled)?;
            let count = output.project.shapes.len();
            w.project = output.project;
            Ok(format!("Script préparé : {count} forme(s) après exécution. {}", output.log.join(" ; ")))
        }
        Action::SetCircleRadius { ids, radius } => {
            check_ids(ids)?;
            for id in ids {
                let e = &w.project.shape(*id).ok_or("Forme absente")?.entity;
                let fablabcam_model::Entity::Circle { cx, cy, .. } = e else {
                    return Err("La sélection contient une forme non circulaire".into());
                };
                let entity = shape::circle((*cx, *cy), *radius).map_err(edit_error)?;
                w.project.replace_shape(*id, entity).map_err(edit_error)?;
            }
            Ok(format!("{} cercle(s), rayon {radius} mm, centres conservés", ids.len()))
        }
        Action::MoveShapes { ids, dx, dy } => {
            check_ids(ids)?;
            if !dx.is_finite() || !dy.is_finite() {
                return Err("Déplacement non fini".into());
            }
            w.project.move_shapes(ids, *dx, *dy).map_err(edit_error)?;
            Ok(format!("{} forme(s) déplacée(s) de {dx} ; {dy} mm", ids.len()))
        }
        Action::DuplicateTool { source, name, diameter } => {
            let mut t = w.project.tool(source).ok_or("Outil source absent du projet")?.clone();
            t.name = name.clone();
            t.diameter = *diameter;
            t.index = 0;
            w.project.add_tool(t).map_err(edit_error)?;
            Ok(format!("Copie {name}, Ø{diameter} ; réglages copiés à vérifier, opérations inchangées"))
        }
        Action::CreateTool { name, kind, diameter, feed, plunge, spindle } => {
            let kind = ToolKind::from_key(kind).ok_or("Type d'outil inconnu")?;
            let mut t = Tool::new(name, kind, *diameter);
            t.feed = *feed;
            t.plunge = *plunge;
            t.spindle = *spindle;
            w.project.add_tool(t).map_err(edit_error)?;
            Ok(format!("Outil {name} créé dans le projet ; aucune opération réaffectée"))
        }
        Action::UpdateTool { name, diameter, feed, plunge, spindle } => {
            let mut t = w.project.tool(name).ok_or("Outil absent du projet")?.clone();
            t.diameter = *diameter;
            t.feed = *feed;
            t.plunge = *plunge;
            t.spindle = *spindle;
            w.project.update_tool(name, t).map_err(edit_error)?;
            Ok(format!("Outil {name} modifié ; les opérations qui le référencent utiliseront ces paramètres"))
        }
        Action::CreateOperation { name, kind, ids, tool, depth, step_down, step_over } => {
            check_ids(ids)?;
            let kind = crate::OpKind::from_key(kind).ok_or("Type d'opération inconnu")?;
            let id = w.project.add_operation(kind, name, ids.clone(), tool).map_err(edit_error)?;
            operation(w, id, tool, *depth, *step_down, *step_over)?;
            Ok(format!("Opération {id} : {name}, trajectoire vérifiée"))
        }
        Action::UpdateOperation { id, tool, depth, step_down, step_over } => {
            operation(w, *id, tool, *depth, *step_down, *step_over)?;
            Ok(format!("Opération {id} modifiée, trajectoire vérifiée"))
        }
        Action::PrepareExport { .. } => unreachable!("handled before domain edits"),
        _ => furniture::apply(w, a),
    }
}

fn check_ids(ids: &[u32]) -> Result<(), String> {
    if ids.is_empty() || ids.len() > 2000 {
        return Err("Sélection de 1 à 2000 identifiants requise".into());
    }
    Ok(())
}

fn operation(w: &mut Workspace, id: u32, tool: &str, depth: f64, step_down: f64, step_over: f64) -> Result<(), String> {
    if [depth, step_down, step_over].iter().any(|x| !x.is_finite()) {
        return Err("Paramètre FAO non fini".into());
    }
    w.project
        .update_operation(id, |o| {
            o.tool = tool.into();
            o.depth = depth;
            o.step_down = step_down;
            o.step_over = step_over;
        })
        .map_err(|e| e.to_string())?;
    crate::studio::compute_op(&w.project, w.project.op(id).ok_or("Opération absente")?)?;
    Ok(())
}
