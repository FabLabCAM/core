//! Modèle de document FabLabCAM et lecture/écriture XML d'un dessin `.cb`.
//!
//! Format observé sur de vrais fichiers format XML 0.9.8 : `<CADFile>` → `<layers>` → `<layer name="…">` →
//! `<objects>` → `<pline id="2" Closed="true">` / `<rect id="1" …>` avec `<mat m="Identity"/>` et
//! `<pts><p>x,y,z</p>…</pts>`. Le reste (courbure `b` d'un sommet, `circle`, `arc`, `points`, matrice autre
//! que `Identity`) n'a pas encore été vu dans un fichier réel : le lecteur reste tolérant sur ces points.
//! Les fichiers de FabLabCAM plus anciens (`<Layers>/<Layer Name>`, `<Point3D X=… Y=…>`) restent lus.

use fablabcam_geom::{self as geom, Pline, PlineSource};

#[derive(Debug, Clone)]
pub enum Entity {
    /// Polyligne (ouverte ou fermée) avec arcs éventuels.
    Polyline(Pline),
    Circle { cx: f64, cy: f64, r: f64 },
    Arc { cx: f64, cy: f64, r: f64, start_deg: f64, sweep_deg: f64 },
    Point { x: f64, y: f64 },
    /// Rectangle paramétrique : coin d'origine `(x, y)`, largeur `w`, hauteur `h`, rayon de coin `r` (0 = coins vifs),
    /// puis rotation de `rot_deg` degrés (sens trigonométrique) autour du coin d'origine. Il se convertit en
    /// polyligne (`to_pline`) pour tout le reste de la chaîne.
    Rect { x: f64, y: f64, w: f64, h: f64, r: f64, rot_deg: f64 },
}

/// Contour d'un rectangle paramétrique : sommets dans l'ordre (origine, +x, +x+y, +y), coins arrondis par des arcs
/// de bulge tan(π/8).
pub fn rect_pline(x: f64, y: f64, w: f64, h: f64, r: f64, rot_deg: f64) -> Pline {
    let b = (std::f64::consts::PI / 8.0).tan();
    let mut v: Vec<(f64, f64, f64)> = if r > 1e-9 {
        vec![(r, 0.0, 0.0), (w - r, 0.0, b), (w, r, 0.0), (w, h - r, b), (w - r, h, 0.0), (r, h, b), (0.0, h - r, 0.0), (0.0, r, b)]
    } else {
        vec![(0.0, 0.0, 0.0), (w, 0.0, 0.0), (w, h, 0.0), (0.0, h, 0.0)]
    };
    // côté entièrement arrondi (2 r = côté) : le segment droit est nul, on retire le doublon
    let n = v.len();
    let same = |a: (f64, f64, f64), c: (f64, f64, f64)| (a.0 - c.0).abs() < 1e-9 && (a.1 - c.1).abs() < 1e-9;
    let keep: Vec<bool> = (0..n).map(|i| !(v[i].2 == 0.0 && same(v[i], v[(i + 1) % n]))).collect();
    let mut k = keep.iter();
    v.retain(|_| *k.next().unwrap_or(&true));
    let (s, c) = rot_deg.to_radians().sin_cos();
    let pts: Vec<(f64, f64, f64)> = v.iter().map(|&(lx, ly, bu)| (x + c * lx - s * ly, y + s * lx + c * ly, bu)).collect();
    geom::pline_from(&pts, true)
}

fn pline_eq(a: &Pline, b: &Pline) -> bool {
    a.is_closed() == b.is_closed()
        && a.vertex_count() == b.vertex_count()
        && (0..a.vertex_count()).all(|i| {
            let (u, v) = (a.at(i), b.at(i));
            (u.x - v.x).abs() < 1e-9 && (u.y - v.y).abs() < 1e-9 && (u.bulge - v.bulge).abs() < 1e-9
        })
}

