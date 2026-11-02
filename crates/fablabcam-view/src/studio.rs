//! FabLabCAM Studio : éditeur 2D (formes, sélection, annuler/rétablir, import/export).
//! Les panneaux Outils, Opérations et 3D s'ajoutent aux jalons suivants.

use fablabcam_backplot::{build_scene, Prim, ViewOptions};
use fablabcam_studio::sim::{self, SimOutput};
use fablabcam_studio::view3d::{sorted_projected, surface_triangles, Camera3};
use fablabcam_studio::cad::{self, Similarity};
use fablabcam_studio::edit::{self, Handle};
use fablabcam_studio::{dxf, persist, shape, svgio, LeadChoice, OpKind, OpSide, Operation, Project, Studio, TabShape, TabTarget, Tool, ToolKind};
use fablabcam_view::view_model::Camera;
use eframe::egui::{self, Color32, Key, Pos2, Sense, Stroke};
use std::path::{Path, PathBuf};

mod desktop;
use desktop::Prefs;

const HELP: &str = "fablabcam-studio-ui — éditeur 2D FabLabCAM

USAGE  fablabcam-studio-ui [fichier.cb|.dxf|.svg] [options]
  --demo-dessin     dessin d'exemple (plusieurs types de formes)
  --select <a,b>    sélectionne les formes de rang a, b… (1 = première)
  --smoke <n>       dessine n images puis quitte (test de bon fonctionnement)
";

/// Action différée jusqu'à la réponse « modifications non enregistrées ».
#[derive(Clone)]
enum Pending {
    New,
    Open(PathBuf),
    Quit,
}

/// Ce qu'il faut faire du chemin choisi dans un dialogue (ouvert dans un fil à part).
enum Purpose {
    Open,
    /// Enregistrement du projet, puis action en attente éventuelle (nouveau, ouvrir, quitter).
    SaveProject(Option<Pending>),
    ExportDrawing(&'static str),
    ExportGcode,
    PickPost,
    LoadScript,
}

type DialogResult = Result<Option<PathBuf>, String>;

/// Commandes des menus et des raccourcis clavier.
#[derive(Clone)]
enum Action {
    New,
    Open,
    OpenRecent(PathBuf),
    Save,
    SaveAs,
    Import,
    ExportDxf,
    ExportSvg,
    ExportGcode,
    Prefs,
    Script,
    Undo,
    Redo,
    Delete,
    SelectAll,
    Cut,
    Copy,
    Paste,
    View(View),
    Fit,
    ToggleToolpaths,
    ToggleConsole,
    Tool(Draw),
    NewOp(OpKind),
    PickPost,
    System,
    Help,
}

/// Valeurs saisies dans « Modifier la sélection ».
#[derive(Clone)]
struct CadParams {
    dx: f64,
    dy: f64,
    angle: f64,
    scale: f64,
    nx: u32,
    ny: u32,
    adx: f64,
    ady: f64,
    pn: u32,
    pangle: f64,
    offset: f64,
    fillet: f64,
}

impl Default for CadParams {
    fn default() -> Self {
        CadParams { dx: 10.0, dy: 0.0, angle: 90.0, scale: 2.0, nx: 3, ny: 1, adx: 20.0, ady: 20.0, pn: 6, pangle: 360.0, offset: 2.0, fillet: 2.0 }
    }
}

#[derive(Clone, Copy)]
enum CadAct {
    Move,
    Copy,
    Rotate,
    Scale,
    /// Miroir d'axe vertical (gauche ↔ droite) si vrai, horizontal sinon.
    Mirror(bool),
    ArrayRect,
    ArrayPolar,
    Offset,
    Fillet,
    Join,
    OpenOffset,
    Divide,
    Explode,
    Bool(cad::BoolOp),
}

/// Vue centrale : plan 2D, résultat 3D, ou simulation pas à pas.
#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Plan,
    Vue3D,
    Simulation,
}

/// Onglet du panneau de gauche.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SideTab {
    Projet,
    Systeme,
}

/// Objet montré dans la grille de propriétés.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Project,
    Shapes,
    Op,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Draw {
    Select,
    Line,
    Polyline,
    Rect,
    Circle,
    Arc,
    Text,
}

impl Draw {
    const ALL: [(Draw, &'static str); 7] = [
        (Draw::Select, "Sélection"),
        (Draw::Line, "Ligne"),
        (Draw::Polyline, "Polyligne"),
        (Draw::Rect, "Rectangle"),
        (Draw::Circle, "Cercle"),
        (Draw::Arc, "Arc"),
        (Draw::Text, "Texte"),
    ];
    /// Nombre de points qui terminent la forme (None = polyligne, terminée à la demande).
    fn clicks(self) -> Option<usize> {
        match self {
            Draw::Select => Some(0),
            Draw::Line | Draw::Rect | Draw::Circle => Some(2),
            Draw::Arc => Some(3),
            Draw::Text => Some(1),
            Draw::Polyline => None,
        }
    }
    fn hint(self) -> &'static str {
        match self {
            Draw::Select => "clic : sélectionner (Maj : ajouter) · glisser : déplacer · Suppr : effacer",
            Draw::Line => "clic 1 : début · clic 2 : fin",
            Draw::Polyline => "clics : sommets · Entrée / clic droit : terminer · C : fermer",
            Draw::Rect => "clic 1 : coin · clic 2 : coin opposé",
            Draw::Circle => "clic 1 : centre · clic 2 : point du cercle",
            Draw::Arc => "clic 1 : centre · clic 2 : début · clic 3 : fin (sens anti-horaire)",
            Draw::Text => "clic : début de la ligne de base du texte",
        }
    }
}

struct App {
    studio: Studio,
    tool: Draw,
    selection: Vec<u32>,
    hover: Option<u32>,
    cam: Camera,
    fit_pending: bool,
    pts: Vec<[f64; 2]>,
    move_start: Option<[f64; 2]>,
    move_delta: [f64; 2],
    snap: bool,
    layer: String,
    path: String,
    status: String,
    smoke: Option<u32>,
    frame: u32,
    view: View,
    side: SideTab,
    focus: Focus,
    show_toolpaths: bool,
    show_console: bool,
    show_help: bool,
    /// Journal des messages (console sous la vue).
    console: Vec<String>,
    logged_status: String,
    /// Sélection déjà décrite dans la console.
    logged_sel: Vec<u32>,
    /// Formes copiées (presse-papiers interne) : (calque, géométrie).
    clipboard: Vec<(String, fablabcam_model::Entity)>,
    hidden_layers: Vec<String>,
    /// Position du pointeur dans le dessin (barre d'état).
    cursor_w: Option<[f64; 2]>,
    /// Bibliothèque d'outils réutilisable (onglet Système).
    library: Vec<Tool>,
    lib_loaded: bool,
    sel_tool: Option<String>,
    tool_draft: Option<(Option<String>, Tool)>,
    replacement: String,
    styles_path: String,
    style_name: String,
    sel_op: Option<u32>,
    op_draft: Option<Operation>,
    /// Clic sur le canvas = poser/retirer un pont manuel de l'opération sélectionnée.
    tab_mode: bool,
    post_path: String,
    stock_draft: Option<fablabcam_studio::Stock>,
    new_kind: OpKind,
    scene: (u64, Vec<Prim>),
    cam3: Option<Camera3>,
    sim_frac: f64,
    sim_res: f64,
    playing: bool,
    show_path: bool,
    sim: Option<(u64, f64, f64, SimOutput)>,
    paths: (u64, Vec<(u32, fablabcam_backplot::Backplot)>),
    /// Fichier `.cb` du projet ouvert (`None` = jamais enregistré).
    doc_path: Option<PathBuf>,
    /// Projet tel qu'enregistré, pour détecter les modifications.
    saved: Project,
    prefs: Prefs,
    show_prefs: bool,
    confirm: Option<Pending>,
    allow_close: bool,
    title: String,
    /// Dialogue de fichier en cours et canal de sa réponse.
    dialog: Option<(Purpose, std::sync::mpsc::Receiver<DialogResult>)>,
    cad: CadParams,
    calc_mat: usize,
    calc_vc: f64,
    calc_fz: f64,
    calc_max_rpm: f64,
    show_script: bool,
    script_code: String,
    script_log: String,
    text_value: String,
    text_height: f64,
    text_spacing: f64,
    /// Police `.ttf` / `.otf` ; vide = police système par défaut.
    text_font: String,
    /// Fichiers déjà chargés depuis la ligne de commande : macOS les renvoie aussi comme documents à ouvrir.
    argv_files: Vec<PathBuf>,
    /// Sommet en cours de déplacement : (forme, indice, position courante).
    vtx_drag: Option<(u32, Handle, [f64; 2])>,
    /// Valeurs en cours de saisie dans le panneau « Objet » : (forme, révision du document, valeurs, sommets).
    prop_draft: Option<PropDraft>,
}

/// Modification d'une forme prête à être appliquée.
type EntityEdit = Box<dyn FnOnce(&fablabcam_model::Entity) -> Result<fablabcam_model::Entity, shape::EditError>>;

struct PropDraft {
    id: u32,
    rev: u64,
    vals: [f64; 6],
    verts: Vec<(f64, f64, f64)>,
}

/// Chemin complet d'un fichier écrit dans le dossier courant (pour les messages d'état).
fn absolute(name: &Path) -> String {
    std::env::current_dir().map_or_else(|_| name.display().to_string(), |d| d.join(name).display().to_string())
}

/// Chemin saisi à la main (champ de secours). Lancée depuis le Finder, l'application n'a pas de dossier
/// courant utile (« / ») : un nom relatif vise alors `~/Documents`. On ne s'y place pas au démarrage, pour que
/// macOS ne demande l'accès à Documents que si l'on s'en sert.
fn resolve(path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    let from_finder = std::env::current_dir().map_or(true, |d| d == Path::new("/"));
    match (p.is_relative() && from_finder, std::env::var_os("HOME")) {
        (true, Some(home)) => Path::new(&home).join("Documents").join(p),
        _ => p,
    }
}

