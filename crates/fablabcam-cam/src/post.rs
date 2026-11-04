//! Post-processeur configurable : gabarits de fichier, de opération, de changement d'outil et de
//! mouvements avec des macros `{$nom}`.
//!
//! Les noms des éléments XML `PostFile`, `Header`, `Footer`, `ToolChange`, `MOP`, `Rapid`, `FeedMove`,
//! `ArcCW`, `ArcCCW`, `Drill`, `DrillDwell`, `DrillPeck`, `MinimumArcLength`, `ArcCenterIncremental`,
//! `ArcCenterAbsolute`, `ClearancePlaneAxis` et `InvertArcs` viennent d'un fichier `.cbpp` réel ; les autres
//! (`Comment`, `EndComment`, `EndOfLine`, `NumberFormat`, `MaximumArcRadius`, `ArcCenterMode`, `UnitsMetric`,
//! `DistanceAbsolute`, `WorkplaneXY`, `SpindleCW`, `SpindleCCW`, `SpindleOff`, `EndRewind`, `CutterCompOff`,
//! `ToolTableItem`, `PostProcessorMacros`, `G0`…`G83`) sont déduits des intitulés de la doc : **inférés**.
//!
//! Numérotation de lignes (`AddLineNumbers`, `LineNumberStart`, `LineNumberIncrement`, `LineNumberPrefix`,
//! `LineNumberFormat`, `LineNumberSkip`, `LineNumberSpaceAfter`) et conversion d'arcs en segments (`ArcOutput`,
//! `ArcToLinesTolerance`) : noms **inférés** de la doc.
//!
//! Non géré : cycles G98/G80, tournage, `Start Cut` / `End Cut`.

use crate::params::Params;
use crate::toolpath::{Move, Toolpath};
use std::collections::HashMap;

/// Convention des I/J d'un arc (option `Arc Center Mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcCenter {
    /// Centre moins point de départ (défaut du logiciel de référence).
    IncrementalFromStart,
    /// Point de départ moins centre.
    IncrementalToStart,
    Absolute,
}

/// Sortie des arcs (option `Arc Output`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcOutput {
    Normal,
    /// Tous les arcs deviennent de petits segments.
    ConvertToLines,
    /// Seuls les arcs hélicoïdaux (Z variable) deviennent des segments.
    HelixConvertToLines,
}

/// Numérotation des lignes (`Line Numbering`).
#[derive(Debug, Clone, PartialEq)]
pub struct LineNumbering {
    pub enabled: bool,
    pub start: i64,
    pub increment: i64,
    pub prefix: String,
    /// `0` = chiffre ou zéro, `#` = chiffre ou espace (ex. `0000`, `###0`).
    pub format: String,
    /// Une ligne dont le premier caractère non blanc est dans cette liste n'est pas numérotée.
    pub skip: String,
    pub space_after: bool,
}

impl Default for LineNumbering {
    fn default() -> Self {
        // valeurs par défaut inférées : la doc ne les donne pas
        LineNumbering { enabled: false, start: 10, increment: 10, prefix: "N".into(), format: "0".into(), skip: "(%;".into(), space_after: true }
    }
}