impl PartialEq for Entity {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Entity::Polyline(a), Entity::Polyline(b)) => pline_eq(a, b),
            (Entity::Circle { cx: a, cy: b, r: c }, Entity::Circle { cx: d, cy: e, r: f }) => (a, b, c) == (d, e, f),
            (
                Entity::Arc { cx: a, cy: b, r: c, start_deg: d, sweep_deg: e },
                Entity::Arc { cx: a2, cy: b2, r: c2, start_deg: d2, sweep_deg: e2 },
            ) => (a, b, c, d, e) == (a2, b2, c2, d2, e2),
            (Entity::Point { x: a, y: b }, Entity::Point { x: c, y: d }) => (a, b) == (c, d),
            (Entity::Rect { x: a, y: b, w: c, h: d, r: e, rot_deg: f }, Entity::Rect { x: a2, y: b2, w: c2, h: d2, r: e2, rot_deg: f2 }) => (a, b, c, d, e, f) == (a2, b2, c2, d2, e2, f2),
            _ => false,
        }
    }
}

impl Entity {
    /// Contour exploitable pour l'usinage (None pour un point).
    pub fn to_pline(&self) -> Option<Pline> {
        match self {
            Entity::Polyline(p) => Some(p.clone()),
            Entity::Circle { cx, cy, r } => Some(geom::circle(*cx, *cy, *r)),
            Entity::Arc { cx, cy, r, start_deg, sweep_deg } => Some(geom::arc(*cx, *cy, *r, *start_deg, *sweep_deg)),
            Entity::Rect { x, y, w, h, r, rot_deg } => Some(rect_pline(*x, *y, *w, *h, *r, *rot_deg)),
            Entity::Point { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layer {
    pub name: String,
    pub entities: Vec<Entity>,
    /// Identifiant XML (`id`) de chaque entité lue, au même rang que `entities` ; les opérations
    /// d'un vrai `.cb` désignent leurs formes par cet identifiant. Vide pour un document construit.
    pub ids: Vec<Option<u32>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    pub layers: Vec<Layer>,
}

impl Document {
    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.layers.iter().flat_map(|l| l.entities.iter())
    }

    pub fn layer(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name == name)
    }
}

#[derive(Debug)]
pub enum ParseError {
    Xml(String),
    MissingDocumentRoot,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Xml(e) => write!(f, "XML invalide : {e}"),
            ParseError::MissingDocumentRoot => write!(f, "élément racine CADFile introuvable"),
        }
    }
}

impl std::error::Error for ParseError {}

fn num(s: Option<&str>) -> Option<f64> {
    s.and_then(|v| v.trim().replace(',', ".").parse::<f64>().ok())
}

/// Valeur numérique d'un attribut (insensible à la casse) ou d'un élément enfant du même nom.
fn field(n: roxmltree::Node, names: &[&str]) -> Option<f64> {
    for name in names {
        for a in n.attributes() {
            if a.name().eq_ignore_ascii_case(name) {
                if let Some(v) = num(Some(a.value())) {
                    return Some(v);
                }
            }
        }
        for c in n.children().filter(|c| c.is_element()) {
            if c.tag_name().name().eq_ignore_ascii_case(name) {
                if let Some(v) = num(c.text()) {
                    return Some(v);
                }
            }
        }
    }
    None
}

fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, names: &[&str]) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| c.is_element() && names.iter().any(|x| c.tag_name().name().eq_ignore_ascii_case(x)))
}

fn flag(n: roxmltree::Node, name: &str) -> bool {
    n.attributes().any(|a| a.name().eq_ignore_ascii_case(name) && a.value().eq_ignore_ascii_case("true"))
}

fn center_of(n: roxmltree::Node) -> Option<(f64, f64)> {
    if let Some(c) = child(n, &["Center", "C"]) {
        let x = field(c, &["X"])?;
        let y = field(c, &["Y"])?;
        return Some((x, y));
    }
    Some((field(n, &["CX", "X"])?, field(n, &["CY", "Y"])?))
}