/// Entrée de menu avec son raccourci ; ferme le menu quand elle est choisie.
fn menu_item(ui: &mut egui::Ui, label: &str, keys: &str) -> bool {
    let clicked = ui.add(egui::Button::new(label).shortcut_text(keys)).clicked();
    if clicked {
        ui.close_menu();
    }
    clicked
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn layer_color(name: &str) -> Color32 {
    const PAL: [[u8; 3]; 6] = [[40, 90, 200], [30, 140, 90], [150, 60, 170], [100, 100, 110], [20, 150, 170], [180, 50, 80]];
    let h = name.bytes().fold(7usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
    rgb(PAL[h % PAL.len()])
}

/// Calque courant d'un projet ouvert : celui de sa première forme (un projet XML importé n'a pas de
/// calque « 0 »), « 0 » pour un projet vide.
fn first_layer(p: &Project) -> String {
    p.shapes.first().map_or_else(|| "0".to_string(), |s| s.layer.clone())
}

impl App {
    fn new(project: Project, smoke: Option<u32>) -> App {
        let saved = project.clone();
        let prefs = Prefs::load();
        let layer = first_layer(&project);
        App {
            studio: Studio::new(project),
            tool: Draw::Select,
            selection: Vec::new(),
            hover: None,
            cam: Camera { center: [0.0, 0.0], scale: 4.0 },
            fit_pending: true,
            pts: Vec::new(),
            move_start: None,
            move_delta: [0.0, 0.0],
            snap: prefs.snap,
            layer,
            path: String::new(),
            status: String::new(),
            smoke,
            frame: 0,
            view: View::Plan,
            side: SideTab::Projet,
            focus: Focus::Project,
            show_toolpaths: true,
            show_console: true,
            show_help: false,
            console: Vec::new(),
            logged_status: String::new(),
            logged_sel: Vec::new(),
            clipboard: Vec::new(),
            hidden_layers: Vec::new(),
            cursor_w: None,
            library: Vec::new(),
            lib_loaded: false,
            sel_tool: None,
            tool_draft: None,
            replacement: String::new(),
            styles_path: String::new(),
            style_name: String::new(),
            sel_op: None,
            op_draft: None,
            tab_mode: false,
            post_path: prefs.post_path.clone(),
            stock_draft: None,
            new_kind: OpKind::Pocket,
            scene: (u64::MAX, Vec::new()),
            cam3: None,
            sim_frac: 1.0,
            sim_res: prefs.sim_res,
            playing: false,
            show_path: true,
            sim: None,
            paths: (u64::MAX, Vec::new()),
            doc_path: None,
            saved,
            prefs,
            show_prefs: false,
            confirm: None,
            allow_close: false,
            title: String::new(),
            dialog: None,
            cad: CadParams::default(),
            calc_mat: 0,
            calc_vc: fablabcam_studio::feeds::MATERIALS[0].vc,
            calc_fz: 0.1,
            calc_max_rpm: 24_000.0,
            show_script: false,
            script_code: "// exemple : plaque percée aux quatre coins\nlayer(\"Plaque\");\nlet p = rect(0, 0, 100, 60);\nlet trous = [];\nfor x in [10, 90] { for y in [10, 50] { trous.push(circle(x, y, 1.5)); } }\nprint(`${shapes().len()} formes`);\n".into(),
            script_log: String::new(),
            text_value: "FabLabCAM".into(),
            text_height: 10.0,
            text_spacing: 0.0,
            text_font: String::new(),
            argv_files: Vec::new(),
            vtx_drag: None,
            prop_draft: None,
        }
    }

    // ---------- modifications de la sélection ----------
    fn selection_center(&self) -> Option<(f64, f64)> {
        let b: Vec<([f64; 2], [f64; 2])> = self.selection.iter().filter_map(|id| self.studio.project().shape(*id)).filter_map(|s| shape::bounds(&s.entity)).collect();
        let lo = b.iter().fold([f64::INFINITY; 2], |a, (l, _)| [a[0].min(l[0]), a[1].min(l[1])]);
        let hi = b.iter().fold([f64::NEG_INFINITY; 2], |a, (_, h)| [a[0].max(h[0]), a[1].max(h[1])]);
        (!b.is_empty()).then(|| ((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0))
    }

    fn apply_cad(&mut self, act: CadAct) {
        let ids = self.selection.clone();
        let c = self.cad.clone();
        let center = self.selection_center().unwrap_or((0.0, 0.0));
        let n = ids.len();
        // résultat : (nouvelle sélection éventuelle, message)
        let r: Result<(Option<Vec<u32>>, String), shape::EditError> = match act {
            CadAct::Move => self.studio.edit(|p| p.move_shapes(&ids, c.dx, c.dy)).map(|_| (None, format!("{n} forme(s) déplacée(s)"))),
            CadAct::Copy => self.studio.edit(|p| p.copy_shapes(&ids, &Similarity::translation(c.dx, c.dy))).map(|v| (Some(v), format!("{n} forme(s) copiée(s)"))),
            CadAct::Rotate => self.studio.edit(|p| p.transform_shapes(&ids, &Similarity::rotation(center, c.angle))).map(|_| (None, format!("rotation de {}°", c.angle))),
            CadAct::Scale => Similarity::scale(center, c.scale).and_then(|t| self.studio.edit(|p| p.transform_shapes(&ids, &t))).map(|_| (None, format!("échelle × {}", c.scale))),
            CadAct::Mirror(vertical) => self.studio.edit(|p| p.transform_shapes(&ids, &Similarity::mirror(center, if vertical { 90.0 } else { 0.0 }))).map(|_| (None, "miroir".into())),
            CadAct::ArrayRect => self.studio.edit(|p| p.array_rect(&ids, c.nx, c.ny, c.adx, c.ady)).map(|v| (None, format!("réseau : {} copie(s)", v.len()))),
            CadAct::ArrayPolar => self.studio.edit(|p| p.array_polar(&ids, (0.0, 0.0), c.pn, c.pangle)).map(|v| (None, format!("réseau circulaire : {} copie(s)", v.len()))),
            CadAct::Offset => self.studio.edit(|p| p.offset_shapes(&ids, c.offset)).map(|v| (Some(v), "décalage ajouté".into())),
            CadAct::OpenOffset => self.studio.edit(|p| p.open_offset_shapes(&ids, c.offset)).map(|v| (Some(v), "contour de rainure ajouté".into())),
            CadAct::Divide => self.studio.edit(|p| p.divide_shapes(&ids, c.pn as usize)).map(|v| (Some(v.clone()), format!("{} point(s) ajouté(s)", v.len()))),
            CadAct::Fillet => self.studio.edit(|p| p.fillet_shapes(&ids, c.fillet)).map(|k| (None, format!("{k} coin(s) arrondi(s)"))),
            CadAct::Join => self.studio.edit(|p| p.join_shapes(&ids, 0.01)).map(|v| (Some(v.clone()), format!("{n} forme(s) jointes en {}", v.len()))),
            CadAct::Explode => self.studio.edit(|p| p.explode_shapes(&ids)).map(|v| (Some(v.clone()), format!("{} segment(s)", v.len()))),
            CadAct::Bool(op) => match ids.as_slice() {
                [a, b] => self.studio.edit(|p| p.boolean_shapes(*a, *b, op)).map(|v| (Some(v), "opération booléenne faite".into())),
                _ => Err(shape::EditError::InvalidParameter("sélectionnez exactement deux formes fermées".into())),
            },
        };
        if let Some((sel, msg)) = self.report(r) {
            if let Some(sel) = sel {
                self.selection = sel;
            }
            self.status = msg;
        }
    }

    fn cad_panel(&mut self, ui: &mut egui::Ui) {
        let mut act = None;
        // longueurs affichées dans l'unité du projet ; seules les valeurs retouchées sont réécrites
        let k = if self.studio.project().gcode_inches { 25.4 } else { 1.0 };
        let o = self.cad.clone();
        let start = CadParams { dx: o.dx / k, dy: o.dy / k, adx: o.adx / k, ady: o.ady / k, offset: o.offset / k, fillet: o.fillet / k, ..o.clone() };
        let mut shown = start.clone();
        let c = &mut shown;
        let field = |ui: &mut egui::Ui, label: &str, v: &mut f64, speed: f64| {
            ui.label(label);
            ui.add(egui::DragValue::new(v).speed(speed));
        };
        let count = |ui: &mut egui::Ui, label: &str, v: &mut u32| {
            ui.label(label);
            ui.add(egui::DragValue::new(v).speed(0.1).clamp_range(1..=1000));
        };
        ui.horizontal(|ui| {
            field(ui, "dx", &mut c.dx, 0.5);
            field(ui, "dy", &mut c.dy, 0.5);
            if ui.button("Déplacer").clicked() {
                act = Some(CadAct::Move);
            }
            if ui.button("Copier").clicked() {
                act = Some(CadAct::Copy);
            }
        });
        ui.horizontal(|ui| {
            field(ui, "angle (°)", &mut c.angle, 1.0);
            if ui.button("Tourner").clicked() {
                act = Some(CadAct::Rotate);
            }
        });
        ui.horizontal(|ui| {
            field(ui, "facteur", &mut c.scale, 0.05);
            if ui.button("Échelle").clicked() {
                act = Some(CadAct::Scale);
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Miroir gauche/droite").clicked() {
                act = Some(CadAct::Mirror(true));
            }
            if ui.button("Miroir haut/bas").clicked() {
                act = Some(CadAct::Mirror(false));
            }
        });
        ui.small("rotation, échelle et miroir : autour du centre de la sélection");
        ui.separator();
        ui.horizontal(|ui| {
            count(ui, "colonnes", &mut c.nx);
            count(ui, "rangées", &mut c.ny);
        });
        ui.horizontal(|ui| {
            field(ui, "pas X", &mut c.adx, 0.5);
            field(ui, "pas Y", &mut c.ady, 0.5);
            if ui.button("Réseau").clicked() {
                act = Some(CadAct::ArrayRect);
            }
        });
        ui.horizontal(|ui| {
            count(ui, "exemplaires", &mut c.pn);
            field(ui, "sur (°)", &mut c.pangle, 5.0);
            if ui.button("Réseau circulaire").clicked() {
                act = Some(CadAct::ArrayPolar);
            }
        });
        ui.small("réseau circulaire : autour de l'origine (0 ; 0)");
        ui.separator();
        ui.horizontal(|ui| {
            field(ui, "distance", &mut c.offset, 0.1);
            if ui.button("Décaler").clicked() {
                act = Some(CadAct::Offset);
            }
        });
        ui.small("> 0 : vers l'extérieur (fermée) ou à gauche du tracé (ouverte)");
        ui.horizontal(|ui| {
            if ui.button("Décalage double (rainure)").on_hover_text("forme ouverte : contour fermé à « distance » de part et d'autre, bouts arrondis").clicked() {
                act = Some(CadAct::OpenOffset);
            }
            if ui.button("Diviser en points").on_hover_text("répartit « exemplaires » points sur le contour (trous de perçage)").clicked() {
                act = Some(CadAct::Divide);
            }
        });
        ui.horizontal(|ui| {
            field(ui, "rayon", &mut c.fillet, 0.1);
            if ui.button("Arrondir les coins").clicked() {
                act = Some(CadAct::Fillet);
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button("Joindre").clicked() {
                act = Some(CadAct::Join);
            }
            if ui.button("Éclater").clicked() {
                act = Some(CadAct::Explode);
            }
        });
        ui.horizontal_wrapped(|ui| {
            for (label, op) in [("Union", cad::BoolOp::Union), ("Différence", cad::BoolOp::Difference), ("Intersection", cad::BoolOp::Intersection)] {
                if ui.button(label).clicked() {
                    act = Some(CadAct::Bool(op));
                }
            }
        });
        ui.small("booléens : deux formes fermées ; différence = 1re sélectionnée − 2e");
        let back = |new: f64, old: f64, stored: f64| if (new - old).abs() > 1e-12 { new * k } else { stored };
        self.cad = CadParams {
            dx: back(shown.dx, start.dx, o.dx),
            dy: back(shown.dy, start.dy, o.dy),
            adx: back(shown.adx, start.adx, o.adx),
            ady: back(shown.ady, start.ady, o.ady),
            offset: back(shown.offset, start.offset, o.offset),
            fillet: back(shown.fillet, start.fillet, o.fillet),
            ..shown.clone()
        };
        if let Some(a) = act {
            self.apply_cad(a);
        }
    }

    /// Contours du texte saisi, ligne de base en `at`, ajoutés sur le calque courant (une seule annulation).
    fn place_text(&mut self, at: (f64, f64)) {
        let path = if self.text_font.trim().is_empty() { fablabcam_studio::text::default_font().map(str::to_string) } else { Some(self.text_font.trim().to_string()) };
        let Some(path) = path else {
            self.status = "Attention : aucune police trouvée — indiquez un fichier .ttf ou .otf".into();
            return;
        };
        let style = fablabcam_studio::text::TextStyle { height: self.text_height, spacing: self.text_spacing, line_spacing: 1.0 };
        let r = std::fs::read(&path).map_err(|e| format!("{path} : {e}")).and_then(|font| fablabcam_studio::text::outlines(&font, &self.text_value.replace("\\n", "\n"), at, &style));
        match r {
            Ok((shapes, missing)) => {
                let layer = self.layer.clone();
                let n = shapes.len();
                let ids = self.studio.edit(|p| shapes.into_iter().map(|e| p.add_shape(&layer, e)).collect::<Result<Vec<u32>, _>>());
                if let Some(ids) = self.report(ids) {
                    self.selection = ids;
                    self.status = format!("texte : {n} contour(s){}", if missing.is_empty() { String::new() } else { format!(" — absents de la police : {missing}") });
                }
            }
            Err(e) => self.status = format!("Attention : {e}"),
        }
    }

    /// Sommet de la polyligne sélectionnée (seule) sous le pointeur.
    /// Poignée de la forme sélectionnée sous le pointeur.
    fn handle_under(&self, w: [f64; 2], tol: f64) -> Option<(u32, Handle)> {
        let [id] = self.selection.as_slice() else { return None };
        let s = self.studio.project().shape(*id)?;
        edit::handles(&s.entity).into_iter().map(|(h, p)| (h, (p.0 - w[0]).hypot(p.1 - w[1]))).filter(|(_, d)| *d <= tol).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(h, _)| (*id, h))
    }

    // ---------- fichiers ----------
    fn dirty(&self) -> bool {
        self.studio.project() != &self.saved
    }

    fn doc_dir(&self) -> Option<PathBuf> {
        self.doc_path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf))
    }

    fn save_prefs(&mut self) {
        self.prefs.snap = self.snap;
        self.prefs.sim_res = self.sim_res;
        self.prefs.post_path = self.post_path.clone();
        if let Err(e) = self.prefs.save() {
            self.status = format!("Attention : préférences : {e}");
        }
    }

    fn remember(&mut self, p: &Path) {
        self.prefs.touch_recent(p);
        self.save_prefs();
    }

    /// Repart d'un projet chargé ou vide : sélections, brouillons et vue remis à zéro.
    fn replace_project(&mut self, project: Project, path: Option<PathBuf>) {
        self.saved = project.clone();
        self.studio = Studio::new(project);
        self.doc_path = path;
        self.selection.clear();
        self.pts.clear();
        self.op_draft = None;
        self.tool_draft = None;
        self.stock_draft = None;
        self.sel_tool = self.studio.project().tools.first().map(|t| t.name.clone());
        self.sel_op = self.studio.project().ops.first().map(|o| o.id);
        self.layer = first_layer(self.studio.project());
        self.fit_pending = true;
    }

    fn new_doc(&mut self, confirmed: bool) {
        if self.dirty() && !confirmed {
            self.confirm = Some(Pending::New);
            return;
        }
        self.replace_project(Project::new(), None);
        self.status = "nouveau projet".into();
    }

    /// Ouvre un fichier : `.cb` remplace le projet, `.dxf` / `.svg` ajoutent leurs formes,
    /// `.cbpp` devient le post-processeur utilisé à l'export.
    fn open_path(&mut self, p: &Path, confirmed: bool) {
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        match ext.as_str() {
            "cb" => {
                if self.dirty() && !confirmed {
                    self.confirm = Some(Pending::Open(p.to_path_buf()));
                    return;
                }
                match std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|t| persist::load(&t)) {
                    Ok(project) => {
                        self.replace_project(project, Some(p.to_path_buf()));
                        self.path = p.display().to_string();
                        self.status = format!("{} ouvert", p.display());
                        self.remember(p);
                    }
                    Err(e) => self.status = format!("Attention : {} : {e}", p.display()),
                }
            }
            "dxf" | "svg" => {
                self.path = p.display().to_string();
                self.import();
                self.remember(p);
            }
            "cbpp" => {
                self.post_path = p.display().to_string();
                self.status = format!("post-processeur : {}", p.display());
            }
            _ => self.status = format!("Attention : {} : format non pris en charge (cb, dxf, svg, cbpp)", p.display()),
        }
    }

    /// Lance un dialogue dans un fil : la fenêtre reste active pendant qu'il est ouvert.
    fn spawn_dialog(&mut self, purpose: Purpose, f: impl FnOnce() -> DialogResult + Send + 'static) {
        if self.dialog.is_some() {
            self.status = "un dialogue de fichier est déjà ouvert".into();
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        self.dialog = Some((purpose, rx));
    }

    fn open_dialog(&mut self, purpose: Purpose, title: &str, exts: &'static [&'static str]) {
        let (title, dir) = (title.to_string(), self.doc_dir());
        self.spawn_dialog(purpose, move || desktop::open_dialog(&title, exts, dir.as_deref()));
    }

    fn save_dialog(&mut self, purpose: Purpose, title: &str, default_name: String) {
        let (title, dir) = (title.to_string(), self.doc_dir());
        self.spawn_dialog(purpose, move || desktop::save_dialog(&title, &default_name, dir.as_deref()));
    }

    /// Réponse d'un dialogue terminé.
    fn dialog_done(&mut self, purpose: Purpose, r: DialogResult, ctx: &egui::Context) {
        let p = match r {
            Ok(Some(p)) => p,
            Ok(None) => return, // annulé
            Err(e) => {
                self.status = format!("Attention : {e} — saisissez le chemin dans le champ « Fichier »");
                return;
            }
        };
        match purpose {
            Purpose::Open => self.open_path(&p, false),
            Purpose::SaveProject(then) => {
                if self.write_project(&desktop::with_extension(p, "cb")) {
                    if let Some(next) = then {
                        self.continue_pending(next, ctx);
                    }
                }
            }
            Purpose::ExportDrawing(ext) => self.write_drawing(&desktop::with_extension(p, ext), ext),
            Purpose::ExportGcode => self.status = self.write_gcode(&desktop::with_extension(p, "nc")),
            Purpose::PickPost => self.post_path = p.display().to_string(),
            Purpose::LoadScript => match std::fs::read_to_string(&p) {
                Ok(t) => {
                    self.script_code = t;
                    self.status = format!("script {} chargé", p.display());
                }
                Err(e) => self.status = format!("Attention : {} : {e}", p.display()),
            },
        }
    }

    fn poll_dialog(&mut self, ctx: &egui::Context) {
        let Some((_, rx)) = &self.dialog else { return };
        match rx.try_recv() {
            Ok(r) => {
                if let Some((purpose, _)) = self.dialog.take() {
                    self.dialog_done(purpose, r, ctx);
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(150)),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.dialog = None,
        }
    }

    fn stem(&self) -> String {
        self.doc_path.as_ref().and_then(|p| p.file_stem()).map_or("projet".into(), |s| s.to_string_lossy().into_owned())
    }

    fn write_project(&mut self, p: &Path) -> bool {
        match std::fs::write(p, persist::save(self.studio.project())) {
            Ok(()) => {
                self.saved = self.studio.project().clone();
                self.doc_path = Some(p.to_path_buf());
                self.status = format!("{} enregistré", p.display());
                self.remember(p);
                true
            }
            Err(e) => {
                self.status = format!("Attention : {} : {e}", p.display());
                false
            }
        }
    }

    /// Enregistre, puis exécute `then`. Sans fichier connu, passe par « Enregistrer sous ».
    fn save_then(&mut self, then: Option<Pending>, ctx: &egui::Context) {
        match self.doc_path.clone() {
            Some(p) => {
                if self.write_project(&p) {
                    if let Some(next) = then {
                        self.continue_pending(next, ctx);
                    }
                }
            }
            None => self.save_as(then),
        }
    }

    fn save_as(&mut self, then: Option<Pending>) {
        let name = format!("{}.cb", self.stem());
        self.save_dialog(Purpose::SaveProject(then), "Enregistrer le projet", name);
    }

    fn continue_pending(&mut self, p: Pending, ctx: &egui::Context) {
        match p {
            Pending::New => self.new_doc(true),
            Pending::Open(path) => self.open_path(&path, true),
            Pending::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn run(&mut self, a: Action, ctx: &egui::Context) {
        match a {
            Action::New => self.new_doc(false),
            Action::Open => self.open_dialog(Purpose::Open, "Ouvrir un projet ou un dessin", &["cb", "dxf", "svg"]),
            Action::OpenRecent(p) => self.open_path(&p, false),
            Action::Save => self.save_then(None, ctx),
            Action::SaveAs => self.save_as(None),
            Action::Import => self.open_dialog(Purpose::Open, "Importer un dessin", &["dxf", "svg"]),
            Action::ExportDxf | Action::ExportSvg => {
                let ext = if matches!(a, Action::ExportDxf) { "dxf" } else { "svg" };
                self.save_dialog(Purpose::ExportDrawing(ext), &format!("Exporter le dessin en {}", ext.to_uppercase()), format!("{}.{ext}", self.stem()));
            }
            Action::ExportGcode => self.save_dialog(Purpose::ExportGcode, "Exporter le G-code", format!("{}.nc", self.stem())),
            Action::Prefs => self.show_prefs = true,
            Action::Script => self.show_script = true,
            Action::Undo => {
                self.studio.undo();
                self.selection.retain(|id| self.studio.project().shape(*id).is_some());
            }
            Action::Redo => {
                self.studio.redo();
            }
            Action::Delete => self.delete_selection(),
            Action::SelectAll => {
                self.selection = self.studio.project().shapes.iter().filter(|s| !self.hidden_layers.contains(&s.layer)).map(|s| s.id).collect();
                self.focus = Focus::Shapes;
            }
            Action::Copy | Action::Cut => {
                self.clipboard = self.selection.iter().filter_map(|id| self.studio.project().shape(*id)).map(|s| (s.layer.clone(), s.entity.clone())).collect();
                let n = self.clipboard.len();
                if matches!(a, Action::Cut) && n > 0 {
                    self.delete_selection();
                    self.status = format!("{n} forme(s) coupée(s)");
                } else {
                    self.status = format!("{n} forme(s) copiée(s)");
                }
            }
            Action::Paste => {
                let items = self.clipboard.clone();
                // collées à la même place, comme dans le logiciel de référence, et sélectionnées pour être déplacées
                let r = self.studio.edit(|p| items.iter().map(|(l, e)| p.add_shape(l, e.clone())).collect::<Result<Vec<u32>, _>>());
                if let Some(ids) = self.report(r) {
                    self.status = format!("{} forme(s) collée(s)", ids.len());
                    self.selection = ids;
                    self.focus = Focus::Shapes;
                }
            }
            Action::View(v) => {
                self.view = v;
                if v == View::Vue3D {
                    self.sim_frac = 1.0;
                    self.playing = false;
                }
            }
            Action::Fit => {
                self.fit_pending = true;
                self.cam3 = None;
            }
            Action::ToggleToolpaths => self.show_toolpaths = !self.show_toolpaths,
            Action::ToggleConsole => self.show_console = !self.show_console,
            Action::Tool(d) => {
                if d != Draw::Select {
                    self.tab_mode = false;
                }
                self.tool = d;
                self.pts.clear();
                self.view = View::Plan;
            }
            Action::NewOp(k) => self.add_op_for_selection(k),
            Action::PickPost => self.open_dialog(Purpose::PickPost, "Choisir un post-processeur", &["cbpp"]),
            Action::System => self.side = SideTab::Systeme,
            Action::Help => self.show_help = true,
        }
    }

    /// Crée une opération sur les formes sélectionnées ; sans outil, en ajoute un par défaut.
    fn add_op_for_selection(&mut self, kind: OpKind) {
        if self.selection.is_empty() {
            self.status = "Attention : sélectionnez d'abord des formes (clic sur le dessin ou dans l'arbre)".into();
            return;
        }
        let mut tool = self.studio.project().tools.iter().find(|t| (t.kind == ToolKind::Drill) == (kind == OpKind::Drill)).or(self.studio.project().tools.first()).map(|t| t.name.clone());
        let mut created = String::new();
        if tool.is_none() {
            let t = if kind == OpKind::Drill { Tool::new("Foret 3 mm", ToolKind::Drill, 3.0) } else { Tool::new("Fraise 3 mm", ToolKind::EndMill, 3.0) };
            let name = t.name.clone();
            if self.studio.edit(|p| p.add_tool(t)).is_ok() {
                created = format!(" avec l'outil par défaut « {name} » (onglet Système)");
                tool = Some(name);
            }
        }
        let Some(tool) = tool else { return };
        let ids = self.selection.clone();
        let mut k = self.studio.project().ops.len() + 1;
        while self.studio.project().ops.iter().any(|o| o.name == format!("{} {k}", kind.label())) {
            k += 1;
        }
        let name = format!("{} {k}", kind.label());
        match self.studio.edit(|p| p.add_operation(kind, &name, ids, &tool)) {
            Ok(id) => {
                self.sel_op = Some(id);
                self.op_draft = None;
                self.focus = Focus::Op;
                self.side = SideTab::Projet;
                self.status = format!("opération « {name} » ajoutée{created}");
            }
            Err(e) => self.status = format!("Attention : {e}"),
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut act = None;
        let item = |ui: &mut egui::Ui, label: &str, keys: &str, a: Action, act: &mut Option<Action>| {
            if menu_item(ui, label, keys) {
                *act = Some(a);
            }
        };
        ui.menu_button("Fichier", |ui| {
            ui.set_min_width(220.0);
            item(ui, "Nouveau", "Cmd+N", Action::New, &mut act);
            item(ui, "Ouvrir…", "Cmd+O", Action::Open, &mut act);
            let recent = self.prefs.recent.clone();
            ui.add_enabled_ui(!recent.is_empty(), |ui| {
                ui.menu_button("Récents", |ui| {
                    for r in recent {
                        let name = Path::new(&r).file_name().map_or(r.clone(), |n| n.to_string_lossy().into_owned());
                        if ui.button(name).on_hover_text(&r).clicked() {
                            act = Some(Action::OpenRecent(PathBuf::from(&r)));
                            ui.close_menu();
                        }
                    }
                });
            });
            ui.separator();
            item(ui, "Enregistrer", "Cmd+S", Action::Save, &mut act);
            item(ui, "Enregistrer sous…", "Maj+Cmd+S", Action::SaveAs, &mut act);
            ui.separator();
            item(ui, "Importer DXF / SVG…", "Cmd+I", Action::Import, &mut act);
            item(ui, "Exporter en DXF…", "", Action::ExportDxf, &mut act);
            item(ui, "Exporter en SVG…", "", Action::ExportSvg, &mut act);
        });
        ui.menu_button("Édition", |ui| {
            ui.set_min_width(220.0);
            item(ui, "Annuler", "Cmd+Z", Action::Undo, &mut act);
            item(ui, "Rétablir", "Maj+Cmd+Z", Action::Redo, &mut act);
            ui.separator();
            item(ui, "Couper", "Cmd+X", Action::Cut, &mut act);
            item(ui, "Copier", "Cmd+C", Action::Copy, &mut act);
            item(ui, "Coller", "Cmd+V", Action::Paste, &mut act);
            item(ui, "Supprimer", "Suppr", Action::Delete, &mut act);
            ui.separator();
            item(ui, "Tout sélectionner", "Cmd+A", Action::SelectAll, &mut act);
        });
        ui.menu_button("Affichage", |ui| {
            ui.set_min_width(220.0);
            item(ui, "Plan 2D", "1", Action::View(View::Plan), &mut act);
            item(ui, "3D", "2", Action::View(View::Vue3D), &mut act);
            item(ui, "Simulation", "3", Action::View(View::Simulation), &mut act);
            ui.separator();
            item(ui, "Cadrer", "F", Action::Fit, &mut act);
            item(ui, if self.show_toolpaths { "Masquer les trajectoires" } else { "Afficher les trajectoires" }, "", Action::ToggleToolpaths, &mut act);
            item(ui, if self.show_console { "Masquer la console" } else { "Afficher la console" }, "", Action::ToggleConsole, &mut act);
        });
        ui.menu_button("Dessin", |ui| {
            ui.set_min_width(220.0);
            for (d, label) in Draw::ALL {
                let keys = match d {
                    Draw::Select => "V",
                    Draw::Line => "L",
                    Draw::Polyline => "P",
                    Draw::Rect => "R",
                    Draw::Circle => "O",
                    Draw::Arc => "A",
                    Draw::Text => "T",
                };
                item(ui, label, keys, Action::Tool(d), &mut act);
            }
        });
        ui.menu_button("Usinage", |ui| {
            ui.set_min_width(220.0);
            for k in OpKind::ALL {
                item(ui, &format!("{} de la sélection", k.label()), "", Action::NewOp(k), &mut act);
            }
            ui.separator();
            item(ui, "Générer le G-code…", "Cmd+E", Action::ExportGcode, &mut act);
            item(ui, "Choisir le post-processeur…", "", Action::PickPost, &mut act);
        });
        ui.menu_button("Script", |ui| {
            ui.set_min_width(220.0);
            item(ui, "Éditeur de script…", "", Action::Script, &mut act);
        });
        ui.menu_button("Outils", |ui| {
            ui.set_min_width(220.0);
            item(ui, "Bibliothèque d'outils", "", Action::System, &mut act);
            item(ui, "Préférences…", "Cmd+,", Action::Prefs, &mut act);
        });
        ui.menu_button("Aide", |ui| {
            ui.set_min_width(220.0);
            item(ui, "Raccourcis et prise en main", "", Action::Help, &mut act);
        });
        act
    }

    /// Barre d'outils : fichiers, annulation, unités, dessin, usinage.
    fn toolbar(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut act = None;
        ui.horizontal_wrapped(|ui| {
            let mut b = |ui: &mut egui::Ui, label: &str, tip: &str, a: Action| {
                if ui.button(label).on_hover_text(tip).clicked() {
                    act = Some(a);
                }
            };
            b(ui, "Nouveau", "Nouveau projet (Cmd+N)", Action::New);
            b(ui, "Ouvrir", "Ouvrir un projet ou un dessin (Cmd+O)", Action::Open);
            b(ui, "Enregistrer", "Enregistrer (Cmd+S)", Action::Save);
            ui.separator();
            if ui.add_enabled(self.studio.editor.can_undo(), egui::Button::new("Annuler")).on_hover_text("Cmd+Z").clicked() {
                act = Some(Action::Undo);
            }
            if ui.add_enabled(self.studio.editor.can_redo(), egui::Button::new("Rétablir")).on_hover_text("Maj+Cmd+Z").clicked() {
                act = Some(Action::Redo);
            }
            ui.separator();
            ui.label("Unités");
            let mut inch = self.studio.project().gcode_inches;
            egui::ComboBox::from_id_source("unites").width(110.0).selected_text(if inch { "Pouces" } else { "Millimètres" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut inch, false, "Millimètres");
                ui.selectable_value(&mut inch, true, "Pouces");
            });
            if inch != self.studio.project().gcode_inches {
                let _ = self.studio.edit(|p| {
                    p.gcode_inches = inch;
                    Ok::<_, shape::EditError>(())
                });
                self.status = format!("unités : {} (affichage, saisie et G-code)", if inch { "pouces" } else { "millimètres" });
            }
            ui.separator();
            for (d, label) in Draw::ALL {
                if ui.selectable_label(self.tool == d, label).on_hover_text(d.hint()).clicked() {
                    act = Some(Action::Tool(d));
                }
            }
            ui.separator();
            for k in OpKind::ALL {
                if ui.add_enabled(!self.selection.is_empty(), egui::Button::new(k.label())).on_hover_text(format!("Créer une opération « {} » sur la sélection", k.label())).on_disabled_hover_text("Sélectionnez d'abord des formes").clicked() {
                    act = Some(Action::NewOp(k));
                }
            }
            if ui.add(egui::Button::new(egui::RichText::new("Générer le G-code").strong()).fill(Color32::from_rgb(160, 80, 20))).on_hover_text("Cmd+E").clicked() {
                act = Some(Action::ExportGcode);
            }
        });
        act
    }

    fn prefs_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_prefs;
        let mut pick_post = false;
        egui::Window::new("Préférences").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
            ui.checkbox(&mut self.snap, "magnétisme");
            ui.horizontal(|ui| {
                ui.label("pas du magnétisme (mm)");
                if ui.add(egui::DragValue::new(&mut self.prefs.snap_step).speed(0.1).clamp_range(0.01..=100.0)).changed() {
                    self.prefs.save().ok();
                }
            });
            egui::ComboBox::from_label("maille de simulation (mm)").selected_text(format!("{}", self.sim_res)).show_ui(ui, |ui| {
                for r in [1.0, 0.5, 0.25, 0.2, 0.1] {
                    ui.selectable_value(&mut self.sim_res, r, format!("{r}"));
                }
            });
            ui.label("post-processeur .cbpp (vide = intégré)");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.post_path);
                pick_post = ui.button("Choisir…").clicked();
            });
            if ui.add_enabled(!self.prefs.recent.is_empty(), egui::Button::new("Oublier les fichiers récents")).clicked() {
                self.prefs.recent.clear();
                self.prefs.save().ok();
            }
            if let Some(f) = Prefs::file() {
                ui.small(format!("enregistrées dans {}", f.display()));
            }
        });
        self.show_prefs = open;
        if pick_post {
            self.open_dialog(Purpose::PickPost, "Choisir un post-processeur", &["cbpp"]);
        }
    }

    fn script_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_script;
        let (mut run, mut load) = (false, false);
        egui::Window::new("Script (Rhai)").open(&mut open).default_width(520.0).show(ctx, |ui| {
            ui.small("rect, circle, line, arc, polyline, text, move, rotate, scale, copy, offset, delete, tool, drill_tool, profile, pocket, drill, engrave, layer, shapes, print");
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut self.script_code).code_editor().desired_rows(14).desired_width(f32::INFINITY));
            });
            ui.horizontal(|ui| {
                run = ui.button("Exécuter").clicked();
                load = ui.button("Ouvrir un script…").clicked();
            });
            if !self.script_log.is_empty() {
                ui.separator();
                ui.monospace(&self.script_log);
            }
        });
        self.show_script = open;
        if load {
            self.open_dialog(Purpose::LoadScript, "Ouvrir un script", &["rhai", "txt"]);
        }
        if run {
            let code = self.script_code.clone();
            // une seule étape d'annulation ; en cas d'erreur le projet reste intact
            let r = self.studio.edit(|p| {
                let out = fablabcam_studio::script::run(p, &code).map_err(shape::EditError::InvalidParameter)?;
                *p = out.project;
                Ok(out.log)
            });
            match r {
                Ok(log) => {
                    self.script_log = if log.is_empty() { "terminé".into() } else { log.join("\n") };
                    self.selection.retain(|id| self.studio.project().shape(*id).is_some());
                    self.status = "script exécuté (Cmd+Z pour annuler)".into();
                }
                Err(e) => self.script_log = format!("Erreur : {e}"),
            }
        }
    }

    fn help_window(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new("Prise en main").open(&mut open).default_width(460.0).show(ctx, |ui| {
            ui.label("1. Dessinez (barre d'outils ou menu Dessin) ou ouvrez un fichier (Cmd+O, glisser-déposer).");
            ui.label("2. Sélectionnez des formes : clic sur le plan (Maj+clic pour ajouter) ou dans l'arbre Calques.");
            ui.label("3. Cliquez Profil, Poche, Perçage ou Gravure dans la barre d'outils : l'opération apparaît sous Usinage.");
            ui.label("4. Réglez-la dans Propriétés : chaque saisie est appliquée dès qu'elle est terminée (Cmd+Z annule).");
            ui.label("5. Vérifiez en 3D / Simulation, puis « Générer le G-code ».");
            ui.separator();
            egui::Grid::new("raccourcis").striped(true).show(ui, |ui| {
                for (k, v) in [
                    ("V L P R O A T", "sélection, ligne, polyligne, rectangle, cercle, arc, texte"),
                    ("1 2 3", "plan 2D, 3D, simulation"),
                    ("F / double-clic", "cadrer"),
                    ("Cmd+C X V", "copier, couper, coller"),
                    ("Cmd+Z / Maj+Cmd+Z", "annuler, rétablir"),
                    ("Cmd+N O S E I", "nouveau, ouvrir, enregistrer, G-code, importer"),
                    ("clic droit", "menu contextuel (en sélection) · terminer la polyligne"),
                    ("molette · clic milieu", "zoom · déplacer la vue"),
                ] {
                    ui.monospace(k);
                    ui.label(v);
                    ui.end_row();
                }
            });
        });
        self.show_help = open;
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.confirm.clone() else { return };
        let mut choice = None;
        egui::Window::new("Modifications non enregistrées").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label(match pending {
                Pending::Quit => "Le projet a des modifications non enregistrées. Les enregistrer avant de quitter ?",
                _ => "Le projet a des modifications non enregistrées. Les enregistrer d'abord ?",
            });
            ui.horizontal(|ui| {
                if ui.button("Enregistrer").clicked() {
                    choice = Some(true);
                }
                if ui.button("Ne pas enregistrer").clicked() {
                    choice = Some(false);
                }
                if ui.button("Annuler").clicked() {
                    self.confirm = None;
                }
            });
        });
        let Some(save_first) = choice else { return };
        self.confirm = None;
        if save_first {
            // l'action suit l'enregistrement ; annuler « Enregistrer sous » l'abandonne
            self.save_then(Some(pending), ctx);
        } else {
            self.continue_pending(pending, ctx);
        }
    }

    fn snap_pt(&self, p: [f64; 2], step: f64) -> [f64; 2] {
        if self.snap { [(p[0] / step).round() * step, (p[1] / step).round() * step] } else { p }
    }

    fn report<T>(&mut self, r: Result<T, shape::EditError>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                self.status = format!("Attention : {e}");
                None
            }
        }
    }

    fn commit_shape(&mut self) {
        let p = &self.pts;
        let e = match self.tool {
            Draw::Line if p.len() >= 2 => shape::line((p[0][0], p[0][1]), (p[1][0], p[1][1])),
            Draw::Rect if p.len() >= 2 => shape::rectangle((p[0][0], p[0][1]), (p[1][0], p[1][1])),
            Draw::Circle if p.len() >= 2 => shape::circle((p[0][0], p[0][1]), (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1])),
            Draw::Text if !p.is_empty() => {
                let at = (p[0][0], p[0][1]);
                self.pts.clear();
                self.place_text(at);
                return;
            }
            Draw::Arc if p.len() >= 3 => {
                let r = (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1]);
                let a0 = (p[1][1] - p[0][1]).atan2(p[1][0] - p[0][0]).to_degrees();
                let a1 = (p[2][1] - p[0][1]).atan2(p[2][0] - p[0][0]).to_degrees();
                let mut sweep = (a1 - a0) % 360.0;
                if sweep <= 0.0 {
                    sweep += 360.0;
                }
                shape::arc((p[0][0], p[0][1]), r, a0, sweep)
            }
            _ => return,
        };
        self.add(e);
        self.pts.clear();
    }

    fn finish_polyline(&mut self, closed: bool) {
        let pts: Vec<(f64, f64)> = self.pts.iter().map(|p| (p[0], p[1])).collect();
        if !pts.is_empty() {
            self.add(shape::polyline(&pts, closed));
        }
        self.pts.clear();
    }

    fn add(&mut self, e: Result<fablabcam_model::Entity, shape::EditError>) {
        let layer = self.layer.clone();
        let r = e.and_then(|e| self.studio.edit(|p| p.add_shape(&layer, e)));
        if let Some(id) = self.report(r) {
            self.selection = vec![id];
            self.focus = Focus::Shapes;
            self.status = format!("forme {id} ajoutée");
        }
    }

    fn delete_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let ids = self.selection.clone();
        let r = self.studio.edit(|p| Ok(p.delete_shapes(&ids)));
        if let Some(touched) = self.report(r) {
            self.status = if touched.is_empty() { format!("{} forme(s) supprimée(s)", ids.len()) } else { format!("{} forme(s) supprimée(s) ; opérations touchées : {}", ids.len(), touched.join(", ")) };
            self.selection.clear();
        }
    }

    fn pick(&self, w: [f64; 2]) -> Option<u32> {
        let tol = 7.0 / self.cam.scale;
        self.studio
            .project()
            .shapes
            .iter()
            .filter(|s| !self.hidden_layers.contains(&s.layer))
            .map(|s| (s.id, shape::distance(&s.entity, (w[0], w[1]))))
            .filter(|(_, d)| *d <= tol)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    fn import(&mut self) {
        let path = resolve(self.path.trim()).display().to_string();
        if path.to_lowercase().ends_with(".cb") {
            self.open_path(Path::new(&path), false);
            return;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                self.status = format!("Attention : {path} : {e}");
                return;
            }
        };
        let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
        let r: Result<(shape::LayeredShapes, Vec<String>), String> = match ext.as_str() {
            "dxf" => dxf::parse_dxf(&text).map(|r| (r.shapes, r.notes.into_iter().chain(r.ignored).collect())),
            "svg" => svgio::parse_svg(&text).map(|r| (r.shapes, r.ignored)),
            "cb" => persist::load(&text).map(|p| {
                self.studio = Studio::new(p.clone());
                (Vec::new(), Vec::new())
            }),
            _ => Err("extension inconnue (cb, dxf, svg)".into()),
        };
        match r {
            Ok((shapes, ignored)) => {
                let n = shapes.len();
                if n > 0 {
                    let _ = self.studio.edit(|p| {
                        for (l, e) in shapes {
                            p.add_shape(&l, e)?;
                        }
                        Ok(())
                    });
                }
                self.selection.clear();
                self.fit_pending = true;
                self.status = format!("{path} importé{}", if ignored.is_empty() { String::new() } else { format!(" — ignoré : {}", ignored.join(", ")) });
            }
            Err(e) => self.status = format!("Attention : import : {e}"),
        }
    }

    fn export(&mut self, ext: &str) {
        let base = self.path.trim().trim_end_matches(|c| c != '.').trim_end_matches('.').to_string();
        let base = if base.is_empty() { "dessin".to_string() } else { base };
        let out = resolve(&format!("{base}.{ext}"));
        self.write_drawing(&out, ext);
    }

    fn write_drawing(&mut self, out: &Path, ext: &str) {
        let shapes: Vec<(String, fablabcam_model::Entity)> = self.studio.project().shapes.iter().map(|s| (s.layer.clone(), s.entity.clone())).collect();
        let text = match ext {
            "dxf" => dxf::write_dxf(&shapes),
            "svg" => svgio::write_svg(&shapes),
            _ => persist::save(self.studio.project()),
        };
        self.status = match std::fs::write(out, text) {
            Ok(()) => format!("{} écrit", absolute(out)),
            Err(e) => format!("Attention : {} : {e}", absolute(out)),
        };
    }

    /// Calques dans l'ordre d'apparition (le calque courant compris).
    fn layers(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for s in &self.studio.project().shapes {
            if !v.contains(&s.layer) {
                v.push(s.layer.clone());
            }
        }
        if !v.contains(&self.layer) {
            v.push(self.layer.clone());
        }
        v
    }

    fn shape_label(e: &fablabcam_model::Entity) -> String {
        match e {
            fablabcam_model::Entity::Circle { r, .. } => format!("Cercle r {r:.2}"),
            fablabcam_model::Entity::Arc { r, .. } => format!("Arc r {r:.2}"),
            fablabcam_model::Entity::Point { .. } => "Point".to_string(),
            fablabcam_model::Entity::Rect { w, h, .. } => format!("Rectangle {w:.2} × {h:.2}"),
            fablabcam_model::Entity::Polyline(p) => format!("{} ({} sommets)", if fablabcam_geom::PlineSource::is_closed(p) { "Polyligne fermée" } else { "Polyligne" }, fablabcam_geom::PlineSource::vertex_count(p)),
        }
    }

    /// Onglet « Dessin » du panneau : arbre du projet, puis propriétés de l'objet choisi.
    fn project_tab(&mut self, ui: &mut egui::Ui) {
        // deux zones qui défilent chacune, comme l'arbre et la grille du logiciel de référence
        let h = ui.available_height();
        egui::ScrollArea::vertical().id_source("arbre").max_height(h * 0.42).auto_shrink([false, true]).show(ui, |ui| self.project_tree(ui));
        ui.separator();
        ui.label(egui::RichText::new("Propriétés").strong());
        egui::ScrollArea::vertical().id_source("proprietes").auto_shrink([false, false]).show(ui, |ui| {
            if self.tool == Draw::Text {
                self.text_props(ui);
                ui.separator();
            }
            match self.focus {
                Focus::Op if self.sel_op.is_some() => self.op_props(ui),
                Focus::Shapes if !self.selection.is_empty() => self.shape_props(ui),
                _ => self.project_props(ui),
            }
        });
    }

    fn project_tree(&mut self, ui: &mut egui::Ui) {
        let name = self.doc_path.as_ref().and_then(|p| p.file_stem()).map_or("Sans titre".to_string(), |n| n.to_string_lossy().into_owned());
        let head = egui::CollapsingHeader::new(egui::RichText::new(name).strong()).id_source("projet").default_open(true).show(ui, |ui| {
            let layers = self.layers();
            egui::CollapsingHeader::new(format!("Calques ({})", layers.len())).id_source("calques").default_open(true).show(ui, |ui| {
                for layer in layers {
                    let items: Vec<(u32, String)> = self.studio.project().shapes.iter().filter(|s| s.layer == layer).map(|s| (s.id, Self::shape_label(&s.entity))).collect();
                    let visible = !self.hidden_layers.contains(&layer);
                    let current = self.layer == layer;
                    let title = format!("{layer} ({}){}{}", items.len(), if current { " · courant" } else { "" }, if visible { "" } else { " · masqué" });
                    egui::CollapsingHeader::new(egui::RichText::new(title).color(layer_color(&layer))).id_source(("calque", layer.clone())).default_open(items.len() <= 8).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let mut vis = visible;
                            if ui.checkbox(&mut vis, "visible").changed() {
                                if vis {
                                    self.hidden_layers.retain(|l| l != &layer);
                                } else {
                                    self.hidden_layers.push(layer.clone());
                                    self.selection.retain(|id| self.studio.project().shape(*id).is_some_and(|s| s.layer != layer));
                                }
                            }
                            if ui.add_enabled(!current, egui::Button::new("courant")).on_hover_text("les nouvelles formes vont dans ce calque").clicked() {
                                self.layer = layer.clone();
                            }
                            if ui.button("tout sélectionner").clicked() {
                                self.selection = items.iter().map(|i| i.0).collect();
                                self.focus = Focus::Shapes;
                            }
                        });
                        let mut delete_id = None;
                        for (id, label) in &items {
                            let sel = self.selection.contains(id);
                            let row = ui.selectable_label(sel, format!("{label}  #{id}"));
                            let row_clicked = row.clicked();
                            row.context_menu(|ui| {
                                if ui.button("Supprimer").clicked() {
                                    delete_id = Some(*id);
                                    ui.close_menu();
                                }
                            });
                            if row_clicked {
                                if ui.input(|i| i.modifiers.shift) {
                                    if sel {
                                        self.selection.retain(|x| x != id);
                                    } else {
                                        self.selection.push(*id);
                                    }
                                } else {
                                    self.selection = vec![*id];
                                }
                                self.focus = Focus::Shapes;
                            }
                        }
                        // clic droit sur une forme : supprime la sélection si elle en fait partie, sinon cette forme seule
                        if let Some(id) = delete_id {
                            if !self.selection.contains(&id) {
                                self.selection = vec![id];
                            }
                            self.delete_selection();
                        }
                    });
                }
            });
            let n = self.studio.project().ops.len();
            egui::CollapsingHeader::new(format!("Usinage ({n})")).id_source("usinage").default_open(true).show(ui, |ui| self.ops_tree(ui));
        });
        if head.header_response.clicked() {
            self.focus = Focus::Project;
        }
    }

    fn text_props(&mut self, ui: &mut egui::Ui) {
        ui.label("Texte (\\n = nouvelle ligne) — clic sur le dessin pour le placer");
        ui.text_edit_singleline(&mut self.text_value);
        let inch = self.studio.project().gcode_inches;
        lenf(ui, "hauteur", &mut self.text_height, 0.5, inch);
        lenf(ui, "espacement", &mut self.text_spacing, 0.1, inch);
        ui.label("police .ttf / .otf (vide = système)");
        ui.text_edit_singleline(&mut self.text_font);
        if self.text_font.trim().is_empty() {
            ui.small(fablabcam_studio::text::default_font().map_or("aucune police système trouvée".to_string(), |p| format!("utilisée : {p}")));
        }
    }

    fn shape_props(&mut self, ui: &mut egui::Ui) {
        let inch = self.studio.project().gcode_inches;
        let k = if inch { 25.4 } else { 1.0 };
        let u = if inch { "po" } else { "mm" };
        ui.horizontal(|ui| {
            ui.label(format!("{} forme(s) sélectionnée(s)", self.selection.len()));
            if ui.button("🗑 Supprimer").on_hover_text("supprime la sélection (touche Suppr) ; annulable").clicked() {
                self.delete_selection();
            }
        });
        if self.selection.is_empty() {
            return;
        }
        for id in self.selection.clone().iter().take(6) {
            if let Some(s) = self.studio.project().shape(*id) {
                let (lo, hi) = shape::bounds(&s.entity).unwrap_or(([0.0; 2], [0.0; 2]));
                ui.monospace(format!("#{id} {}\n   {:.3} ; {:.3} → {:.3} ; {:.3} {u}  [{}]", Self::shape_label(&s.entity), lo[0] / k, lo[1] / k, hi[0] / k, hi[1] / k, s.layer));
            }
        }
        if self.selection.len() > 6 {
            ui.small(format!("… et {} autres", self.selection.len() - 6));
        }
        if let [id] = self.selection.as_slice() {
            let id = *id;
            egui::CollapsingHeader::new("Objet : propriétés").default_open(true).show(ui, |ui| self.object_props(ui, id));
        }
        // changer de calque
        let layers = self.layers();
        let mut target = String::new();
        egui::ComboBox::from_label("déplacer vers le calque").selected_text("—").show_ui(ui, |ui| {
            for l in layers {
                if ui.selectable_label(false, &l).clicked() {
                    target = l;
                }
            }
        });
        if !target.is_empty() {
            let ids = self.selection.clone();
            let _ = self.studio.edit(|p| {
                for s in p.shapes.iter_mut().filter(|s| ids.contains(&s.id)) {
                    s.layer = target.clone();
                }
                Ok::<_, shape::EditError>(())
            });
            self.status = format!("{} forme(s) dans le calque « {target} »", ids.len());
        }
        egui::CollapsingHeader::new("Modifier la sélection").default_open(true).show(ui, |ui| self.cad_panel(ui));
    }

    /// Applique une modification d'objet (un seul pas d'annulation) et affiche le résultat ou l'erreur.
    fn edit_selected(&mut self, id: u32, ok: &str, f: impl FnOnce(&fablabcam_model::Entity) -> Result<fablabcam_model::Entity, shape::EditError>) {
        let r = self.studio.edit(|p| p.edit_shape(id, f));
        if self.report(r).is_some() {
            self.status = ok.into();
        }
    }

    /// Propriétés numériques de l'objet sélectionné : valeurs saisies puis « Appliquer » (un pas d'annulation).
    fn object_props(&mut self, ui: &mut egui::Ui, id: u32) {
        let Some(entity) = self.studio.project().shape(id).map(|s| s.entity.clone()) else { return };
        let inch = self.studio.project().gcode_inches;
        let k = if inch { 25.4 } else { 1.0 };
        // le brouillon est relu dès que la forme ou le document change (annulation, déplacement à la souris…)
        let fresh = !matches!(&self.prop_draft, Some(d) if d.id == id && d.rev == self.studio.rev);
        if fresh {
            let mut vals = [0.0; 6];
            let mut verts = Vec::new();
            match &entity {
                fablabcam_model::Entity::Rect { x, y, w, h, r, rot_deg } => vals = [*x, *y, *w, *h, *r, *rot_deg],
                fablabcam_model::Entity::Circle { cx, cy, r } => vals = [*cx, *cy, *r, 0.0, 0.0, 0.0],
                fablabcam_model::Entity::Arc { cx, cy, r, start_deg, sweep_deg } => vals = [*cx, *cy, *r, *start_deg, *sweep_deg, 0.0],
                fablabcam_model::Entity::Polyline(p) => verts = (0..fablabcam_geom::PlineSource::vertex_count(p)).map(|i| { let v = fablabcam_geom::PlineSource::at(p, i); (v.x, v.y, v.bulge) }).collect(),
                fablabcam_model::Entity::Point { .. } => {}
            }
            self.prop_draft = Some(PropDraft { id, rev: self.studio.rev, vals, verts });
        }
        let Some(mut d) = self.prop_draft.take() else { return };
        let mut act: Option<EntityEdit> = None;
        let mut msg = "objet modifié";
        match &entity {
            fablabcam_model::Entity::Rect { .. } => {
                for (i, label) in ["origine X", "origine Y", "largeur", "hauteur", "rayon de coin"].iter().enumerate() {
                    lenf(ui, label, &mut d.vals[i], 0.5, inch);
                }
                num(ui, "rotation (°)", &mut d.vals[5], 1.0);
                let v = d.vals;
                if ui.button("Appliquer").clicked() {
                    act = Some(Box::new(move |e| edit::set_rect(e, v[0], v[1], v[2], v[3], v[4], v[5])));
                    msg = "rectangle modifié";
                }
                ui.small("la rotation tourne le rectangle autour de son coin d'origine ; poignées : les 4 coins");
                if ui.button("Convertir en polyligne (éditer les sommets)").clicked() {
                    act = Some(Box::new(|e| e.to_pline().map(fablabcam_model::Entity::Polyline).ok_or_else(|| shape::EditError::InvalidParameter("conversion impossible".into()))));
                    msg = "rectangle converti en polyligne";
                }
            }
            fablabcam_model::Entity::Circle { .. } => {
                lenf(ui, "centre X", &mut d.vals[0], 0.5, inch);
                lenf(ui, "centre Y", &mut d.vals[1], 0.5, inch);
                lenf(ui, "rayon", &mut d.vals[2], 0.2, inch);
                let mut dia = d.vals[2] * 2.0;
                lenf(ui, "diamètre", &mut dia, 0.4, inch);
                if (dia - d.vals[2] * 2.0).abs() > 1e-12 {
                    d.vals[2] = dia / 2.0;
                }
                let v = d.vals;
                if ui.button("Appliquer").clicked() {
                    act = Some(Box::new(move |e| edit::set_circle(e, v[0], v[1], v[2])));
                    msg = "cercle modifié";
                }
            }
            fablabcam_model::Entity::Arc { .. } => {
                lenf(ui, "centre X", &mut d.vals[0], 0.5, inch);
                lenf(ui, "centre Y", &mut d.vals[1], 0.5, inch);
                lenf(ui, "rayon", &mut d.vals[2], 0.2, inch);
                num(ui, "début (°)", &mut d.vals[3], 1.0);
                num(ui, "balayage (°)", &mut d.vals[4], 1.0);
                let v = d.vals;
                if ui.button("Appliquer").clicked() {
                    act = Some(Box::new(move |e| edit::set_arc(e, v[0], v[1], v[2], v[3], v[4])));
                    msg = "arc modifié";
                }
            }
            fablabcam_model::Entity::Polyline(p) => {
                let closed = fablabcam_geom::PlineSource::is_closed(p);
                ui.label(format!("{} sommets, {}", d.verts.len(), if closed { "fermée" } else { "ouverte" }));
                let mut insert = None;
                let mut remove = None;
                let mut start = None;
                egui::ScrollArea::vertical().max_height(180.0).id_source("verts").show(ui, |ui| {
                    for (i, v) in d.verts.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.monospace(format!("{:>2}", i + 1));
                            let (mut x, mut y) = (v.0 / k, v.1 / k);
                            if ui.add(egui::DragValue::new(&mut x).speed(0.5 / k).max_decimals(3)).changed() {
                                v.0 = x * k;
                            }
                            if ui.add(egui::DragValue::new(&mut y).speed(0.5 / k).max_decimals(3)).changed() {
                                v.1 = y * k;
                            }
                            ui.add(egui::DragValue::new(&mut v.2).speed(0.05).max_decimals(4)).on_hover_text("bulge = tan(balayage / 4) ; 0 = segment droit");
                            if ui.small_button("＋").on_hover_text("insérer un sommet au milieu du segment suivant").clicked() {
                                insert = Some(i);
                            }
                            if ui.small_button("✕").on_hover_text("supprimer ce sommet").clicked() {
                                remove = Some(i);
                            }
                            if closed && ui.small_button("▶").on_hover_text("faire de ce sommet le point de départ").clicked() {
                                start = Some(i);
                            }
                        });
                    }
                });
                let verts = d.verts.clone();
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Appliquer").clicked() {
                        act = Some(Box::new(move |e| edit::set_vertices(e, &verts)));
                        msg = "sommets modifiés";
                    }
                    if ui.button(if closed { "Ouvrir" } else { "Fermer" }).clicked() {
                        act = Some(Box::new(move |e| edit::set_closed(e, !closed)));
                        msg = "polyligne ouverte/fermée";
                    }
                    if ui.button("Inverser le sens").clicked() {
                        act = Some(Box::new(edit::reverse));
                        msg = "sens inversé";
                    }
                    if ui.button("Nettoyer").on_hover_text("retire les sommets en double").clicked() {
                        act = Some(Box::new(edit::clean));
                        msg = "doublons retirés";
                    }
                    if edit::recognize_rect(&entity).is_some() && ui.button("Convertir en rectangle").clicked() {
                        act = Some(Box::new(|e| edit::as_rect(e).ok_or_else(|| shape::EditError::InvalidParameter("la forme n'est pas un rectangle".into()))));
                        msg = "polyligne convertie en rectangle";
                    }
                });
                if let Some(i) = insert {
                    act = Some(Box::new(move |e| edit::insert_vertex(e, i, 0.5)));
                    msg = "sommet inséré";
                }
                if let Some(i) = remove {
                    act = Some(Box::new(move |e| edit::remove_vertex(e, i)));
                    msg = "sommet supprimé";
                }
                if let Some(i) = start {
                    act = Some(Box::new(move |e| edit::set_start(e, i)));
                    msg = "point de départ changé";
                }
            }
            fablabcam_model::Entity::Point { .. } => {
                ui.small("point : se déplace avec « Modifier la sélection »");
            }
        }
        self.prop_draft = Some(d);
        if let Some(f) = act {
            self.edit_selected(id, msg, f);
        }
    }

    /// Propriétés du projet : unités, brut, post-processeur, fichiers par chemin.
    fn project_props(&mut self, ui: &mut egui::Ui) {
        let p = self.studio.project();
        ui.label(format!("{} forme(s) · {} outil(s) · {} opération(s)", p.shapes.len(), p.tools.len(), p.ops.len()));
        ui.horizontal(|ui| {
            ui.label("calque courant");
            ui.text_edit_singleline(&mut self.layer);
        });
        ui.checkbox(&mut self.snap, format!("magnétisme ({} mm)", self.prefs.snap_step));
        ui.separator();
        self.stock_editor(ui);
        ui.separator();
        ui.label(egui::RichText::new("G-code").strong());
        ui.horizontal(|ui| {
            ui.label("post-processeur");
            ui.text_edit_singleline(&mut self.post_path);
        });
        if ui.button("Choisir un post-processeur…").clicked() {
            self.run(Action::PickPost, ui.ctx());
        }
        ui.small("vide = post-processeur intégré");
        if ui.button("Générer le G-code…").clicked() {
            self.run(Action::ExportGcode, ui.ctx());
        }
        ui.separator();
        egui::CollapsingHeader::new("Fichiers par chemin (secours)").default_open(false).show(ui, |ui| {
            ui.text_edit_singleline(&mut self.path);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Importer").clicked() {
                    self.import();
                }
                for ext in ["cb", "dxf", "svg"] {
                    if ui.button(format!("Exporter {}", ext.to_uppercase())).clicked() {
                        self.export(ext);
                    }
                }
            });
        });
    }

    fn fit(&mut self, size: (f64, f64)) {
        if let Some((lo, hi)) = self.studio.project().bounds() {
            self.cam = Camera::fit(lo, hi, size.0, size.1, 50.0);
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        let size = (rect.width() as f64, rect.height() as f64);
        if size.0 < 10.0 || size.1 < 10.0 {
            return;
        }
        // double-clic = cadrer, seulement en sélection : en dessin, deux clics rapides sont deux points
        if self.fit_pending || (resp.double_clicked() && self.tool == Draw::Select && self.pts.is_empty()) {
            self.fit(size);
            self.fit_pending = false;
        }
        let cam = self.cam;
        let to = move |p: [f64; 2]| {
            let (x, y) = cam.to_screen(p, size);
            Pos2::new(rect.left() + x as f32, rect.top() + y as f32)
        };
        let world = |pos: Pos2, cam: &Camera| cam.to_world(((pos.x - rect.left()) as f64, (pos.y - rect.top()) as f64), size);
        let hover_w = resp.hover_pos().map(|p| world(p, &cam));
        self.cursor_w = hover_w.or(self.cursor_w);

        // ---- navigation ----
        if resp.dragged_by(egui::PointerButton::Secondary) || resp.dragged_by(egui::PointerButton::Middle) {
            let d = resp.drag_delta();
            self.cam.pan_px(d.x as f64, d.y as f64);
        }
        if resp.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.scroll_delta.y, i.zoom_delta()));
            let f = (scroll as f64 * 0.01).exp() * zoom as f64;
            if (f - 1.0).abs() > 1e-6 {
                if let Some(p) = resp.hover_pos() {
                    self.cam.zoom_at(f, ((p.x - rect.left()) as f64, (p.y - rect.top()) as f64), size);
                }
            }
        }

        // ---- interactions ----
        self.hover = if self.tool == Draw::Select { hover_w.and_then(|w| self.pick(w)) } else { None };
        let shift = ui.input(|i| i.modifiers.shift);
        let tab_edit = self.focus == Focus::Op && self.tab_mode && self.tool == Draw::Select;
        if let (true, true, Some(w), Some(op)) = (tab_edit, resp.clicked_by(egui::PointerButton::Primary), hover_w, self.sel_op) {
            let radius = 8.0 / cam.scale; // 8 px à l'écran
            // accroché au contour de l'opération : un clic ailleurs ne pose rien
            let r = self.studio.edit(|p| p.click_tab(op, w, radius, 12.0 / cam.scale));
            if let Some(res) = self.report(r) {
                self.status = match res {
                    Some(true) => {
                        let last = self.studio.project().op(op).and_then(|o| o.tab_points.last().copied()).unwrap_or(w);
                        format!("pont posé sur le contour en ({:.1} ; {:.1})", last[0], last[1])
                    }
                    Some(false) => "pont retiré".into(),
                    None => "Attention : cliquez sur le contour de l'opération (la forme ou sa trajectoire) pour poser un pont".into(),
                };
                self.op_draft = self.studio.project().op(op).cloned().zip(self.op_draft.take()).map(|(fresh, mut d)| {
                    d.tab_points = fresh.tab_points;
                    d
                });
            }
        }
        match self.tool {
            Draw::Select => {
                // poignées de la polyligne sélectionnée : glisser un sommet le déplace
                if !tab_edit && resp.drag_started_by(egui::PointerButton::Primary) {
                    if let Some((id, i)) = hover_w.and_then(|w| self.handle_under(w, 8.0 / cam.scale)) {
                        self.vtx_drag = Some((id, i, hover_w.unwrap_or([0.0, 0.0])));
                    }
                }
                if let (Some((id, i, _)), Some(w)) = (self.vtx_drag, hover_w) {
                    if resp.dragged_by(egui::PointerButton::Primary) {
                        self.vtx_drag = Some((id, i, self.snap_pt(w, self.prefs.snap_step)));
                    }
                }
                if resp.drag_released_by(egui::PointerButton::Primary) {
                    if let Some((id, i, w)) = self.vtx_drag.take() {
                        let moved = self.studio.project().shape(id).and_then(|s| edit::drag_handle(&s.entity, i, (w[0], w[1])));
                        if let Some(e) = moved {
                            let r = self.studio.edit(|p| p.replace_shape(id, e));
                            if self.report(r).is_some() {
                                self.status = format!("poignée déplacée en ({:.2} ; {:.2})", w[0], w[1]);
                            }
                        }
                    }
                }
                // pendant la pose des ponts, un clic ne sélectionne ni ne déplace de forme
                if self.vtx_drag.is_none() && !tab_edit && resp.drag_started_by(egui::PointerButton::Primary) {
                    if let Some(w) = hover_w {
                        if let Some(id) = self.pick(w) {
                            if !self.selection.contains(&id) {
                                if !shift {
                                    self.selection.clear();
                                }
                                self.selection.push(id);
                            }
                            self.move_start = Some(w);
                            self.move_delta = [0.0, 0.0];
                        }
                    }
                }
                if self.vtx_drag.is_none() && resp.dragged_by(egui::PointerButton::Primary) {
                    match (self.move_start, hover_w) {
                        (Some(s), Some(w)) => {
                            let d = [w[0] - s[0], w[1] - s[1]];
                            let st = self.prefs.snap_step;
                            self.move_delta = if self.snap { [(d[0] / st).round() * st, (d[1] / st).round() * st] } else { d };
                        }
                        (None, _) => {
                            let d = resp.drag_delta();
                            self.cam.pan_px(d.x as f64, d.y as f64);
                        }
                        _ => {}
                    }
                }
                if resp.drag_released_by(egui::PointerButton::Primary) {
                    if self.move_start.take().is_some() && (self.move_delta[0] != 0.0 || self.move_delta[1] != 0.0) {
                        let (ids, d) = (self.selection.clone(), self.move_delta);
                        let r = self.studio.edit(|p| p.move_shapes(&ids, d[0], d[1]));
                        if self.report(r).is_some() {
                            self.status = format!("{} forme(s) déplacée(s) de ({:.1} ; {:.1})", ids.len(), d[0], d[1]);
                        }
                    }
                    self.move_delta = [0.0, 0.0];
                }
                if resp.clicked() && !tab_edit {
                    let hit = hover_w.and_then(|w| self.pick(w));
                    match hit {
                        Some(id) if shift => {
                            if let Some(i) = self.selection.iter().position(|x| *x == id) {
                                self.selection.remove(i);
                            } else {
                                self.selection.push(id);
                            }
                            self.focus = Focus::Shapes;
                        }
                        Some(id) => {
                            self.selection = vec![id];
                            self.focus = Focus::Shapes;
                        }
                        None => {
                            self.selection.clear();
                            if self.focus == Focus::Shapes {
                                self.focus = Focus::Project;
                            }
                        }
                    }
                }
            }
            _ => {
                if resp.clicked_by(egui::PointerButton::Primary) {
                    if let Some(w) = hover_w {
                        let w = self.snap_pt(w, self.prefs.snap_step);
                        self.pts.push(w);
                        if Some(self.pts.len()) == self.tool.clicks() {
                            self.commit_shape();
                        }
                    }
                }
                if resp.clicked_by(egui::PointerButton::Secondary) && self.tool == Draw::Polyline {
                    self.finish_polyline(false);
                }
            }
        }

        // ---- menu contextuel (clic droit, en sélection) ----
        if self.tool == Draw::Select {
            let mut act: Option<Action> = None;
            let mut xf: Option<Similarity> = None;
            let mut cad_act: Option<CadAct> = None;
            let has_sel = !self.selection.is_empty();
            let center = self.selection_center().unwrap_or((0.0, 0.0));
            resp.context_menu(|ui| {
                let pick = |ui: &mut egui::Ui, label: &str, keys: &str, enabled: bool, a: Action, act: &mut Option<Action>| {
                    if ui.add_enabled(enabled, egui::Button::new(label).shortcut_text(keys)).clicked() {
                        *act = Some(a);
                        ui.close_menu();
                    }
                };
                ui.menu_button("Affichage", |ui| {
                    pick(ui, "Cadrer", "F", true, Action::Fit, &mut act);
                    pick(ui, "3D", "2", true, Action::View(View::Vue3D), &mut act);
                    pick(ui, "Simulation", "3", true, Action::View(View::Simulation), &mut act);
                });
                ui.add_enabled_ui(has_sel, |ui| {
                    ui.menu_button("Transformer", |ui| {
                        for (label, t) in [
                            ("Tourner de +90°", Similarity::rotation(center, 90.0)),
                            ("Tourner de −90°", Similarity::rotation(center, -90.0)),
                            ("Miroir gauche/droite", Similarity::mirror(center, 90.0)),
                            ("Miroir haut/bas", Similarity::mirror(center, 0.0)),
                        ] {
                            if ui.button(label).clicked() {
                                xf = Some(t);
                                ui.close_menu();
                            }
                        }
                    });
                    ui.menu_button("Modifier", |ui| {
                        for (label, c) in [("Décaler", CadAct::Offset), ("Arrondir les coins", CadAct::Fillet), ("Joindre", CadAct::Join), ("Éclater", CadAct::Explode)] {
                            if ui.button(label).clicked() {
                                cad_act = Some(c);
                                ui.close_menu();
                            }
                        }
                    });
                    ui.menu_button("Usinage", |ui| {
                        for k in OpKind::ALL {
                            if ui.button(format!("{} de la sélection", k.label())).clicked() {
                                act = Some(Action::NewOp(k));
                                ui.close_menu();
                            }
                        }
                    });
                });
                ui.separator();
                pick(ui, "Tout sélectionner", "Cmd+A", true, Action::SelectAll, &mut act);
                pick(ui, "Couper", "Cmd+X", has_sel, Action::Cut, &mut act);
                pick(ui, "Copier", "Cmd+C", has_sel, Action::Copy, &mut act);
                pick(ui, "Coller", "Cmd+V", !self.clipboard.is_empty(), Action::Paste, &mut act);
                pick(ui, "Supprimer", "Suppr", has_sel, Action::Delete, &mut act);
                ui.separator();
                pick(ui, "Annuler", "Cmd+Z", self.studio.editor.can_undo(), Action::Undo, &mut act);
                pick(ui, "Rétablir", "Maj+Cmd+Z", self.studio.editor.can_redo(), Action::Redo, &mut act);
                if self.focus == Focus::Op {
                    ui.separator();
                    if ui.checkbox(&mut self.tab_mode, "Ponts de maintien : poser au clic").clicked() {
                        ui.close_menu();
                    }
                }
            });
            if let Some(a) = act {
                self.run(a, ui.ctx());
            }
            if let Some(t) = xf {
                let ids = self.selection.clone();
                let r = self.studio.edit(|p| p.transform_shapes(&ids, &t));
                if self.report(r).is_some() {
                    self.status = format!("{} forme(s) transformée(s)", ids.len());
                }
            }
            if let Some(c) = cad_act {
                self.apply_cad(c);
            }
        }

        // ---- dessin ----
        let painter = painter.with_clip_rect(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(14, 17, 20));
        let step = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0].into_iter().find(|s| s * cam.scale >= 40.0).unwrap_or(500.0);
        let (w0, w1) = (cam.to_world((0.0, size.1), size), cam.to_world((size.0, 0.0), size));
        let grid = Stroke::new(1.0_f32, Color32::from_rgb(30, 36, 42));
        let mut x = (w0[0] / step).floor() * step;
        while x <= w1[0] {
            painter.line_segment([to([x, w0[1]]), to([x, w1[1]])], grid);
            x += step;
        }
        let mut y = (w0[1] / step).floor() * step;
        while y <= w1[1] {
            painter.line_segment([to([w0[0], y]), to([w1[0], y])], grid);
            y += step;
        }
        let axes = Stroke::new(1.0_f32, Color32::from_rgb(60, 80, 94));
        painter.line_segment([to([w0[0], 0.0]), to([w1[0], 0.0])], axes);
        painter.line_segment([to([0.0, w0[1]]), to([0.0, w1[1]])], axes);
        painter.text(rect.right_bottom() - egui::vec2(8.0, 8.0), egui::Align2::RIGHT_BOTTOM, if self.studio.project().gcode_inches { format!("grille {step} mm ({:.3} po)", step / 25.4) } else { format!("grille {step} mm") }, egui::FontId::proportional(11.0), Color32::from_gray(140));

        let op_focus = self.focus == Focus::Op;
        for s in &self.studio.project().shapes {
            if self.hidden_layers.contains(&s.layer) {
                continue;
            }
            let selected = self.selection.contains(&s.id);
            let off = if selected { self.move_delta } else { [0.0, 0.0] };
            let in_op = op_focus && self.sel_op.and_then(|o| self.studio.project().op(o)).is_some_and(|o| o.shapes.contains(&s.id));
            let color = if in_op { Color32::from_rgb(40, 200, 180) } else if op_focus && !selected { Color32::from_gray(110) } else if selected { Color32::from_rgb(255, 122, 0) } else if self.hover == Some(s.id) { Color32::from_rgb(0, 140, 255) } else { layer_color(&s.layer) };
            let width = if selected || self.hover == Some(s.id) { 2.6_f32 } else { 1.6 };
            let edited = self.vtx_drag.filter(|v| v.0 == s.id).and_then(|(_, h, w)| edit::drag_handle(&s.entity, h, (w[0], w[1])));
            let ent = edited.as_ref().unwrap_or(&s.entity);
            let pts: Vec<Pos2> = shape::sample(ent).iter().map(|p| to([p[0] + off[0], p[1] + off[1]])).collect();
            if selected && self.selection.len() == 1 && self.tool == Draw::Select {
                for (h, v) in edit::handles(ent) {
                    let c = to([v.0, v.1]);
                    let sq = egui::Rect::from_center_size(c, egui::vec2(7.0, 7.0));
                    let stroke = Stroke::new(1.5_f32, Color32::from_rgb(255, 122, 0));
                    // carré : sommet ou coin ; rond : centre, rayon, extrémité d'arc
                    if matches!(h, Handle::Vertex(_) | Handle::Corner(_)) {
                        painter.rect_stroke(sq, 0.0, stroke);
                    } else {
                        painter.circle_stroke(c, 4.5, stroke);
                    }
                }
            }
            if pts.len() == 1 {
                painter.circle_filled(pts[0], 3.0, color);
            } else {
                if selected {
                    painter.add(egui::Shape::line(pts.clone(), Stroke::new(8.0_f32, Color32::from_rgba_unmultiplied(255, 122, 0, 70))));
                }
                painter.add(egui::Shape::line(pts, Stroke::new(width, color)));
            }
        }
        if self.show_toolpaths {
            if self.scene.0 != self.studio.rev {
                let bp = self.studio.backplot();
                self.scene = (self.studio.rev, build_scene(&bp, &ViewOptions { arrow_spacing_mm: 14.0, ..ViewOptions::default() }));
            }
            for p in &self.scene.1 {
                match p {
                    Prim::Line { pts, rgb: c, width, dashed } => {
                        let pts: Vec<Pos2> = pts.iter().map(|q| to(*q)).collect();
                        let stroke = Stroke::new(*width, rgb(*c));
                        if *dashed {
                            painter.extend(egui::Shape::dashed_line(&pts, stroke, 5.0, 4.0));
                        } else {
                            painter.add(egui::Shape::line(pts, stroke));
                        }
                    }
                    Prim::Arrow { pos, dir, rgb: c } => {
                        let o = to(*pos);
                        let d = egui::Vec2::new(dir[0] as f32, -dir[1] as f32);
                        let n = egui::Vec2::new(-d.y, d.x);
                        painter.add(egui::Shape::convex_polygon(vec![o + d * 5.0, o - d * 3.0 + n * 3.0, o - d * 3.0 - n * 3.0], rgb(*c), Stroke::NONE));
                    }
                    Prim::Dot { pos, rgb: c, radius, ring } => {
                        let o = to(*pos);
                        if *ring {
                            painter.circle_stroke(o, *radius, Stroke::new(2.0_f32, rgb(*c)));
                        } else {
                            painter.circle_filled(o, *radius, rgb(*c));
                        }
                    }
                }
            }
        }
        if op_focus {
            let marks: Vec<[f64; 2]> = self.sel_op.and_then(|id| self.studio.project().op(id)).map(|o| o.tab_points.clone()).unwrap_or_default();
            for m in marks {
                painter.circle_filled(to(m), 5.0, Color32::from_rgb(142, 68, 173));
                painter.circle_stroke(to(m), 8.0, Stroke::new(1.5_f32, Color32::from_rgb(142, 68, 173)));
            }
        }
        if tab_edit {
            let msg = "Ponts manuels : clic sur le contour = poser · clic sur un pont = retirer · Échap = terminer";
            let r = egui::Rect::from_min_size(rect.left_top() + egui::vec2(10.0, 10.0), egui::vec2(640.0, 26.0));
            painter.rect_filled(r, 6.0, Color32::from_rgba_unmultiplied(120, 50, 160, 220));
            painter.text(r.left_center() + egui::vec2(10.0, 0.0), egui::Align2::LEFT_CENTER, msg, egui::FontId::proportional(13.0), Color32::WHITE);
        }
        // aperçu de la forme en cours
        if let (Some(cur), false) = (hover_w, self.pts.is_empty()) {
            let cur = self.snap_pt(cur, self.prefs.snap_step);
            let preview = Stroke::new(1.4_f32, Color32::from_rgb(220, 60, 60));
            let mut all = self.pts.clone();
            all.push(cur);
            let e = match self.tool {
                Draw::Line => shape::line((all[0][0], all[0][1]), (cur[0], cur[1])).ok(),
                Draw::Rect => shape::rectangle((all[0][0], all[0][1]), (cur[0], cur[1])).ok(),
                Draw::Circle => shape::circle((all[0][0], all[0][1]), (cur[0] - all[0][0]).hypot(cur[1] - all[0][1])).ok(),
                Draw::Arc if all.len() == 2 => shape::line((all[0][0], all[0][1]), (cur[0], cur[1])).ok(),
                Draw::Arc => {
                    let r = (all[1][0] - all[0][0]).hypot(all[1][1] - all[0][1]);
                    let a0 = (all[1][1] - all[0][1]).atan2(all[1][0] - all[0][0]).to_degrees();
                    let a1 = (cur[1] - all[0][1]).atan2(cur[0] - all[0][0]).to_degrees();
                    let mut sw = (a1 - a0) % 360.0;
                    if sw <= 0.0 {
                        sw += 360.0;
                    }
                    shape::arc((all[0][0], all[0][1]), r, a0, sw).ok()
                }
                Draw::Polyline => shape::polyline(&all.iter().map(|p| (p[0], p[1])).collect::<Vec<_>>(), false).ok(),
                Draw::Select | Draw::Text => None,
            };
            if let Some(e) = e {
                painter.add(egui::Shape::line(shape::sample(&e).iter().map(|p| to(*p)).collect(), preview));
            }
            painter.text(to(cur) + egui::vec2(10.0, -10.0), egui::Align2::LEFT_BOTTOM, {
                let k = if self.studio.project().gcode_inches { 25.4 } else { 1.0 };
                format!("{:.3} ; {:.3}", cur[0] / k, cur[1] / k)
            }, egui::FontId::monospace(11.0), Color32::from_rgb(255, 120, 100));
        }
        for p in &self.pts {
            painter.circle_filled(to(*p), 3.5, Color32::from_rgb(220, 60, 60));
        }
        // position (aimantée en dessin) affichée dans la barre d'état, dans l'unité du projet
        if let Some(w) = hover_w {
            self.cursor_w = Some(if self.tool == Draw::Select { w } else { self.snap_pt(w, self.prefs.snap_step) });
        }
    }
}