impl LineNumbering {
    fn number(&self, n: i64) -> String {
        let s = n.to_string();
        let width = self.format.chars().count();
        let pad: String = self.format.chars().take(width.saturating_sub(s.len())).map(|c| if c == '#' { ' ' } else { '0' }).collect();
        format!("{}{}{}", self.prefix, pad, s)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostDef {
    pub file: String,
    pub header: String,
    pub footer: String,
    pub mop: String,
    pub tool_change: String,
    pub tool_table_item: String,
    pub rapid: String,
    pub feed_move: String,
    pub arc_cw: String,
    pub arc_ccw: String,
    pub drill: String,
    pub drill_dwell: String,
    pub drill_peck: String,
    pub comment: String,
    pub end_comment: String,
    pub eol: String,
    pub number_format: String,
    /// Arcs plus courts que cette longueur : sortis en G1 (0 = jamais).
    pub min_arc_length: f64,
    /// Arcs de rayon supérieur : sortis en G1 (0 = jamais).
    pub max_arc_radius: f64,
    pub arc_center: ArcCenter,
    pub arc_output: ArcOutput,
    /// Écart maximal entre arc et segments (mm) pour la conversion en lignes.
    pub arc_tolerance: f64,
    pub line_numbers: LineNumbering,
    /// Textes remplaçant les codes par défaut (`g0`, `units`, `spindle_off`…).
    pub codes: HashMap<String, String>,
    /// Macros utilisateur (`$nom=valeur`), noms en minuscules.
    pub macros: HashMap<String, String>,
}

impl Default for PostDef {
    fn default() -> Self {
        PostDef {
            file: "{$comment} {$cbfile.name} {$endcomment}\n{$header}\n{$mops}\n{$footer}\n".into(),
            // Declare the same initial state as the integrated emitter, while
            // honoring the configured arc-center convention and custom header.
            header: "{$tooltable}\n{$units} {$distancemode} {$workplane} G94 {$cuttercomp(off)} G49 G80 {$arccentermode}\n{$toolchange(first)}".into(),
            footer: "{$clearance}\n{$spindle(off)}\n{$cbfile.footer}\n{$endrewind}".into(),
            mop: "{$comment} {$mop.name} {$endcomment}\n{$toolchange}\n{$mop.header}\n{$spindle} {$s}\n{$blocks}\n{$mop.footer}".into(),
            tool_change: "{$clearance}\n{$comment} T{$tool.index} : {$tool.diameter} {$endcomment}\nT{$tool.index} M6".into(),
            tool_table_item: "{$comment} T{$tool.index} : {$tool.diameter} {$endcomment}".into(),
            rapid: "{$g0} {$_f} {$_x} {$_y} {$_z}".into(),
            feed_move: "{$_g1} {$_f} {$_x} {$_y} {$_z}".into(),
            arc_cw: "{$g2} {$_f} {$_x} {$_y} {$_z} {$i} {$j}".into(),
            arc_ccw: "{$g3} {$_f} {$_x} {$_y} {$_z} {$i} {$j}".into(),
            drill: "{$g81} {$_x} {$_y} {$_z} {$_r} {$_f}".into(),
            drill_dwell: "{$g82} {$_x} {$_y} {$_z} {$p} {$_r} {$_f}".into(),
            drill_peck: "{$g83} {$_x} {$_y} {$_z} {$_q} {$_r} {$_f}".into(),
            comment: "(".into(),
            end_comment: ")".into(),
            eol: "\n".into(),
            number_format: "0.###".into(),
            min_arc_length: 0.0,
            max_arc_radius: 0.0,
            arc_center: ArcCenter::IncrementalFromStart,
            arc_output: ArcOutput::Normal,
            arc_tolerance: 0.001,
            line_numbers: LineNumbering::default(),
            codes: HashMap::new(),
            macros: HashMap::new(),
        }
    }
}

impl PostDef {
    /// Lit une définition `.cbpp`. Renvoie aussi les éléments ignorés (inconnus ou sans effet ici).
    pub fn parse(xml: &str) -> Result<(PostDef, Vec<String>), String> {
        let doc = roxmltree::Document::parse(xml).map_err(|e| format!("XML invalide : {e}"))?;
        let root = doc.root_element();
        if root.tag_name().name() != "PostProcessor" {
            return Err(format!("élément racine « {} » : « PostProcessor » attendu", root.tag_name().name()));
        }
        let mut d = PostDef::default();
        let mut ignored = Vec::new();
        for n in root.children().filter(|n| n.is_element()) {
            let tag = n.tag_name().name();
            let text = n.text().unwrap_or("").to_string();
            let num = || text.trim().parse::<f64>().ok();
            match tag {
                "PostFile" => d.file = text,
                "Header" => d.header = text,
                "Footer" => d.footer = text,
                "MOP" => d.mop = text,
                "ToolChange" => d.tool_change = text,
                "ToolTableItem" => d.tool_table_item = text,
                "Rapid" => d.rapid = text,
                "FeedMove" => d.feed_move = text,
                "ArcCW" => d.arc_cw = text,
                "ArcCCW" => d.arc_ccw = text,
                "Drill" => d.drill = text,
                "DrillDwell" => d.drill_dwell = text,
                "DrillPeck" => d.drill_peck = text,
                "Comment" => d.comment = text,
                "EndComment" => d.end_comment = text,
                "EndOfLine" => d.eol = text.replace("\\r", "\r").replace("\\n", "\n"),
                "NumberFormat" => d.number_format = text.trim().to_string(),
                "MinimumArcLength" => d.min_arc_length = num().unwrap_or(0.0),
                "MaximumArcRadius" => d.max_arc_radius = num().unwrap_or(0.0),
                "ArcCenterMode" => {
                    d.arc_center = match text.trim().to_ascii_lowercase().as_str() {
                        "absolute" => ArcCenter::Absolute,
                        "incrementalp1c" | "incremental (p1-c)" => ArcCenter::IncrementalToStart,
                        _ => ArcCenter::IncrementalFromStart,
                    }
                }
                "ArcOutput" => {
                    d.arc_output = match text.trim().to_ascii_lowercase().as_str() {
                        "converttolines" => ArcOutput::ConvertToLines,
                        "helixconverttolines" => ArcOutput::HelixConvertToLines,
                        _ => ArcOutput::Normal,
                    }
                }
                "ArcToLinesTolerance" => d.arc_tolerance = num().filter(|v| *v > 0.0).unwrap_or(d.arc_tolerance),
                "AddLineNumbers" => d.line_numbers.enabled = text.trim().eq_ignore_ascii_case("true"),
                "LineNumberStart" => d.line_numbers.start = num().map_or(d.line_numbers.start, |v| v as i64),
                "LineNumberIncrement" => d.line_numbers.increment = num().map_or(d.line_numbers.increment, |v| v as i64),
                "LineNumberPrefix" => d.line_numbers.prefix = text,
                "LineNumberFormat" => d.line_numbers.format = text.trim().to_string(),
                "LineNumberSkip" => d.line_numbers.skip = text,
                "LineNumberSpaceAfter" => d.line_numbers.space_after = text.trim().eq_ignore_ascii_case("true"),
                "ArcCenterIncremental" => {
                    d.codes.insert("arccenter_inc".into(), text.trim().into());
                }
                "ArcCenterAbsolute" => {
                    d.codes.insert("arccenter_abs".into(), text.trim().into());
                }
                "UnitsInches" => {
                    d.codes.insert("units_inch".into(), text.trim().into());
                }
                "UnitsMetric" | "DistanceAbsolute" | "WorkplaneXY" | "SpindleCW" | "SpindleCCW" | "SpindleOff" | "EndRewind" | "CutterCompOff" | "CutterCompLeft"
                | "CutterCompRight" => {
                    d.codes.insert(code_key(tag).into(), text.trim().into());
                }
                "G0" | "G1" | "G2" | "G3" | "G81" | "G82" | "G83" => {
                    d.codes.insert(tag.to_ascii_lowercase(), text.trim().into());
                }
                "PostProcessorMacros" => d.add_macros(&text),
                // lus mais sans effet dans ce moteur : fraisage uniquement
                "ClearancePlaneAxis" | "InvertArcs" => ignored.push(tag.into()),
                other => ignored.push(other.into()),
            }
        }
        Ok((d, ignored))
    }

    /// Ajoute des macros au format `$nom=valeur`, une par ligne (propriété `Post Processor Macros`).
    pub fn add_macros(&mut self, text: &str) {
        for l in text.lines() {
            if let Some((k, v)) = l.trim().strip_prefix('$').and_then(|r| r.split_once('=')) {
                self.macros.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
            }
        }
    }
}

fn code_key(tag: &str) -> &'static str {
    match tag {
        "UnitsMetric" => "units",
        "DistanceAbsolute" => "distancemode",
        "WorkplaneXY" => "workplane",
        "SpindleCW" => "spindle_cw",
        "SpindleCCW" => "spindle_ccw",
        "SpindleOff" => "spindle_off",
        "EndRewind" => "endrewind",
        "CutterCompOff" => "cuttercomp_off",
        "CutterCompLeft" => "cuttercomp_l",
        _ => "cuttercomp_r",
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolInfo {
    pub index: usize,
    pub name: String,
    pub diameter: f64,
    /// `{$tool.comment}`.
    pub comment: String,
    /// Texte de changement d'outil propre à cet outil : remplace le gabarit `ToolChange`.
    pub change: String,
}

pub struct Mop<'a> {
    pub name: String,
    /// Script inséré avant / après l'opération (`{$mop.header}` / `{$mop.footer}`).
    pub header: String,
    pub footer: String,
    pub tool: ToolInfo,
    pub path: &'a Toolpath,
    pub params: &'a Params,
}

pub struct Program<'a> {
    pub name: String,
    pub date: String,
    /// `{$cbfile.header}` / `{$cbfile.footer}`.
    pub file_header: String,
    pub file_footer: String,
    /// Programme en pouces : `{$units}` donne G20 (les trajectoires sont déjà converties par l'appelant).
    pub inches: bool,
    pub mops: Vec<Mop<'a>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostOutput {
    pub gcode: String,
    /// Macros inconnues, une fois chacune.
    pub warnings: Vec<String>,
}

/// Produit le G-code d'un programme avec la définition donnée.
pub fn render(def: &PostDef, prog: &Program) -> PostOutput {
    let mut r = Renderer { def, prog, warnings: Vec::new(), mop: None, tool_scope: None, tool: None, last: HashMap::new(), last_g: None, vals: HashMap::new(), pos: (0.0, 0.0, 0.0), depth: 0, coords: 0 };
    let text = r.expand(&def.file);
    let ln = &def.line_numbers;
    let mut n = ln.start;
    let numbered: Vec<String> = text
        .lines()
        .map(|l| {
            let skipped = l.trim_start().chars().next().is_some_and(|c| ln.skip.contains(c));
            if !ln.enabled || skipped {
                return l.to_string();
            }
            let s = format!("{}{}{}", ln.number(n), if ln.space_after { " " } else { "" }, l);
            n += ln.increment;
            s
        })
        .collect();
    let mut gcode = numbered.join(&def.eol);
    gcode.push_str(&def.eol);
    PostOutput { gcode, warnings: r.warnings }
}

struct Renderer<'a> {
    def: &'a PostDef,
    prog: &'a Program<'a>,
    warnings: Vec<String>,
    /// Opération en cours (pour `{$s}`, `{$mop.*}`, `{$clearance}`).
    mop: Option<usize>,
    /// Opération dont l'outil est visé par `{$tool.*}`.
    tool_scope: Option<usize>,
    /// Outil courant (indice dans `prog.mops` de l'opération qui l'a monté).
    tool: Option<usize>,
    last: HashMap<char, String>,
    last_g: Option<String>,
    vals: HashMap<char, f64>,
    pos: (f64, f64, f64),
    depth: usize,
    /// Nombre de mots de coordonnées (X Y Z I J R Q P) écrits : un mouvement sans coordonnée est supprimé.
    coords: usize,
}