/// Sommet : texte `x,y,z` (le logiciel de référence) ou attributs / enfants `X`, `Y` ; courbure `b` / `Bulge`.
fn vertex(p: roxmltree::Node) -> Option<(f64, f64, f64)> {
    let bulge = field(p, &["b", "Bulge"]).unwrap_or(0.0);
    if let Some(t) = p.text().map(str::trim).filter(|t| !t.is_empty()) {
        let v: Vec<f64> = t.split(',').filter_map(|c| c.trim().parse().ok()).collect();
        if v.len() >= 2 {
            return Some((v[0], v[1], bulge));
        }
    }
    Some((field(p, &["X"])?, field(p, &["Y"])?, bulge))
}

/// Point `x,y,z` porté par un attribut (`p` ou `c`).
fn xyz_attr(n: roxmltree::Node, name: &str) -> Option<(f64, f64)> {
    let a = n.attributes().find(|a| a.name().eq_ignore_ascii_case(name))?;
    let v: Vec<f64> = a.value().split(',').filter_map(|c| c.trim().parse().ok()).collect();
    (v.len() >= 2).then(|| (v[0], v[1]))
}

fn points_of(n: roxmltree::Node) -> Option<Vec<(f64, f64, f64)>> {
    let pts = child(n, &["pts", "Points"])?;
    pts.children().filter(|c| c.is_element()).map(vertex).collect()
}

fn parse_pline(n: roxmltree::Node) -> Option<Entity> {
    let v = points_of(n)?;
    (v.len() >= 2).then(|| Entity::Polyline(geom::pline_from(&v, flag(n, "Closed"))))
}

/// Rectangle XML : ses sommets (`pts`) s'ils sont présents, sinon le coin `p` et `w` × `h`.
fn parse_rect(n: roxmltree::Node) -> Option<Entity> {
    let param = xyz_attr(n, "p").zip(field(n, &["w"]).zip(field(n, &["h"])));
    let mut fallback = None;
    if let Some(((x, y), (w, h))) = param {
        let (r, rot_deg) = (field(n, &["corner"]).unwrap_or(0.0), field(n, &["rot"]).unwrap_or(0.0));
        let rect = Entity::Rect { x, y, w, h, r, rot_deg };
        // un vrai `.cb` porte aussi les 4 coins : si l'utilisateur les a déplacés, ce n'est plus un rectangle
        let own = child(n, &["pts"]).is_none() || n.attribute("corner").is_some() || n.attribute("rot").is_some();
        match (own, parse_pline(n), matrix(n)) {
            (true, _, None) => return Some(rect),
            (false, Some(Entity::Polyline(p)), None) if rect.to_pline().is_some_and(|q| pline_eq(&p, &q)) => return Some(rect),
            _ => fallback = rect.to_pline().map(Entity::Polyline),
        }
    }
    if let Some(e) = parse_pline(n).or(fallback) {
        return Some(match e {
            Entity::Polyline(p) if !p.is_closed() => {
                let v: Vec<_> = (0..p.vertex_count()).map(|i| (p.at(i).x, p.at(i).y, p.at(i).bulge)).collect();
                Entity::Polyline(geom::pline_from(&v, true))
            }
            e => e,
        });
    }
    None
}

/// Matrice `<mat m="…"/>` : `Identity` ou 16 nombres (matrice 4 × 4, translation en dernière ligne,
/// convention supposée). Renvoie la transformation plane (a, b, c, d, tx, ty) : x' = a·x + c·y + tx.
fn matrix(n: roxmltree::Node) -> Option<[f64; 6]> {
    let m = child(n, &["mat"])?.attribute("m")?.trim();
    if m.eq_ignore_ascii_case("Identity") {
        return None;
    }
    let v: Vec<f64> = m.split([' ', ',', ';']).filter_map(|c| c.trim().parse().ok()).collect();
    (v.len() == 16).then(|| [v[0], v[1], v[4], v[5], v[12], v[13]])
}