/// Longueur affichée et saisie dans l'unité du projet ; la valeur reste en mm.
fn lenf(ui: &mut egui::Ui, label: &str, v: &mut f64, speed: f64, inch: bool) {
    unit_field(ui, label, v, speed, inch, "")
}

/// Avance (longueur par minute) dans l'unité du projet.
fn ratef(ui: &mut egui::Ui, label: &str, v: &mut f64, speed: f64, inch: bool) {
    unit_field(ui, label, v, speed, inch, "/min")
}

fn unit_field(ui: &mut egui::Ui, label: &str, v: &mut f64, speed: f64, inch: bool, per: &str) {
    let k = if inch { 25.4 } else { 1.0 };
    let mut d = *v / k;
    ui.horizontal(|ui| {
        ui.label(format!("{label} ({}{per})", if inch { "po" } else { "mm" }));
        // seule une vraie saisie réécrit la valeur : pas de dérive par conversion
        if ui.add(egui::DragValue::new(&mut d).speed(speed / k).max_decimals(if inch { 4 } else { 3 })).changed() {
            *v = d * k;
        }
    });
}

fn num(ui: &mut egui::Ui, label: &str, v: &mut f64, speed: f64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(v).speed(speed));
    });
}

impl App {
    // ---------- onglet Outils ----------
    fn load_tool_draft(&mut self) {
        let want = self.sel_tool.clone();
        let have = self.tool_draft.as_ref().and_then(|d| d.0.clone());
        if self.tool_draft.as_ref().map_or(true, |d| d.0.is_some()) && want != have {
            self.tool_draft = want.and_then(|n| self.studio.project().tool(&n).cloned().map(|t| (Some(n), t)));
        }
    }

