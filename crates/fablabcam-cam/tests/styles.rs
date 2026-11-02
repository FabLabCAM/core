//! Styles synthétiques FabLabCAM : héritage, surcharges et paramètres imbriqués.

use fablabcam_cam::{LeadKind, Params, StyleLibrary, TabMethod, TabStyle};

fn lib() -> StyleLibrary {
    StyleLibrary::parse(include_str!("../../../tests/fixtures/styles-demo.xml")).unwrap()
}

#[test]
fn reads_all_demo_styles() {
    assert_eq!(lib().names().count(), 7);
}

#[test]
fn base_style_defaults() {
    let s = lib().resolve("").unwrap();
    assert_eq!(s.f64("CutFeedrate"), Some(800.0));
    assert_eq!(s.f64("PlungeFeedrate"), Some(300.0));
    assert_eq!(s.f64("DepthIncrement"), Some(0.4));
    assert_eq!(s.get("SpindleDirection"), Some("CW"));
    // `state="Auto"` n'est pas une valeur définie
    assert_eq!(s.get("ToolNumber"), None);
}

#[test]
fn inheritance_chain_is_resolved() {
    let l = lib();
    let s = l.resolve("cutout+tabs+corners").unwrap();
    assert_eq!(s.get("CornerOvercut"), Some("True"));
    // hérité de la racine via cutout+tabs → cutout → ""
    assert_eq!(s.f64("CutFeedrate"), Some(800.0));
}

#[test]
fn child_overrides_parent() {
    let s = lib().resolve("engrave").unwrap();
    assert_eq!(s.f64("TargetDepth"), Some(-0.3));
    assert_eq!(s.get("InsideOutside"), Some("Inside"));
}

#[test]
fn params_from_tool_style() {
    let s = lib().resolve("demo-aluminium").unwrap();
    let p = Params::from_style(&s);
    assert_eq!(p.tool_diameter, 4.0);
    assert_eq!(p.cut_feedrate, 400.0); // surcharge le 800 du style de base
    assert_eq!(p.spindle_speed, 21000.0);
    assert_eq!(p.stock_surface, 10.0);
    assert_eq!(p.clearance_plane, 40.0);
}

#[test]
fn unknown_style_is_none() {
    assert!(lib().resolve("n'existe pas").is_none());
}

#[test]
fn holding_tabs_are_read_from_nested_properties() {
    let p = Params::from_style(&lib().resolve("cutout+tabs").unwrap());
    assert_eq!(p.tabs.method, TabMethod::Automatic);
    assert_eq!((p.tabs.width, p.tabs.height), (1.5, 1.5));
    assert_eq!((p.tabs.min, p.tabs.max), (3, 6));
    assert_eq!((p.tabs.distance, p.tabs.size_threshold), (30.0, 15.0));
    assert_eq!(p.tabs.style, TabStyle::Triangle);
    // le style de base n'a pas de tabs
    assert_eq!(Params::from_style(&lib().resolve("").unwrap()).tabs.method, TabMethod::None);
}

#[test]
fn lead_moves_are_read_from_nested_properties() {
    let l = lib();
    let spiral = Params::from_style(&l.resolve("cutout").unwrap());
    assert_eq!(spiral.lead_in.kind, LeadKind::Spiral);
    assert_eq!(spiral.lead_out.kind, LeadKind::None);
    let with_tabs = Params::from_style(&l.resolve("cutout+tabs+corners").unwrap());
    assert_eq!(with_tabs.lead_in.kind, LeadKind::Spiral); // hérité deux niveaux plus haut
    assert!(l.names().any(|n| Params::from_style(&l.resolve(n).unwrap()).lead_out.kind == LeadKind::Tangent));
}
