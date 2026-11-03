//! Interface en ligne de commande de FabLabCAM (logique testable, `main.rs` n'est qu'un appel).

pub mod machine;

use fablabcam_cam::{drill, emit, pocket_shapes, profile, LeadKind, MillingDirection, Params, Post, Side, StyleLibrary, TabMethod, TabStyle, Toolpath};
use fablabcam_geom::{Pline, PlineSource};
use fablabcam_backplot::{parse_gcode, svg, web, ColorBy, ViewOptions};
use fablabcam_model::{parse_cb, Entity};

pub const USAGE: &str = "\
fablabcam — FAO 2,5D en Rust (dessins XML .cb, DXF et SVG)

USAGE
  fablabcam version                              version du produit
  fablabcam machine help                         profils, validation et archives G-code
  fablabcam info   <fichier.cb>
  fablabcam styles <styles.xml>
  fablabcam gcode  <fichier.cb> --op <profile-outside|profile-inside|profile-on|pocket|drill> [options]
  fablabcam convert <entrée.(cb|dxf|svg)> <sortie.(cb|dxf|svg)>      conversion de dessins
  fablabcam web    <projet.cb|dxf|svg|demo|demo-dessin> -o page.html [--select a,b]    page « studio » navigable (captures)
  fablabcam sim    <projet.cb|demo> [--res 0.2] [--at 1.0]       simulation de matière (chrono, volume, gouges)
  fablabcam script <script.rhai> [projet.cb] [-o sortie.cb]   exécute un script Rhai (dessin, outils, opérations)
  fablabcam build  <projet.cb> [-o programme.nc] [--post def.cbpp] [--macros '$o=1;$x=2']   G-code des opérations du projet (outils + liste)
  fablabcam plot   <fichier.cb|programme.nc> [options de gcode pour un .cb] [options de tracé] -o trace.svg|trace.html
  fablabcam-view   <fichier.cb|programme.nc> [mêmes options]   fenêtre interactive (zoom, Z, animation)

OPTIONS de gcode
  --layer <nom>       ne traiter que ce calque (défaut : tous)
  --styles <fichier>  bibliothèque de styles (tests/fixtures/styles-demo.xml)
  --style <nom>       style à appliquer (défaut : le style de base)
  --tool <mm>         diamètre d'outil
  --depth <z>         profondeur cible (négative)
  --step <mm>         incrément de passe
  --stock <z>         altitude de la surface du brut
  --clearance <z>     plan de dégagement
  --feed <mm/min>     avance de coupe
  --plunge <mm/min>   avance de plongée
  --speed <tr/min>    vitesse de broche
  --tabs <méthode>    none | auto | outer | inner : languettes de maintien (profils)
  --tab-width <mm>    --tab-height <mm>   --tab-distance <mm>   --tab-style triangle|square|skip
  --lead-in <type>    none | spiral | tangent   (--lead-out : tangent | none)
  --angle <deg>       angle de spirale (0 = un incrément de profondeur par tour)
  --radius <mm>       rayon des arcs tangents
  --cut-order <o>     depth | level   (ordre des passes des poches)
  --milling <sens>    climb | conventional | mixed   (défaut : climb)
  --spindle <sens>    cw | ccw   (M3 / M4 ; le sens de fraisage en dépend)
  --overcut           surcoupe dans les coins intérieurs des profils
  --finish <mm>       distance de la passe de finition des poches (0 = aucune)
  --finish-all        finition à chaque niveau au lieu du seul niveau final
  --final-step <mm>   incrément de la dernière passe en profondeur
  --crossover <f>     transition max. entre anneaux, fraction du diamètre (0 = aucune)
  --stepover-feed <v> avance des transitions
  --no-canned         expanser les perçages (pas de G81/G83)

