//! `Studio` = éditeur + trajectoires calculées. Chaque modification recalcule les opérations ;
//! un recalcul en échec garde le dernier résultat valide et mémorise l'erreur.

use crate::history::Editor;
use crate::project::{LeadChoice, OpKind, OpSide, Operation, Project, TabShape, TabTarget};
use crate::shape::{self, EditError};
use fablabcam_cam::{drill, emit, render_post, Mop, PostDef, PostOutput, Program, ToolInfo, pocket_shapes, profile, CamError, CutOrdering, Lead, LeadKind, MillingDirection, Params, Post, Side, TabMethod, TabStyle, Tabs, Toolpath};
use fablabcam_geom::Pline;
use fablabcam_model::Entity;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct Computed {
    pub path: Option<Toolpath>,
    pub params: Option<Params>,
    /// Dernière erreur de calcul (le `path` éventuel date d'un calcul antérieur).
    pub error: Option<String>,
}

/// Paramètres convertis pour un G-code dans une autre unité (diamètre et hauteurs pour les commentaires et cycles).
fn scale_params(p: &Params, k: f64) -> Params {
    Params {
        tool_diameter: p.tool_diameter * k,
        stock_surface: p.stock_surface * k,
        target_depth: p.target_depth * k,
        clearance_plane: p.clearance_plane * k,
        cut_feedrate: p.cut_feedrate * k,
        plunge_feedrate: p.plunge_feedrate * k,
        ..p.clone()
    }
}

/// Ajoute un script utilisateur ligne par ligne (lignes vides ignorées).
fn push_script(out: &mut String, script: &str) {
    for l in script.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()) {
        *out += l;
        out.push('\n');
    }
}

fn cam_error(e: CamError) -> String {
    match e {
        CamError::OpenContour => "contour ouvert : une forme fermée est requise".into(),
        CamError::NoDepth => "la profondeur doit être sous la surface du brut".into(),
        CamError::ToolTooLarge => "l'outil est trop gros pour cette forme".into(),
        CamError::EmptyInput => "aucune forme exploitable".into(),
        CamError::LeadDoesNotFit => "le rayon d'approche ne tient pas dans la zone dégagée".into(),
        CamError::ClearanceTooNegative => e.to_string(),
    }
}

