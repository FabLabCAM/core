//! Projet : formes, table d'outils, liste d'opérations et brut. C'est l'état que l'éditeur
//! modifie, que l'historique mémorise et que le `.cb` sauvegarde.

use crate::cad;
use crate::shape::{self, EditError};
use fablabcam_model::{Document, Entity, Layer};

#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub id: u32,
    pub layer: String,
    pub entity: Entity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    EndMill,
    /// Fraise torique : simulée comme une fraise droite (le rayon de coin n'est pas modélisé).
    BullNose,
    BallNose,
    Drill,
    VBit,
}

impl ToolKind {
    pub const ALL: [ToolKind; 5] = [ToolKind::EndMill, ToolKind::BullNose, ToolKind::BallNose, ToolKind::Drill, ToolKind::VBit];
    pub fn label(self) -> &'static str {
        match self {
            ToolKind::EndMill => "Fraise droite",
            ToolKind::BullNose => "Fraise torique",
            ToolKind::BallNose => "Fraise boule",
            ToolKind::Drill => "Foret",
            ToolKind::VBit => "Fraise en V",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            ToolKind::EndMill => "endmill",
            ToolKind::BullNose => "bullnose",
            ToolKind::BallNose => "ballnose",
            ToolKind::Drill => "drill",
            ToolKind::VBit => "vbit",
        }
    }
    pub fn from_key(k: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.key() == k)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    pub name: String,
    /// Numéro d'outil (`T…`, `M6`), unique dans la table ; 0 = attribuer le premier numéro libre.
    pub index: u32,
    pub kind: ToolKind,
    pub diameter: f64,
    pub flutes: u32,
    pub feed: f64,
    pub plunge: f64,
    pub spindle: f64,
    /// Propriétés des outils d'usinage. Les longueurs et angles à 0
    /// sont « non renseignés ». Informatives, sauf `vee_angle` (simulation), `tool_change` et `comment` (G-code).
    pub flute_length: f64,
    pub length: f64,
    pub shank_diameter: f64,
    pub helix_angle: f64,
    pub vee_angle: f64,
    pub max_ramp_angle: f64,
    pub axial_depth: f64,
    pub radial_depth: f64,
    pub tooth_load: f64,
    pub material: String,
    pub coating: String,
    pub part_code: String,
    pub notes: String,
    /// Texte pour la macro `{$tool.comment}` du post-processeur.
    pub comment: String,
    /// Remplace le changement d'outil par défaut pour cet outil uniquement (lignes de G-code).
    pub tool_change: String,
}

impl Tool {
    pub fn new(name: &str, kind: ToolKind, diameter: f64) -> Tool {
        Tool {
            name: name.into(),
            index: 0,
            kind,
            diameter,
            flutes: 2,
            feed: 800.0,
            plunge: 300.0,
            spindle: 12000.0,
            flute_length: 0.0,
            length: 0.0,
            shank_diameter: 0.0,
            helix_angle: 0.0,
            vee_angle: if kind == ToolKind::VBit { 90.0 } else { 0.0 },
            max_ramp_angle: 0.0,
            axial_depth: 0.0,
            radial_depth: 0.0,
            tooth_load: 0.0,
            material: String::new(),
            coating: String::new(),
            part_code: String::new(),
            notes: String::new(),
            comment: String::new(),
            tool_change: String::new(),
        }
    }

    /// Outil construit d'après un style XML (diamètre, avances, broche).
    pub fn from_style(name: &str, lib: &fablabcam_cam::StyleLibrary, style: &str) -> Result<Tool, EditError> {
        let st = lib.resolve(style).ok_or_else(|| EditError::InvalidParameter(format!("style « {style} » introuvable")))?;
        let p = fablabcam_cam::Params::from_style(&st);
        Ok(Tool { feed: p.cut_feedrate, plunge: p.plunge_feedrate, spindle: p.spindle_speed, ..Tool::new(name, ToolKind::EndMill, p.tool_diameter) })
    }

