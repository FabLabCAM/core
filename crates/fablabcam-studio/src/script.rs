//! Scripts d'automatisation (langage Rhai) : dessin, transformations, outils et opérations.
//!
//! Le script travaille sur une copie du projet ; en cas d'erreur rien n'est modifié. Fonctions disponibles
//! (les nombres acceptent entiers et décimaux, `ids` est un identifiant ou un tableau d'identifiants) :
//!
//! | fonction | effet |
//! |---|---|
//! | `layer(nom)` | calque des formes créées ensuite |
//! | `line(x0, y0, x1, y1)`, `rect(x0, y0, x1, y1)`, `circle(cx, cy, r)` | crée une forme, renvoie son id |
//! | `arc(cx, cy, r, debut, angle)` | arc (degrés, sens trigonométrique) |
//! | `polyline([[x, y], …], fermee)` | polyligne |
//! | `text(texte, x, y, hauteur)` | contours du texte (police système), renvoie les ids |
//! | `shapes()` | ids de toutes les formes |
//! | `move(ids, dx, dy)`, `rotate(ids, cx, cy, deg)`, `scale(ids, cx, cy, k)` | transforme |
//! | `copy(ids, dx, dy)` | copie décalée, renvoie les nouveaux ids |
//! | `offset(ids, d)` | décalage, renvoie les nouveaux ids |
//! | `delete(ids)` | supprime |
//! | `tool(nom, diametre)`, `drill_tool(nom, diametre)` | ajoute un outil (fraise droite / foret) |
//! | `profile(nom, ids, outil, prof)`, `pocket(…)`, `drill(…)`, `engrave(…)` | ajoute une opération, renvoie son id |
//! | `print(…)` | écrit dans le journal |

use crate::cad::Similarity;
use crate::project::{OpKind, Project, Tool, ToolKind};
use crate::shape::{self, EditError};
use fablabcam_model::Entity;
use rhai::{Array, Dynamic, Engine, EvalAltResult};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct ScriptOutput {
    pub project: Project,
    /// Sorties de `print`.
    pub log: Vec<String>,
}

struct State {
    project: Project,
    layer: String,
    log: Vec<String>,
}

type Res<T> = Result<T, Box<EvalAltResult>>;

fn err<T>(m: impl ToString) -> Res<T> {
    Err(m.to_string().into())
}

fn num(d: &Dynamic) -> Res<f64> {
    if let Ok(f) = d.as_float() {
        Ok(f)
    } else if let Ok(i) = d.as_int() {
        Ok(i as f64)
    } else {
        err(format!("nombre attendu, reçu {}", d.type_name()))
    }
}

fn shape_id(i: i64) -> Res<u32> {
    // Ne pas tronquer : un entier hors plage pourrait sinon viser une autre forme.
    u32::try_from(i).or_else(|_| err(format!("identifiant de forme hors plage (0..={}) : {i}", u32::MAX)))
}

fn ids(d: &Dynamic) -> Res<Vec<u32>> {
    if let Ok(i) = d.as_int() {
        return Ok(vec![shape_id(i)?]);
    }
    match d.clone().try_cast::<Array>() {
        Some(a) => a.iter().map(|x| x.as_int().or_else(|_| err("identifiant de forme attendu")).and_then(shape_id)).collect(),
        None => err("identifiant ou tableau d'identifiants attendu"),
    }
}

fn to_array(v: Vec<u32>) -> Array {
    v.into_iter().map(|i| Dynamic::from(i as i64)).collect()
}

fn edit<T>(e: Result<T, EditError>) -> Res<T> {
    e.or_else(|e| err(e))
}

/// Exécute `script` sur une copie de `project`. Au-delà de `max_ops` opérations élémentaires, le script est
/// arrêté (boucle infinie).
pub fn run(project: &Project, script: &str) -> Result<ScriptOutput, String> {
    run_controlled(project, script, std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)))
}

/// Compile only: never executes a generated script while editing its text.
pub fn validate(script: &str) -> Result<(), String> {
    if script.len() > 64_000 { return Err("Script limité à 64 Ko".into()); }
    Engine::new().compile(script).map(|_| ()).map_err(|e| format!("script : {e}"))
}