/// Paramètres CAM d'une opération : valeurs de l'outil, surchargées par celles de l'opération.
pub fn op_params(p: &Project, op: &Operation) -> Result<Params, String> {
    let tool = p.tool(&op.tool).ok_or_else(|| format!("outil « {} » introuvable", op.tool))?;
    let c = &op.cam;
    if [c.clearance, c.surface, op.depth].iter().any(|v| !v.is_finite()) {
        return Err("valeur non numérique".into());
    }
    // égale à la surface : une passe à la surface (comme le logiciel de référence)
    if op.depth > c.surface {
        return Err(format!("la profondeur doit être sous la surface du brut (Z {}) ou à son niveau", c.surface));
    }
    if c.clearance <= c.surface {
        return Err("le plan de dégagement doit être au-dessus de la surface du brut".into());
    }
    if op.kind != OpKind::Drill {
        if op.step_down.is_nan() || op.step_down <= 0.0 {
            return Err("l'incrément de passe doit être > 0".into());
        }
        if op.step_over.is_nan() || op.step_over <= 0.0 || op.step_over > 1.0 {
            return Err("le recouvrement doit être dans ]0 ; 1]".into());
        }
        if !(c.finish_stepover >= 0.0 && c.final_increment >= 0.0) {
            return Err("finition et incrément final doivent être ≥ 0".into());
        }
        if !(0.0..=1.0).contains(&c.max_crossover) {
            return Err("la transition maximale doit être dans [0 ; 1] (fraction du diamètre)".into());
        }
        if !(0.0..90.0).contains(&c.lead_angle) || c.lead_radius.is_nan() || c.lead_radius < 0.0 {
            return Err("angle de rampe dans [0 ; 90[ et rayon d'entrée ≥ 0".into());
        }
    }
    let tabs_on = op.tabs && op.kind == OpKind::Profile;
    if tabs_on {
        if !(c.tab_width > 0.0 && c.tab_height > 0.0 && c.tab_distance >= 0.0 && c.tab_threshold >= 0.0) {
            return Err("ponts : largeur et hauteur > 0, distance et seuil ≥ 0".into());
        }
        if c.tab_min > c.tab_max {
            return Err("ponts : le nombre minimal dépasse le maximal".into());
        }
    }
    let radius = if c.lead_radius > 0.0 { c.lead_radius } else { tool.diameter };
    let lead = |k: LeadChoice| Lead {
        kind: match k {
            LeadChoice::None => LeadKind::None,
            LeadChoice::Spiral => LeadKind::Spiral,
            LeadChoice::Tangent => LeadKind::Tangent,
        },
        tangent_radius: radius,
        spiral_angle: c.lead_angle,
        ..Lead::default()
    };
    let style = match c.tab_shape {
        TabShape::Triangle => TabStyle::Triangle,
        TabShape::Square => TabStyle::Square,
        TabShape::Skip => TabStyle::Skip,
    };
    let base = Tabs { width: c.tab_width, height: c.tab_height, min: c.tab_min as usize, max: c.tab_max as usize, distance: c.tab_distance, size_threshold: c.tab_threshold, style, ..Tabs::default() };
    Ok(Params {
        tool_diameter: tool.diameter,
        stock_surface: c.surface,
        clearance_plane: c.clearance,
        target_depth: op.depth,
        depth_increment: op.step_down,
        step_over: op.step_over,
        drill_dwell: if op.dwell.is_finite() { op.dwell.max(0.0) } else { 0.0 },
        roughing_clearance: if matches!(op.kind, OpKind::Drill | OpKind::Engrave) || !op.stock_to_leave.is_finite() { 0.0 } else { op.stock_to_leave },
        cut_feedrate: op.feed.unwrap_or(tool.feed),
        plunge_feedrate: op.plunge.unwrap_or(tool.plunge),
        spindle_speed: op.spindle.unwrap_or(tool.spindle),
        spindle_cw: !c.spindle_ccw,
        milling: if op.climb { MillingDirection::Climb } else { MillingDirection::Conventional },
        cut_ordering: if c.level_first { CutOrdering::LevelFirst } else { CutOrdering::DepthFirst },
        corner_overcut: c.corner_overcut,
        finish_stepover: c.finish_stepover,
        finish_at_target_only: c.finish_at_target_only,
        final_depth_increment: c.final_increment,
        max_crossover: c.max_crossover,
        // manuel : méthode choisie, ou points posés (anciens documents) ; sans point, aucun pont
        tabs: match (tabs_on, c.tab_target == TabTarget::Manual || !op.tab_points.is_empty()) {
            (false, _) => Tabs::default(),
            (true, true) => Tabs { method: TabMethod::Manual, positions: op.tab_points.iter().map(|t| (t[0], t[1])).collect(), ..base },
            (true, false) => Tabs {
                method: match c.tab_target {
                    TabTarget::Outer => TabMethod::AutomaticOuter,
                    TabTarget::Inner => TabMethod::AutomaticInner,
                    _ => TabMethod::Automatic,
                },
                ..base
            },
        },
        lead_in: lead(op.lead),
        lead_out: lead(op.lead),
        ..Params::default()
    })
}

/// Calcule la trajectoire d'une opération.
pub fn compute_op(p: &Project, op: &Operation) -> Result<(Toolpath, Params), String> {
    use crate::diagnostics;
    use serde_json::json;
    let started = std::time::Instant::now();
    diagnostics::record(true, "operation_start", json!({"operation_id": op.id, "kind": op.kind.label(), "shape_count": op.shapes.len()}));
    // The kernel only receives immutable input and builds temporary output.
    // Contain an unexpected upstream unwind before it reaches winit's locked
    // callback; discard every partial path. The panic hook still records it.
    let result = std::panic::catch_unwind(|| compute_op_inner(p, op))
        .unwrap_or_else(|_| Err(format!("Opération #{} : erreur interne du calcul géométrique ; consultez le rapport de diagnostic", op.id)));
    diagnostics::record(result.is_ok(), "operation_end", json!({
        "operation_id": op.id, "success": result.is_ok(), "elapsed_ms": started.elapsed().as_millis(),
        "moves": result.as_ref().map(|(path, _)| path.moves.len()).ok(),
        "error": result.as_ref().err(),
    }));
    result
}