    pub fn validate(&self) -> Result<(), EditError> {
        let bad = |m: &str| Err(EditError::InvalidParameter(format!("outil « {} » : {m}", self.name)));
        if self.name.trim().is_empty() {
            return bad("nom vide");
        }
        if self.diameter.is_nan() || self.diameter <= 0.0 {
            return bad("diamètre doit être > 0");
        }
        if self.feed <= 0.0 || self.plunge <= 0.0 || self.spindle <= 0.0 {
            return bad("avances et vitesse de broche doivent être > 0");
        }
        for (label, v) in [
            ("longueur de coupe", self.flute_length),
            ("longueur", self.length),
            ("diamètre de queue", self.shank_diameter),
            ("profondeur de passe axiale", self.axial_depth),
            ("passe radiale", self.radial_depth),
            ("charge par dent", self.tooth_load),
        ] {
            if v.is_nan() || v < 0.0 {
                return bad(&format!("{label} ne peut pas être négatif(ve)"));
            }
        }
        for (label, v, max) in [("angle d'hélice", self.helix_angle, 90.0), ("angle de rampe maximal", self.max_ramp_angle, 90.0), ("angle du V", self.vee_angle, 180.0)] {
            if v.is_nan() || v < 0.0 || v >= max {
                return bad(&format!("{label} doit être dans [0 ; {max}["));
            }
        }
        if self.kind == ToolKind::VBit && self.vee_angle <= 0.0 {
            return bad("une fraise en V demande un angle de V > 0");
        }
        if self.length > 0.0 && self.flute_length > self.length {
            return bad("la longueur de coupe dépasse la longueur de l'outil");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Profile,
    Pocket,
    Drill,
    /// Gravure : l'axe de l'outil suit le tracé (ouvert ou fermé), sans décalage.
    Engrave,
}

impl OpKind {
    pub const ALL: [OpKind; 4] = [OpKind::Profile, OpKind::Pocket, OpKind::Drill, OpKind::Engrave];
    pub fn label(self) -> &'static str {
        match self {
            OpKind::Profile => "Profil",
            OpKind::Pocket => "Poche",
            OpKind::Drill => "Perçage",
            OpKind::Engrave => "Gravure",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            OpKind::Profile => "profile",
            OpKind::Pocket => "pocket",
            OpKind::Drill => "drill",
            OpKind::Engrave => "engrave",
        }
    }
    pub fn from_key(k: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.key() == k)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpSide {
    Outside,
    Inside,
    On,
}

impl OpSide {
    pub const ALL: [OpSide; 3] = [OpSide::Outside, OpSide::Inside, OpSide::On];
    pub fn label(self) -> &'static str {
        match self {
            OpSide::Outside => "extérieur",
            OpSide::Inside => "intérieur",
            OpSide::On => "sur le trait",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            OpSide::Outside => "outside",
            OpSide::Inside => "inside",
            OpSide::On => "on",
        }
    }
    pub fn from_key(k: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.key() == k)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeadChoice {
    None,
    Spiral,
    Tangent,
}

impl LeadChoice {
    pub const ALL: [LeadChoice; 3] = [LeadChoice::None, LeadChoice::Spiral, LeadChoice::Tangent];
    pub fn label(self) -> &'static str {
        match self {
            LeadChoice::None => "aucune",
            LeadChoice::Spiral => "spirale",
            LeadChoice::Tangent => "tangente",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            LeadChoice::None => "none",
            LeadChoice::Spiral => "spiral",
            LeadChoice::Tangent => "tangent",
        }
    }
    pub fn from_key(k: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.key() == k)
    }
}

/// Forme des ponts de maintien.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabShape {
    Triangle,
    Square,
    /// Carré, franchi en rapide (découpe plasma / laser).
    Skip,
}

impl TabShape {
    pub const ALL: [TabShape; 3] = [TabShape::Triangle, TabShape::Square, TabShape::Skip];
    pub fn label(self) -> &'static str {
        match self {
            TabShape::Triangle => "triangle",
            TabShape::Square => "carré",
            TabShape::Skip => "sauté (rapide)",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            TabShape::Triangle => "Triangle",
            TabShape::Square => "Square",
            TabShape::Skip => "Skip",
        }
    }
    pub fn from_key(k: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.key() == k)
    }
}

/// Placement des ponts (`TabMethod` du logiciel de référence) : automatique, ou manuel aux points posés au clic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabTarget {
    /// Tous les contours.
    All,
    /// Contours extérieurs seulement (profil extérieur).
    Outer,
    /// Contours intérieurs seulement (profil intérieur).
    Inner,
    /// Seulement aux points de `Operation::tab_points` (aucun pont tant qu'aucun n'est posé).
    Manual,
}

impl TabTarget {
    pub const ALL: [TabTarget; 4] = [TabTarget::All, TabTarget::Outer, TabTarget::Inner, TabTarget::Manual];
    pub const AUTO: [TabTarget; 3] = [TabTarget::All, TabTarget::Outer, TabTarget::Inner];
    pub fn label(self) -> &'static str {
        match self {
            TabTarget::All => "automatique",
            TabTarget::Outer => "automatique, extérieurs seulement",
            TabTarget::Inner => "automatique, intérieurs seulement",
            TabTarget::Manual => "manuel (clics sur le dessin)",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            TabTarget::All => "Automatic",
            TabTarget::Outer => "AutomaticOuter",
            TabTarget::Inner => "AutomaticInner",
            TabTarget::Manual => "Manual",
        }
    }
    pub fn from_key(k: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.key() == k)
    }
}

