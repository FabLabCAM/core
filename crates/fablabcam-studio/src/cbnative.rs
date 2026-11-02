//! Import des usinages d'un vrai `.cb` de format XML 0.9.8 : `<parts>` → `<part Name>` → `<machineops>` →
//! `<profile>`, `<pocket>`, `<drill>`, `<engrave>`. Chaque réglage est un élément enfant
//! (`<TargetDepth state="Value">-18</TargetDepth>`) ; la valeur est lue quel que soit `state`, car le logiciel de référence
//! écrit aussi les valeurs par défaut. Les formes sont désignées par leur `id` (`<primitive><prim>2</prim>`).
//!
//! Observé sur de vrais fichiers : profil (côté, sens, passes, ponts automatiques, ponts enregistrés
//! `<HoldingTab>`). Lu d'après les noms des réglages, sans fichier réel pour le confirmer : poche,
//! perçage, gravure, entrées (`LeadInMove`). Le reste (3D, tour, styles nommés) est signalé et ignoré.

use crate::project::*;
use fablabcam_geom::PlineSource;
use std::collections::HashMap;

fn text<'a>(n: roxmltree::Node<'a, '_>, name: &str) -> Option<&'a str> {
    n.children().find(|c| c.is_element() && c.tag_name().name() == name).and_then(|c| c.text()).map(str::trim)
}

fn num(n: roxmltree::Node, name: &str) -> Option<f64> {
    text(n, name).and_then(|t| t.parse::<f64>().ok()).filter(|v| v.is_finite())
}

fn yes(n: roxmltree::Node, name: &str) -> Option<bool> {
    text(n, name).map(|t| t.eq_ignore_ascii_case("true"))
}

fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| c.is_element() && c.tag_name().name() == name)
}

fn tool_kind(profile: &str, op: OpKind) -> ToolKind {
    match profile.to_ascii_lowercase().as_str() {
        "ballnose" => ToolKind::BallNose,
        "bullnose" => ToolKind::BullNose,
        "vcutter" | "vbit" | "engrave" => ToolKind::VBit,
        "drill" => ToolKind::Drill,
        _ if op == OpKind::Drill => ToolKind::Drill,
        _ => ToolKind::EndMill,
    }
}

/// Outil de la table correspondant au diamètre / profil / numéro de l'opération (créé au besoin).
fn tool_for(p: &mut Project, kind: ToolKind, diameter: f64, number: u32, feeds: (f64, f64, f64)) -> Result<String, String> {
    if let Some(t) = p.tools.iter().find(|t| t.kind == kind && (t.diameter - diameter).abs() < 1e-9 && (number == 0 || t.index == number)) {
        return Ok(t.name.clone());
    }
    let base = if number > 0 { format!("T{number} {} Ø{diameter}", kind.label()) } else { format!("{} Ø{diameter}", kind.label()) };
    let name = (1..).map(|i| if i == 1 { base.clone() } else { format!("{base} ({i})") }).find(|n| p.tool(n).is_none()).unwrap_or(base);
    let free = number > 0 && !p.tools.iter().any(|t| t.index == number);
    let tool = Tool { index: if free { number } else { 0 }, feed: feeds.0, plunge: feeds.1, spindle: feeds.2, ..Tool::new(&name, kind, diameter) };
    p.add_tool(tool).map_err(|e| e.to_string())?;
    Ok(name)
}

/// Point d'une forme à la fraction `t` ∈ [0 ; 1] de sa longueur (ponts `<HoldingTab>` du logiciel de référence :
/// `ParametricPoint` le long de l'entité `ParentEntityID` ; repère supposé proportionnel à la longueur).
fn point_at(p: &Project, shape: u32, t: f64) -> Option<[f64; 2]> {
    let pl = p.shape(shape)?.entity.to_pline()?;
    let n = pl.vertex_count();
    let segs = if pl.is_closed() { n } else { n.saturating_sub(1) };
    let pts: Vec<[f64; 2]> = (0..=segs).map(|i| { let v = pl.at(i % n); [v.x, v.y] }).collect();
    let len: Vec<f64> = pts.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt()).collect();
    let total: f64 = len.iter().sum();
    if total <= 0.0 {
        return pts.first().copied();
    }
    let mut left = t.clamp(0.0, 1.0) * total;
    for (i, l) in len.iter().enumerate() {
        if left <= *l || i + 1 == len.len() {
            let k = if *l > 0.0 { (left / l).min(1.0) } else { 0.0 };
            return Some([pts[i][0] + k * (pts[i + 1][0] - pts[i][0]), pts[i][1] + k * (pts[i + 1][1] - pts[i][1])]);
        }
        left -= l;
    }
    None
}