fn compute_op_inner(p: &Project, op: &Operation) -> Result<(Toolpath, Params), String> {
    let params = op_params(p, op)?;
    let ents: Vec<&Entity> = op.shapes.iter().filter_map(|i| p.shape(*i)).map(|s| &s.entity).collect();
    if ents.is_empty() {
        return Err("aucune forme rattachée à l'opération".into());
    }
    for id in &op.shapes {
        if let Some(contour) = p.shape(*id).and_then(|s| shape::pline(&s.entity)) {
            use fablabcam_geom::PlineSource;
            if contour.iter_vertexes().any(|v| !v.x.is_finite() || !v.y.is_finite() || !v.bulge.is_finite()) {
                return Err(format!("Forme #{id} : coordonnées non finies, calcul refusé"));
            }
            crate::diagnostics::record(true, "contour_input", serde_json::json!({
                "operation_id": op.id, "shape_id": id, "vertices": contour.vertex_count(),
                "closed": contour.is_closed(), "side": format!("{:?}", op.side),
                "tool_diameter_mm": params.tool_diameter, "wall_offset_mm": params.wall_offset(),
            }));
        }
    }
    let plines = || -> Vec<Pline> { ents.iter().filter_map(|e| shape::pline(e)).collect() };
    let path = match op.kind {
        OpKind::Profile => {
            let side = match op.side { OpSide::Outside => Side::Outside, OpSide::Inside => Side::Inside, OpSide::On => Side::On };
            let mut all = Toolpath::default();
            for s in plines() {
                all.moves.extend(profile(&s, side, &params).map_err(cam_error)?.moves);
            }
            all
        }
        OpKind::Pocket => pocket_shapes(&plines(), &params).map_err(cam_error)?,
        OpKind::Engrave => {
            let mut all = Toolpath::default();
            for s in plines() {
                all.moves.extend(profile(&s, Side::On, &params).map_err(cam_error)?.moves);
            }
            all
        }
        OpKind::Drill => {
            let pts: Vec<(f64, f64)> = ents
                .iter()
                .filter_map(|e| match e {
                    Entity::Circle { cx, cy, .. } => Some((*cx, *cy)),
                    Entity::Point { x, y } => Some((*x, *y)),
                    _ => None,
                })
                .collect();
            if pts.is_empty() {
                return Err("le perçage demande des cercles ou des points".into());
            }
            drill(&pts, &params).map_err(cam_error)?
        }
    };
    Ok((path, params))
}

#[derive(Debug, Clone, Default)]
pub struct Studio {
    pub editor: Editor,
    pub computed: BTreeMap<u32, Computed>,
    /// Incrémenté à chaque changement (pour invalider les caches d'affichage).
    pub rev: u64,
}

/// A coherent project/cache pair, calculated off the render thread before installation.
#[derive(Debug, Clone)]
pub struct PreparedProject {
    project: Project,
    computed: BTreeMap<u32, Computed>,
}
impl PreparedProject {
    pub fn new(project: Project) -> Self {
        let studio=Studio::new(project);
        Self { project:studio.editor.project, computed:studio.computed }
    }
    pub fn diagnostics(&self)->Vec<String> {
        self.computed.iter().filter_map(|(id,c)|c.error.as_ref().map(|e|format!("Opération {id} : {e}"))).collect()
    }
}

impl Studio {
    /// One history entry; reuse the cache prepared for this exact project.
    pub fn install_prepared(&mut self, prepared: PreparedProject)->Result<(),EditError> {
        self.editor.edit(|p|{*p=prepared.project;Ok(())})?;
        self.computed=prepared.computed;
        self.rev+=1;
        Ok(())
    }
    pub fn new(project: Project) -> Self {
        let mut s = Studio { editor: Editor::new(project), computed: BTreeMap::new(), rev: 0 };
        s.recalc();
        s
    }

    pub fn project(&self) -> &Project {
        &self.editor.project
    }

    /// Cached previews may show the previous valid path after a failed edit;
    /// never treat that cache as a valid new export.
    pub fn check_computed(&self) -> Result<(), String> {
        for op in self.project().ops.iter().filter(|op| op.enabled) {
            match self.computed.get(&op.id) {
                Some(Computed { error: Some(error), .. }) => return Err(format!("Opération #{} : {error}", op.id)),
                Some(Computed { path: Some(_), .. }) => {},
                _ => return Err(format!("Opération #{} : trajectoire absente", op.id)),
            }
        }
        Ok(())
    }