/// Réglages d'usinage d'une opération qui reprennent ceux du moteur CAM. Les valeurs par défaut sont
/// celles de `fablabcam_cam::Params::default()` : un document sans ces réglages produit le même G-code.
#[derive(Debug, Clone, PartialEq)]
pub struct CamSettings {
    /// Plan de dégagement (Z absolu).
    pub clearance: f64,
    /// Surface du brut pour cette opération (Z absolu).
    pub surface: f64,
    /// Tous les contours d'un niveau avant de descendre (sinon : chaque contour jusqu'au fond).
    pub level_first: bool,
    /// Distance de la passe de finition sur la paroi (mm) ; 0 = aucune.
    pub finish_stepover: f64,
    pub finish_at_target_only: bool,
    /// Incrément de la dernière passe (mm) ; 0 = comme les autres.
    pub final_increment: f64,
    pub corner_overcut: bool,
    /// Transition horizontale maximale, en fraction du diamètre d'outil (0 = toujours remonter).
    pub max_crossover: f64,
    /// Broche anti-horaire (M4).
    pub spindle_ccw: bool,
    pub tab_shape: TabShape,
    pub tab_width: f64,
    pub tab_height: f64,
    pub tab_min: u32,
    pub tab_max: u32,
    /// Distance visée entre ponts automatiques (mm) ; 0 = toujours le minimum.
    pub tab_distance: f64,
    pub tab_target: TabTarget,
    /// Périmètre (mm) en dessous duquel un contour ne reçoit pas de pont automatique.
    pub tab_threshold: f64,
    /// Angle de la rampe en spirale (°) ; 0 = une passe par tour.
    pub lead_angle: f64,
    /// Rayon de l'entrée tangente (mm) ; 0 = diamètre de l'outil.
    pub lead_radius: f64,
}

