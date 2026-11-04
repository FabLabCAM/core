use super::*;
use crate::{App, SideTab, View};
#[cfg(test)]
mod tests;
use assistant::{context::ContextMeta, provider, worker::Job, Mode};

impl App {
    fn assistant_page(&self) -> &'static str {
        if self.show_script {
            "Scripts"
        } else if self.furniture.open {
            "Meubles"
        } else if self.machines.open {
            "Profils machine"
        } else if self.side == SideTab::Systeme {
            "Outils"
        } else {
            match self.view {
                View::Plan => "CAO / Plan 2D",
                View::Vue3D => "3D",
                View::Simulation => "Simulation",
            }
        }
    }
    fn assistant_workspace(&self) -> Workspace {
        Workspace {
            document_id: self.assistant_document.clone(),
            revision: self.studio.rev,
            project: self.studio.project().clone(),
            script: self.script_code.clone(),
            furniture: self.furniture.assistant_document(),
        }
    }
    fn assistant_context(&self, w: &Workspace) -> Result<ContextSnapshot, String> {
        let diagnostics =
            self.studio.computed.iter().filter_map(|(id, c)| c.error.as_ref().map(|e| format!("Opération {id} : {e}"))).collect();
        let simulation = match &self.sim {
            Some((rev, fraction, resolution, out)) => {
                serde_json::json!({"progress":fraction,"resolution_mm":resolution,"project_revision":rev,
                "current":*rev==self.studio.rev,"complete":*fraction>=1.0,"gouges":out.gouges.len(),
                "stock":{"min":out.map.min,"grid_size":[out.map.nx,out.map.ny],"resolution_mm":out.map.res,"top":out.map.top,"bottom":out.map.bottom},
                "gouge_details":out.gouges.iter().take(50).map(|g|serde_json::json!({"kind":format!("{:?}",g.kind),"position":g.pos,"operation":g.op})).collect::<Vec<_>>(),
                "gouge_details_truncated":out.gouges.len()>50,"path_length_mm":out.total_len,
                "scope":"Simulation de matière 2.5D ; pas une qualification machine"})
            }
            None => serde_json::json!({"available":false,"complete":false}),
        };
        let meta = ContextMeta {
            page: self.assistant_page().into(),
            selection: self.selection.clone(),
            cell: self.furniture.assistant_cell(),
            diagnostics,
            simulation,
            machine: serde_json::to_value(self.machines.selected_profile().or_else(|| self.studio.project().machine_profile.clone()))
                .unwrap_or_default(),
        };
        ContextSnapshot::capture(w, &meta, &self.assistant.session.options)
    }
    pub(crate) fn assistant_panel(&mut self, ctx: &egui::Context) {
        self.assistant.poll();
        let page = self.assistant_page();
        if let Some(action) = self.assistant.show(ctx, page, self.studio.rev) {
            if let Err(e) = self.assistant_action(ctx, action) {
                self.assistant.message = e;
            }
        }
    }
    fn assistant_action(&mut self, ctx: &egui::Context, action: UiAction) -> Result<(), String> {
        match action {
            UiAction::Stop => self.assistant.stop(),
            UiAction::Clear => {
                self.assistant.session.entries.clear();
                self.assistant.plan = None;
                self.assistant.prepared = None;
                self.assistant.context = None;
                self.assistant.last_reply = None;
                self.assistant.persist();
            }
            UiAction::Pin => self.assistant.pinned = Some(self.assistant_context(&self.assistant_workspace())?),
            UiAction::Unpin => self.assistant.pinned = None,
            UiAction::Send | UiAction::Probe => self.assistant_send(ctx, matches!(action, UiAction::Probe))?,
            UiAction::Prepare => {
                let w = self.assistant_workspace();
                let current = self.assistant_context(&w)?;
                let context = self.assistant.context.clone().ok_or("Envoyez d'abord une demande")?;
                if context.token != current.token {
                    return Err("Le contexte a changé : envoyez à nouveau la demande avec le contexte actuel".into());
                }
                let plan = self.assistant.plan.clone().ok_or("Aucun plan")?;
                self.assistant.generation += 1;
                self.assistant.job =
                    Some(Job::prepare(self.assistant.generation, w, context, plan, self.assistant.session.mode, AssistantUi::wake(ctx)));
                self.assistant.message = "Calcul de l'aperçu…".into();
            }
            UiAction::Apply => self.assistant_apply(ctx)?,
            UiAction::Undo | UiAction::Redo => self.assistant_restore(matches!(action, UiAction::Redo))?,
        }
        Ok(())
    }
    fn assistant_send(&mut self, ctx: &egui::Context, probe: bool) -> Result<(), String> {
        let mut c = self.assistant.session.connection();
        let key = self.assistant.key(&c)?;
        let w = self.assistant_workspace();
        let context = if probe {
            let options = assistant::ContextOptions {
                drawing: false,
                script: false,
                furniture: false,
                tools: false,
                diagnostics: false,
                history: false,
                ..Default::default()
            };
            ContextSnapshot::capture(
                &Workspace {
                    document_id: "test-connexion".into(),
                    revision: 0,
                    project: Default::default(),
                    script: String::new(),
                    furniture: None,
                },
                &ContextMeta { page: "Test connexion".into(), ..Default::default() },
                &options,
            )?
        } else {
            self.assistant.pinned.clone().map(Ok).unwrap_or_else(|| self.assistant_context(&w))?
        };
        let mode = if probe {
            c.tools = true;
            Mode::Plan
        } else {
            self.assistant.session.mode
        };
        let prompt = if probe {
            "Teste submit_plan sans effet : titre Test de connexion, assumptions [], missing [], steps []. Ne propose aucune modification."
                .into()
        } else {
            self.assistant.prompt.clone()
        };
        let history = if self.assistant.session.options.history && !probe { self.assistant.session.entries.as_slice() } else { &[] };
        let request = provider::request(&c, mode, &context, history, &prompt)?;
        if !probe {
            self.assistant.session.push(true, assistant::storage::redact(&prompt, &key), &context, &c);
            self.assistant.prompt.clear();
        }
        self.assistant.generation += 1;
        self.assistant.turns = 1;
        self.assistant.job = Some(Job::chat(self.assistant.generation, c.clone(), key, request, mode, AssistantUi::wake(ctx)));
        self.assistant.active_connection = Some(c);
        self.assistant.context = Some(context);
        self.assistant.probe = probe;
        self.assistant.prepared = None;
        self.assistant.plan = None;
        self.assistant.partial.clear();
        self.assistant.last_reply = None;
        self.assistant.message = "Connexion et génération…".into();
        self.assistant.persist();
        Ok(())
    }
    fn assistant_apply(&mut self, ctx: &egui::Context) -> Result<(), String> {
        let prepared = self.assistant.prepared.as_ref().ok_or("Préparez l'aperçu")?;
        let w = self.assistant_workspace();
        let current = self.assistant_context(&w)?;
        let c = self.assistant.active_connection.clone().ok_or("Connexion absente")?;
        if c.identity() != self.assistant.session.connection().identity() {
            return Err("Fournisseur ou modèle modifié : envoyez une nouvelle demande".into());
        }
        if self.dialog.is_some() {
            return Err("Terminez le dialogue de fichier avant d'appliquer".into());
        }
        // This button is the explicit Plan -> Agent authorization, for this exact preview.
        self.assistant.session.mode = Mode::Agent;
        assistant::actions::authorize(prepared, &w, &current, self.assistant.session.mode, false)?;
        let p = self.assistant.prepared.take().expect("checked");
        self.assistant_install(&p.after, p.after_project.clone())?;
        let after = self.assistant_workspace();
        if p.exports.is_empty() {
            self.assistant.undo =
                Some(UndoLot { before: p.before, after, before_project: p.before_project, after_project: p.after_project });
            self.assistant.redone = false;
            self.assistant.message = "Lot appliqué — Annuler le lot reste disponible".into();
            self.status = format!("[Assistant] {} étape(s) appliquée(s)", p.plan.steps.len());
        } else {
            self.assistant.message =
                "Dialogue d'export ouvert ; aucun fichier confirmé. Un fichier exporté n'est pas annulable par le lot.".into();
            self.status = "[Assistant] Destination d'export à choisir".into();
        }
        let results = p.results.join("\n");
        self.assistant.session.push(false, format!("Résultat FabLabCAM :\n{results}"), &current, &c);
        self.assistant.plan = None;
        // Exports remain explicit native actions and are never reported as files already written.
        for format in p.exports {
            self.assistant_export(format);
        }
        if self.assistant.turns < c.max_turns {
            if let Some(reply) = self.assistant.last_reply.take() {
                let next_context = self.assistant_context(&self.assistant_workspace())?;
                let approved = serde_json::to_string(&p.plan).map_err(|_| "Plan non sérialisable")?;
                let request=reply.continuation(&format!("Lot réellement validé par l'utilisateur (paramètres éventuellement corrigés) : {approved}\nRésultat : {results}\nContexte actuel remplaçant le précédent : {}\nRends compte du résultat. Un export préparé n'est pas encore un fichier écrit. Toute autre modification nécessite un nouveau lot validé.",next_context.data))?;
                let key = self.assistant.key(&c)?;
                self.assistant.context = Some(next_context);
                self.assistant.turns += 1;
                self.assistant.generation += 1;
                self.assistant.job = Some(Job::chat(self.assistant.generation, c, key, request, Mode::Agent, AssistantUi::wake(ctx)));
            }
        }
        self.assistant.persist();
        Ok(())
    }
    fn assistant_install(&mut self, w: &Workspace, project: fablabcam_studio::studio::PreparedProject) -> Result<(), String> {
        self.furniture.assistant_check_install(w.furniture.as_ref())?;
        if self.studio.project() != &w.project {
            self.studio.install_prepared(project).map_err(|e| e.to_string())?;
        }
        let furniture_changed = self.furniture.assistant_document() != w.furniture;
        self.furniture.assistant_install(w.furniture.clone());
        if furniture_changed && w.furniture.is_some() {
            self.furniture.open = true;
            self.show_script = false;
            self.machines.open = false;
        }
        self.script_code = w.script.clone();
        self.selection.retain(|id| self.studio.project().shape(*id).is_some());
        self.op_draft = None;
        self.tool_draft = None;
        self.fit_pending = true;
        Ok(())
    }
    fn assistant_restore(&mut self, redo: bool) -> Result<(), String> {
        let mut lot = self.assistant.undo.clone().ok_or("Aucun lot à annuler")?;
        let current = self.assistant_workspace();
        let expected = if redo { &lot.before } else { &lot.after };
        if current.fingerprint() != expected.fingerprint() {
            return Err("Le document a changé depuis ce lot : utilisez son historique natif".into());
        }
        self.assistant_install(
            if redo { &lot.after } else { &lot.before },
            if redo { lot.after_project.clone() } else { lot.before_project.clone() },
        )?;
        let installed = self.assistant_workspace();
        if redo {
            lot.after = installed;
        } else {
            lot.before = installed;
        }
        self.assistant.undo = Some(lot);
        self.assistant.redone = !redo;
        self.assistant.prepared = None;
        self.assistant.message = if redo { "Lot rétabli" } else { "Lot annulé" }.into();
        Ok(())
    }
    fn assistant_export(&mut self, format: assistant::plan::ExportFormat) {
        use assistant::plan::ExportFormat as F;
        match format {
            F::Cb => self.save_as(None),
            F::Svg | F::Dxf => {
                let ext = if format == F::Svg { "svg" } else { "dxf" };
                self.save_dialog(crate::Purpose::ExportDrawing(ext), "Exporter le dessin", format!("{}.{ext}", self.stem()));
            }
            F::Gcode => self.save_dialog(crate::Purpose::ExportGcode, "Exporter le G-code", format!("{}.nc", self.stem())),
            F::FurnitureJson => self.furniture.export_native(),
            F::FurnitureZip => self.furniture.assistant_export_zip(),
        }
    }
}
