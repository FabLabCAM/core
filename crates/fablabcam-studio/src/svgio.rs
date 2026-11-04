//! SVG geometry import: inherited affine transforms, resource exclusion and strict path grammar.
//! Coordinates stay in the document's existing user-unit convention. The Y flip is applied
//! after the complete SVG transform (`y_doc = C - y_svg`). See docs/svg-import.md for limits.

mod affine;
mod curves;
mod export;
mod path;
mod primitives;

use affine::Affine;
pub use export::write_svg;
use fablabcam_model::Entity;

type V = (f64, f64, f64);
const SVG_NS: &str = "http://www.w3.org/2000/svg";

#[derive(Debug, Default)]
pub struct SvgImport {
    pub shapes: Vec<(String, Entity)>,
    pub ignored: Vec<String>,
}

fn len(s: Option<&str>) -> Result<f64, String> {
    s.unwrap_or("0")
        .parse::<svgtypes::Length>()
        .map(|l| l.number)
        .map_err(|e| format!("longueur SVG invalide : {e}"))
}

fn numbers(s: &str) -> Result<Vec<f64>, String> {
    svgtypes::NumberListParser::from(s)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("liste de nombres SVG invalide : {e}"))
}

fn inline_property<'a>(style: Option<&'a str>, name: &str) -> Option<&'a str> {
    style.and_then(|style| {
        style
            .split(';')
            .filter_map(|item| {
                let (key, value) = item.split_once(':')?;
                (key.trim() == name).then_some(value.trim())
            })
            .next_back()
    })
}

fn property<'a>(n: roxmltree::Node<'a, 'a>, name: &str) -> Option<&'a str> {
    inline_property(n.attribute("style"), name).or_else(|| n.attribute(name))
}

fn resource(tag: &str) -> bool {
    matches!(
        tag,
        "defs"
            | "clipPath"
            | "mask"
            | "pattern"
            | "marker"
            | "symbol"
            | "metadata"
            | "title"
            | "desc"
            | "style"
            | "linearGradient"
            | "radialGradient"
    )
}

fn label(n: roxmltree::Node<'_, '_>) -> String {
    let id = n
        .attribute("id")
        .map(|id| format!(" #{id}"))
        .unwrap_or_default();
    format!("<{}>{id}", n.tag_name().name())
}

pub fn parse_svg(text: &str) -> Result<SvgImport, String> {
    let doc = roxmltree::Document::parse(text).map_err(|e| format!("SVG invalide : {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg" || root.tag_name().namespace().is_some_and(|ns| ns != SVG_NS)
    {
        return Err("l'élément racine n'est pas <svg>".into());
    }
    let vb = root
        .attribute("viewBox")
        .map(numbers)
        .transpose()?
        .unwrap_or_default();
    if !vb.is_empty() && (vb.len() != 4 || vb[2] <= 0.0 || vb[3] <= 0.0) {
        return Err("viewBox SVG invalide".into());
    }
    let c = match root.attribute("data-doc-ymax") {
        Some(value) => value
            .parse::<f64>()
            .map_err(|_| "data-doc-ymax SVG invalide")?,
        None if vb.len() == 4 => vb[1] + vb[3],
        None => len(root.attribute("height"))?,
    };
    if !c.is_finite() {
        return Err("origine Y SVG non finie".into());
    }
    let mut out = SvgImport::default();
    // An explicit stack keeps parent frames isolated and avoids recursion on untrusted XML.
    let mut stack = vec![(root, Affine::default())];
    while let Some((n, parent)) = stack.pop() {
        if !n.is_element() || n.tag_name().namespace().is_some_and(|ns| ns != SVG_NS) {
            continue;
        }
        let tag = n.tag_name().name();
        if resource(tag) || property(n, "display") == Some("none") {
            continue;
        }
        let local =
            Affine::parse(n.attribute("transform")).map_err(|e| format!("{} : {e}", label(n)))?;
        let transform = parent
            .compose(local)
            .map_err(|e| format!("{} : {e}", label(n)))?;
        for effect in ["clip-path", "mask", "filter"] {
            if property(n, effect).is_some_and(|v| v != "none") {
                out.ignored.push(format!(
                    "{effect} de {} non pris en charge (import incomplet)",
                    label(n)
                ));
            }
        }
        if inline_property(n.attribute("style"), "transform").is_some() {
            out.ignored.push(format!(
                "transformation CSS de {} non prise en charge",
                label(n)
            ));
        }
        match tag {
            "svg" if n == root => stack.extend(n.children().rev().map(|child| (child, transform))),
            "g" | "a" => stack.extend(n.children().rev().map(|child| (child, transform))),
            "line" | "polyline" | "polygon" | "rect" | "circle" | "path" | "ellipse" => {
                primitives::import(n, transform, c, &mut out)
                    .map_err(|e| format!("{} : {e}", label(n)))?;
            }
            _ => out.ignored.push(format!(
                "{} non pris en charge (import incomplet)",
                label(n)
            )),
        }
    }
    Ok(out)
}