impl Default for CamSettings {
    fn default() -> Self {
        CamSettings {
            clearance: 3.0,
            surface: 0.0,
            level_first: false,
            finish_stepover: 0.0,
            finish_at_target_only: true,
            final_increment: 0.0,
            corner_overcut: false,
            max_crossover: 0.7,
            spindle_ccw: false,
            tab_shape: TabShape::Triangle,
            tab_width: 3.0,
            tab_height: 3.0,
            tab_min: 3,
            tab_max: 6,
            tab_distance: 40.0,
            tab_target: TabTarget::All,
            tab_threshold: 4.0,
            lead_angle: 0.0,
            lead_radius: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    pub id: u32,
    pub name: String,
    pub kind: OpKind,
    pub side: OpSide,
    pub shapes: Vec<u32>,
    pub tool: String,
    /// Profondeur finale (négative sous la surface du brut).
    pub depth: f64,
    pub step_down: f64,
    /// Recouvrement latéral, fraction du diamètre d'outil.
    pub step_over: f64,
    pub climb: bool,
    /// Matière laissée sur les parois (profil, poche) ; négatif = surcoupe.
    pub stock_to_leave: f64,
    /// Script G-code inséré avant / après l'opération (« Custom MOP Header / Footer » du logiciel de référence).
    pub header: String,
    pub footer: String,
    pub tabs: bool,
    /// Points de placement manuel des ponts ; vide = placement automatique.
    pub tab_points: Vec<[f64; 2]>,
    pub lead: LeadChoice,
    pub cam: CamSettings,
    /// Temporisation au fond des perçages (s, G82) ; 0 = aucune. Sans effet avec un incrément (picage).
    pub dwell: f64,
    /// Pièce à laquelle appartient l'opération (regroupement, « CAM Part » du logiciel de référence) ; vide = aucune.
    pub part: String,
    /// Surcharges des valeurs de l'outil.
    pub feed: Option<f64>,
    pub plunge: Option<f64>,
    pub spindle: Option<f64>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stock {
    pub min: [f64; 2],
    pub max: [f64; 2],
    /// Hauteur de la face supérieure.
    pub top: f64,
    /// Hauteur de la face inférieure (point le plus bas usiné).
    pub bottom: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Project {
    /// Explicit machine snapshot for furniture CAM; persisted as a FabLabCAM XML extension.
    pub machine_profile: Option<fablabcam_cam::gcode::machine::Profile>,
    pub shapes: Vec<Shape>,
    pub tools: Vec<Tool>,
    pub ops: Vec<Operation>,
    pub stock: Option<Stock>,
    /// G-code écrit en pouces (G20). Le dessin reste en millimètres.
    pub gcode_inches: bool,
    next_id: u32,
}

impl Project {
    pub fn new() -> Self {
        Project { next_id: 1, ..Default::default() }
    }

    fn fresh_id(&mut self) -> u32 {
        self.next_id = self.next_id.max(1);
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Réserve les identifiants déjà utilisés (lecture d'un fichier).
    pub fn reserve_ids(&mut self) {
        let m = self.shapes.iter().map(|s| s.id).chain(self.ops.iter().map(|o| o.id)).max().unwrap_or(0);
        self.next_id = self.next_id.max(m + 1);
    }

    // ---- formes ----
    pub fn add_shape(&mut self, layer: &str, entity: Entity) -> Result<u32, EditError> {
        if let Entity::Circle { r, .. } = &entity {
            if *r <= 0.0 {
                return Err(EditError::Degenerate("cercle de rayon nul"));
            }
        }
        let id = self.fresh_id();
        self.shapes.push(Shape { id, layer: layer.to_string(), entity });
        Ok(id)
    }

    pub fn shape(&self, id: u32) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.id == id)
    }

    pub fn move_shapes(&mut self, ids: &[u32], dx: f64, dy: f64) -> Result<(), EditError> {
        for id in ids {
            if self.shape(*id).is_none() {
                return Err(EditError::UnknownShape(*id));
            }
        }
        for s in self.shapes.iter_mut().filter(|s| ids.contains(&s.id)) {
            s.entity = shape::translate(&s.entity, dx, dy);
        }
        // les ponts manuels suivent les formes de leur opération
        for o in self.ops.iter_mut().filter(|o| o.shapes.iter().any(|i| ids.contains(i))) {
            for t in &mut o.tab_points {
                *t = [t[0] + dx, t[1] + dy];
            }
        }
        Ok(())
    }

    fn check_ids(&self, ids: &[u32]) -> Result<(), EditError> {
        match ids.iter().find(|id| self.shape(**id).is_none()) {
            Some(id) => Err(EditError::UnknownShape(*id)),
            None if ids.is_empty() => Err(EditError::InvalidParameter("aucune forme sélectionnée".into())),
            None => Ok(()),
        }
    }

    fn layer_of(&self, id: u32) -> String {
        self.shape(id).map_or_else(|| "0".to_string(), |s| s.layer.clone())
    }

    /// Remplace la géométrie d'une forme (édition d'un sommet, congés…).
    pub fn replace_shape(&mut self, id: u32, entity: Entity) -> Result<(), EditError> {
        let s = self.shapes.iter_mut().find(|s| s.id == id).ok_or(EditError::UnknownShape(id))?;
        s.entity = entity;
        Ok(())
    }

    /// Modifie la géométrie d'une forme avec une fonction de `edit` ; refus = forme inchangée. Les ponts manuels
    /// restent à leurs coordonnées.
    pub fn edit_shape(&mut self, id: u32, f: impl FnOnce(&Entity) -> Result<Entity, EditError>) -> Result<(), EditError> {
        let new = f(&self.shape(id).ok_or(EditError::UnknownShape(id))?.entity)?;
        self.replace_shape(id, new)
    }

    /// Applique une similitude (rotation, échelle, miroir) aux formes ; les ponts manuels de leurs
    /// opérations suivent, comme pour un déplacement.
    pub fn transform_shapes(&mut self, ids: &[u32], t: &cad::Similarity) -> Result<(), EditError> {
        self.check_ids(ids)?;
        for s in self.shapes.iter_mut().filter(|s| ids.contains(&s.id)) {
            s.entity = cad::transform(&s.entity, t);
        }
        for o in self.ops.iter_mut().filter(|o| o.shapes.iter().any(|i| ids.contains(i))) {
            for p in &mut o.tab_points {
                let q = t.apply((p[0], p[1]));
                *p = [q.0, q.1];
            }
        }
        Ok(())
    }

    /// Copies transformées des formes (même calque) ; renvoie les nouveaux identifiants.
    pub fn copy_shapes(&mut self, ids: &[u32], t: &cad::Similarity) -> Result<Vec<u32>, EditError> {
        self.check_ids(ids)?;
        let src: Vec<(String, Entity)> = ids.iter().filter_map(|id| self.shape(*id)).map(|s| (s.layer.clone(), cad::transform(&s.entity, t))).collect();
        src.into_iter().map(|(l, e)| self.add_shape(&l, e)).collect()
    }

    /// Réseau rectangulaire de `nx` × `ny` exemplaires (l'original compris) au pas (`dx`, `dy`).
    pub fn array_rect(&mut self, ids: &[u32], nx: u32, ny: u32, dx: f64, dy: f64) -> Result<Vec<u32>, EditError> {
        if nx == 0 || ny == 0 || nx * ny > 10_000 || !(dx.is_finite() && dy.is_finite()) {
            return Err(EditError::InvalidParameter("réseau : nombres de 1 à 10 000 exemplaires, pas numériques".into()));
        }
        let mut out = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                if i + j > 0 {
                    out.extend(self.copy_shapes(ids, &cad::Similarity::translation(i as f64 * dx, j as f64 * dy))?);
                }
            }
        }
        Ok(out)
    }

    /// Réseau circulaire de `n` exemplaires autour de `center`, répartis sur `total_deg` (360 = tour complet).
    pub fn array_polar(&mut self, ids: &[u32], center: (f64, f64), n: u32, total_deg: f64) -> Result<Vec<u32>, EditError> {
        if !(2..=10_000).contains(&n) || !total_deg.is_finite() || total_deg.abs() < 1e-9 {
            return Err(EditError::InvalidParameter("réseau circulaire : 2 à 10 000 exemplaires, angle non nul".into()));
        }
        let full = (total_deg.abs() - 360.0).abs() < 1e-9;
        let step = if full { total_deg / n as f64 } else { total_deg / (n - 1) as f64 };
        let mut out = Vec::new();
        for k in 1..n {
            out.extend(self.copy_shapes(ids, &cad::Similarity::rotation(center, step * k as f64))?);
        }
        Ok(out)
    }

    /// Ajoute les décalages des formes (les originales restent).
    pub fn offset_shapes(&mut self, ids: &[u32], d: f64) -> Result<Vec<u32>, EditError> {
        self.check_ids(ids)?;
        let mut made = Vec::new();
        for id in ids {
            let s = self.shape(*id).ok_or(EditError::UnknownShape(*id))?;
            made.extend(cad::offset(&s.entity, d)?.into_iter().map(|e| (s.layer.clone(), e)));
        }
        made.into_iter().map(|(l, e)| self.add_shape(&l, e)).collect()
    }

    /// Ajoute le décalage double (contour fermé autour du tracé) de chaque forme ouverte.
    pub fn open_offset_shapes(&mut self, ids: &[u32], d: f64) -> Result<Vec<u32>, EditError> {
        self.check_ids(ids)?;
        let mut made = Vec::new();
        for id in ids {
            let s = self.shape(*id).ok_or(EditError::UnknownShape(*id))?;
            made.push((s.layer.clone(), cad::open_offset(&s.entity, d)?));
        }
        made.into_iter().map(|(l, e)| self.add_shape(&l, e)).collect()
    }

    /// Ajoute `n` points répartis sur chaque forme (centres de perçage d'un cercle de trous, par exemple).
    pub fn divide_shapes(&mut self, ids: &[u32], n: usize) -> Result<Vec<u32>, EditError> {
        self.check_ids(ids)?;
        let mut made = Vec::new();
        for id in ids {
            let s = self.shape(*id).ok_or(EditError::UnknownShape(*id))?;
            made.extend(cad::divide(&s.entity, n)?.into_iter().map(|e| (s.layer.clone(), e)));
        }
        made.into_iter().map(|(l, e)| self.add_shape(&l, e)).collect()
    }

    /// Remplace les formes ouvertes jointives par des chaînes (fermées si elles bouclent).
    pub fn join_shapes(&mut self, ids: &[u32], tol: f64) -> Result<Vec<u32>, EditError> {
        self.check_ids(ids)?;
        let ents: Vec<Entity> = ids.iter().filter_map(|id| self.shape(*id)).map(|s| s.entity.clone()).collect();
        let chains = cad::join(&ents, tol)?;
        let layer = self.layer_of(ids[0]);
        self.delete_shapes(ids);
        chains.into_iter().map(|e| self.add_shape(&layer, e)).collect()
    }

    /// Décompose les polylignes (et cercles) en segments.
    pub fn explode_shapes(&mut self, ids: &[u32]) -> Result<Vec<u32>, EditError> {
        self.check_ids(ids)?;
        let parts: Vec<(String, Vec<Entity>)> = ids.iter().filter_map(|id| self.shape(*id)).map(|s| (s.layer.clone(), cad::explode(&s.entity))).collect();
        self.delete_shapes(ids);
        let mut out = Vec::new();
        for (l, es) in parts {
            for e in es {
                out.push(self.add_shape(&l, e)?);
            }
        }
        Ok(out)
    }

    /// Remplace les deux formes fermées `a` et `b` par le résultat de l'opération booléenne.
    pub fn boolean_shapes(&mut self, a: u32, b: u32, op: cad::BoolOp) -> Result<Vec<u32>, EditError> {
        self.check_ids(&[a, b])?;
        let (ea, eb) = (self.shape(a).map(|s| s.entity.clone()), self.shape(b).map(|s| s.entity.clone()));
        let res = cad::boolean(&ea.ok_or(EditError::UnknownShape(a))?, &eb.ok_or(EditError::UnknownShape(b))?, op)?;
        let layer = self.layer_of(a);
        self.delete_shapes(&[a, b]);
        res.into_iter().map(|e| self.add_shape(&layer, e)).collect()
    }

    /// Arrondit les coins des polylignes ; renvoie le nombre de coins arrondis.
    pub fn fillet_shapes(&mut self, ids: &[u32], r: f64) -> Result<usize, EditError> {
        self.check_ids(ids)?;
        let mut total = 0;
        for id in ids {
            let e = self.shape(*id).map(|s| s.entity.clone()).ok_or(EditError::UnknownShape(*id))?;
            if matches!(e, Entity::Polyline(_) | Entity::Rect { .. }) {
                let (ne, n) = cad::fillet(&e, r)?;
                if n > 0 {
                    self.replace_shape(*id, ne)?;
                    total += n;
                }
            }
        }
        if total == 0 {
            return Err(EditError::InvalidParameter("aucun coin arrondi : coins entre segments droits, assez longs pour le rayon".into()));
        }
        Ok(total)
    }

    /// Active ou désactive toutes les opérations d'une pièce.
    pub fn set_part_enabled(&mut self, part: &str, enabled: bool) -> usize {
        let mut n = 0;
        for o in self.ops.iter_mut().filter(|o| o.part == part) {
            o.enabled = enabled;
            n += 1;
        }
        n
    }

    /// Pièces dans l'ordre de première apparition (sans la pièce vide).
    pub fn parts(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for o in &self.ops {
            if !o.part.is_empty() && !v.contains(&o.part) {
                v.push(o.part.clone());
            }
        }
        v
    }

    /// Point du contour d'une opération le plus proche de `pt`, s'il est à moins de `tol` + rayon de l'outil
    /// (on peut cliquer sur la forme ou sur la trajectoire, décalée d'un rayon).
    pub fn contour_point(&self, op: u32, pt: [f64; 2], tol: f64) -> Option<[f64; 2]> {
        let o = self.op(op)?;
        let r = self.tool(&o.tool).map_or(0.0, |t| t.diameter / 2.0);
        o.shapes
            .iter()
            .filter_map(|id| self.shape(*id))
            .filter_map(|s| shape::pline(&s.entity))
            .filter_map(|p| fablabcam_geom::closest_point(&p, (pt[0], pt[1])))
            .filter(|(_, d, _)| *d <= tol + r)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(q, _, _)| [q.0, q.1])
    }

    /// Clic de pose des ponts manuels : retire le pont à moins de `radius`, sinon en pose un sur le contour
    /// de l'opération (point projeté). Renvoie `Some(true)` posé, `Some(false)` retiré, `None` si le clic est
    /// trop loin du contour (rien ne change).
    pub fn click_tab(&mut self, op: u32, pt: [f64; 2], radius: f64, tol: f64) -> Result<Option<bool>, EditError> {
        let o = self.op(op).ok_or(EditError::UnknownOperation(op))?;
        let near = o.tab_points.iter().any(|t| (t[0] - pt[0]).hypot(t[1] - pt[1]) <= radius);
        if near {
            return self.toggle_tab_point(op, pt, radius).map(Some);
        }
        match self.contour_point(op, pt, tol) {
            Some(q) => self.toggle_tab_point(op, q, 0.0).map(Some),
            None => Ok(None),
        }
    }

    /// Pose un pont manuel en `pt`, ou retire celui qui en est à moins de `radius`.
    /// Renvoie `true` si un pont a été posé.
    pub fn toggle_tab_point(&mut self, op: u32, pt: [f64; 2], radius: f64) -> Result<bool, EditError> {
        let o = self.ops.iter_mut().find(|o| o.id == op).ok_or(EditError::UnknownOperation(op))?;
        let d = |t: &[f64; 2]| (t[0] - pt[0]).hypot(t[1] - pt[1]);
        let near = o.tab_points.iter().enumerate().filter(|(_, t)| d(t) <= radius).min_by(|a, b| d(a.1).total_cmp(&d(b.1))).map(|(i, _)| i);
        match near {
            Some(i) => {
                o.tab_points.remove(i);
                Ok(false)
            }
            None => {
                o.tab_points.push(pt);
                Ok(true)
            }
        }
    }

    /// Fixe le brut (`None` = calculé d'après les trajectoires).
    pub fn set_stock(&mut self, stock: Option<Stock>) -> Result<(), EditError> {
        if let Some(s) = stock {
            let bad = |m: &str| Err(EditError::InvalidParameter(format!("brut : {m}")));
            if [s.min[0], s.min[1], s.max[0], s.max[1], s.top, s.bottom].iter().any(|v| !v.is_finite()) {
                return bad("valeur non numérique");
            }
            if s.max[0] <= s.min[0] || s.max[1] <= s.min[1] {
                return bad("le coin maxi doit être au-delà du coin mini en X et en Y");
            }
            if s.top <= s.bottom {
                return bad("le dessus doit être au-dessus du dessous");
            }
        }
        self.stock = stock;
        Ok(())
    }

    /// Supprime des formes ; renvoie le nom des opérations qui les référençaient.
    pub fn delete_shapes(&mut self, ids: &[u32]) -> Vec<String> {
        self.shapes.retain(|s| !ids.contains(&s.id));
        let mut touched = Vec::new();
        for o in &mut self.ops {
            let before = o.shapes.len();
            o.shapes.retain(|i| !ids.contains(i));
            if o.shapes.len() != before {
                touched.push(o.name.clone());
            }
        }
        touched
    }

    // ---- outils ----
    pub fn tool(&self, name: &str) -> Option<&Tool> {
        self.tools.iter().find(|t| t.name == name)
    }

    pub fn add_tool(&mut self, t: Tool) -> Result<(), EditError> {
        let mut t = t;
        t.validate()?;
        if self.tool(&t.name).is_some() {
            return Err(EditError::DuplicateName(t.name));
        }
        if t.index == 0 {
            t.index = (1..).find(|i| !self.tools.iter().any(|x| x.index == *i)).unwrap_or(1);
        }
        self.check_index(&t, None)?;
        self.tools.push(t);
        Ok(())
    }

    /// Le numéro d'outil doit être unique (`skip` : outil remplacé, ignoré).
    fn check_index(&self, t: &Tool, skip: Option<usize>) -> Result<(), EditError> {
        match self.tools.iter().enumerate().find(|(i, x)| Some(*i) != skip && x.index == t.index) {
            Some((_, other)) => Err(EditError::InvalidParameter(format!("numéro d'outil {} déjà utilisé par « {} »", t.index, other.name))),
            None => Ok(()),
        }
    }

    /// Numéro d'outil (`T…`) d'un outil de la table ; 0 s'il n'existe pas.
    pub fn tool_number(&self, name: &str) -> u32 {
        self.tool(name).map_or(0, |t| t.index)
    }

    /// Remplace l'outil `name` (renommage possible : les opérations suivent).
    pub fn update_tool(&mut self, name: &str, new: Tool) -> Result<(), EditError> {
        let mut new = new;
        new.validate()?;
        let i = self.tools.iter().position(|t| t.name == name).ok_or_else(|| EditError::UnknownTool(name.into()))?;
        if new.index == 0 {
            new.index = self.tools[i].index;
        }
        self.check_index(&new, Some(i))?;
        if new.name != name && self.tool(&new.name).is_some() {
            return Err(EditError::DuplicateName(new.name));
        }
        if new.name != name {
            for o in self.ops.iter_mut().filter(|o| o.tool == name) {
                o.tool = new.name.clone();
            }
        }
        self.tools[i] = new;
        Ok(())
    }

    /// Supprime un outil ; s'il est utilisé il faut un outil de remplacement.
    pub fn delete_tool(&mut self, name: &str, replacement: Option<&str>) -> Result<(), EditError> {
        if self.tool(name).is_none() {
            return Err(EditError::UnknownTool(name.into()));
        }
        if let Some(o) = self.ops.iter().find(|o| o.tool == name) {
            let Some(r) = replacement else {
                return Err(EditError::ToolInUse { tool: name.into(), operation: o.name.clone() });
            };
            if r == name || self.tool(r).is_none() {
                return Err(EditError::UnknownTool(r.into()));
            }
            for o in self.ops.iter_mut().filter(|o| o.tool == name) {
                o.tool = r.to_string();
            }
        }
        self.tools.retain(|t| t.name != name);
        Ok(())
    }

    // ---- opérations ----
    pub fn op(&self, id: u32) -> Option<&Operation> {
        self.ops.iter().find(|o| o.id == id)
    }

    pub fn add_operation(&mut self, kind: OpKind, name: &str, shapes: Vec<u32>, tool: &str) -> Result<u32, EditError> {
        if self.tool(tool).is_none() {
            return Err(EditError::UnknownTool(tool.into()));
        }
        if self.ops.iter().any(|o| o.name == name) {
            return Err(EditError::DuplicateName(name.into()));
        }
        for s in &shapes {
            if self.shape(*s).is_none() {
                return Err(EditError::UnknownShape(*s));
            }
        }
        let id = self.fresh_id();
        self.ops.push(Operation {
            id,
            name: name.into(),
            kind,
            side: if kind == OpKind::Profile { OpSide::Outside } else { OpSide::Inside },
            shapes,
            tool: tool.into(),
            depth: -1.0,
            step_down: 0.5,
            step_over: 0.4,
            climb: true,
            stock_to_leave: 0.0,
            header: String::new(),
            footer: String::new(),
            tabs: false,
            tab_points: Vec::new(),
            lead: LeadChoice::None,
            cam: CamSettings::default(),
            dwell: 0.0,
            part: String::new(),
            feed: None,
            plunge: None,
            spindle: None,
            enabled: true,
        });
        Ok(id)
    }

    pub fn update_operation(&mut self, id: u32, f: impl FnOnce(&mut Operation)) -> Result<(), EditError> {
        let i = self.ops.iter().position(|o| o.id == id).ok_or(EditError::UnknownOperation(id))?;
        let old_name = self.ops[i].name.clone();
        let mut o = self.ops[i].clone();
        f(&mut o);
        if o.name != old_name && self.ops.iter().any(|x| x.name == o.name) {
            return Err(EditError::DuplicateName(o.name));
        }
        if self.tool(&o.tool).is_none() {
            return Err(EditError::UnknownTool(o.tool));
        }
        self.ops[i] = o;
        Ok(())
    }

    pub fn delete_operation(&mut self, id: u32) -> Result<(), EditError> {
        let n = self.ops.len();
        self.ops.retain(|o| o.id != id);
        if self.ops.len() == n { Err(EditError::UnknownOperation(id)) } else { Ok(()) }
    }

    pub fn duplicate_operation(&mut self, id: u32) -> Result<u32, EditError> {
        let src = self.op(id).ok_or(EditError::UnknownOperation(id))?.clone();
        let mut name = format!("{} (copie)", src.name);
        let mut k = 2;
        while self.ops.iter().any(|o| o.name == name) {
            name = format!("{} (copie {k})", src.name);
            k += 1;
        }
        let new_id = self.fresh_id();
        let pos = self.ops.iter().position(|o| o.id == id).map_or(self.ops.len(), |p| p + 1);
        self.ops.insert(pos, Operation { id: new_id, name, ..src });
        Ok(new_id)
    }

    /// Déplace l'opération à la position `to` (0 = première).
    pub fn move_operation(&mut self, id: u32, to: usize) -> Result<(), EditError> {
        let from = self.ops.iter().position(|o| o.id == id).ok_or(EditError::UnknownOperation(id))?;
        let o = self.ops.remove(from);
        self.ops.insert(to.min(self.ops.len()), o);
        Ok(())
    }

    // ---- brut et document ----
    pub fn bounds(&self) -> Option<([f64; 2], [f64; 2])> {
        let mut r: Option<([f64; 2], [f64; 2])> = None;
        for s in &self.shapes {
            if let Some((lo, hi)) = shape::bounds(&s.entity) {
                r = Some(match r {
                    None => (lo, hi),
                    Some((a, b)) => ([a[0].min(lo[0]), a[1].min(lo[1])], [b[0].max(hi[0]), b[1].max(hi[1])]),
                });
            }
        }
        r
    }

    pub fn to_document(&self) -> Document {
        let mut doc = Document::default();
        for s in &self.shapes {
            if !doc.layers.iter().any(|l| l.name == s.layer) {
                doc.layers.push(Layer { name: s.layer.clone(), ..Layer::default() });
            }
            if let Some(l) = doc.layers.iter_mut().find(|l| l.name == s.layer) {
                l.entities.push(s.entity.clone());
            }
        }
        doc
    }

    pub fn from_document(doc: &Document) -> Project {
        let mut p = Project::new();
        for l in &doc.layers {
            for e in &l.entities {
                p.shapes.push(Shape { id: p.next_id, layer: l.name.clone(), entity: e.clone() });
                p.next_id += 1;
            }
        }
        p
    }
}