    /// Modification annulable suivie d'un recalcul.
    pub fn edit<R>(&mut self, f: impl FnOnce(&mut Project) -> Result<R, EditError>) -> Result<R, EditError> {
        let r = self.editor.edit(f)?;
        self.recalc();
        Ok(r)
    }

    pub fn undo(&mut self) -> bool {
        let ok = self.editor.undo();
        self.recalc();
        ok
    }

    pub fn redo(&mut self) -> bool {
        let ok = self.editor.redo();
        self.recalc();
        ok
    }

    pub fn recalc(&mut self) {
        self.rev += 1;
        let project = &self.editor.project;
        let ids: Vec<u32> = project.ops.iter().map(|o| o.id).collect();
        self.computed.retain(|k, _| ids.contains(k));
        for op in &project.ops {
            let c = self.computed.entry(op.id).or_default();
            match compute_op(project, op) {
                Ok((path, params)) => *c = Computed { path: Some(path), params: Some(params), error: None },
                Err(e) => c.error = Some(e),
            }
        }
    }

    /// Trajet reconstruit à partir du G-code complet (ce que la machine exécutera).
    pub fn backplot(&self) -> fablabcam_backplot::Backplot {
        fablabcam_backplot::parse_gcode(&self.gcode())
    }

    /// G-code produit par un post-processeur configurable (`.cbpp`), en-têtes/pieds d'opération inclus.
    pub fn gcode_with_post(&self, def: &PostDef) -> PostOutput {
        let p = self.project();
        // en pouces : trajectoires et paramètres convertis avant le rendu
        let k = if p.gcode_inches { 1.0 / 25.4 } else { 1.0 };
        let conv: Vec<(&Operation, Toolpath, Params)> = p
            .ops
            .iter()
            .filter(|o| o.enabled)
            .filter_map(|op| match self.computed.get(&op.id) {
                Some(Computed { path: Some(path), params: Some(params), .. }) => Some((op, fablabcam_cam::scaled(path, k), scale_params(params, k))),
                _ => None,
            })
            .collect();
        let mut mops = Vec::new();
        for (op, path, params) in &conv {
            let tool = p.tool(&op.tool);
            let index = tool.map_or(0, |t| t.index as usize);
            mops.push(Mop {
                name: op.name.clone(),
                header: op.header.clone(),
                footer: op.footer.clone(),
                tool: ToolInfo { index, name: op.tool.clone(), diameter: params.tool_diameter, comment: tool.map(|t| t.comment.clone()).unwrap_or_default(), change: tool.map(|t| t.tool_change.clone()).unwrap_or_default() },
                path,
                params,
            });
        }
        // en pouces, 3 décimales ne donnent que 0,025 mm : 4 si la définition garde le format par défaut
        let inch_def;
        let def = if p.gcode_inches && def.number_format == PostDef::default().number_format {
            inch_def = PostDef { number_format: "0.####".into(), ..def.clone() };
            &inch_def
        } else {
            def
        };
        render_post(def, &Program { name: "FabLabCAM".into(), date: String::new(), file_header: String::new(), file_footer: String::new(), inches: p.gcode_inches, mops })
    }