fn transform(e: Entity, m: [f64; 6]) -> Entity {
    let [a, b, c, d, tx, ty] = m;
    let t = |x: f64, y: f64| (a * x + c * y + tx, b * x + d * y + ty);
    let scale = (a * d - b * c).abs().sqrt();
    let mirror = a * d - b * c < 0.0;
    match e {
        Entity::Polyline(p) => {
            let v: Vec<_> = (0..p.vertex_count())
                .map(|i| {
                    let q = p.at(i);
                    let (x, y) = t(q.x, q.y);
                    (x, y, if mirror { -q.bulge } else { q.bulge })
                })
                .collect();
            Entity::Polyline(geom::pline_from(&v, p.is_closed()))
        }
        Entity::Circle { cx, cy, r } => {
            let (cx, cy) = t(cx, cy);
            Entity::Circle { cx, cy, r: r * scale }
        }
        Entity::Arc { .. } | Entity::Rect { .. } | Entity::Point { .. } => match e.to_pline() {
            Some(p) => transform(Entity::Polyline(p), m),
            None => {
                let Entity::Point { x, y } = e else { unreachable!() };
                let (x, y) = t(x, y);
                Entity::Point { x, y }
            }
        },
    }
}

fn center_or_attr(n: roxmltree::Node, attr: &str) -> Option<(f64, f64)> {
    xyz_attr(n, attr).or_else(|| center_of(n))
}

fn parse_entity(n: roxmltree::Node) -> Option<Vec<Entity>> {
    let one = |e: Option<Entity>| e.map(|e| vec![e]);
    let v = match n.tag_name().name().to_ascii_lowercase().as_str() {
        "pline" | "polyline" => one(parse_pline(n)),
        "rect" => one(parse_rect(n)),
        "circle" => {
            let (cx, cy) = center_or_attr(n, "c")?;
            let r = field(n, &["Radius", "R"]).or_else(|| field(n, &["Diameter", "D"]).map(|d| d / 2.0))?;
            one(Some(Entity::Circle { cx, cy, r }))
        }
        "arc" => {
            let (cx, cy) = center_or_attr(n, "p")?;
            one(Some(Entity::Arc {
                cx,
                cy,
                r: field(n, &["Radius", "R"])?,
                start_deg: field(n, &["Start", "StartAngle", "s"]).unwrap_or(0.0),
                sweep_deg: field(n, &["Sweep", "Extent", "w"]).unwrap_or(90.0),
            }))
        }
        "point" => one(Some(Entity::Point { x: field(n, &["X"])?, y: field(n, &["Y"])? })),
        // liste de points XML (perçages) : un point par sommet
        "points" | "pointlist" => Some(points_of(n)?.into_iter().map(|(x, y, _)| Entity::Point { x, y }).collect()),
        _ => None,
    }?;
    Some(match matrix(n) {
        Some(m) => v.into_iter().map(|e| transform(e, m)).collect(),
        None => v,
    })
}

fn collect_entities(n: roxmltree::Node, layer: &mut Layer) {
    for c in n.children().filter(|c| c.is_element()) {
        match parse_entity(c) {
            Some(es) => {
                let id = c.attributes().find(|a| a.name().eq_ignore_ascii_case("id")).and_then(|a| a.value().parse().ok());
                for e in es {
                    layer.entities.push(e);
                    layer.ids.push(id);
                }
            }
            None => collect_entities(c, layer),
        }
    }
}

/// Lit un dessin `.cb` (XML). Un BOM UTF-8 éventuel est ignoré.
pub fn parse_cb(xml: &str) -> Result<Document, ParseError> {
    let xml = xml.trim_start_matches('\u{feff}');
    let tree = roxmltree::Document::parse(xml).map_err(|e| ParseError::Xml(e.to_string()))?;
    let root = tree.root_element();
    if !root.tag_name().name().eq_ignore_ascii_case("CADFile") {
        return Err(ParseError::MissingDocumentRoot);
    }
    let mut doc = Document::default();
    let layers_node = child(root, &["Layers"]).unwrap_or(root);
    let layers: Vec<_> = layers_node.children().filter(|c| c.is_element() && c.tag_name().name().eq_ignore_ascii_case("Layer")).collect();
    if layers.is_empty() {
        let mut layer = Layer { name: "0".into(), ..Layer::default() };
        collect_entities(root, &mut layer);
        doc.layers.push(layer);
    } else {
        for l in layers {
            let name = l.attributes().find(|a| a.name().eq_ignore_ascii_case("Name")).map(|a| a.value().to_string()).unwrap_or_default();
            let mut layer = Layer { name, ..Layer::default() };
            collect_entities(l, &mut layer);
            doc.layers.push(layer);
        }
    }
    Ok(doc)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
}

