//! Sauvegarde du projet dans le `.cb` : dessin au format de `fablabcam-model`, puis les éléments
//! `<Stock>`, `<Tools>` et `<Operations>` ajoutés avant `</CADFile>`.
//!
//! Ces éléments sont une **extension de FabLabCAM** distincte du schéma importé
//! `parts/part/machineops`. Un lecteur externe peut les ignorer ; un `.cb` sans eux se charge
//! avec des listes vides. Les opérations désignent leurs formes par rang (1 = première forme du
//! dessin dans l'ordre calque par calque).

use crate::project::*;
use fablabcam_model::{parse_cb, write_cb};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
}

fn esc_multiline(s: &str) -> String {
    esc(s).replace('\r', "&#13;").replace('\n', "&#10;")
}

/// Réglages d'usinage sous forme (attribut, valeur) ; l'ordre est celui de l'écriture.
fn cam_attrs(c: &CamSettings) -> Vec<(&'static str, String)> {
    vec![
        ("Clearance", c.clearance.to_string()),
        ("Surface", c.surface.to_string()),
        ("LevelFirst", c.level_first.to_string()),
        ("FinishStepover", c.finish_stepover.to_string()),
        ("FinishAtTargetOnly", c.finish_at_target_only.to_string()),
        ("FinalIncrement", c.final_increment.to_string()),
        ("CornerOvercut", c.corner_overcut.to_string()),
        ("MaxCrossover", c.max_crossover.to_string()),
        ("SpindleCCW", c.spindle_ccw.to_string()),
        ("TabShape", c.tab_shape.key().to_string()),
        ("TabWidth", c.tab_width.to_string()),
        ("TabHeight", c.tab_height.to_string()),
        ("TabMin", c.tab_min.to_string()),
        ("TabMax", c.tab_max.to_string()),
        ("TabDistance", c.tab_distance.to_string()),
        ("TabMethod", c.tab_target.key().to_string()),
        ("TabThreshold", c.tab_threshold.to_string()),
        ("LeadAngle", c.lead_angle.to_string()),
        ("LeadRadius", c.lead_radius.to_string()),
    ]
}

fn read_cam(n: roxmltree::Node) -> CamSettings {
    let d = CamSettings::default();
    let f = |k: &str, v: f64| n.attribute(k).and_then(|x| x.parse::<f64>().ok()).unwrap_or(v);
    let b = |k: &str, v: bool| n.attribute(k).map_or(v, |x| x == "true");
    let u = |k: &str, v: u32| n.attribute(k).and_then(|x| x.parse::<u32>().ok()).unwrap_or(v);
    CamSettings {
        clearance: f("Clearance", d.clearance),
        surface: f("Surface", d.surface),
        level_first: b("LevelFirst", d.level_first),
        finish_stepover: f("FinishStepover", d.finish_stepover),
        finish_at_target_only: b("FinishAtTargetOnly", d.finish_at_target_only),
        final_increment: f("FinalIncrement", d.final_increment),
        corner_overcut: b("CornerOvercut", d.corner_overcut),
        max_crossover: f("MaxCrossover", d.max_crossover),
        spindle_ccw: b("SpindleCCW", d.spindle_ccw),
        tab_shape: n.attribute("TabShape").and_then(TabShape::from_key).unwrap_or(d.tab_shape),
        tab_width: f("TabWidth", d.tab_width),
        tab_height: f("TabHeight", d.tab_height),
        tab_min: u("TabMin", d.tab_min),
        tab_max: u("TabMax", d.tab_max),
        tab_distance: f("TabDistance", d.tab_distance),
        tab_target: n.attribute("TabMethod").and_then(TabTarget::from_key).unwrap_or(d.tab_target),
        tab_threshold: f("TabThreshold", d.tab_threshold),
        lead_angle: f("LeadAngle", d.lead_angle),
        lead_radius: f("LeadRadius", d.lead_radius),
    }
}

fn opt(v: Option<f64>) -> String {
    v.map_or(String::new(), |x| format!(" {{K}}=\"{x}\""))
}