/// Bounded, cancellable execution shared by the assistant and ordinary script runner.
pub fn run_controlled(project: &Project, script: &str,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<ScriptOutput, String> {
    validate(script)?;
    let st = Rc::new(RefCell::new(State { project: project.clone(), layer: "0".into(), log: Vec::new() }));
    let mut engine = Engine::new();
    engine.set_module_resolver(rhai::module_resolvers::DummyModuleResolver::new());
    engine.set_max_operations(5_000_000);
    engine.set_max_expr_depths(64, 64);
    engine.set_max_string_size(64_000);
    engine.set_max_array_size(10_000);
    engine.set_max_map_size(1000);
    engine.set_max_call_levels(32);
    engine.set_max_variables(256);
    engine.set_max_modules(0);
    let started=std::time::Instant::now();
    engine.on_progress(move |_| {
        (cancelled.load(std::sync::atomic::Ordering::Relaxed) || started.elapsed().as_secs() >= 10)
            .then(|| Dynamic::from("script arrêté ou délai de 10 secondes dépassé"))
    });
    {
        let s = st.clone();
        engine.on_print(move |t| { if s.borrow().log.len()<1000 {s.borrow_mut().log.push(t.chars().take(1000).collect());} });
    }
    macro_rules! with {
        ($s:ident, $body:expr) => {{
            let $s = st.clone();
            $body
        }};
    }
    let add = |s: &Rc<RefCell<State>>, e: Result<Entity, EditError>| -> Res<i64> {
        let e = edit(e)?;
        let mut b = s.borrow_mut();
        if b.project.shapes.len()>=10_000 {return err("script limité à 10 000 formes");}
        let layer = b.layer.clone();
        edit(b.project.add_shape(&layer, e)).map(|i| i as i64)
    };
    with!(s, engine.register_fn("layer", move |name: &str| s.borrow_mut().layer = name.to_string()));
    with!(s, engine.register_fn("line", move |a: Dynamic, b: Dynamic, c: Dynamic, d: Dynamic| -> Res<i64> { add(&s, shape::line((num(&a)?, num(&b)?), (num(&c)?, num(&d)?))) }));
    with!(s, engine.register_fn("rect", move |a: Dynamic, b: Dynamic, c: Dynamic, d: Dynamic| -> Res<i64> { add(&s, shape::rectangle((num(&a)?, num(&b)?), (num(&c)?, num(&d)?))) }));
    with!(s, engine.register_fn("circle", move |a: Dynamic, b: Dynamic, r: Dynamic| -> Res<i64> { add(&s, shape::circle((num(&a)?, num(&b)?), num(&r)?)) }));
    with!(s, engine.register_fn("arc", move |a: Dynamic, b: Dynamic, r: Dynamic, st0: Dynamic, sw: Dynamic| -> Res<i64> { add(&s, shape::arc((num(&a)?, num(&b)?), num(&r)?, num(&st0)?, num(&sw)?)) }));
    with!(s, engine.register_fn("polyline", move |pts: Array, closed: bool| -> Res<i64> {
        let v: Res<Vec<(f64, f64)>> = pts
            .iter()
            .map(|p| {
                let a = p.clone().try_cast::<Array>().filter(|a| a.len() == 2).map_or_else(|| err("point [x, y] attendu"), Ok)?;
                Ok((num(&a[0])?, num(&a[1])?))
            })
            .collect();
        add(&s, shape::polyline(&v?, closed))
    }));
    with!(s, engine.register_fn("text", move |t: &str, x: Dynamic, y: Dynamic, h: Dynamic| -> Res<Array> {
        if t.chars().count()>1000 {return err("texte limité à 1000 caractères par appel");}
        let font = crate::text::default_font().map_or_else(|| err("aucune police système trouvée"), Ok)?;
        let data = std::fs::read(font).or_else(|e| err(format!("{font} : {e}")))?;
        let style = crate::text::TextStyle { height: num(&h)?, ..Default::default() };
        let (shapes, _) = crate::text::outlines(&data, t, (num(&x)?, num(&y)?), &style).or_else(err)?;
        let mut out = Vec::new();
        for e in shapes {
            out.push(add(&s, Ok(e))? as u32);
        }
        Ok(to_array(out))
    }));
    with!(s, engine.register_fn("shapes", move || -> Array { to_array(s.borrow().project.shapes.iter().map(|x| x.id).collect()) }));
    with!(s, engine.register_fn("move", move |i: Dynamic, dx: Dynamic, dy: Dynamic| -> Res<()> { edit(s.borrow_mut().project.move_shapes(&ids(&i)?, num(&dx)?, num(&dy)?)) }));
    with!(s, engine.register_fn("rotate", move |i: Dynamic, cx: Dynamic, cy: Dynamic, deg: Dynamic| -> Res<()> {
        edit(s.borrow_mut().project.transform_shapes(&ids(&i)?, &Similarity::rotation((num(&cx)?, num(&cy)?), num(&deg)?)))
    }));
    with!(s, engine.register_fn("scale", move |i: Dynamic, cx: Dynamic, cy: Dynamic, k: Dynamic| -> Res<()> {
        let t = edit(Similarity::scale((num(&cx)?, num(&cy)?), num(&k)?))?;
        edit(s.borrow_mut().project.transform_shapes(&ids(&i)?, &t))
    }));
    with!(s, engine.register_fn("copy", move |i: Dynamic, dx: Dynamic, dy: Dynamic| -> Res<Array> {
        let selected=ids(&i)?;
        if s.borrow().project.shapes.len()+selected.len()>10_000 {return err("script limité à 10 000 formes");}
        edit(s.borrow_mut().project.copy_shapes(&selected, &Similarity::translation(num(&dx)?, num(&dy)?))).map(to_array)
    }));
    with!(s, engine.register_fn("offset", move |i: Dynamic, d: Dynamic| -> Res<Array> {
        let mut b=s.borrow_mut();
        let out=edit(b.project.offset_shapes(&ids(&i)?, num(&d)?))?;
        if b.project.shapes.len()>10_000 {return err("script limité à 10 000 formes");}
        Ok(to_array(out))
    }));
    with!(s, engine.register_fn("delete", move |i: Dynamic| -> Res<()> {
        let v = ids(&i)?;
        let mut b = s.borrow_mut();
        if let Some(bad) = v.iter().find(|id| b.project.shape(**id).is_none()) {
            return err(EditError::UnknownShape(*bad));
        }
        b.project.delete_shapes(&v);
        Ok(())
    }));
    for (name, kind) in [("tool", ToolKind::EndMill), ("drill_tool", ToolKind::Drill)] {
        with!(s, engine.register_fn(name, move |n: &str, d: Dynamic| -> Res<()> { if s.borrow().project.tools.len()>=1000 {return err("script limité à 1000 outils");} edit(s.borrow_mut().project.add_tool(Tool::new(n, kind, num(&d)?))) }));
    }
    for (name, kind) in [("profile", OpKind::Profile), ("pocket", OpKind::Pocket), ("drill", OpKind::Drill), ("engrave", OpKind::Engrave)] {
        with!(s, engine.register_fn(name, move |n: &str, i: Dynamic, tool: &str, depth: Dynamic| -> Res<i64> {
            let (v, depth) = (ids(&i)?, num(&depth)?);
            let mut b = s.borrow_mut();
            if b.project.tool(tool).is_none() {
                return err(EditError::UnknownTool(tool.into()));
            }
            if b.project.ops.len()>=200 {return err("script limité à 200 opérations");}
            let id = edit(b.project.add_operation(kind, n, v, tool))?;
            edit(b.project.update_operation(id, |o| o.depth = depth))?;
            Ok(id as i64)
        }));
    }
    engine.run(script).map_err(|e| format!("script : {e}"))?;
    if st.borrow().project.shapes.len()>10_000 {return Err("script limité à 10 000 formes".into());}
    drop(engine);
    let st = Rc::try_unwrap(st).map_err(|_| "état du script encore utilisé".to_string())?.into_inner();
    Ok(ScriptOutput { project: st.project, log: st.log })
}