    /// Onglet « Système » : table d'outils du projet, fiche de l'outil, bibliothèque commune, post-processeur.
    fn system_tab(&mut self, ui: &mut egui::Ui) {
        self.tools_side(ui);
        ui.separator();
        self.tool_form(ui);
        ui.separator();
        self.library_panel(ui);
        ui.separator();
        ui.label(egui::RichText::new("Post-processeur").strong());
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.post_path);
        });
        if ui.button("Choisir…").clicked() {
            self.run(Action::PickPost, ui.ctx());
        }
        ui.small("vide = post-processeur intégré ; enregistré dans les préférences");
        if ui.button("Préférences…").clicked() {
            self.show_prefs = true;
        }
    }

    fn library_path() -> Option<PathBuf> {
        Prefs::file().and_then(|f| f.parent().map(|d| d.join("outils.cb")))
    }

    fn save_library(&mut self) {
        let Some(path) = Self::library_path() else { return };
        let mut p = Project::new();
        for t in &self.library {
            let _ = p.add_tool(Tool { index: 0, ..t.clone() });
        }
        let r = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::write(&path, persist::save(&p)));
        if let Err(e) = r {
            self.status = format!("Attention : bibliothèque : {e}");
        }
    }

    /// Outils réutilisables d'un projet à l'autre (fichier `outils.cb` à côté des préférences).
    fn library_panel(&mut self, ui: &mut egui::Ui) {
        if !self.lib_loaded {
            self.lib_loaded = true;
            if let Some(t) = Self::library_path().and_then(|p| std::fs::read_to_string(p).ok()) {
                self.library = persist::load(&t).map(|p| p.tools).unwrap_or_default();
            }
        }
        ui.label(egui::RichText::new("Bibliothèque d'outils (tous les projets)").strong());
        let current = self.sel_tool.clone().and_then(|n| self.studio.project().tool(&n).cloned());
        if let Some(t) = current {
            if ui.button(format!("Ranger « {} » dans la bibliothèque", t.name)).clicked() {
                self.library.retain(|x| x.name != t.name);
                self.library.push(t.clone());
                self.save_library();
                self.status = format!("outil « {} » ajouté à la bibliothèque", t.name);
            }
        }
        if self.library.is_empty() {
            ui.small("vide : sélectionnez un outil du projet puis « Ranger… »");
        }
        let mut add = None;
        let mut remove = None;
        for (i, t) in self.library.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{} — Ø{} {}", t.name, t.diameter, t.kind.label()));
                if ui.small_button("ajouter au projet").clicked() {
                    add = Some(t.clone());
                }
                if ui.small_button("×").on_hover_text("Retirer de la bibliothèque").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(t) = add {
            let name = t.name.clone();
            match self.studio.edit(|p| p.add_tool(Tool { index: 0, ..t })) {
                Ok(()) => {
                    self.sel_tool = Some(name.clone());
                    self.tool_draft = None;
                    self.status = format!("outil « {name} » ajouté au projet");
                }
                Err(e) => self.status = format!("Attention : {e}"),
            }
        }
        if let Some(i) = remove {
            self.library.remove(i);
            self.save_library();
        }
    }

    fn tools_side(&mut self, ui: &mut egui::Ui) {
        ui.heading("Table d'outils");
        ui.label(format!("{} outil(s)", self.studio.project().tools.len()));
        ui.separator();
        let names: Vec<String> = self.studio.project().tools.iter().map(|t| t.name.clone()).collect();
        for n in names.iter() {
            let t = self.studio.project().tool(n).cloned();
            let label = t.map_or(n.clone(), |t| format!("T{} {} — Ø{} mm", t.index, t.name, t.diameter));
            if ui.selectable_label(self.sel_tool.as_deref() == Some(n.as_str()), label).clicked() {
                self.sel_tool = Some(n.clone());
                self.tool_draft = None;
            }
        }
        ui.separator();
        if ui.button("Nouvel outil").clicked() {
            let mut k = names.len() + 1;
            while names.contains(&format!("Outil {k}")) {
                k += 1;
            }
            self.tool_draft = Some((None, Tool::new(&format!("Outil {k}"), ToolKind::EndMill, 3.0)));
            self.sel_tool = None;
        }
        ui.separator();
        ui.label("Créer depuis un style XML");
        ui.text_edit_singleline(&mut self.styles_path);
        ui.horizontal(|ui| {
            ui.label("style");
            ui.text_edit_singleline(&mut self.style_name);
        });
        if ui.button("Créer l'outil depuis le style").clicked() {
            let res = std::fs::read_to_string(&self.styles_path)
                .map_err(|e| e.to_string())
                .and_then(|t| fablabcam_cam::StyleLibrary::parse(&t).map_err(|e| e.to_string()))
                .and_then(|lib| Tool::from_style(&format!("Style {}", self.style_name), &lib, &self.style_name).map_err(|e| e.to_string()));
            match res {
                Ok(t) => {
                    self.tool_draft = Some((None, t));
                    self.sel_tool = None;
                }
                Err(e) => self.status = format!("Attention : {e}"),
            }
        }
    }

    fn tool_form(&mut self, ui: &mut egui::Ui) {
        let inch = self.studio.project().gcode_inches;
        self.load_tool_draft();
        let Some((orig, mut t)) = self.tool_draft.clone() else {
            ui.heading("Outil");
            ui.label("Sélectionnez un outil ou créez-en un.");
            return;
        };
        ui.heading(if orig.is_some() { "Modifier l'outil" } else { "Nouvel outil" });
        ui.add_space(6.0);
        ui.vertical(|ui| {
            egui::CollapsingHeader::new("Général").default_open(true).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("nom");
                    ui.text_edit_singleline(&mut t.name);
                });
                let mut idx = t.index as f64;
                num(ui, "numéro d'outil (0 = auto)", &mut idx, 1.0);
                t.index = idx.max(0.0) as u32;
                egui::ComboBox::from_label("type").selected_text(t.kind.label()).show_ui(ui, |ui| {
                    for k in ToolKind::ALL {
                        ui.selectable_value(&mut t.kind, k, k.label());
                    }
                });
                lenf(ui, "diamètre", &mut t.diameter, 0.1, inch);
                let mut fl = t.flutes as f64;
                num(ui, "nombre de dents", &mut fl, 0.1);
                t.flutes = fl.max(1.0) as u32;
            });
            egui::CollapsingHeader::new("Avances et broche").default_open(true).show(ui, |ui| {
                ratef(ui, "avance de coupe", &mut t.feed, 10.0, inch);
                ratef(ui, "avance de plongée", &mut t.plunge, 10.0, inch);
                num(ui, "broche (tr/min)", &mut t.spindle, 100.0);
                lenf(ui, "charge par dent", &mut t.tooth_load, 0.001, inch);
            });
            egui::CollapsingHeader::new("Calculateur de vitesses et avances").default_open(false).show(ui, |ui| {
                let mats = fablabcam_studio::feeds::MATERIALS;
                let cur = mats.get(self.calc_mat).map_or("—", |m| m.name);
                egui::ComboBox::from_label("matière (valeurs indicatives)").selected_text(cur).show_ui(ui, |ui| {
                    for (i, m) in mats.iter().enumerate() {
                        if ui.selectable_value(&mut self.calc_mat, i, m.name).clicked() {
                            self.calc_vc = m.vc;
                            self.calc_fz = (m.fz_per_d * t.diameter * 1000.0).round() / 1000.0;
                        }
                    }
                });
                num(ui, "vitesse de coupe Vc (m/min)", &mut self.calc_vc, 5.0);
                lenf(ui, "avance par dent fz", &mut self.calc_fz, 0.001, inch);
                num(ui, "broche maxi de la machine (tr/min)", &mut self.calc_max_rpm, 500.0);
                match fablabcam_studio::feeds::compute(t.diameter, t.flutes, self.calc_vc, self.calc_fz, self.calc_max_rpm) {
                    Ok(c) => {
                        ui.label(format!("broche {} tr/min · avance {} mm/min · plongée {} mm/min", c.rpm, c.feed, c.plunge));
                        if c.limited {
                            ui.small(format!("broche plafonnée : Vc réelle {:.0} m/min", c.vc));
                        }
                        if ui.button("Appliquer à l'outil").clicked() {
                            (t.spindle, t.feed, t.plunge, t.tooth_load) = (c.rpm, c.feed, c.plunge, self.calc_fz);
                        }
                    }
                    Err(e) => {
                        ui.colored_label(Color32::from_rgb(220, 80, 60), e);
                    }
                }
                ui.small("ordres de grandeur pour fraise carbure ; suivez les données du fabricant");
            });
            egui::CollapsingHeader::new("Géométrie").default_open(true).show(ui, |ui| {
                lenf(ui, "longueur de coupe", &mut t.flute_length, 0.5, inch);
                lenf(ui, "longueur totale", &mut t.length, 0.5, inch);
                lenf(ui, "diamètre de queue", &mut t.shank_diameter, 0.1, inch);
                num(ui, "angle d'hélice (°)", &mut t.helix_angle, 1.0);
                if t.kind == ToolKind::VBit {
                    num(ui, "angle du V (°)", &mut t.vee_angle, 1.0);
                }
                num(ui, "angle de rampe maxi (°)", &mut t.max_ramp_angle, 1.0);
            });
            egui::CollapsingHeader::new("Limites de coupe").default_open(false).show(ui, |ui| {
                lenf(ui, "profondeur de passe axiale", &mut t.axial_depth, 0.1, inch);
                lenf(ui, "passe radiale", &mut t.radial_depth, 0.1, inch);
            });
            egui::CollapsingHeader::new("Informations").default_open(false).show(ui, |ui| {
                for (label, field) in [("matière", &mut t.material), ("revêtement", &mut t.coating), ("référence", &mut t.part_code), ("commentaire ({$tool.comment})", &mut t.comment)] {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.text_edit_singleline(field);
                    });
                }
                ui.label("notes");
                ui.add(egui::TextEdit::multiline(&mut t.notes).desired_rows(2).desired_width(f32::INFINITY));
            });
            egui::CollapsingHeader::new("Changement d'outil (G-code)").default_open(false).show(ui, |ui| {
                ui.small("Si renseigné, remplace le changement d'outil par défaut pour cet outil.");
                ui.add(egui::TextEdit::multiline(&mut t.tool_change).desired_rows(2).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
            });
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            // outil existant : enregistré dès la fin de la saisie ; nouvel outil : bouton « Créer »
            let editing = ui.input(|i| i.pointer.any_down()) || ui.ctx().wants_keyboard_input();
            let changed = orig.as_ref().and_then(|o| self.studio.project().tool(o)).is_some_and(|st| st != &t);
            let commit = if orig.is_some() { changed && !editing } else { ui.button("Créer").clicked() };
            if changed && editing {
                ui.small("modification en cours…");
            }
            if commit {
                let name = t.name.clone();
                let r = match &orig {
                    Some(o) => self.studio.edit(|p| p.update_tool(o, t.clone())),
                    None => self.studio.edit(|p| p.add_tool(t.clone())),
                };
                match r {
                    Ok(()) => {
                        self.status = format!("outil « {name} » enregistré");
                        self.sel_tool = Some(name.clone());
                        self.tool_draft = Some((Some(name), t.clone()));
                    }
                    Err(e) => self.status = format!("Attention : {e}"),
                }
            }
            if let Some(o) = &orig {
                if ui.button("Supprimer").clicked() {
                    let rep = (!self.replacement.is_empty()).then(|| self.replacement.clone());
                    match self.studio.edit(|p| p.delete_tool(o, rep.as_deref())) {
                        Ok(()) => {
                            self.status = format!("outil « {o} » supprimé");
                            self.sel_tool = self.studio.project().tools.first().map(|t| t.name.clone());
                            self.tool_draft = None;
                        }
                        Err(e) => self.status = format!("Attention : {e} — choisissez un outil de remplacement ci-dessous"),
                    }
                }
            }
        });
        if let Some(o) = &orig {
            egui::ComboBox::from_label("remplacer par (si l'outil est utilisé)")
                .selected_text(if self.replacement.is_empty() { "—" } else { self.replacement.as_str() })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.replacement, String::new(), "—");
                    for n in self.studio.project().tools.iter().map(|x| x.name.clone()).filter(|n| n != o) {
                        ui.selectable_value(&mut self.replacement, n.clone(), n);
                    }
                });
            let users: Vec<&str> = self.studio.project().ops.iter().filter(|op| &op.tool == o).map(|op| op.name.as_str()).collect();
            if !users.is_empty() {
                ui.label(format!("utilisé par : {}", users.join(", ")));
            }
        }
        if let Some(d) = &mut self.tool_draft {
            d.1 = t;
        }
    }

    // ---------- onglet Opérations ----------
    /// Écrit le G-code dans `out`, avec le post-processeur `.cbpp` indiqué le cas échéant.
    fn write_gcode(&self, out: &Path) -> String {
        let code = if self.post_path.trim().is_empty() {
            self.studio.gcode()
        } else {
            let parsed = std::fs::read_to_string(self.post_path.trim()).map_err(|e| format!("{} : {e}", self.post_path.trim())).and_then(|x| fablabcam_cam::PostDef::parse(&x));
            match parsed {
                Ok((def, _)) => {
                    let out = self.studio.gcode_with_post(&def);
                    if let Some(w) = out.warnings.first() {
                        return format!("Attention : post-processeur : {w}");
                    }
                    out.gcode
                }
                Err(e) => return format!("Attention : {e}"),
            }
        };
        match std::fs::write(out, code) {
            Ok(()) => format!("{} écrit", absolute(out)),
            Err(e) => format!("Attention : {} : {e}", absolute(out)),
        }
    }

    fn load_op_draft(&mut self) {
        let want = self.sel_op;
        if self.op_draft.as_ref().map(|o| o.id) != want {
            self.op_draft = want.and_then(|id| self.studio.project().op(id).cloned());
        }
    }

    /// Branche « Usinage » de l'arbre : pièces et opérations, création et ordre.
    fn ops_tree(&mut self, ui: &mut egui::Ui) {
        let ops: Vec<(u32, String, bool, OpKind, String)> = self.studio.project().ops.iter().map(|o| (o.id, o.name.clone(), o.enabled, o.kind, o.part.clone())).collect();
        let mut shown_part: Option<&str> = None;
        let mut to_delete: Option<(u32, String)> = None;
        for (id, name, enabled, kind, part) in &ops {
            // en-tête de pièce : une case pour activer / désactiver toutes ses opérations
            if !part.is_empty() && shown_part != Some(part.as_str()) {
                shown_part = Some(part);
                let all_on = ops.iter().filter(|o| &o.4 == part).all(|o| o.2);
                let mut on = all_on;
                if ui.checkbox(&mut on, egui::RichText::new(format!("Pièce : {part}")).strong()).changed() {
                    let part = part.clone();
                    let _ = self.studio.edit(|p| Ok::<_, shape::EditError>(p.set_part_enabled(&part, on)));
                    self.op_draft = None;
                }
            } else if part.is_empty() {
                shown_part = None;
            }
            let err = self.studio.computed.get(id).and_then(|c| c.error.as_ref()).is_some();
            ui.horizontal(|ui| {
                let mut en = *enabled;
                if ui.checkbox(&mut en, "").changed() {
                    let id = *id;
                    let _ = self.studio.edit(|p| p.update_operation(id, |o| o.enabled = en));
                    self.op_draft = None;
                }
                let label = format!("{}{}{} — {}", if part.is_empty() { "" } else { "   " }, if err { "! " } else { "" }, name, kind.label());
                let focused = self.sel_op == Some(*id) && self.focus == Focus::Op;
                if ui.selectable_label(focused, label).clicked() {
                    self.sel_op = Some(*id);
                    self.op_draft = None;
                    self.focus = Focus::Op;
                }
                // suppression directe depuis la ligne (annulable par Cmd+Z)
                if ui.small_button("×").on_hover_text(format!("Supprimer « {name} »")).clicked() {
                    to_delete = Some((*id, name.clone()));
                }
            });
        }
        if let Some((id, name)) = to_delete {
            if self.studio.edit(|p| p.delete_operation(id)).is_ok() {
                if self.sel_op == Some(id) {
                    self.sel_op = self.studio.project().ops.first().map(|o| o.id);
                }
                self.op_draft = None;
                self.status = format!("opération « {name} » supprimée (Cmd+Z pour annuler)");
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_source("newkind").selected_text(self.new_kind.label()).show_ui(ui, |ui| {
                for k in OpKind::ALL {
                    ui.selectable_value(&mut self.new_kind, k, k.label());
                }
            });
            let n_sel = self.selection.len();
            if ui.add_enabled(n_sel > 0, egui::Button::new(format!("Ajouter ({n_sel} forme(s))"))).on_disabled_hover_text("Sélectionnez d'abord des formes").clicked() {
                self.add_op_for_selection(self.new_kind);
            }
        });
        if let (Some(id), Focus::Op) = (self.sel_op, self.focus) {
            let pos = ops.iter().position(|o| o.0 == id);
            ui.horizontal(|ui| {
                if ui.add_enabled(pos.is_some_and(|p| p > 0), egui::Button::new("Monter")).clicked() {
                    let to = pos.unwrap_or(0).saturating_sub(1);
                    let _ = self.studio.edit(|p| p.move_operation(id, to));
                }
                if ui.add_enabled(pos.is_some_and(|p| p + 1 < ops.len()), egui::Button::new("Descendre")).clicked() {
                    let to = pos.unwrap_or(0) + 1;
                    let _ = self.studio.edit(|p| p.move_operation(id, to));
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Dupliquer").clicked() {
                    if let Ok(n) = self.studio.edit(|p| p.duplicate_operation(id)) {
                        self.sel_op = Some(n);
                        self.op_draft = None;
                    }
                }
                if ui.button("Supprimer l'opération").clicked() && self.studio.edit(|p| p.delete_operation(id)).is_ok() {
                    self.sel_op = self.studio.project().ops.first().map(|o| o.id);
                    self.op_draft = None;
                }
            });
        }
        if self.selection.is_empty() && ops.is_empty() {
            ui.colored_label(Color32::from_rgb(230, 170, 60), "Sélectionnez une forme (clic sur le dessin ou dans Calques), puis un type d'opération dans la barre d'outils.");
        }
    }

    /// Propriétés de l'opération choisie : appliquées dès qu'une saisie est terminée.
    fn op_props(&mut self, ui: &mut egui::Ui) {
        self.load_op_draft();
        let Some(mut o) = self.op_draft.clone() else {
            ui.label("Sélectionnez une opération.");
            return;
        };
        let inch = self.studio.project().gcode_inches;
        ui.label(egui::RichText::new(format!("Opération — {}", o.kind.label())).strong());
        ui.horizontal(|ui| {
            ui.label("nom");
            ui.text_edit_singleline(&mut o.name);
        });
        if o.kind == OpKind::Profile {
            egui::ComboBox::from_label("côté").selected_text(o.side.label()).show_ui(ui, |ui| {
                for s in OpSide::ALL {
                    ui.selectable_value(&mut o.side, s, s.label());
                }
            });
        }
        let tools: Vec<String> = self.studio.project().tools.iter().map(|t| t.name.clone()).collect();
        egui::ComboBox::from_label("outil").selected_text(o.tool.clone()).show_ui(ui, |ui| {
            for t in &tools {
                ui.selectable_value(&mut o.tool, t.clone(), t);
            }
        });
        ui.horizontal(|ui| {
            ui.label("pièce");
            ui.text_edit_singleline(&mut o.part);
        });
        lenf(ui, "profondeur", &mut o.depth, 0.1, inch);
        if o.kind == OpKind::Drill {
            num(ui, "temporisation au fond (s, G82)", &mut o.dwell, 0.1);
            if o.dwell > 0.0 && o.step_down > 0.0 {
                ui.small("sans effet avec le picage (incrément > 0)");
            }
        }
        if o.kind != OpKind::Drill {
            lenf(ui, "incrément de passe", &mut o.step_down, 0.1, inch);
            num(ui, "recouvrement (0-1)", &mut o.step_over, 0.01);
            if o.kind != OpKind::Engrave {
                lenf(ui, "matière laissée", &mut o.stock_to_leave, 0.05, inch);
            }
            ui.checkbox(&mut o.climb, "fraisage en avalant (climb)");
            egui::ComboBox::from_label("entrée/sortie").selected_text(o.lead.label()).show_ui(ui, |ui| {
                for l in LeadChoice::ALL {
                    ui.selectable_value(&mut o.lead, l, l.label());
                }
            });
        }
        egui::CollapsingHeader::new("Hauteurs").id_source(("h", o.id)).show(ui, |ui| {
            lenf(ui, "surface du brut Z", &mut o.cam.surface, 0.1, inch);
            lenf(ui, "plan de dégagement Z", &mut o.cam.clearance, 0.5, inch);
        });
        if o.kind != OpKind::Drill {
            egui::CollapsingHeader::new("Passes et finition").id_source(("p", o.id)).show(ui, |ui| {
                ui.checkbox(&mut o.cam.level_first, "niveau par niveau (sinon contour par contour)");
                lenf(ui, "dernière passe, 0 = idem", &mut o.cam.final_increment, 0.05, inch);
                lenf(ui, "passe de finition, 0 = aucune", &mut o.cam.finish_stepover, 0.05, inch);
                if o.cam.finish_stepover > 0.0 {
                    ui.checkbox(&mut o.cam.finish_at_target_only, "finition seulement à la profondeur finale");
                }
                if o.kind == OpKind::Profile {
                    ui.checkbox(&mut o.cam.corner_overcut, "surcoupe des coins intérieurs");
                }
                num(ui, "transition maxi (fraction du Ø)", &mut o.cam.max_crossover, 0.05);
            });
            if o.lead != LeadChoice::None {
                egui::CollapsingHeader::new("Entrée / sortie").id_source(("l", o.id)).show(ui, |ui| match o.lead {
                    LeadChoice::Spiral => num(ui, "angle de rampe (°, 0 = auto)", &mut o.cam.lead_angle, 0.5),
                    _ => lenf(ui, "rayon tangent, 0 = Ø outil", &mut o.cam.lead_radius, 0.1, inch),
                });
            }
        }
        egui::CollapsingHeader::new("Broche et avances").id_source(("b", o.id)).show(ui, |ui| {
            ui.checkbox(&mut o.cam.spindle_ccw, "broche anti-horaire (M4)");
            let tool = self.studio.project().tool(&o.tool).cloned();
            for (label, field, from_tool) in [
                ("avance de coupe", &mut o.feed, tool.as_ref().map(|t| t.feed)),
                ("avance de plongée", &mut o.plunge, tool.as_ref().map(|t| t.plunge)),
                ("broche (tr/min)", &mut o.spindle, tool.as_ref().map(|t| t.spindle)),
            ] {
                let mut own = field.is_some();
                ui.checkbox(&mut own, format!("{label} propre à l'opération"));
                match (own, field.as_mut()) {
                    (true, Some(v)) => num(ui, label, v, 10.0),
                    (true, None) => *field = from_tool.or(Some(100.0)),
                    (false, _) => {
                        *field = None;
                        ui.small(format!("   outil : {}", from_tool.map_or("—".into(), |v| format!("{v}"))));
                    }
                }
            }
        });
        if o.kind == OpKind::Profile {
            egui::CollapsingHeader::new(if o.tabs { "Ponts de maintien (Holding Tabs) : actifs" } else { "Ponts de maintien (Holding Tabs)" }).id_source(("t", o.id)).default_open(o.tabs).show(ui, |ui| {
                // méthode : aucun, automatique (toutes / extérieures / intérieures) ou manuel (clic sur le dessin)
                let manual = o.tabs && (o.cam.tab_target == TabTarget::Manual || !o.tab_points.is_empty());
                let current = if !o.tabs { "aucun" } else if manual { TabTarget::Manual.label() } else { o.cam.tab_target.label() };
                let mut start_manual = false;
                // boutons radio plutôt qu'une liste : toujours visibles, même en bas de la fenêtre
                ui.label("méthode");
                let _ = current;
                ui.indent("methode_ponts", |ui| {
                    if ui.radio(!o.tabs, "aucun").clicked() {
                        o.tabs = false;
                        self.tab_mode = false;
                    }
                    for t in TabTarget::AUTO {
                        if ui.radio(o.tabs && !manual && o.cam.tab_target == t, t.label()).clicked() {
                            o.tabs = true;
                            o.cam.tab_target = t;
                            o.tab_points.clear();
                            self.tab_mode = false;
                        }
                    }
                    if ui.radio(manual, TabTarget::Manual.label()).clicked() {
                        o.tabs = true;
                        o.cam.tab_target = TabTarget::Manual;
                        start_manual = true;
                    }
                });
                if start_manual {
                    // les clics sur le plan posent des ponts : il faut l'outil Sélection et la vue 2D
                    self.tab_mode = true;
                    self.tool = Draw::Select;
                    self.pts.clear();
                    self.view = View::Plan;
                    self.status = "ponts manuels : cliquez sur le contour pour poser un pont, sur un pont pour le retirer".into();
                }
                if o.tabs {
                    egui::ComboBox::from_label("forme").selected_text(o.cam.tab_shape.label()).show_ui(ui, |ui| {
                        for t in TabShape::ALL {
                            ui.selectable_value(&mut o.cam.tab_shape, t, t.label());
                        }
                    });
                    lenf(ui, "largeur", &mut o.cam.tab_width, 0.1, inch);
                    lenf(ui, "hauteur depuis le fond", &mut o.cam.tab_height, 0.1, inch);
                    if manual {
                        let mut pose = self.tab_mode;
                        if ui.checkbox(&mut pose, "poser / retirer au clic sur le plan").changed() {
                            self.tab_mode = pose;
                            if pose {
                                self.tool = Draw::Select;
                                self.view = View::Plan;
                            }
                        }
                        ui.small(if self.tab_mode { "clic sur le contour : pose un pont · clic sur un pont : le retire · Échap : terminer" } else { "cochez la case pour poser des ponts au clic" });
                        ui.horizontal(|ui| {
                            ui.label(format!("{} pont(s) posé(s)", o.tab_points.len()));
                            if ui.add_enabled(!o.tab_points.is_empty(), egui::Button::new("Effacer les ponts")).clicked() {
                                o.tab_points.clear();
                            }
                        });
                    } else {
                        let (mut lo, mut hi) = (o.cam.tab_min as f64, o.cam.tab_max as f64);
                        num(ui, "nombre mini", &mut lo, 0.1);
                        num(ui, "nombre maxi", &mut hi, 0.1);
                        (o.cam.tab_min, o.cam.tab_max) = (lo.max(0.0) as u32, hi.max(0.0) as u32);
                        lenf(ui, "distance visée, 0 = mini", &mut o.cam.tab_distance, 1.0, inch);
                        lenf(ui, "seuil de taille (périmètre mini)", &mut o.cam.tab_threshold, 1.0, inch);
                    }
                    ui.small("la hauteur doit rester sous la profondeur de coupe");
                }
            });
        }
        ui.collapsing("G-code avant / après l'opération", |ui| {
            ui.small("en-tête (avant)");
            ui.add(egui::TextEdit::multiline(&mut o.header).desired_rows(2).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
            ui.small("pied (après)");
            ui.add(egui::TextEdit::multiline(&mut o.footer).desired_rows(2).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
        });
        ui.small(format!("{} forme(s) rattachée(s)", o.shapes.len()));
        if ui.button("Rattacher la sélection du dessin").clicked() {
            o.shapes = self.selection.clone();
        }
        // effet immédiat : la modification est enregistrée (annulable) quand la saisie est terminée
        let stored = self.studio.project().op(o.id).cloned();
        let editing = ui.input(|i| i.pointer.any_down()) || ui.ctx().wants_keyboard_input();
        if stored.as_ref() != Some(&o) {
            if editing {
                ui.small("modification en cours…");
            } else {
                let od = o.clone();
                match self.studio.edit(|p| p.update_operation(od.id, |x| *x = od.clone())) {
                    Ok(()) => self.status = format!("opération « {} » mise à jour", o.name),
                    Err(e) => {
                        self.status = format!("Attention : {e}");
                        o = stored.unwrap_or(o);
                    }
                }
            }
        }
        if let Some(c) = self.studio.computed.get(&o.id) {
            match (&c.error, &c.path) {
                (Some(e), _) => {
                    ui.colored_label(Color32::from_rgb(220, 80, 60), format!("Attention : {e}"));
                    if c.path.is_some() {
                        ui.small("(la trajectoire affichée date du dernier calcul valide)");
                    }
                }
                (None, Some(p)) => {
                    ui.colored_label(Color32::from_rgb(70, 170, 100), format!("OK : {} mouvements", p.moves.len()));
                }
                _ => {}
            }
        }
        self.op_draft = Some(o);
    }
}

impl App {
    // ---------- onglet 3D / simulation ----------
    fn refresh_sim(&mut self) {
        let stale = self.sim.as_ref().map_or(true, |(rev, f, r, _)| *rev != self.studio.rev || (*f - self.sim_frac).abs() > 1e-9 || (*r - self.sim_res).abs() > 1e-9);
        if stale {
            self.sim = sim::simulate(&self.studio, self.sim_res, self.sim_frac).map(|o| (self.studio.rev, self.sim_frac, self.sim_res, o));
            if let (Some((_, _, _, o)), true) = (&self.sim, self.sim_frac >= 1.0) {
                self.console.push(format!("Simulation : volume retiré {:.0} mm³, trajet {:.0} mm, {} gouge(s)", o.map.removed_volume(), o.total_len, o.gouges.len()));
                for g in &o.gouges {
                    let k = match g.kind {
                        sim::GougeKind::BelowFloor => "sous le fond programmé",
                        sim::GougeKind::IntoTab => "dans un pont de maintien",
                        sim::GougeKind::RapidInMaterial => "rapide dans la matière",
                    };
                    self.console.push(format!("Attention : gouge {k} en ({:.1} ; {:.1} ; {:.1}) — {}", g.pos[0], g.pos[1], g.pos[2], g.op));
                }
            }
        }
        if self.paths.0 != self.studio.rev {
            self.paths = (self.studio.rev, sim::op_backplots(&self.studio));
        }
    }

    /// Brut : automatique (d'après les trajectoires) ou saisi à la main.
    fn stock_editor(&mut self, ui: &mut egui::Ui) {
        let inch = self.studio.project().gcode_inches;
        ui.label(egui::RichText::new("Brut").strong());
        let current = self.studio.project().stock;
        let mut manual = current.is_some();
        if ui.checkbox(&mut manual, "défini à la main (sinon d'après le trajet)").changed() {
            let new = if manual { sim::auto_stock(&self.studio) } else { None };
            let r = self.studio.edit(|p| p.set_stock(new));
            self.report(r);
            self.stock_draft = None;
        }
        match current {
            None => {
                if let Some(s) = sim::auto_stock(&self.studio) {
                    ui.small(format!("X {:.1} → {:.1}   Y {:.1} → {:.1}", s.min[0], s.max[0], s.min[1], s.max[1]));
                    ui.small(format!("Z {:.2} → {:.2}", s.bottom, s.top));
                } else {
                    ui.small("aucune trajectoire : brut indéterminé");
                }
            }
            Some(cur) => {
                let mut d = self.stock_draft.unwrap_or(cur);
                lenf(ui, "X mini", &mut d.min[0], 0.5, inch);
                lenf(ui, "Y mini", &mut d.min[1], 0.5, inch);
                lenf(ui, "X maxi", &mut d.max[0], 0.5, inch);
                lenf(ui, "Y maxi", &mut d.max[1], 0.5, inch);
                lenf(ui, "dessus Z", &mut d.top, 0.1, inch);
                lenf(ui, "dessous Z", &mut d.bottom, 0.1, inch);
                self.stock_draft = Some(d);
                ui.horizontal(|ui| {
                    if ui.button("Appliquer").clicked() {
                        let r = self.studio.edit(|p| p.set_stock(Some(d)));
                        if self.report(r).is_some() {
                            self.status = "brut mis à jour".into();
                            self.stock_draft = None;
                        }
                    }
                    if ui.button("Ajuster au trajet").clicked() {
                        // `auto_stock` ignore le brut saisi : une seule étape d'annulation
                        let auto = sim::auto_stock(&self.studio);
                        let r = self.studio.edit(|p| p.set_stock(auto));
                        self.report(r);
                        self.stock_draft = None;
                    }
                });
            }
        }
    }

    /// Bandeau au-dessus de la vue : Plan 2D / 3D / Simulation et leurs réglages.
    fn view_bar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none().fill(Color32::from_rgb(28, 30, 33)).inner_margin(egui::Margin::symmetric(8.0, 4.0)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (v, label) in [(View::Plan, "Plan 2D"), (View::Vue3D, "3D"), (View::Simulation, "Simulation")] {
                    if ui.selectable_label(self.view == v, label).clicked() {
                        self.run(Action::View(v), ui.ctx());
                    }
                }
                ui.separator();
                match self.view {
                    View::Plan => {
                        ui.checkbox(&mut self.show_toolpaths, "trajectoires");
                    }
                    View::Vue3D | View::Simulation => {
                        if self.view == View::Simulation {
                            if ui.button(if self.playing { "Pause" } else { "Lecture" }).clicked() {
                                if self.sim_frac >= 1.0 {
                                    self.sim_frac = 0.0;
                                }
                                self.playing = !self.playing;
                            }
                            if ui.button("Début").clicked() {
                                self.playing = false;
                                self.sim_frac = 0.0;
                            }
                            if ui.button("Fin").clicked() {
                                self.playing = false;
                                self.sim_frac = 1.0;
                            }
                            ui.add(egui::Slider::new(&mut self.sim_frac, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
                        }
                        egui::ComboBox::from_id_source("maille").width(70.0).selected_text(format!("maille {}", self.sim_res)).show_ui(ui, |ui| {
                            for r in [1.0, 0.5, 0.25, 0.2, 0.1] {
                                ui.selectable_value(&mut self.sim_res, r, format!("{r} mm"));
                            }
                        });
                        ui.checkbox(&mut self.show_path, "trajet");
                        if let Some((_, _, _, o)) = &self.sim {
                            if o.gouges.is_empty() {
                                ui.colored_label(Color32::from_rgb(90, 190, 120), "aucune gouge");
                            } else {
                                ui.colored_label(Color32::from_rgb(230, 90, 70), format!("{} gouge(s) — voir la console", o.gouges.len()));
                            }
                        }
                    }
                }
                if ui.button("Cadrer").on_hover_text("F").clicked() {
                    self.run(Action::Fit, ui.ctx());
                }
            });
        });
    }

    fn view3d(&mut self, ui: &mut egui::Ui) {
        self.refresh_sim();
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        painter.rect_filled(rect, 0.0, Color32::from_rgb(34, 36, 40));
        let Some((_, _, _, out)) = &self.sim else {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Aucune opération à simuler", egui::FontId::proportional(16.0), Color32::GRAY);
            return;
        };
        let m = &out.map;
        let (xmax, ymax) = (m.min[0] + m.nx as f64 * m.res, m.min[1] + m.ny as f64 * m.res);
        let cam = self.cam3.get_or_insert_with(|| Camera3::fit([m.min[0], m.min[1], m.bottom], [xmax, ymax, m.top]));
        if resp.dragged() {
            let d = resp.drag_delta();
            cam.orbit(-d.x as f64 * 0.008, d.y as f64 * 0.008);
        }
        if resp.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.scroll_delta.y, i.zoom_delta()));
            let f = (-scroll as f64 * 0.01).exp() * (1.0 / zoom as f64);
            if (f - 1.0).abs() > 1e-6 {
                cam.zoom(f);
            }
        }
        let cam = *cam;
        let focal = rect.height() as f64 * 1.05;
        let c = rect.center();
        let to = |p: [f64; 3]| {
            let q = cam.project(p, focal);
            Pos2::new(c.x + q[0] as f32, c.y - q[1] as f32)
        };
        // base du brut, puis surface triangulée du fond vers l'avant
        let base = [[m.min[0], m.min[1]], [xmax, m.min[1]], [xmax, ymax], [m.min[0], ymax]];
        painter.add(egui::Shape::convex_polygon(base.iter().map(|b| to([b[0], b[1], m.bottom])).collect(), Color32::from_rgb(60, 50, 40), Stroke::new(1.0_f32, Color32::from_gray(120))));
        let tris = surface_triangles(m, 110);
        let mut mesh = egui::epaint::Mesh::default();
        for (t, col) in sorted_projected(&cam, &tris, focal) {
            let i = mesh.vertices.len() as u32;
            for v in t {
                mesh.colored_vertex(Pos2::new(c.x + v[0] as f32, c.y - v[1] as f32), Color32::from_rgb(col[0], col[1], col[2]));
            }
            mesh.add_triangle(i, i + 1, i + 2);
        }
        painter.add(egui::Shape::mesh(mesh));
        // arêtes du brut
        let top = |p: [f64; 2]| to([p[0], p[1], m.top]);
        let outline: Vec<Pos2> = base.iter().chain(std::iter::once(&base[0])).map(|b| top(*b)).collect();
        painter.add(egui::Shape::line(outline, Stroke::new(1.2_f32, Color32::from_gray(200))));
        for b in &base {
            painter.line_segment([top(*b), to([b[0], b[1], m.bottom])], Stroke::new(1.0_f32, Color32::from_gray(120)));
        }
        // trajet joué
        if self.show_path {
            let mut left = self.sim_frac * out.total_len;
            let mut tip: Option<[f64; 3]> = None;
            'paths: for (_, bp) in &self.paths.1 {
                for s in &bp.segs {
                    if left <= 0.0 {
                        break 'paths;
                    }
                    let part = if s.len > left { left / s.len } else { 1.0 };
                    left -= s.len * part;
                    if s.kind == fablabcam_backplot::Kind::Rapid {
                        tip = Some(s.point_at(part));
                        continue;
                    }
                    let n = s.polyline(0.1).len().max(2);
                    let pts: Vec<Pos2> = (0..=n).map(|k| to(s.point_at(part * k as f64 / n as f64))).collect();
                    tip = Some(s.point_at(part));
                    painter.add(egui::Shape::line(pts, Stroke::new(1.2_f32, Color32::from_rgba_unmultiplied(80, 190, 255, 200))));
                }
            }
            if let (Some(t), true) = (tip, self.sim_frac < 1.0 && self.sim_frac > 0.0) {
                painter.circle_stroke(to(t), 7.0, Stroke::new(2.5_f32, Color32::from_rgb(255, 80, 20)));
            }
        }
        for g in &out.gouges {
            painter.circle_filled(to(g.pos), 6.0, Color32::from_rgb(230, 40, 40));
        }
        painter.text(rect.left_bottom() + egui::vec2(10.0, -10.0), egui::Align2::LEFT_BOTTOM, format!("{:.0} % du trajet — volume retiré {:.0} mm³", self.sim_frac * 100.0, m.removed_volume()), egui::FontId::monospace(12.0), Color32::from_gray(210));
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame += 1;
        // fichiers reçus du Finder (double-clic, « Ouvrir avec ») ou déposés sur la fenêtre
        let mut incoming = desktop::finder::take();
        incoming.extend(ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect::<Vec<_>>()));
        for p in incoming {
            // macOS transmet aussi les arguments de la ligne de commande : on ignore ce qui n'est pas un
            // fichier et ce qui a déjà été chargé au démarrage
            let canon = p.canonicalize().ok();
            if let Some(i) = self.argv_files.iter().position(|a| Some(a) == canon.as_ref()) {
                self.argv_files.remove(i);
                continue;
            }
            if p.is_file() {
                self.open_path(&p, false);
            }
        }
        let typing = ctx.wants_keyboard_input();
        let shortcut = ctx.input(|i| {
            let (c, sh) = (i.modifiers.command, i.modifiers.shift);
            if !c {
                return None;
            }
            [
                (Key::N, false, Action::New),
                (Key::O, false, Action::Open),
                (Key::S, false, Action::Save),
                (Key::S, true, Action::SaveAs),
                (Key::I, false, Action::Import),
                (Key::E, false, Action::ExportGcode),
                (Key::Comma, false, Action::Prefs),
            ]
            .into_iter()
            .find(|(k, s, _)| i.key_pressed(*k) && *s == sh)
            .map(|(_, _, a)| a)
        });
        self.poll_dialog(ctx);
        if let Some(a) = shortcut {
            self.run(a, ctx);
        }
        // outils de dessin au clavier (sans modificateur, hors saisie, hors tracé en cours)
        if !typing && self.pts.is_empty() {
            let pick = ctx.input(|i| {
                if i.modifiers.command || i.modifiers.alt {
                    return None;
                }
                [(Key::V, Draw::Select), (Key::L, Draw::Line), (Key::P, Draw::Polyline), (Key::R, Draw::Rect), (Key::O, Draw::Circle), (Key::A, Draw::Arc), (Key::T, Draw::Text)].into_iter().find(|(k, _)| i.key_pressed(*k)).map(|(_, d)| d)
            });
            if let Some(d) = pick {
                self.run(Action::Tool(d), ctx);
            }
            let view_key = ctx.input(|i| {
                if i.modifiers.command {
                    return None;
                }
                [(Key::Num1, View::Plan), (Key::Num2, View::Vue3D), (Key::Num3, View::Simulation)].into_iter().find(|(k, _)| i.key_pressed(*k)).map(|(_, v)| v)
            });
            if let Some(v) = view_key {
                self.run(Action::View(v), ctx);
            }
            let edit_key = ctx.input(|i| {
                if !i.modifiers.command {
                    return None;
                }
                [(Key::A, Action::SelectAll), (Key::C, Action::Copy), (Key::X, Action::Cut), (Key::V, Action::Paste)].into_iter().find(|(k, _)| i.key_pressed(*k)).map(|(_, a)| a)
            });
            if let Some(a) = edit_key {
                self.run(a, ctx);
            }
        }
        let (undo, redo, del, esc, enter, close, fit) = ctx.input(|i| {
            let cmd = i.modifiers.command;
            (
                cmd && !i.modifiers.shift && i.key_pressed(Key::Z),
                cmd && i.modifiers.shift && i.key_pressed(Key::Z),
                i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::C) && !cmd,
                i.key_pressed(Key::F),
            )
        });
        if self.playing {
            let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1) as f64;
            self.sim_frac = (self.sim_frac + dt * 0.08).min(1.0);
            if self.sim_frac >= 1.0 {
                self.playing = false;
            }
            ctx.request_repaint();
        }
        if !typing {
            if undo {
                self.studio.undo();
                self.selection.retain(|id| self.studio.project().shape(*id).is_some());
            }
            if redo {
                self.studio.redo();
            }
            if del {
                self.delete_selection();
            }
            if esc {
                self.tab_mode = false;
                self.pts.clear();
                self.selection.clear();
            }
            if self.tool == Draw::Polyline && (enter || close) {
                self.finish_polyline(close);
            }
            if fit {
                self.fit_pending = true;
            }
        }
        let mut menu_action = None;
        egui::TopBottomPanel::top("menus").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                menu_action = self.menu_bar(ui);
            });
        });
        egui::TopBottomPanel::top("barre").show(ctx, |ui| {
            if let Some(a) = self.toolbar(ui) {
                menu_action = Some(a);
            }
        });
        if let Some(a) = menu_action {
            self.run(a, ctx);
        }
        // barre d'état : message, magnétisme, unité, position du pointeur
        let inch = self.studio.project().gcode_inches;
        egui::TopBottomPanel::bottom("etat").exact_height(24.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (k, u) = if inch { (25.4, "po") } else { (1.0, "mm") };
                    if let Some(w) = self.cursor_w {
                        ui.monospace(format!("X {:.3}  Y {:.3}", w[0] / k, w[1] / k));
                    }
                    ui.label(u);
                    ui.checkbox(&mut self.snap, format!("magnétisme {} mm", self.prefs.snap_step));
                });
            });
        });
        // console de messages
        if self.show_console {
            egui::TopBottomPanel::bottom("console").resizable(true).default_height(110.0).min_height(50.0).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Messages").strong());
                    if ui.small_button("effacer").clicked() {
                        self.console.clear();
                    }
                });
                egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                    for l in &self.console {
                        let c = if l.starts_with("Attention") { Color32::from_rgb(240, 180, 110) } else { Color32::from_gray(200) };
                        ui.label(egui::RichText::new(l).monospace().size(12.0).color(c));
                    }
                });
            });
        }
        egui::SidePanel::left("side").default_width(340.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.side, SideTab::Projet, "Dessin");
                ui.selectable_value(&mut self.side, SideTab::Systeme, "Système");
            });
            ui.separator();
            match self.side {
                SideTab::Projet => self.project_tab(ui),
                SideTab::Systeme => {
                    egui::ScrollArea::vertical().id_source("systeme").show(ui, |ui| self.system_tab(ui));
                }
            }
        });
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            self.view_bar(ui);
            match self.view {
                View::Plan => self.canvas(ui),
                View::Vue3D | View::Simulation => self.view3d(ui),
            }
        });
        // journal : chaque nouveau message, et la description de la sélection quand elle change
        if !self.status.is_empty() && self.status != self.logged_status {
            self.logged_status = self.status.clone();
            self.console.push(self.status.clone());
        }
        if self.selection != self.logged_sel {
            self.logged_sel = self.selection.clone();
            let b: Vec<([f64; 2], [f64; 2])> = self.selection.iter().filter_map(|id| self.studio.project().shape(*id)).filter_map(|s| shape::bounds(&s.entity)).collect();
            if !b.is_empty() {
                let lo = b.iter().fold([f64::INFINITY; 2], |a, (l, _)| [a[0].min(l[0]), a[1].min(l[1])]);
                let hi = b.iter().fold([f64::NEG_INFINITY; 2], |a, (_, h)| [a[0].max(h[0]), a[1].max(h[1])]);
                let (k, u) = if inch { (25.4, "po") } else { (1.0, "mm") };
                self.console.push(format!(
                    "Sélection : {} forme(s) — min {:.3} ; {:.3} — max {:.3} ; {:.3} — largeur {:.3} — hauteur {:.3} {u}",
                    b.len(),
                    lo[0] / k,
                    lo[1] / k,
                    hi[0] / k,
                    hi[1] / k,
                    (hi[0] - lo[0]) / k,
                    (hi[1] - lo[1]) / k
                ));
            }
        }
        if self.console.len() > 500 {
            let n = self.console.len() - 500;
            self.console.drain(..n);
        }
        if self.show_help {
            self.help_window(ctx);
        }
        // fenêtres : préférences, question avant de perdre des modifications
        if self.show_prefs {
            self.prefs_window(ctx);
        }
        if self.show_script {
            self.script_window(ctx);
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty() && !self.allow_close && self.smoke.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Pending::Quit);
        }
        self.confirm_window(ctx);
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let r = ctx.screen_rect();
            let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop")));
            p.rect_filled(r, 0.0, Color32::from_black_alpha(150));
            p.text(r.center(), egui::Align2::CENTER_CENTER, "Déposer pour ouvrir (.cb) ou importer (.dxf, .svg)", egui::FontId::proportional(22.0), Color32::WHITE);
        }
        // titre : nom du projet, point si modifié
        let name = self.doc_path.as_ref().and_then(|p| p.file_name()).map_or("Sans titre".to_string(), |n| n.to_string_lossy().into_owned());
        let title = format!("{}{name} — FabLabCAM Studio", if self.dirty() { "• " } else { "" });
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        // préférences enregistrées dès qu'elles changent
        if self.smoke.is_none() && (self.prefs.snap != self.snap || self.prefs.sim_res != self.sim_res || self.prefs.post_path != self.post_path) {
            self.save_prefs();
        }
        if let Some(n) = self.smoke {
            ctx.request_repaint();
            if self.frame >= n {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{HELP}");
        return;
    }
    let mut take = |name: &str, with_value: bool| -> Option<String> {
        let i = args.iter().position(|a| a == name)?;
        let v = if with_value { args.get(i + 1).cloned() } else { Some(String::new()) };
        args.drain(i..(i + if with_value { 2 } else { 1 }).min(args.len()));
        v
    };
    let smoke = take("--smoke", true).map(|n| n.parse().unwrap_or(30));
    let select = take("--select", true);
    let demo = take("--demo-dessin", false).is_some();
    let demo_full = take("--demo", false).is_some();
    let tab = take("--tab", true);
    let at = take("--at", true).and_then(|v| v.replace(',', ".").parse::<f64>().ok());
    let res = take("--res", true).and_then(|v| v.replace(',', ".").parse::<f64>().ok());
    let op_rank = take("--op", true).and_then(|v| v.parse::<usize>().ok());
    let mut project = if demo_full { fablabcam_studio::studio::demo_project() } else if demo { fablabcam_studio::studio::drawing_demo() } else { Project::new() };
    let mut path = String::new();
    if let Some(f) = args.iter().find(|a| !a.starts_with('-')) {
        path = f.clone();
        match std::fs::read_to_string(f) {
            Ok(t) if f.ends_with(".cb") => match persist::load(&t) {
                Ok(p) => project = p,
                Err(e) => eprintln!("erreur : {e}"),
            },
            Ok(t) if f.ends_with(".dxf") => match dxf::parse_dxf(&t) {
                Ok(r) => r.shapes.into_iter().for_each(|(l, e)| drop(project.add_shape(&l, e))),
                Err(e) => eprintln!("erreur : {e}"),
            },
            Ok(t) if f.ends_with(".svg") => match svgio::parse_svg(&t) {
                Ok(r) => r.shapes.into_iter().for_each(|(l, e)| drop(project.add_shape(&l, e))),
                Err(e) => eprintln!("erreur : {e}"),
            },
            Ok(_) => eprintln!("extension non gérée : {f}"),
            Err(e) => eprintln!("{f} : {e}"),
        }
    }
    let mut app = App::new(project, smoke);
    if path.to_lowercase().ends_with(".cb") && std::path::Path::new(&path).is_file() {
        app.doc_path = Some(PathBuf::from(&path));
    }
    app.argv_files = std::env::args().skip(1).filter_map(|a| std::path::Path::new(&a).canonicalize().ok()).filter(|p| p.is_file()).collect();
    app.path = path;
    match tab.as_deref() {
        Some("tools") | Some("outils") | Some("systeme") => app.side = SideTab::Systeme,
        Some("ops") | Some("operations") => app.focus = Focus::Op,
        Some("3d") => app.view = View::Simulation,
        _ => {}
    }
    if let Some(a) = at {
        app.sim_frac = a.clamp(0.0, 1.0);
    }
    if let Some(r) = res {
        app.sim_res = r.max(0.05);
    }
    app.sel_tool = app.studio.project().tools.first().map(|t| t.name.clone());
    app.sel_op = op_rank.and_then(|r| app.studio.project().ops.get(r.wrapping_sub(1)).map(|o| o.id)).or_else(|| app.studio.project().ops.first().map(|o| o.id));
    if let Some(sel) = select {
        let ids: Vec<u32> = app.studio.project().shapes.iter().map(|s| s.id).collect();
        app.selection = sel.split(',').filter_map(|r| r.trim().parse::<usize>().ok()).filter_map(|r| ids.get(r.wrapping_sub(1)).copied()).collect();
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_title("FabLabCAM Studio")
            .with_app_id("org.fablabcam.studio")
            .with_icon(egui::IconData { rgba: include_bytes!("../assets/icon-128.rgba").to_vec(), width: 128, height: 128 }),
        ..Default::default()
    };
    desktop::finder::install();
    if let Err(e) = eframe::run_native(
        "fablabcam-studio-ui",
        options,
        Box::new(move |cc| {
            desktop::finder::set_context(&cc.egui_ctx);
            Box::new(app)
        }),
    ) {
        eprintln!("erreur d'affichage : {e}");
        std::process::exit(1);
    }
}