pub fn save(p: &Project) -> String {
    let doc = p.to_document();
    // rang d'une forme dans l'ordre d'écriture (calque par calque)
    let mut order: Vec<u32> = Vec::new();
    for l in &doc.layers {
        for s in p.shapes.iter().filter(|s| s.layer == l.name) {
            order.push(s.id);
        }
    }
    let mut xml = write_cb(&doc);
    let mut ext = String::new();
    if let Some(s) = p.stock {
        ext += &format!("<Stock MinX=\"{}\" MinY=\"{}\" MaxX=\"{}\" MaxY=\"{}\" Top=\"{}\" Bottom=\"{}\"/>\n", s.min[0], s.min[1], s.max[0], s.max[1], s.top, s.bottom);
    }
    if p.gcode_inches {
        ext += "<Settings GcodeUnits=\"in\"/>\n";
    }
    ext += "<Tools>\n";
    for t in &p.tools {
        let mut extra = String::new();
        for (k, v) in [
            ("FluteLength", t.flute_length),
            ("Length", t.length),
            ("ShankDiameter", t.shank_diameter),
            ("HelixAngle", t.helix_angle),
            ("VeeAngle", t.vee_angle),
            ("MaxRampAngle", t.max_ramp_angle),
            ("AxialDepth", t.axial_depth),
            ("RadialDepth", t.radial_depth),
            ("ToothLoad", t.tooth_load),
        ] {
            if v != 0.0 {
                extra += &format!(" {k}=\"{v}\"");
            }
        }
        for (k, v) in [("Material", &t.material), ("Coating", &t.coating), ("PartCode", &t.part_code), ("Notes", &t.notes), ("Comment", &t.comment), ("ToolChange", &t.tool_change)] {
            if !v.is_empty() {
                extra += &format!(" {k}=\"{}\"", esc_multiline(v));
            }
        }
        ext += &format!(
            "<Tool Name=\"{}\" Index=\"{}\" Kind=\"{}\" Diameter=\"{}\" Flutes=\"{}\" Feed=\"{}\" Plunge=\"{}\" Spindle=\"{}\"{extra}/>\n",
            esc(&t.name), t.index, t.kind.key(), t.diameter, t.flutes, t.feed, t.plunge, t.spindle
        );
    }
    ext += "</Tools>\n<Operations>\n";
    for o in &p.ops {
        let refs: Vec<String> = o.shapes.iter().filter_map(|id| order.iter().position(|x| x == id)).map(|i| (i + 1).to_string()).collect();
        let mut extra = String::new();
        for (k, v) in [("Feed", o.feed), ("Plunge", o.plunge), ("Spindle", o.spindle)] {
            extra += &opt(v).replace("{K}", k);
        }
        if !o.tab_points.is_empty() {
            let pts: Vec<String> = o.tab_points.iter().map(|t| format!("{},{}", t[0], t[1])).collect();
            extra += &format!(" TabPoints=\"{}\"", pts.join(" "));
        }
        let defaults = cam_attrs(&CamSettings::default());
        for ((k, v), (_, dv)) in cam_attrs(&o.cam).into_iter().zip(defaults) {
            if v != dv {
                extra += &format!(" {k}=\"{v}\"");
            }
        }
        if o.dwell != 0.0 {
            extra += &format!(" Dwell=\"{}\"", o.dwell);
        }
        if !o.part.is_empty() {
            extra += &format!(" Part=\"{}\"", esc(&o.part));
        }
        if o.stock_to_leave != 0.0 {
            extra += &format!(" StockToLeave=\"{}\"", o.stock_to_leave);
        }
        for (k, v) in [("Header", &o.header), ("Footer", &o.footer)] {
            if !v.is_empty() {
                extra += &format!(" {k}=\"{}\"", esc_multiline(v));
            }
        }
        ext += &format!(
            "<Operation Name=\"{}\" Kind=\"{}\" Side=\"{}\" Tool=\"{}\" Depth=\"{}\" StepDown=\"{}\" StepOver=\"{}\" Climb=\"{}\" Tabs=\"{}\" Lead=\"{}\" Enabled=\"{}\"{extra} Shapes=\"{}\"/>\n",
            esc(&o.name), o.kind.key(), o.side.key(), esc(&o.tool), o.depth, o.step_down, o.step_over, o.climb, o.tabs, o.lead.key(), o.enabled, refs.join(" ")
        );
    }
    ext += "</Operations>\n";
    if let Some(i) = xml.rfind("</CADFile>") {
        xml.insert_str(i, &ext);
    }
    xml
}

pub fn load(xml: &str) -> Result<Project, String> {
    load_with_notes(xml).map(|(p, _)| p)
}