/// Ajoute au projet les usinages d'un projet `.cb` importé. `ids` : identifiant XML → formes du projet
/// (plusieurs pour une liste de points).
/// Renvoie les remarques (réglages ignorés, formes introuvables…).
pub fn import(p: &mut Project, root: roxmltree::Node, ids: &HashMap<u32, Vec<u32>>) -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    let Some(parts) = child(root, "parts") else { return Ok(notes) };
    for part in parts.children().filter(|c| c.is_element() && c.tag_name().name() == "part") {
        let part_name = part.attribute("Name").unwrap_or("").to_string();
        let part_on = part.attribute("Enabled").map_or(true, |v| !v.eq_ignore_ascii_case("false"));
        let Some(mops) = child(part, "machineops") else { continue };
        for m in mops.children().filter(|c| c.is_element()) {
            let tag = m.tag_name().name();
            let kind = match tag {
                "profile" => OpKind::Profile,
                "pocket" => OpKind::Pocket,
                "drill" => OpKind::Drill,
                "engrave" => OpKind::Engrave,
                other => {
                    notes.push(format!("opération « {other} » non prise en charge, ignorée"));
                    continue;
                }
            };
            let name = text(m, "Name").filter(|s| !s.is_empty()).unwrap_or(tag).to_string();
            let name = (1..).map(|i| if i == 1 { name.clone() } else { format!("{name} ({i})") }).find(|n| !p.ops.iter().any(|o| &o.name == n)).unwrap_or(name);
            let mut shapes = Vec::new();
            if let Some(prims) = child(m, "primitive") {
                for id in prims.children().filter(|c| c.is_element()).filter_map(|c| c.text()?.trim().parse::<u32>().ok()) {
                    match ids.get(&id) {
                        Some(s) => shapes.extend(s),
                        None => notes.push(format!("« {name} » : forme {id} introuvable")),
                    }
                }
            }
            let feed = num(m, "CutFeedrate").unwrap_or(800.0);
            let plunge = num(m, "PlungeFeedrate").unwrap_or(300.0);
            let spindle = num(m, "SpindleSpeed").unwrap_or(1000.0);
            let mut diameter = num(m, "ToolDiameter").unwrap_or(0.0);
            if diameter <= 0.0 {
                diameter = 3.0;
                notes.push(format!("« {name} » : diamètre d'outil non défini (style), 3 mm pris par défaut"));
            }
            let number = num(m, "ToolNumber").map_or(0, |v| v.max(0.0) as u32);
            let tk = tool_kind(text(m, "ToolProfile").unwrap_or(""), kind);
            let tool = tool_for(p, tk, diameter, number, (feed, plunge, spindle))?;
            let id = p.add_operation(kind, &name, shapes, &tool).map_err(|e| e.to_string())?;
            let surface = num(m, "StockSurface").unwrap_or(0.0);
            let depth = num(m, "TargetDepth").unwrap_or(surface);
            let mut tab_points = Vec::new();
            let mut cam = CamSettings { surface, ..CamSettings::default() };
            cam.clearance = num(m, "ClearancePlane").unwrap_or(cam.clearance);
            cam.final_increment = num(m, "FinalDepthIncrement").unwrap_or(0.0).max(0.0);
            cam.level_first = text(m, "CutOrdering").is_some_and(|t| t.eq_ignore_ascii_case("LevelFirst"));
            cam.corner_overcut = yes(m, "CornerOvercut").unwrap_or(false);
            cam.max_crossover = num(m, "MaxCrossoverDistance").unwrap_or(cam.max_crossover).clamp(0.0, 1.0);
            cam.spindle_ccw = text(m, "SpindleDirection").is_some_and(|t| t.eq_ignore_ascii_case("CCW"));
            let mut tabs = false;
            if let Some(h) = child(m, "HoldingTabs") {
                let method = text(h, "TabMethod").unwrap_or("None");
                tabs = !method.eq_ignore_ascii_case("None");
                cam.tab_width = num(h, "Width").unwrap_or(cam.tab_width);
                cam.tab_height = num(h, "Height").unwrap_or(cam.tab_height);
                cam.tab_min = num(h, "MinimumTabs").map_or(cam.tab_min, |v| v.max(0.0) as u32);
                cam.tab_max = num(h, "MaximumTabs").map_or(cam.tab_max, |v| v.max(0.0) as u32);
                cam.tab_distance = num(h, "TabDistance").unwrap_or(cam.tab_distance);
                cam.tab_threshold = num(h, "SizeThreshold").unwrap_or(cam.tab_threshold);
                cam.tab_shape = match text(h, "TabStyle").unwrap_or("") {
                    s if s.eq_ignore_ascii_case("Square") => TabShape::Square,
                    _ => TabShape::Triangle,
                };
                if method.eq_ignore_ascii_case("Manual") {
                    cam.tab_target = TabTarget::Manual;
                    for t in child(m, "Tabs").into_iter().flat_map(|t| t.children()).filter(|c| c.is_element() && c.tag_name().name() == "HoldingTab") {
                        let at = num(t, "ParentEntityID").and_then(|e| ids.get(&(e as u32))).and_then(|s| point_at(p, *s.first()?, num(t, "ParametricPoint").unwrap_or(0.0)));
                        match at {
                            Some(pt) => tab_points.push(pt),
                            None => notes.push(format!("« {name} » : pont manuel sur une forme introuvable, ignoré")),
                        }
                    }
                }
            }
            let mut lead = LeadChoice::None;
            if let Some(l) = child(m, "LeadInMove") {
                match text(l, "LeadInType").unwrap_or("None") {
                    t if t.eq_ignore_ascii_case("Spiral") => lead = LeadChoice::Spiral,
                    t if t.eq_ignore_ascii_case("Tangent") => lead = LeadChoice::Tangent,
                    _ => {}
                }
                cam.lead_angle = num(l, "SpiralAngle").unwrap_or(0.0);
                cam.lead_radius = num(l, "TangentRadius").unwrap_or(0.0);
            }
            if child(m, "LeadOutMove").is_some_and(|l| text(l, "LeadInType").is_some_and(|t| !t.eq_ignore_ascii_case("None"))) {
                notes.push(format!("« {name} » : sortie (LeadOutMove) non prise en charge"));
            }
            if kind == OpKind::Drill && text(m, "DrillingMethod").is_some_and(|t| !t.eq_ignore_ascii_case("CannedCycle")) {
                notes.push(format!("« {name} » : méthode de perçage {} remplacée par un cycle G81/G83", text(m, "DrillingMethod").unwrap_or("")));
            }
            let increment = num(m, "DepthIncrement").unwrap_or(0.0);
            let full = (surface - depth).abs().max(0.1);
            p.update_operation(id, |o| {
                o.depth = depth;
                o.step_down = match kind {
                    OpKind::Drill => num(m, "PeckDistance").unwrap_or(0.0).max(0.0),
                    // 0 dans le logiciel de référence : toute la profondeur en une passe
                    _ if increment > 0.0 => increment,
                    _ => full,
                };
                o.step_over = num(m, "StepOver").filter(|v| *v > 0.0 && *v <= 1.0).unwrap_or(o.step_over);
                o.climb = text(m, "MillingDirection").is_some_and(|t| t.eq_ignore_ascii_case("Climb"));
                o.side = match (kind, text(m, "InsideOutside").unwrap_or("")) {
                    (OpKind::Engrave, _) => OpSide::On,
                    (_, s) if s.eq_ignore_ascii_case("Inside") => OpSide::Inside,
                    _ => OpSide::Outside,
                };
                o.stock_to_leave = num(m, "RoughingClearance").unwrap_or(0.0);
                o.header = text(m, "CustomMOPHeader").unwrap_or("").to_string();
                o.footer = text(m, "CustomMOPFooter").unwrap_or("").to_string();
                o.tabs = tabs && kind == OpKind::Profile;
                o.tab_points = tab_points;
                o.lead = lead;
                o.cam = cam;
                o.dwell = num(m, "Dwell").unwrap_or(0.0).max(0.0);
                o.part = part_name.clone();
                o.feed = Some(feed);
                o.plunge = Some(plunge);
                o.spindle = Some(spindle);
                o.enabled = part_on && m.attribute("Enabled").map_or(true, |v| !v.eq_ignore_ascii_case("false"));
            })
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(notes)
}