/// Écrit un document au format lu par [`parse_cb`] (aller-retour garanti).
pub fn write_cb(doc: &Document) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<CADFile>\n<Layers>\n");
    for l in &doc.layers {
        s += &format!("<Layer Name=\"{}\">\n", esc(&l.name));
        for (i, e) in l.entities.iter().enumerate() {
            let id = l.ids.get(i).copied().flatten().map_or(String::new(), |id| format!(" id=\"{id}\""));
            match e {
                Entity::Polyline(p) => {
                    s += &format!("<pline{id} Closed=\"{}\"><Pts>", p.is_closed());
                    for i in 0..p.vertex_count() {
                        let v = p.at(i);
                        s += &format!("<Point3D X=\"{}\" Y=\"{}\" Z=\"0\" Bulge=\"{}\"/>", v.x, v.y, v.bulge);
                    }
                    s += "</Pts></pline>\n";
                }
                Entity::Circle { cx, cy, r } => s += &format!("<circle{id}><Center X=\"{cx}\" Y=\"{cy}\" Z=\"0\"/><Radius>{r}</Radius></circle>\n"),
                Entity::Arc { cx, cy, r, start_deg, sweep_deg } => {
                    s += &format!("<arc{id}><Center X=\"{cx}\" Y=\"{cy}\" Z=\"0\"/><Radius>{r}</Radius><Start>{start_deg}</Start><Sweep>{sweep_deg}</Sweep></arc>\n")
                }
                Entity::Point { x, y } => s += &format!("<point{id} X=\"{x}\" Y=\"{y}\"/>\n"),
                Entity::Rect { x, y, w, h, r, rot_deg } => s += &format!("<rect{id} Closed=\"true\" p=\"{x},{y},0\" w=\"{w}\" h=\"{h}\" corner=\"{r}\" rot=\"{rot_deg}\"/>\n"),
            }
        }
        s += "</Layer>\n";
    }
    s += "</Layers>\n</CADFile>\n";
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<CADFile>
<Layers>
<Layer Name="Contour">
<pline Id="1" Closed="true"><Pts><Point3D X="0" Y="0" Z="0"/><Point3D X="100" Y="0" Z="0"/><Point3D X="100" Y="50" Z="0"/><Point3D X="0" Y="50" Z="0"/></Pts></pline>
</Layer>
<Layer Name="Trous">
<circle><Center X="10" Y="10" Z="0"/><Radius>3</Radius></circle>
<circle X="90" Y="40" R="3"/>
<point X="50" Y="25"/>
</Layer>
</Layers>
</CADFile>"#;

    #[test]
    fn parses_layers_and_entities() {
        let d = parse_cb(SAMPLE).unwrap();
        assert_eq!(d.layers.len(), 2);
        assert_eq!(d.layer("Contour").unwrap().entities.len(), 1);
        let holes = &d.layer("Trous").unwrap().entities;
        assert_eq!(holes.len(), 3);
        assert_eq!(holes[0], Entity::Circle { cx: 10.0, cy: 10.0, r: 3.0 });
        assert_eq!(holes[1], Entity::Circle { cx: 90.0, cy: 40.0, r: 3.0 });
    }

    #[test]
    fn polyline_is_closed_with_four_vertices() {
        let d = parse_cb(SAMPLE).unwrap();
        match &d.layers[0].entities[0] {
            Entity::Polyline(p) => {
                assert!(p.is_closed());
                assert_eq!(p.vertex_count(), 4);
            }
            e => panic!("{e:?}"),
        }
    }

    #[test]
    fn round_trip() {
        let d = parse_cb(SAMPLE).unwrap();
        assert_eq!(parse_cb(&write_cb(&d)).unwrap(), d);
    }

    #[test]
    fn bom_and_decimal_comma_are_tolerated() {
        let d = parse_cb("\u{feff}<CADFile><point X=\"1,5\" Y=\"2\"/></CADFile>").unwrap();
        assert_eq!(d.layers[0].entities[0], Entity::Point { x: 1.5, y: 2.0 });
    }

    /// Extrait d'un vrai fichier format XML 0.9.8 (structure et attributs observés).
    const REAL: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<CADFile xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" Version="0.9.8.0" Name="t">
  <layers>
    <layer name="Default" color="127,255,0">
      <objects>
        <rect id="1" Closed="true" p="-50,-43,0" w="91" h="516">
          <mat m="Identity" />
          <pts><p>-50,-43,0</p><p>41,-43,0</p><p>41,473,0</p><p>-50,473,0</p></pts>
        </rect>
        <pline id="2"><mat m="Identity" /><pts><p>0,0,0</p><p>-510,0,0</p></pts></pline>
      </objects>
    </layer>
  </layers>