/// Comme [`load`], avec les remarques de l'import des usinages d'un projet XML importé (réglages ignorés…).
pub fn load_with_notes(xml: &str) -> Result<(Project, Vec<String>), String> {
    let doc = parse_cb(xml).map_err(|e| e.to_string())?;
    let mut p = Project::from_document(&doc);
    // identifiant XML → forme, dans l'ordre de `from_document`
    let mut cb_ids: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    let ranks = doc.layers.iter().flat_map(|l| (0..l.entities.len()).map(move |i| l.ids.get(i).copied().flatten()));
    for (cb, s) in ranks.zip(p.shapes.iter().map(|s| s.id)) {
        if let Some(cb) = cb {
            cb_ids.entry(cb).or_default().push(s);
        }
    }
    let tree = roxmltree::Document::parse(xml.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    let root = tree.root_element();
    let f = |n: roxmltree::Node, k: &str| n.attribute(k).and_then(|v| v.parse::<f64>().ok());
    for n in root.children().filter(|n| n.is_element()) {
        match n.tag_name().name() {
            "Settings" => p.gcode_inches = n.attribute("GcodeUnits") == Some("in"),
            "Stock" => {
                if let (Some(a), Some(b), Some(c), Some(d), Some(t)) = (f(n, "MinX"), f(n, "MinY"), f(n, "MaxX"), f(n, "MaxY"), f(n, "Top")) {
                    p.stock = Some(Stock { min: [a, b], max: [c, d], top: t, bottom: f(n, "Bottom").unwrap_or(t - 10.0) });
                }
            }
            "Tools" => {
                for t in n.children().filter(|c| c.is_element() && c.tag_name().name() == "Tool") {
                    let kind = t.attribute("Kind").and_then(ToolKind::from_key).unwrap_or(ToolKind::EndMill);
                    let s = |k: &str| t.attribute(k).unwrap_or("").to_string();
                    let tool = Tool {
                        // sans `Index` (anciens documents) : numéro attribué dans l'ordre de la table
                        index: f(t, "Index").map_or(0, |v| v as u32),
                        flutes: f(t, "Flutes").unwrap_or(2.0) as u32,
                        feed: f(t, "Feed").unwrap_or(800.0),
                        plunge: f(t, "Plunge").unwrap_or(300.0),
                        spindle: f(t, "Spindle").unwrap_or(12000.0),
                        flute_length: f(t, "FluteLength").unwrap_or(0.0),
                        length: f(t, "Length").unwrap_or(0.0),
                        shank_diameter: f(t, "ShankDiameter").unwrap_or(0.0),
                        helix_angle: f(t, "HelixAngle").unwrap_or(0.0),
                        vee_angle: f(t, "VeeAngle").unwrap_or(if kind == ToolKind::VBit { 90.0 } else { 0.0 }),
                        max_ramp_angle: f(t, "MaxRampAngle").unwrap_or(0.0),
                        axial_depth: f(t, "AxialDepth").unwrap_or(0.0),
                        radial_depth: f(t, "RadialDepth").unwrap_or(0.0),
                        tooth_load: f(t, "ToothLoad").unwrap_or(0.0),
                        material: s("Material"),
                        coating: s("Coating"),
                        part_code: s("PartCode"),
                        notes: s("Notes"),
                        comment: s("Comment"),
                        tool_change: s("ToolChange"),
                        ..Tool::new(t.attribute("Name").unwrap_or(""), kind, f(t, "Diameter").unwrap_or(3.0))
                    };
                    p.add_tool(tool).map_err(|e| e.to_string())?;
                }
            }
            "Operations" => {
                for o in n.children().filter(|c| c.is_element() && c.tag_name().name() == "Operation") {
                    let shapes: Vec<u32> = o
                        .attribute("Shapes")
                        .unwrap_or("")
                        .split_whitespace()
                        .filter_map(|r| r.parse::<usize>().ok())
                        .filter_map(|r| p.shapes.get(r.wrapping_sub(1)).map(|s| s.id))
                        .collect();
                    let kind = o.attribute("Kind").and_then(OpKind::from_key).ok_or("opération sans type valide")?;
                    let id = p
                        .add_operation(kind, o.attribute("Name").unwrap_or("op"), shapes, o.attribute("Tool").unwrap_or(""))
                        .map_err(|e| e.to_string())?;
                    p.update_operation(id, |op| {
                        op.side = o.attribute("Side").and_then(OpSide::from_key).unwrap_or(op.side);
                        op.depth = f(o, "Depth").unwrap_or(op.depth);
                        op.step_down = f(o, "StepDown").unwrap_or(op.step_down);
                        op.step_over = f(o, "StepOver").unwrap_or(op.step_over);
                        op.stock_to_leave = f(o, "StockToLeave").unwrap_or(0.0);
                        op.cam = read_cam(o);
                        op.dwell = f(o, "Dwell").unwrap_or(0.0);
                        op.part = o.attribute("Part").unwrap_or("").to_string();
                        op.header = o.attribute("Header").unwrap_or("").to_string();
                        op.footer = o.attribute("Footer").unwrap_or("").to_string();
                        op.climb = o.attribute("Climb").map_or(op.climb, |v| v == "true");
                        op.tabs = o.attribute("Tabs").map_or(op.tabs, |v| v == "true");
                        op.tab_points = o
                            .attribute("TabPoints")
                            .unwrap_or("")
                            .split_whitespace()
                            .filter_map(|s| s.split_once(',').and_then(|(x, y)| Some([x.parse().ok()?, y.parse().ok()?])))
                            .collect();
                        op.lead = o.attribute("Lead").and_then(LeadChoice::from_key).unwrap_or(op.lead);
                        op.enabled = o.attribute("Enabled").map_or(true, |v| v == "true");
                        op.feed = f(o, "Feed");
                        op.plunge = f(o, "Plunge");
                        op.spindle = f(o, "Spindle");
                    })
                    .map_err(|e| e.to_string())?;
                }
            }
            _ => {}
        }
    }
    let notes = crate::cbnative::import(&mut p, root, &cb_ids)?;
    p.reserve_ids();
    Ok((p, notes))
}