    /// G-code des opérations actives, dans l'ordre de la liste, avec changements d'outil.
    pub fn gcode(&self) -> String {
        let p = self.project();
        let k = if p.gcode_inches { 1.0 / 25.4 } else { 1.0 };
        let mut out = format!("(FabLabCAM studio)\n{}\n", fablabcam_cam::gcode::milling_preamble(p.gcode_inches));
        let mut current: Option<&str> = None;
        let mut current_part: Option<&str> = None;
        let mut any = false;
        for op in p.ops.iter().filter(|o| o.enabled) {
            let Some(Computed { path: Some(path), params: Some(params), .. }) = self.computed.get(&op.id) else { continue };
            let (path, params) = (&fablabcam_cam::scaled(path, k), &scale_params(params, k));
            if current != Some(op.tool.as_str()) {
                let n = p.tool_number(&op.tool);
                if current.is_some() {
                    out += "M5\n";
                }
                out += &format!("(Changement d'outil : {} D{})\n", op.tool.replace(['(', ')'], ""), params.tool_diameter);
                match p.tool(&op.tool).map(|t| t.tool_change.as_str()).filter(|c| !c.trim().is_empty()) {
                    // texte propre à l'outil : remplace l'appel M6 par défaut
                    Some(custom) => push_script(&mut out, custom),
                    None => out += &format!("M6 T{n}\n"),
                }
                current = Some(&op.tool);
            }
            if !op.part.is_empty() && current_part != Some(op.part.as_str()) {
                out += &format!("(Pièce : {})\n", op.part.replace(['(', ')'], ""));
                current_part = Some(&op.part);
            }
            out += &format!("(Operation : {})\n", op.name.replace(['(', ')'], ""));
            push_script(&mut out, &op.header);
            let body = emit(path, params, &Post { program_name: op.name.replace(['(', ')'], ""), decimals: if p.gcode_inches { 4 } else { 3 }, ..Post::default() });
            for l in body.lines().skip(2).filter(|l| *l != "M30" && *l != "M5") {
                out += l;
                out.push('\n');
            }
            push_script(&mut out, &op.footer);
            any = true;
        }
        if any {
            out += "M5\nM30\n";
        } else {
            out += "M30\n";
        }
        out
    }
}

/// Projet d'exemple : plaque 80×60 avec un îlot circulaire, quatre perçages et deux outils.
pub fn demo_project() -> Project {
    use crate::project::{Tool, ToolKind};
    let mut p = Project::new();
    let plate = p.add_shape("Contour", shape::rectangle((0.0, 0.0), (80.0, 60.0)).unwrap()).unwrap();
    let pocket = p.add_shape("Poche", shape::rectangle((10.0, 10.0), (70.0, 50.0)).unwrap()).unwrap();
    let island = p.add_shape("Poche", shape::circle((40.0, 30.0), 8.0).unwrap()).unwrap();
    let mut holes = Vec::new();
    for (x, y) in [(5.0, 5.0), (75.0, 5.0), (75.0, 55.0), (5.0, 55.0)] {
        holes.push(p.add_shape("Trous", shape::circle((x, y), 1.5).unwrap()).unwrap());
    }
    p.add_tool(Tool::new("Fraise 6 mm", ToolKind::EndMill, 6.0)).unwrap();
    p.add_tool(Tool::new("Foret 3 mm", ToolKind::Drill, 3.0)).unwrap();
    let a = p.add_operation(OpKind::Pocket, "Poche centrale", vec![pocket, island], "Fraise 6 mm").unwrap();
    p.update_operation(a, |o| { o.depth = -2.0; o.step_down = 1.0; }).unwrap();
    let b = p.add_operation(OpKind::Drill, "Perçages", holes, "Foret 3 mm").unwrap();
    p.update_operation(b, |o| o.depth = -4.0).unwrap();
    let c = p.add_operation(OpKind::Profile, "Découpe", vec![plate], "Fraise 6 mm").unwrap();
    p.update_operation(c, |o| { o.depth = -4.0; o.step_down = 2.0; o.tabs = true; }).unwrap();
    p
}

/// Dessin d'exemple : un de chaque type de forme.
pub fn drawing_demo() -> Project {
    let mut p = Project::new();
    let add = |p: &mut Project, l: &str, e: Result<fablabcam_model::Entity, EditError>| p.add_shape(l, e.unwrap()).unwrap();
    add(&mut p, "Contour", shape::rectangle((0.0, 0.0), (80.0, 60.0)));
    add(&mut p, "Contour", shape::polyline(&[(10.0, 10.0), (35.0, 10.0), (45.0, 25.0), (30.0, 45.0), (10.0, 40.0)], true));
    add(&mut p, "Poche", shape::circle((58.0, 30.0), 12.0));
    add(&mut p, "Poche", shape::arc((58.0, 30.0), 18.0, 200.0, 140.0));
    add(&mut p, "Cotes", shape::line((0.0, -8.0), (80.0, -8.0)));
    add(&mut p, "Trous", shape::circle((5.0, 5.0), 1.5));
    add(&mut p, "Trous", shape::circle((75.0, 55.0), 1.5));
    p
}