</CADFile>"#;

    #[test]
    fn reads_native_layers_objects_and_ids() {
        let d = parse_cb(REAL).unwrap();
        let l = d.layer("Default").unwrap();
        assert_eq!(l.ids, vec![Some(1), Some(2)]);
        assert_eq!(l.entities[0], Entity::Rect { x: -50.0, y: -43.0, w: 91.0, h: 516.0, r: 0.0, rot_deg: 0.0 });
        let r = l.entities[0].to_pline().unwrap();
        assert!(r.is_closed() && r.vertex_count() == 4);
        assert_eq!((r.at(2).x, r.at(2).y), (41.0, 473.0));
        let Entity::Polyline(line) = &l.entities[1] else { panic!() };
        assert!(!line.is_closed());
        assert_eq!((line.at(1).x, line.at(1).y), (-510.0, 0.0));
    }

    #[test]
    fn rect_without_points_uses_corner_and_size() {
        let d = parse_cb(r#"<CADFile><layers><layer name="a"><objects><rect id="4" p="1,2,0" w="10" h="5"/></objects></layer></layers></CADFile>"#).unwrap();
        assert_eq!(d.layers[0].entities[0], Entity::Rect { x: 1.0, y: 2.0, w: 10.0, h: 5.0, r: 0.0, rot_deg: 0.0 });
        let r = d.layers[0].entities[0].to_pline().unwrap();
        assert!(r.is_closed());
        assert_eq!((r.at(2).x, r.at(2).y), (11.0, 7.0));
    }

    #[test]
    fn rect_with_moved_corners_stays_a_polyline() {
        let d = parse_cb(r#"<CADFile><layers><layer name="a"><objects><rect id="4" Closed="true" p="0,0,0" w="10" h="5"><mat m="Identity"/><pts><p>0,0,0</p><p>10,0,0</p><p>12,5,0</p><p>0,5,0</p></pts></rect></objects></layer></layers></CADFile>"#).unwrap();
        assert!(matches!(d.layers[0].entities[0], Entity::Polyline(_)));
    }

    #[test]
    fn rounded_rotated_rect_round_trips_through_cb() {
        let doc = Document { layers: vec![Layer { name: "a".into(), entities: vec![Entity::Rect { x: 3.0, y: 4.0, w: 20.0, h: 10.0, r: 2.5, rot_deg: 30.0 }], ids: vec![Some(7)] }] };
        let back = parse_cb(&write_cb(&doc)).unwrap();
        assert_eq!(back.layers[0].entities, doc.layers[0].entities);
        assert_eq!(back.layers[0].ids, vec![Some(7)]);
    }

    #[test]
    fn rejects_other_xml() {
        assert!(matches!(parse_cb("<foo/>"), Err(ParseError::MissingDocumentRoot)));
        assert!(matches!(parse_cb("<CADFile>"), Err(ParseError::Xml(_))));
    }
}