OPTIONS de plot  (relit le G-code et trace la vue de dessus : couleur = profondeur Z, flèches = sens
                  de parcours, pointillés = rapides, points orange = plongées ; profil Z en bas)
  --no-rapids  --no-arrows  --no-plunges   masquer ces éléments
  --color <c>         z (défaut) | type
  --z-range <a,b>     n'afficher que les passes entre les altitudes a et b
  --title <texte>     titre de l'image
  (-o trace.html : page interactive autonome — pan/zoom, animation, survol ; ?at=0.5 dans l'URL)

POCHES  Les formes contenues dans une autre sont traitées comme des îlots.
  -o <fichier>        écrire le G-code dans un fichier (défaut : sortie standard)
";

struct Args {
    pos: Vec<String>,
    opts: Vec<(String, Option<String>)>,
}

const FLAGS: &[&str] = &["--no-canned", "--overcut", "--finish-all", "--no-rapids", "--no-arrows", "--no-plunges"];

fn split(args: &[String]) -> Result<Args, String> {
    let (mut pos, mut opts) = (Vec::new(), Vec::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a.starts_with('-') && a.len() > 1 {
            if FLAGS.contains(&a.as_str()) {
                opts.push((a.clone(), None));
            } else {
                let v = it.next().ok_or_else(|| format!("{a} attend une valeur"))?;
                opts.push((a.clone(), Some(v.clone())));
            }
        } else {
            pos.push(a.clone());
        }
    }
    Ok(Args { pos, opts })
}

impl Args {
    fn get(&self, k: &str) -> Option<&str> {
        self.opts.iter().find(|(o, _)| o == k).and_then(|(_, v)| v.as_deref())
    }
    fn has(&self, k: &str) -> bool {
        self.opts.iter().any(|(o, _)| o == k)
    }
    fn num(&self, k: &str) -> Result<Option<f64>, String> {
        self.get(k).map(|v| v.replace(',', ".").parse::<f64>().map_err(|_| format!("{k} : « {v} » n'est pas un nombre"))).transpose()
    }
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{path} : {e}"))
}

fn info(path: &str) -> Result<String, String> {
    let doc = parse_cb(&read(path)?).map_err(|e| e.to_string())?;
    let mut s = String::new();
    for l in &doc.layers {
        let (mut pl, mut ci, mut ar, mut pt) = (0, 0, 0, 0);
        for e in &l.entities {
            match e {
                Entity::Polyline(_) | Entity::Rect { .. } => pl += 1,
                Entity::Circle { .. } => ci += 1,
                Entity::Arc { .. } => ar += 1,
                Entity::Point { .. } => pt += 1,
            }
        }
        s += &format!("calque « {} » : {pl} polyligne(s), {ci} cercle(s), {ar} arc(s), {pt} point(s)\n", l.name);
    }
    Ok(s)
}

fn styles(path: &str) -> Result<String, String> {
    let lib = StyleLibrary::parse(&read(path)?).map_err(|e| e.to_string())?;
    Ok(lib.names().map(|n| if n.is_empty() { "(base)" } else { n }).collect::<Vec<_>>().join("\n") + "\n")
}

fn apply_tabs_and_leads(a: &Args, p: &mut Params) -> Result<(), String> {
    if let Some(m) = a.get("--tabs") {
        p.tabs.method = match m {
            "none" => TabMethod::None,
            "auto" => TabMethod::Automatic,
            "outer" => TabMethod::AutomaticOuter,
            "inner" => TabMethod::AutomaticInner,
            o => return Err(format!("--tabs : « {o} » inconnu (none|auto|outer|inner)")),
        };
    }
    if let Some(v) = a.num("--tab-width")? { p.tabs.width = v }
    if let Some(v) = a.num("--tab-height")? { p.tabs.height = v }
    if let Some(v) = a.num("--tab-distance")? { p.tabs.distance = v }
    if let Some(st) = a.get("--tab-style") {
        p.tabs.style = match st {
            "triangle" => TabStyle::Triangle,
            "square" => TabStyle::Square,
            "skip" => TabStyle::Skip,
            o => return Err(format!("--tab-style : « {o} » inconnu (triangle|square|skip)")),
        };
    }
    let kind = |v: &str, k: &str| match v {
        "none" => Ok(LeadKind::None),
        "spiral" => Ok(LeadKind::Spiral),
        "tangent" => Ok(LeadKind::Tangent),
        o => Err(format!("{k} : « {o} » inconnu (none|spiral|tangent)")),
    };
    if let Some(v) = a.get("--lead-in") { p.lead_in.kind = kind(v, "--lead-in")? }
    if let Some(v) = a.get("--lead-out") { p.lead_out.kind = kind(v, "--lead-out")? }
    if let Some(v) = a.num("--angle")? { p.lead_in.spiral_angle = v; p.lead_out.spiral_angle = v }
    if let Some(v) = a.num("--radius")? { p.lead_in.tangent_radius = v; p.lead_out.tangent_radius = v }
    match a.get("--milling") {
        None => {}
        Some("climb") => p.milling = MillingDirection::Climb,
        Some("conventional") => p.milling = MillingDirection::Conventional,
        Some("mixed") => p.milling = MillingDirection::Mixed,
        Some(o) => return Err(format!("--milling : « {o} » inconnu (climb|conventional|mixed)")),
    }
    match a.get("--spindle") {
        None => {}
        Some("cw") => p.spindle_cw = true,
        Some("ccw") => p.spindle_cw = false,
        Some(o) => return Err(format!("--spindle : « {o} » inconnu (cw|ccw)")),
    }
    if a.has("--overcut") { p.corner_overcut = true }
    if a.has("--finish-all") { p.finish_at_target_only = false }
    if let Some(v) = a.num("--finish")? { p.finish_stepover = v }
    if let Some(v) = a.num("--final-step")? { p.final_depth_increment = v }
    if let Some(v) = a.num("--crossover")? { p.max_crossover = v }
    if let Some(v) = a.num("--stepover-feed")? { p.stepover_feedrate = v }
    match a.get("--cut-order") {
        None => {}
        Some("depth") => p.cut_ordering = fablabcam_cam::CutOrdering::DepthFirst,
        Some("level") => p.cut_ordering = fablabcam_cam::CutOrdering::LevelFirst,
        Some(o) => return Err(format!("--cut-order : « {o} » inconnu (depth|level)")),
    }
    Ok(())
}

fn gcode(a: &Args) -> Result<String, String> {
    let file = a.pos.get(1).ok_or("fichier .cb manquant")?;
    let op = a.get("--op").ok_or("--op est requis")?;
    let doc = parse_cb(&read(file)?).map_err(|e| e.to_string())?;

    let mut p = match (a.get("--styles"), a.get("--style")) {
        (Some(f), name) => {
            let lib = StyleLibrary::parse(&read(f)?).map_err(|e| e.to_string())?;
            let name = name.unwrap_or("");
            Params::from_style(&lib.resolve(name).ok_or_else(|| format!("style « {name} » introuvable"))?)
        }
        (None, Some(_)) => return Err("--style nécessite --styles".into()),
        (None, None) => Params::default(),
    };
    if let Some(v) = a.num("--tool")? { p.tool_diameter = v }
    if let Some(v) = a.num("--depth")? { p.target_depth = v }
    if let Some(v) = a.num("--step")? { p.depth_increment = v }
    if let Some(v) = a.num("--stock")? { p.stock_surface = v }
    if let Some(v) = a.num("--clearance")? { p.clearance_plane = v }
    if let Some(v) = a.num("--feed")? { p.cut_feedrate = v }
    if let Some(v) = a.num("--plunge")? { p.plunge_feedrate = v }
    if let Some(v) = a.num("--speed")? { p.spindle_speed = v }

    apply_tabs_and_leads(a, &mut p)?;

    let layer = a.get("--layer");
    let entities: Vec<&Entity> = doc
        .layers
        .iter()
        .filter(|l| layer.is_none_or(|n| l.name == n))
        .flat_map(|l| l.entities.iter())
        .collect();
    if entities.is_empty() {
        return Err(match layer {
            Some(n) => format!("calque « {n} » vide ou introuvable"),
            None => "le dessin ne contient aucune entité".into(),
        });
    }

    let mut all = Toolpath::default();
    let mut extend = |t: Toolpath| all.moves.extend(t.moves);
    let shapes = || entities.iter().filter_map(|e| e.to_pline()).collect::<Vec<Pline>>();
    match op {
        "profile-outside" => shapes().iter().try_for_each(|s| profile(s, Side::Outside, &p).map(&mut extend)),
        "profile-inside" => shapes().iter().try_for_each(|s| profile(s, Side::Inside, &p).map(&mut extend)),
        "profile-on" => shapes().iter().try_for_each(|s| profile(s, Side::On, &p).map(&mut extend)),
        "pocket" => pocket_shapes(&shapes(), &p).map(&mut extend),
        "drill" => {
            let pts: Vec<(f64, f64)> = entities
                .iter()
                .filter_map(|e| match e {
                    Entity::Circle { cx, cy, .. } => Some((*cx, *cy)),
                    Entity::Point { x, y } => Some((*x, *y)),
                    Entity::Polyline(pl) if pl.vertex_count() == 1 => Some((pl.at(0).x, pl.at(0).y)),
                    _ => None,
                })
                .collect();
            drill(&pts, &p).map(&mut extend)
        }
        other => return Err(format!("opération inconnue : {other}")),
    }
    .map_err(|e| e.to_string())?;

    Ok(emit(&all, &p, &Post { canned_drill: !a.has("--no-canned"), ..Post::default() }))
}

fn plot(a: &Args) -> Result<String, String> {
    let file = a.pos.get(1).ok_or("fichier .cb ou G-code manquant")?;
    let is_drawing = file.ends_with(".cb");
    // un dessin est d'abord converti en G-code (mêmes options que `gcode`) : on trace ce que la machine exécutera
    let text = if is_drawing { gcode(a)? } else { read(file)? };
    let bp = parse_gcode(&text);
    let mut view = ViewOptions {
        show_rapids: !a.has("--no-rapids"),
        show_arrows: !a.has("--no-arrows"),
        show_plunges: !a.has("--no-plunges"),
        ..ViewOptions::default()
    };
    view.color_by = match a.get("--color") {
        None | Some("z") => ColorBy::Z,
        Some("type") => ColorBy::Type,
        Some(o) => return Err(format!("--color : « {o} » inconnu (z|type)")),
    };
    if let Some(r) = a.get("--z-range") {
        let (lo, hi) = r.split_once(',').ok_or("--z-range attend « bas,haut »")?;
        let num = |v: &str| v.trim().replace(',', ".").parse::<f64>().map_err(|_| format!("--z-range : « {v} » n'est pas un nombre"));
        let (lo, hi) = (num(lo)?, num(hi)?);
        view.z_range = Some((lo.min(hi), lo.max(hi)));
    }
    let title = a.get("--title").map(String::from).unwrap_or_else(|| {
        std::path::Path::new(file).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| file.clone())
    });
    let html = a.get("-o").is_some_and(|p| p.ends_with(".html") || p.ends_with(".htm"));
    let doc = if html { web::to_html(&bp, &title) } else { svg::to_svg(&bp, &view, &svg::SvgOptions { title, ..Default::default() }) };
    match a.get("-o") {
        Some(path) => {
            std::fs::write(path, &doc).map_err(|e| format!("{path} : {e}"))?;
            let mut msg = format!("{path} écrit ({} mouvements)\n", bp.segs.len());
            for w in &bp.warnings {
                msg += &format!("attention : {w}\n");
            }
            Ok(msg)
        }
        None => Ok(doc),
    }
}