impl Renderer<'_> {
    /// Format de nombre : `0` = chiffre toujours écrit, `#` = chiffre facultatif (ex. `0.0###`).
    fn fmt(&self, v: f64) -> String {
        let frac = self.def.number_format.split_once('.').map_or("", |(_, f)| f);
        let max = frac.chars().filter(|c| *c == '0' || *c == '#').count();
        let min = frac.chars().filter(|c| *c == '0').count();
        let mut s = format!("{v:.max$}");
        if let Some(dot) = s.find('.') {
            while s.len() > dot + 1 + min && s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.pop();
            }
        }
        if s.starts_with('-') && s.parse::<f64>() == Ok(0.0) {
            s.remove(0);
        }
        s
    }

    fn code(&self, key: &str, default: &str) -> String {
        self.def.codes.get(key).cloned().unwrap_or_else(|| default.to_string())
    }

    /// Développe un gabarit : une ligne à la fois, espaces multiples réduits, lignes vides supprimées.
    fn expand(&mut self, tpl: &str) -> String {
        if self.depth > 8 {
            self.warn("gabarits imbriqués trop profondément (récursion ?)");
            return String::new();
        }
        self.depth += 1;
        let mut out: Vec<String> = Vec::new();
        for line in tpl.lines() {
            let mut s = String::new();
            let mut rest = line;
            while let Some(i) = rest.find("{$") {
                s.push_str(&rest[..i]);
                let Some(j) = rest[i..].find('}') else {
                    s.push_str(&rest[i..]);
                    rest = "";
                    break;
                };
                let inner = &rest[i + 2..i + j];
                let (name, arg) = match inner.split_once('(') {
                    Some((n, a)) => (n, Some(a.trim_end_matches(')'))),
                    None => (inner, None),
                };
                s.push_str(&self.macro_value(&name.trim().to_ascii_lowercase(), arg.map(|a| a.trim().to_ascii_lowercase())));
                rest = &rest[i + j + 1..];
            }
            s.push_str(rest);
            for l in s.lines() {
                let l = collapse_spaces(l);
                if !l.is_empty() {
                    out.push(l);
                }
            }
        }
        self.depth -= 1;
        out.join("\n")
    }

    fn warn(&mut self, msg: &str) {
        if !self.warnings.iter().any(|w| w == msg) {
            self.warnings.push(msg.to_string());
        }
    }

    fn cur_mop(&self) -> Option<&Mop<'_>> {
        self.mop.or(self.tool_scope).and_then(|i| self.prog.mops.get(i))
    }

    fn macro_value(&mut self, name: &str, arg: Option<String>) -> String {
        let arg = arg.as_deref();
        match name {
            "comment" => return self.def.comment.clone(),
            "endcomment" => return self.def.end_comment.clone(),
            "header" => return self.expand(&self.def.header.clone()),
            "footer" => return self.expand(&self.def.footer.clone()),
            "mops" => {
                let mut parts = Vec::new();
                for i in 0..self.prog.mops.len() {
                    self.mop = Some(i);
                    self.tool_scope = Some(i);
                    parts.push(self.expand(&self.def.mop.clone()));
                }
                self.mop = None;
                self.tool_scope = None;
                return parts.join("\n");
            }
            "blocks" => return self.blocks(),
            "toolchange" => {
                let target = if arg == Some("first") { 0 } else { self.mop.unwrap_or(0) };
                let Some(m) = self.prog.mops.get(target) else { return String::new() };
                if self.tool.map(|t| &self.prog.mops[t].tool) == Some(&m.tool) {
                    return String::new();
                }
                self.tool = Some(target);
                let m_change = m.tool.change.clone();
                let (old_m, old_t) = (self.mop, self.tool_scope);
                self.mop = Some(target);
                self.tool_scope = Some(target);
                let tpl = if m_change.trim().is_empty() { self.def.tool_change.clone() } else { m_change };
                let s = self.expand(&tpl);
                self.mop = old_m;
                self.tool_scope = old_t;
                return s;
            }
            "tooltable" => {
                let mut seen: Vec<&ToolInfo> = Vec::new();
                let mut firsts = Vec::new();
                for (i, m) in self.prog.mops.iter().enumerate() {
                    if !seen.contains(&&m.tool) {
                        seen.push(&m.tool);
                        firsts.push(i);
                    }
                }
                let old = self.tool_scope;
                let mut parts = Vec::new();
                for i in firsts {
                    self.tool_scope = Some(i);
                    parts.push(self.expand(&self.def.tool_table_item.clone()));
                }
                self.tool_scope = old;
                return parts.join("\n");
            }
            "clearance" => {
                let i = self.mop.or(self.tool_scope).unwrap_or(if self.tool.is_some() { self.prog.mops.len().saturating_sub(1) } else { 0 });
                let Some(z) = self.prog.mops.get(i).map(|m| m.params.clearance_plane) else { return String::new() };
                return self.move_text(&Move::Rapid { x: None, y: None, z: Some(z) });
            }
            "cbfile.name" => return self.prog.name.clone(),
            "cbfile.header" => return self.prog.file_header.clone(),
            "cbfile.footer" => return self.prog.file_footer.clone(),
            "date" => return self.prog.date.clone(),
            "mop.name" => return self.cur_mop().map(|m| m.name.clone()).unwrap_or_default(),
            "mop.header" => return self.cur_mop().map(|m| m.header.clone()).unwrap_or_default(),
            "mop.footer" => return self.cur_mop().map(|m| m.footer.clone()).unwrap_or_default(),
            "tool.index" => return self.cur_mop().map(|m| m.tool.index.to_string()).unwrap_or_default(),
            "tool.comment" => return self.cur_mop().map(|m| m.tool.comment.clone()).unwrap_or_default(),
            "tool.name" => return self.cur_mop().map(|m| m.tool.name.clone()).unwrap_or_default(),
            "tool.diameter" => {
                let d = self.cur_mop().map(|m| m.tool.diameter);
                return d.map(|d| self.fmt(d)).unwrap_or_default();
            }
            "tool.radius" => {
                let d = self.cur_mop().map(|m| m.tool.diameter / 2.0);
                return d.map(|d| self.fmt(d)).unwrap_or_default();
            }
            "units" => return if self.prog.inches { self.code("units_inch", "G20") } else { self.code("units", "G21") },
            "distancemode" => return self.code("distancemode", "G90"),
            "workplane" => return self.code("workplane", "G17"),
            "endrewind" => return self.code("endrewind", "M30"),
            "arccentermode" => {
                return if self.def.arc_center == ArcCenter::Absolute { self.code("arccenter_abs", "G90.1") } else { self.code("arccenter_inc", "G91.1") };
            }
            "spindle" => {
                return match arg {
                    Some("off") => self.code("spindle_off", "M5"),
                    _ => match self.cur_mop().map(|m| m.params.spindle_cw) {
                        Some(false) => self.code("spindle_ccw", "M4"),
                        _ => self.code("spindle_cw", "M3"),
                    },
                };
            }
            "cuttercomp" => {
                return match arg {
                    Some("l") => self.code("cuttercomp_l", "G41"),
                    Some("r") => self.code("cuttercomp_r", "G42"),
                    _ => self.code("cuttercomp_off", "G40"),
                };
            }
            "s" => {
                let s = self.cur_mop().map(|m| m.params.spindle_speed);
                return s.filter(|s| *s > 0.0).map(|s| format!("S{}", self.fmt(s))).unwrap_or_default();
            }
            "a" | "b" | "c" | "k" | "_a" | "_b" | "_c" | "_k" | "velocitymode" => {
                if let Some(v) = self.def.macros.get(name) {
                    return v.clone();
                }
                return String::new();
            }
            _ => {}
        }
        // macros utilisateur avant les mots de mouvement : elles peuvent les remplacer
        if let Some(v) = self.def.macros.get(name) {
            return v.clone();
        }
        let (modal, bare) = match name.strip_prefix('_') {
            Some(b) => (true, b),
            None => (false, name),
        };
        if let Some(g) = bare.strip_prefix('g').filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())) {
            let text = self.code(bare, &format!("G{g}"));
            if modal && self.last_g.as_deref() == Some(text.as_str()) {
                return String::new();
            }
            self.last_g = Some(text.clone());
            return text;
        }
        if let (1, Some(letter)) = (bare.len(), bare.chars().next()) {
            if "xyzfijrqp".contains(letter) {
                let Some(v) = self.vals.get(&letter).copied() else { return String::new() };
                let s = format!("{}{}", letter.to_ascii_uppercase(), self.fmt(v));
                if modal && self.last.get(&letter) == Some(&s) {
                    return String::new();
                }
                self.last.insert(letter, s.clone());
                self.coords += 1;
                return s;
            }
        }
        self.warn(&format!("macro inconnue : {{${name}}}"));
        String::new()
    }

    fn blocks(&mut self) -> String {
        let Some(i) = self.mop else { return String::new() };
        let prog = self.prog;
        let mut lines = Vec::new();
        for m in &prog.mops[i].path.moves {
            let t = self.move_text(m);
            if !t.is_empty() {
                lines.push(t);
            }
        }
        lines.join("\n")
    }

    /// Développe le gabarit d'un mouvement. S'il n'écrit aucune coordonnée (position déjà atteinte),
    /// la ligne est supprimée et l'état modal restauré.
    fn with_vals(&mut self, vals: &[(char, Option<f64>)], tpl: &str) -> String {
        self.vals = vals.iter().filter_map(|(k, v)| v.map(|v| (*k, v))).collect();
        let (saved_last, saved_g, before) = (self.last.clone(), self.last_g.clone(), self.coords);
        let s = self.expand(tpl);
        self.vals.clear();
        if self.coords == before {
            self.last = saved_last;
            self.last_g = saved_g;
            return String::new();
        }
        s
    }

    fn move_text(&mut self, m: &Move) -> String {
        let def = self.def;
        match m {
            Move::Comment(c) => format!("{}{}{}", def.comment, c, def.end_comment),
            Move::Rapid { x, y, z } => {
                let s = self.with_vals(&[('x', *x), ('y', *y), ('z', *z)], &def.rapid);
                self.advance(*x, *y, *z);
                s
            }
            Move::Feed { x, y, z, f } => {
                let s = self.with_vals(&[('x', *x), ('y', *y), ('z', *z), ('f', Some(*f))], &def.feed_move);
                self.advance(*x, *y, *z);
                s
            }
            Move::Arc { ccw, x, y, z, i, j, f } => {
                let (px, py) = (self.pos.0, self.pos.1);
                let r = i.hypot(*j);
                let sweep = arc_sweep(px, py, *x, *y, px + i, py + j, *ccw);
                let too_short = def.min_arc_length > 0.0 && r * sweep < def.min_arc_length;
                let too_big = def.max_arc_radius > 0.0 && r > def.max_arc_radius;
                let helical = z.is_some_and(|z| (z - self.pos.2).abs() > 1e-9);
                let to_lines = match def.arc_output {
                    ArcOutput::Normal => false,
                    ArcOutput::ConvertToLines => true,
                    ArcOutput::HelixConvertToLines => helical,
                };
                let s = if to_lines && r > 1e-9 && !(too_short || too_big) {
                    self.arc_as_lines((px, py, self.pos.2), (*x, *y, *z), (px + i, py + j), r, sweep, *ccw, *f)
                } else if too_short || too_big || to_lines {
                    self.with_vals(&[('x', Some(*x)), ('y', Some(*y)), ('z', *z), ('f', Some(*f))], &def.feed_move)
                } else {
                    let (ci, cj) = match def.arc_center {
                        ArcCenter::IncrementalFromStart => (*i, *j),
                        ArcCenter::IncrementalToStart => (-i, -j),
                        ArcCenter::Absolute => (px + i, py + j),
                    };
                    let tpl = if *ccw { &def.arc_ccw } else { &def.arc_cw };
                    self.with_vals(&[('x', Some(*x)), ('y', Some(*y)), ('z', *z), ('i', Some(ci)), ('j', Some(cj)), ('f', Some(*f))], tpl)
                };
                self.advance(Some(*x), Some(*y), *z);
                s
            }
            Move::Drill { x, y, z, retract, f, peck, dwell } => {
                let tpl = match (peck, *dwell > 0.0) {
                    (Some(_), _) => &def.drill_peck,
                    (None, true) => &def.drill_dwell,
                    (None, false) => &def.drill,
                };
                let p = (*dwell > 0.0).then_some(*dwell);
                let s = self.with_vals(&[('x', Some(*x)), ('y', Some(*y)), ('z', Some(*z)), ('r', Some(*retract)), ('q', *peck), ('p', p), ('f', Some(*f))], tpl);
                self.advance(Some(*x), Some(*y), None);
                s
            }
        }
    }

    /// Arc remplacé par des segments dont l'écart à l'arc ne dépasse pas `arc_tolerance`.
    #[allow(clippy::too_many_arguments)]
    fn arc_as_lines(&mut self, start: (f64, f64, f64), end: (f64, f64, Option<f64>), c: (f64, f64), r: f64, sweep: f64, ccw: bool, f: f64) -> String {
        let tol = self.def.arc_tolerance.max(1e-9);
        let step = if tol >= r { std::f64::consts::PI } else { 2.0 * (1.0 - tol / r).acos() };
        let n = ((sweep / step).ceil() as usize).clamp(1, 100_000);
        let a0 = (start.1 - c.1).atan2(start.0 - c.0);
        let dir = if ccw { 1.0 } else { -1.0 };
        let z1 = end.2.unwrap_or(start.2);
        let mut lines = Vec::new();
        for k in 1..=n {
            let t = k as f64 / n as f64;
            let (x, y) = if k == n { (end.0, end.1) } else { (c.0 + r * (a0 + dir * sweep * t).cos(), c.1 + r * (a0 + dir * sweep * t).sin()) };
            let z = end.2.map(|_| start.2 + (z1 - start.2) * t);
            let feed = self.def.feed_move.clone();
            let l = self.with_vals(&[('x', Some(x)), ('y', Some(y)), ('z', z), ('f', Some(f))], &feed);
            self.advance(Some(x), Some(y), z);
            if !l.is_empty() {
                lines.push(l);
            }
        }
        lines.join("\n")
    }

    fn advance(&mut self, x: Option<f64>, y: Option<f64>, z: Option<f64>) {
        self.pos = (x.unwrap_or(self.pos.0), y.unwrap_or(self.pos.1), z.unwrap_or(self.pos.2));
    }
}

/// Angle balayé par un arc de (x0,y0) à (x1,y1) autour de (cx,cy) ; un arc fermé fait un tour complet.
fn arc_sweep(x0: f64, y0: f64, x1: f64, y1: f64, cx: f64, cy: f64, ccw: bool) -> f64 {
    let a0 = (y0 - cy).atan2(x0 - cx);
    let a1 = (y1 - cy).atan2(x1 - cx);
    let d = if ccw { (a1 - a0).rem_euclid(std::f64::consts::TAU) } else { (a0 - a1).rem_euclid(std::f64::consts::TAU) };
    if d < 1e-9 {
        std::f64::consts::TAU
    } else {
        d
    }
}

fn collapse_spaces(l: &str) -> String {
    let mut out = String::with_capacity(l.len());
    let mut prev_space = false;
    for c in l.trim().chars() {
        if c == ' ' {
            if !prev_space {
                out.push(c);
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out
}
