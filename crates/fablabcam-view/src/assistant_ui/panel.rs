use super::*;
use assistant::{Mode, Provider};

impl AssistantUi {
    pub(super) fn show(&mut self, ctx: &egui::Context, page: &str, revision: u64) -> Option<UiAction> {
        if !self.open {
            return None;
        }
        let mut action = None;
        egui::SidePanel::right("assistant-global").default_width(380.0).width_range(300.0..=640.0).resizable(true).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Assistant");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("×").on_hover_text("Replier l'assistant").clicked() {
                        self.open = false;
                    }
                    self.settings |= ui.small_button("Connexion").clicked();
                });
            });
            ui.add_enabled_ui(self.job.is_none(), |ui| {
                ui.horizontal(|ui| {
                    for mode in [Mode::Ask, Mode::Plan, Mode::Agent] {
                        if ui.selectable_value(&mut self.session.mode, mode, format!("{mode:?}")).changed() {
                            self.prepared = None;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_source("assistant-provider").width(95.0).selected_text(self.session.provider.label()).show_ui(
                        ui,
                        |ui| {
                            for p in Provider::ALL {
                                if ui.selectable_value(&mut self.session.provider, p, p.label()).changed() {
                                    self.prepared = None;
                                    self.plan = None;
                                    self.last_reply = None;
                                }
                            }
                        },
                    );
                    let c = self.session.connection();
                    ui.small(if c.model.is_empty() { "Modèle à configurer" } else { &c.model });
                });
            });
            ui.small(match self.session.mode {
                Mode::Ask => "Comprendre · lecture seule",
                Mode::Plan => "Préparer les étapes · sans modifier",
                Mode::Agent => "Agir après vérification de l'aperçu",
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Contexte · {page} · rév. {revision}"));
                if self.pinned.is_some() {
                    if ui.small_button("Désépingler").clicked() {
                        action = Some(UiAction::Unpin);
                    }
                } else if ui.small_button("Épingler").clicked() {
                    action = Some(UiAction::Pin);
                }
            });
            if let Some(c) = &self.pinned {
                ui.small(format!("Épinglé : {} · rév. {}", c.page, c.revision));
            }
            egui::CollapsingHeader::new("Éléments transmis").show(ui, |ui| {
                let o = &mut self.session.options;
                ui.checkbox(&mut o.drawing, "Dessin");
                ui.checkbox(&mut o.selected_only, "Seulement la sélection");
                ui.checkbox(&mut o.script, "Texte du script");
                ui.checkbox(&mut o.furniture, "Meuble ouvert");
                ui.checkbox(&mut o.tools, "Outillage, opérations et profil");
                ui.checkbox(&mut o.diagnostics, "Diagnostics et simulation");
                ui.checkbox(&mut o.history, "20 derniers échanges");
                if let Some(c) = self.pinned.as_ref().or(self.context.as_ref()) {
                    egui::CollapsingHeader::new("Dernier contexte envoyé").show(ui, |ui| {
                        egui::ScrollArea::vertical().id_source("context-json").max_height(160.0).show(ui, |ui| {
                            ui.monospace(serde_json::to_string_pretty(&c.data).unwrap_or_default());
                        });
                    });
                }
            });
            let height = (ui.available_height() - 155.0).max(120.0);
            egui::ScrollArea::vertical()
                .id_source("assistant-conversation")
                .max_height(height)
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if self.session.entries.is_empty() {
                        ui.heading("Un coup de main dans l'atelier");
                        ui.label("Expliquez ce que vous souhaitez faire. Le contexte choisi accompagne votre demande.");
                        for (label, prompt) in [
                            ("Comprendre la sélection", "Explique les dimensions et les propriétés des objets sélectionnés."),
                            (
                                "Préparer un meuble",
                                "Prépare un caisson de 600 × 400 × 800 mm en panneaux de 18 mm, avec deux tablettes et un dos.",
                            ),
                            ("Relire le script", "Explique ce script et les effets d'une deuxième exécution."),
                        ] {
                            if ui.button(label).clicked() {
                                self.prompt = prompt.into();
                            }
                        }
                    }
                    for (index, entry) in self.session.entries.iter().enumerate() {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            super::message::actions(ui, index, entry.user, &entry.text, &mut self.prompt);
                            if entry.user {
                                ui.label(&entry.text);
                            } else {
                                egui_commonmark::CommonMarkViewer::new(("assistant-message", index)).explicit_image_uri_scheme(true).show(
                                    ui,
                                    &mut self.markdown,
                                    &entry.text,
                                );
                            }
                            ui.small(format!(
                                "{} · rév. {} · {}",
                                entry.page,
                                entry.revision,
                                entry.provider.split('|').next().unwrap_or("")
                            ));
                        });
                        ui.add_space(6.0);
                    }
                    if !self.partial.is_empty() {
                        ui.label(&self.partial);
                    }
                    ui.add_enabled_ui(self.job.is_none(), |ui| {
                        if let Some(plan) = &mut self.plan {
                            ui.separator();
                            ui.heading("Proposition");
                            let before = serde_json::to_string(plan).unwrap_or_default();
                            ui.text_edit_singleline(&mut plan.title);
                            for assumption in &plan.assumptions {
                                ui.small(format!("Hypothèse · {assumption}"));
                            }
                            plan.missing.retain_mut(|missing| {
                                let mut resolved = false;
                                ui.colored_label(egui::Color32::YELLOW, "À préciser avant préparation");
                                ui.add(egui::TextEdit::multiline(missing).desired_rows(2).desired_width(f32::INFINITY));
                                if ui.button("J'ai renseigné ce point dans les paramètres").clicked() {
                                    resolved = true;
                                }
                                !resolved
                            });
                            let mut remove_step = None;
                            for (i, step) in plan.steps.iter_mut().enumerate() {
                                egui::CollapsingHeader::new(format!("{}. {}", i + 1, step.action.label()))
                                    .id_source(("plan-step", i))
                                    .show(ui, |ui| {
                                        ui.add(
                                            egui::TextEdit::multiline(&mut step.explanation).desired_rows(2).desired_width(f32::INFINITY),
                                        );
                                        if ui.small_button("Retirer cette étape").clicked() {
                                            remove_step = Some(i);
                                        }
                                        let mut value = serde_json::to_value(&step.action).unwrap_or_default();
                                        edit_parameters(ui, &mut value);
                                        if let Ok(a) = serde_json::from_value(value) {
                                            step.action = a;
                                        }
                                    });
                            }
                            if let Some(i) = remove_step {
                                plan.steps.remove(i);
                            }
                            if before != serde_json::to_string(plan).unwrap_or_default() {
                                self.prepared = None;
                            }
                            if ui.add_enabled(self.job.is_none(), egui::Button::new("Préparer l'aperçu")).clicked() {
                                action = Some(UiAction::Prepare);
                            }
                        }
                    });
                    if let Some(p) = &self.prepared {
                        ui.separator();
                        ui.strong("Changements préparés");
                        for (i, result) in p.results.iter().enumerate() {
                            ui.label(format!("{}. {result}", i + 1));
                        }
                        if p.before.script != p.after.script {
                            egui::CollapsingHeader::new("Script · avant / après").show(ui, |ui| {
                                ui.strong("Avant");
                                ui.monospace(&p.before.script);
                                ui.strong("Après");
                                ui.monospace(&p.after.script);
                            });
                        }
                        ui.small("Le projet n'a pas encore été modifié.");
                        if ui
                            .add_enabled(
                                self.job.is_none(),
                                egui::Button::new(if self.session.mode == Mode::Agent {
                                    "Appliquer ce lot"
                                } else {
                                    "Passer en Agent et appliquer"
                                }),
                            )
                            .clicked()
                        {
                            action = Some(UiAction::Apply);
                        }
                    }
                    if self.undo.is_some() {
                        ui.horizontal(|ui| {
                            if ui.add_enabled(!self.redone && self.job.is_none(), egui::Button::new("Annuler le lot")).clicked() {
                                action = Some(UiAction::Undo);
                            }
                            if ui.add_enabled(self.redone && self.job.is_none(), egui::Button::new("Rétablir le lot")).clicked() {
                                action = Some(UiAction::Redo);
                            }
                        });
                    }
                });
            ui.separator();
            if self.session.debug {
                ui.small(format!("Requête {} · {} · {} tour(s)", self.generation, self.session.connection().model, self.turns));
                if let Some(c) = &self.context {
                    ui.small(format!("Contexte {}", &c.token[..c.token.len().min(12)]));
                }
            }
            if self.job.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(&self.message);
                    if ui.button("Arrêter").clicked() {
                        action = Some(UiAction::Stop);
                    }
                });
            } else {
                ui.small(&self.message);
            }
            egui::ScrollArea::vertical().id_source("assistant-input").max_height(72.0).show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.prompt)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .hint_text("Demander à l'assistant…"),
                );
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(self.job.is_none() && !self.prompt.trim().is_empty(), egui::Button::new("Envoyer")).clicked() {
                    action = Some(UiAction::Send);
                }
                if ui.add_enabled(self.job.is_none(), egui::Button::new("Effacer la conversation")).clicked() {
                    action = Some(UiAction::Clear);
                }
            });
        });
        if let Some(a) = self.settings(ctx) {
            action = Some(a);
        }
        action
    }
}

fn edit_parameters(ui: &mut egui::Ui, value: &mut serde_json::Value) {
    let Some(fields) = value.as_object_mut() else {
        return;
    };
    for (key, v) in fields {
        if key == "type" {
            ui.small(v.as_str().unwrap_or(""));
            continue;
        }
        ui.label(key.replace('_', " "));
        match v {
            serde_json::Value::String(s) => {
                if key == "code" {
                    ui.add(egui::TextEdit::multiline(s).code_editor().desired_rows(6).desired_width(f32::INFINITY));
                } else {
                    ui.text_edit_singleline(s);
                }
            }
            serde_json::Value::Bool(b) => {
                ui.checkbox(b, "Oui");
            }
            serde_json::Value::Number(n) => {
                let mut x = n.as_f64().unwrap_or_default();
                if ui.add(egui::DragValue::new(&mut x).speed(0.1)).changed() {
                    *v = if n.is_u64() { serde_json::json!(x.max(0.0) as u64) } else { serde_json::json!(x) };
                }
            }
            serde_json::Value::Object(_) => {
                ui.indent(key, |ui| edit_parameters(ui, v));
            }
            _ => {
                ui.monospace(v.to_string());
            }
        }
    }
}