fn ext(p: &str) -> String {
    std::path::Path::new(p).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn load_shapes(path: &str) -> Result<(fablabcam_studio::shape::LayeredShapes, Vec<String>), String> {
    let text = read(path)?;
    match ext(path).as_str() {
        "dxf" => fablabcam_studio::dxf::parse_dxf(&text).map(|r| (r.shapes, r.ignored)),
        "svg" => fablabcam_studio::svgio::parse_svg(&text).map(|r| (r.shapes, r.ignored)),
        "cb" => {
            let p = fablabcam_studio::persist::load(&text)?;
            Ok((p.shapes.into_iter().map(|s| (s.layer, s.entity)).collect(), Vec::new()))
        }
        e => Err(format!("format d'entrée « {e} » non géré (cb, dxf, svg)")),
    }
}

fn convert(a: &Args) -> Result<String, String> {
    let (src, dst) = (a.pos.get(1).ok_or("fichier d'entrée manquant")?, a.pos.get(2).ok_or("fichier de sortie manquant")?);
    let (shapes, ignored) = load_shapes(src)?;
    let out = match ext(dst).as_str() {
        "dxf" => fablabcam_studio::dxf::write_dxf(&shapes),
        "svg" => fablabcam_studio::svgio::write_svg(&shapes),
        "cb" => {
            let mut p = fablabcam_studio::Project::new();
            for (l, e) in shapes.iter().cloned() {
                p.add_shape(&l, e).map_err(|e| e.to_string())?;
            }
            fablabcam_studio::persist::save(&p)
        }
        e => return Err(format!("format de sortie « {e} » non géré (cb, dxf, svg)")),
    };
    std::fs::write(dst, out).map_err(|e| format!("{dst} : {e}"))?;
    let mut msg = format!("{dst} écrit ({} formes)\n", shapes.len());
    for i in ignored {
        msg += &format!("ignoré : {i}\n");
    }
    Ok(msg)
}

fn open_project(src: &str) -> Result<fablabcam_studio::Project, String> {
    match src {
        "demo" => Ok(fablabcam_studio::studio::demo_project()),
        "demo-dessin" => Ok(fablabcam_studio::studio::drawing_demo()),
        f if ext(f) == "cb" => fablabcam_studio::persist::load(&read(f)?),
        f => {
            let (shapes, _) = load_shapes(f)?;
            let mut p = fablabcam_studio::Project::new();
            for (l, e) in shapes {
                p.add_shape(&l, e).map_err(|e| e.to_string())?;
            }
            Ok(p)
        }
    }
}

fn web(a: &Args) -> Result<String, String> {
    let src = a.pos.get(1).ok_or("projet manquant (fichier, demo ou demo-dessin)")?;
    let out = a.get("-o").ok_or("-o page.html est requis")?;
    let studio = fablabcam_studio::Studio::new(open_project(src)?);
    let ids: Vec<u32> = studio.project().shapes.iter().map(|s| s.id).collect();
    let sel: Vec<u32> = a
        .get("--select")
        .map(|v| v.split(',').filter_map(|r| r.trim().parse::<usize>().ok()).filter_map(|r| ids.get(r.wrapping_sub(1)).copied()).collect())
        .unwrap_or_default();
    let extra = fablabcam_studio::web::sim_json(&studio, 120).unwrap_or_default();
    let html = fablabcam_studio::web::to_html(&studio, &sel, src, &extra);
    std::fs::write(out, html).map_err(|e| format!("{out} : {e}"))?;
    Ok(format!("{out} écrit ({} formes)\n", ids.len()))
}

fn sim(a: &Args) -> Result<String, String> {
    use fablabcam_studio::sim::{simulate, GougeKind};
    let src = a.pos.get(1).ok_or("projet manquant (fichier ou demo)")?;
    let studio = fablabcam_studio::Studio::new(open_project(src)?);
    let res = a.num("--res")?.unwrap_or(0.2);
    let at = a.num("--at")?.unwrap_or(1.0);
    let t = std::time::Instant::now();
    let out = simulate(&studio, res, at).ok_or("rien à simuler (aucune opération calculée)")?;
    let el = t.elapsed().as_secs_f64();
    let (lo, hi) = out.map.z_range();
    let mut msg = format!(
        "grille {}×{} (maille {res} mm), trajet joué {:.0} %\nZ de {lo:.3} à {hi:.3} mm, volume retiré {:.1} mm³\ntemps de simulation : {el:.2} s\n",
        out.map.nx, out.map.ny, at * 100.0, out.map.removed_volume()
    );
    for g in &out.gouges {
        let k = match g.kind {
            GougeKind::BelowFloor => "sous le fond programmé",
            GougeKind::IntoTab => "dans un pont de maintien",
            GougeKind::RapidInMaterial => "rapide dans la matière",
        };
        msg += &format!("gouge : {k} en ({:.2} ; {:.2} ; {:.2}) — opération « {} »\n", g.pos[0], g.pos[1], g.pos[2], g.op);
    }
    if out.gouges.is_empty() {
        msg += "aucune gouge détectée\n";
    }
    Ok(msg)
}

fn build(a: &Args) -> Result<String, String> {
    let file = a.pos.get(1).ok_or("projet .cb manquant")?;
    let project = fablabcam_studio::persist::load(&read(file)?)?;
    if project.ops.is_empty() {
        return Err("le projet ne contient aucune opération".into());
    }
    let studio = fablabcam_studio::Studio::new(project);
    let mut warn = String::new();
    for o in &studio.project().ops {
        if let Some(e) = studio.computed.get(&o.id).and_then(|c| c.error.as_ref()) {
            warn += &format!("attention : opération « {} » : {e}\n", o.name);
        }
    }
    let g = match a.get("--post") {
        Some(pp) => {
            let (mut def, ignored) = fablabcam_cam::PostDef::parse(&read(pp)?).map_err(|e| format!("{pp} : {e}"))?;
            if let Some(m) = a.get("--macros") {
                def.add_macros(&m.replace(';', "\n"));
            }
            let out = studio.gcode_with_post(&def);
            for w in &out.warnings {
                warn += &format!("attention : post-processeur : {w}\n");
            }
            if !ignored.is_empty() {
                warn += &format!("post-processeur : éléments sans effet : {}\n", ignored.join(", "));
            }
            out.gcode
        }
        None => studio.gcode(),
    };
    match a.get("-o") {
        Some(path) => {
            std::fs::write(path, &g).map_err(|e| format!("{path} : {e}"))?;
            Ok(format!("{path} écrit ({} lignes)\n{warn}", g.lines().count()))
        }
        None => Ok(format!("{g}{warn}")),
    }
}

fn script(a: &Args) -> Result<String, String> {
    let file = a.pos.get(1).ok_or("script .rhai manquant")?;
    let code = read(file)?;
    let base = match a.pos.get(2) {
        Some(p) => fablabcam_studio::persist::load(&read(p)?)?,
        None => fablabcam_studio::Project::new(),
    };
    let out = fablabcam_studio::script::run(&base, &code)?;
    let mut msg = out.log.join("\n");
    if !msg.is_empty() {
        msg.push('\n');
    }
    match a.get("-o") {
        Some(path) => {
            std::fs::write(path, fablabcam_studio::persist::save(&out.project)).map_err(|e| format!("{path} : {e}"))?;
            Ok(format!("{msg}{path} écrit ({} formes, {} opérations)\n", out.project.shapes.len(), out.project.ops.len()))
        }
        None => Ok(format!("{msg}{} formes, {} opérations (ajoutez -o sortie.cb pour enregistrer)\n", out.project.shapes.len(), out.project.ops.len())),
    }
}

/// Exécute la commande et renvoie le texte à afficher (ou écrit dans `-o`).
pub fn run(args: &[String]) -> Result<String, String> {
    if args == ["version"] || args == ["--version"] { return Ok(format!("FabLabCAM {}\n", env!("CARGO_PKG_VERSION"))); }
    if args.first().is_some_and(|s| s == "machine") {
        let out = machine::run(&args[1..])?;
        return if out.exit_code == 0 { Ok(out.text) } else { Err(out.text) };
    }
    let a = split(args)?;
    match a.pos.first().map(String::as_str) {
        None | Some("help") => Ok(USAGE.to_string()),
        Some("info") => info(a.pos.get(1).ok_or("fichier .cb manquant")?),
        Some("styles") => styles(a.pos.get(1).ok_or("fichier de styles manquant")?),
        Some("plot") => plot(&a),
        Some("convert") => convert(&a),
        Some("build") => build(&a),
        Some("script") => script(&a),
        Some("web") => web(&a),
        Some("sim") => sim(&a),
        Some("gcode") => {
            let out = gcode(&a)?;
            match a.get("-o") {
                Some(path) => {
                    std::fs::write(path, &out).map_err(|e| format!("{path} : {e}"))?;
                    Ok(format!("{path} écrit ({} lignes)\n", out.lines().count()))
                }
                None => Ok(out),
            }
        }
        Some(c) => Err(format!("commande inconnue : {c}\n\n{USAGE}")),
    }
}
