//! Paramètres d'usinage typés, dérivés d'un [`CamStyle`] et surchargeables.

use crate::style::CamStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabMethod {
    None,
    Automatic,
    AutomaticOuter,
    AutomaticInner,
    /// Tabs aux points de `Tabs::positions`, projetés sur le contour d'usinage.
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabStyle {
    Triangle,
    Square,
    /// Comme Square, mais traversée au-dessus de la tab en rapide (découpe plasma).
    Skip,
}

/// Languettes de maintien.
#[derive(Debug, Clone, PartialEq)]
pub struct Tabs {
    pub method: TabMethod,
    /// Largeur à l'endroit le plus étroit.
    pub width: f64,
    /// Hauteur mesurée depuis la profondeur cible.
    pub height: f64,
    pub min: usize,
    pub max: usize,
    /// Distance approximative entre tabs ; 0 = toujours le minimum.
    pub distance: f64,
    /// Périmètre en dessous duquel aucune tab n'est insérée.
    pub size_threshold: f64,
    pub style: TabStyle,
    /// Points de placement pour `TabMethod::Manual` (non lus depuis les styles).
    pub positions: Vec<(f64, f64)>,
}

impl Default for Tabs {
    fn default() -> Self {
        Tabs { method: TabMethod::None, width: 3.0, height: 3.0, min: 3, max: 6, distance: 40.0, size_threshold: 4.0, style: TabStyle::Triangle, positions: Vec::new() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeadKind {
    None,
    Spiral,
    Tangent,
}

/// Mouvement d'entrée ou de sortie.
#[derive(Debug, Clone, PartialEq)]
pub struct Lead {
    pub kind: LeadKind,
    /// Angle de rampe en degrés ; 0 = calculé pour descendre d'un incrément par tour.
    pub spiral_angle: f64,
    pub tangent_radius: f64,
    /// 0 = utiliser l'avance de coupe.
    pub feedrate: f64,
}

impl Default for Lead {
    fn default() -> Self {
        Lead { kind: LeadKind::None, spiral_angle: 0.0, tangent_radius: 0.0, feedrate: 0.0 }
    }
}

/// Sens de fraisage (doc : `Conventional | Climb | Mixed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MillingDirection {
    /// Fraisage en avalant : la paroi usinée est à gauche du sens de parcours pour une broche
    /// horaire (M3), à droite pour une broche anti-horaire (M4).
    Climb,
    /// Fraisage en opposition : l'inverse.
    Conventional,
    /// Le sens géométrique du contour est conservé.
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutOrdering {
    /// Chaque contour est usiné jusqu'à la profondeur cible avant le suivant (défaut des styles).
    DepthFirst,
    /// Tous les contours d'un niveau avant de descendre.
    LevelFirst,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    pub tool_diameter: f64,
    /// Altitude de la surface du brut.
    pub stock_surface: f64,
    /// Profondeur finale (négative sous la surface).
    pub target_depth: f64,
    /// Incrément de passe (0 = une seule passe).
    pub depth_increment: f64,
    pub clearance_plane: f64,
    pub cut_feedrate: f64,
    pub plunge_feedrate: f64,
    pub spindle_speed: f64,
    /// Recouvrement latéral, fraction du diamètre d'outil (0,4 = 40 %). [inféré]
    pub step_over: f64,
    pub tabs: Tabs,
    pub lead_in: Lead,
    pub lead_out: Lead,
    pub cut_ordering: CutOrdering,
    pub milling: MillingDirection,
    /// Broche horaire (M3) ; sinon anti-horaire (M4).
    pub spindle_cw: bool,
    /// Passe supplémentaire dans les coins intérieurs (profils).
    pub corner_overcut: bool,
    /// Distance entre l'anneau de finition (sur la paroi) et le premier anneau d'ébauche, en mm ; 0 = aucune.
    pub finish_stepover: f64,
    /// Si vrai, la finition n'a lieu qu'à la profondeur cible.
    pub finish_at_target_only: bool,
    /// Incrément de la dernière passe en profondeur ; 0 = identique aux autres.
    pub final_depth_increment: f64,
    /// Distance maximale d'une transition horizontale, en fraction du diamètre d'outil (0 = jamais).
    pub max_crossover: f64,
    /// Avance des transitions ; 0 = avance de coupe.
    pub stepover_feedrate: f64,
    /// Matière laissée sur les parois après la coupe finale (négatif = surcoupe). [sens inféré :
    /// la doc dit « stock to leave after the final cut » sans préciser parois/fond ; appliqué aux parois]
    pub roughing_clearance: f64,
    /// Temporisation au fond des perçages (s) ; 0 = aucune. Ignorée avec le picage.
    pub drill_dwell: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            tool_diameter: 3.0,
            stock_surface: 0.0,
            target_depth: -1.0,
            depth_increment: 0.4,
            clearance_plane: 3.0,
            cut_feedrate: 800.0,
            plunge_feedrate: 300.0,
            spindle_speed: 1000.0,
            step_over: 0.4,
            tabs: Tabs::default(),
            lead_in: Lead::default(),
            lead_out: Lead::default(),
            cut_ordering: CutOrdering::DepthFirst,
            milling: MillingDirection::Climb,
            spindle_cw: true,
            corner_overcut: false,
            finish_stepover: 0.0,
            finish_at_target_only: true,
            final_depth_increment: 0.0,
            max_crossover: 0.7,
            stepover_feedrate: 0.0,
            roughing_clearance: 0.0,
            drill_dwell: 0.0,
        }
    }
}

fn tabs_from(style: &CamStyle, d: &Tabs) -> Tabs {
    let n = |k: &str| style.f64(&format!("HoldingTabs.{k}"));
    Tabs {
        method: match style.get("HoldingTabs.TabMethod") {
            Some("Automatic") => TabMethod::Automatic,
            Some("AutomaticOuter") => TabMethod::AutomaticOuter,
            Some("AutomaticInner") => TabMethod::AutomaticInner,
            Some("Manual") => TabMethod::Manual, // positions non lues du style : à fournir via `positions`
            _ => TabMethod::None,
        },
        width: n("Width").unwrap_or(d.width),
        height: n("Height").unwrap_or(d.height),
        min: n("MinimumTabs").map_or(d.min, |v| v.max(0.0) as usize),
        max: n("MaximumTabs").map_or(d.max, |v| v.max(0.0) as usize),
        distance: n("TabDistance").unwrap_or(d.distance),
        size_threshold: n("SizeThreshold").unwrap_or(d.size_threshold),
        style: match style.get("HoldingTabs.TabStyle") {
            Some("Square") => TabStyle::Square,
            Some("Skip") => TabStyle::Skip,
            _ => TabStyle::Triangle,
        },
        positions: d.positions.clone(),
    }
}

fn lead_from(style: &CamStyle, name: &str) -> Lead {
    let n = |k: &str| style.f64(&format!("{name}.{k}"));
    Lead {
        kind: match style.get(&format!("{name}.LeadInType")) {
            Some("Spiral") => LeadKind::Spiral,
            Some("Tangent") => LeadKind::Tangent,
            _ => LeadKind::None,
        },
        spiral_angle: n("SpiralAngle").unwrap_or(0.0),
        tangent_radius: n("TangentRadius").unwrap_or(0.0),
        feedrate: n("LeadInFeedrate").unwrap_or(0.0),
    }
}

impl Params {
    /// Valeurs du style, les clés absentes ou en `Auto` gardent la valeur par défaut.
    pub fn from_style(style: &CamStyle) -> Params {
        let d = Params::default();
        Params {
            tool_diameter: style.f64("ToolDiameter").filter(|v| *v > 0.0).unwrap_or(d.tool_diameter),
            stock_surface: style.f64("StockSurface").unwrap_or(d.stock_surface),
            target_depth: style.f64("TargetDepth").unwrap_or(d.target_depth),
            depth_increment: style.f64("DepthIncrement").unwrap_or(d.depth_increment),
            clearance_plane: style.f64("ClearancePlane").unwrap_or(d.clearance_plane),
            cut_feedrate: style.f64("CutFeedrate").unwrap_or(d.cut_feedrate),
            plunge_feedrate: style.f64("PlungeFeedrate").unwrap_or(d.plunge_feedrate),
            spindle_speed: style.f64("SpindleSpeed").unwrap_or(d.spindle_speed),
            step_over: style.f64("StepOver").filter(|v| *v > 0.0).unwrap_or(d.step_over),
            tabs: tabs_from(style, &d.tabs),
            lead_in: lead_from(style, "LeadInMove"),
            lead_out: lead_from(style, "LeadOutMove"),
            cut_ordering: match style.get("CutOrdering") {
                Some("LevelFirst") => CutOrdering::LevelFirst,
                _ => d.cut_ordering,
            },
            milling: match style.get("MillingDirection") {
                Some("Conventional") => MillingDirection::Conventional,
                Some("Mixed") => MillingDirection::Mixed,
                _ => d.milling,
            },
            spindle_cw: style.get("SpindleDirection").map_or(d.spindle_cw, |v| v != "CCW"),
            corner_overcut: style.get("CornerOvercut").map_or(d.corner_overcut, |v| v.eq_ignore_ascii_case("true")),
            finish_stepover: style.f64("FinishStepover").filter(|v| *v >= 0.0).unwrap_or(d.finish_stepover),
            finish_at_target_only: style.get("FinishStepoverAtTargetDepth").map_or(d.finish_at_target_only, |v| v.eq_ignore_ascii_case("true")),
            final_depth_increment: style.f64("FinalDepthIncrement").filter(|v| *v >= 0.0).unwrap_or(d.final_depth_increment),
            max_crossover: style.f64("MaxCrossoverDistance").filter(|v| *v >= 0.0).unwrap_or(d.max_crossover),
            stepover_feedrate: style.f64("StepoverFeedrate").filter(|v| *v >= 0.0).unwrap_or(d.stepover_feedrate),
            roughing_clearance: style.f64("RoughingClearance").unwrap_or(d.roughing_clearance),
            drill_dwell: d.drill_dwell,
        }
    }

    pub fn tool_radius(&self) -> f64 {
        self.tool_diameter / 2.0
    }

    /// Distance entre la paroi et l'axe de l'outil : rayon + matière laissée.
    pub fn wall_offset(&self) -> f64 {
        self.tool_radius() + self.roughing_clearance
    }

    /// Altitudes de passe, de haut en bas, la dernière étant toujours `target_depth`.
    /// Avec `final_depth_increment` > 0, la dernière passe ne descend que de cette valeur.
    pub fn depth_levels(&self) -> Vec<f64> {
        let (top, bottom) = (self.stock_surface, self.target_depth);
        if bottom > top + 1e-9 {
            return Vec::new();
        }
        if bottom >= top - 1e-9 {
            return vec![bottom]; // profondeur = surface : une seule passe à la surface
        }
        if self.depth_increment <= 0.0 {
            return vec![bottom];
        }
        // avec un incrément final, la dernière passe démarre à `bottom + fdi`
        let fdi = if self.final_depth_increment > 0.0 { self.final_depth_increment.min(top - bottom) } else { 0.0 };
        let floor = bottom + fdi;
        let mut v = Vec::new();
        let mut z = top - self.depth_increment;
        while z > floor + 1e-9 {
            v.push(z);
            z -= self.depth_increment;
        }
        if fdi > 0.0 && floor > bottom + 1e-9 && floor < v.last().copied().unwrap_or(top) - 1e-9 {
            v.push(floor);
        }
        v.push(bottom);
        v
    }
}
