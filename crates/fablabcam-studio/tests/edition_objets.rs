//! Modification d'objets existants : annulation, recalcul des opérations, enregistrement `.cb`.

use fablabcam_model::Entity;
use fablabcam_studio::edit;
use fablabcam_studio::persist;
use fablabcam_studio::{OpKind, Project, Studio, Tool, ToolKind};

fn projet_rectangle() -> (Studio, u32, u32) {
    let mut p = Project::new();
    p.add_tool(Tool::new("Fraise 6 mm", ToolKind::EndMill, 6.0)).unwrap();
    let id = p.add_shape("0", Entity::Rect { x: 0.0, y: 0.0, w: 40.0, h: 20.0, r: 0.0, rot_deg: 0.0 }).unwrap();
    let op = p.add_operation(OpKind::Profile, "Contour", vec![id], "Fraise 6 mm").unwrap();
    p.update_operation(op, |o| o.depth = -3.0).unwrap();
    (Studio::new(p), id, op)
}

fn rect_of(s: &Studio, id: u32) -> Entity {
    s.project().shape(id).unwrap().entity.clone()
}

#[test]
fn une_modification_un_pas_d_annulation() {
    let (mut s, id, _) = projet_rectangle();
    let avant = rect_of(&s, id);
    s.edit(|p| p.edit_shape(id, |e| edit::set_rect(e, 0.0, 0.0, 60.0, 20.0, 0.0, 0.0))).unwrap();
    let apres = rect_of(&s, id);
    assert!(matches!(apres, Entity::Rect { w, .. } if w == 60.0));
    assert!(s.undo());
    assert_eq!(rect_of(&s, id), avant);
    assert!(s.redo());
    assert_eq!(rect_of(&s, id), apres);
    assert!(s.undo() && !s.undo(), "un seul pas pour la modification");
}

#[test]
fn modification_refusee_ne_change_rien_et_ne_cree_pas_de_pas() {
    let (mut s, id, _) = projet_rectangle();
    let avant = rect_of(&s, id);
    assert!(s.edit(|p| p.edit_shape(id, |e| edit::set_rect(e, 0.0, 0.0, -1.0, 20.0, 0.0, 0.0))).is_err());
    assert_eq!(rect_of(&s, id), avant);
    assert!(s.edit(|p| p.edit_shape(999, |e| Ok(e.clone()))).is_err());
}

#[test]
fn rectangle_agrandi_le_gcode_du_profil_suit() {
    let (mut s, id, op) = projet_rectangle();
    s.recalc();
    let avant = s.gcode();
    s.edit(|p| p.edit_shape(id, |e| edit::set_rect(e, 0.0, 0.0, 80.0, 20.0, 0.0, 0.0))).unwrap();
    s.recalc();
    assert!(s.computed[&op].error.is_none(), "{:?}", s.computed[&op].error);
    let apres = s.gcode();
    assert_ne!(avant, apres);
    // le contour extérieur dépasse maintenant 80 mm en X (fraise de 6 mm : jusqu'à 83)
    assert!(apres.contains("X83"), "{apres}");
}

#[test]
fn coins_arrondis_changent_la_trajectoire() {
    let (mut s, id, op) = projet_rectangle();
    s.recalc();
    let vif = s.gcode();
    s.edit(|p| p.edit_shape(id, |e| edit::set_rect(e, 0.0, 0.0, 40.0, 20.0, 5.0, 0.0))).unwrap();
    s.recalc();
    assert!(s.computed[&op].error.is_none());
    assert_ne!(s.gcode(), vif);
}

#[test]
fn enregistrement_cb_conserve_le_rectangle_modifie() {
    let (mut s, id, op) = projet_rectangle();
    s.edit(|p| p.edit_shape(id, |e| edit::set_rect(e, 3.0, 4.0, 25.0, 12.0, 2.0, 15.0))).unwrap();
    let xml = persist::save(s.project());
    let (back, _) = persist::load_with_notes(&xml).unwrap();
    assert_eq!(back.shapes.len(), 1);
    assert_eq!(back.shapes[0].entity, Entity::Rect { x: 3.0, y: 4.0, w: 25.0, h: 12.0, r: 2.0, rot_deg: 15.0 });
    assert_eq!(back.ops.len(), 1);
    assert_eq!(back.op(op).map(|o| o.shapes.clone()), Some(vec![back.shapes[0].id]));
}

#[test]
fn vrai_cb_modifie_enregistre_et_rouvert() {
    let xml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../fablabcam-cli/tests/fixtures/reference/tirroire-haut.cb")).unwrap();
    let (p, _) = persist::load_with_notes(&xml).unwrap();
    let id = p.shapes[0].id;
    assert!(matches!(p.shapes[0].entity, Entity::Rect { w, h, .. } if w == 91.0 && h == 516.0), "{:?}", p.shapes[0].entity);
    let ops = p.ops.len();
    assert!(ops > 0, "le vrai fichier porte ses opérations");
    let mut s = Studio::new(p);
    s.edit(|p| p.edit_shape(id, |e| edit::set_rect(e, -50.0, -43.0, 100.0, 516.0, 0.0, 0.0))).unwrap();
    let (back, _) = persist::load_with_notes(&persist::save(s.project())).unwrap();
    assert!(matches!(back.shapes[0].entity, Entity::Rect { w, .. } if w == 100.0));
    assert_eq!(back.ops.len(), ops);
    let mut s2 = Studio::new(back);
    s2.recalc();
    assert!(s2.computed.values().all(|c| c.error.is_none()), "opérations valides après relecture");
}

#[test]
fn polyligne_rectangulaire_devient_modifiable() {
    let mut p = Project::new();
    let pl = fablabcam_studio::shape::polyline(&[(0.0, 0.0), (30.0, 0.0), (30.0, 10.0), (0.0, 10.0)], true).unwrap();
    let id = p.add_shape("0", pl).unwrap();
    let mut s = Studio::new(p);
    s.edit(|p| p.edit_shape(id, |e| edit::set_rect(e, 0.0, 0.0, 50.0, 10.0, 0.0, 0.0))).unwrap();
    assert!(matches!(rect_of(&s, id), Entity::Rect { w, .. } if w == 50.0));
}

#[test]
fn edition_de_polyligne_dans_le_projet() {
    let mut p = Project::new();
    let id = p.add_shape("0", fablabcam_studio::shape::polyline(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], false).unwrap()).unwrap();
    let mut s = Studio::new(p);
    s.edit(|p| p.edit_shape(id, |e| edit::insert_vertex(e, 0, 0.5))).unwrap();
    assert_eq!(edit::handles(&rect_of(&s, id)).len(), 4);
    s.edit(|p| p.edit_shape(id, |e| edit::set_closed(e, true))).unwrap();
    s.edit(|p| p.edit_shape(id, edit::reverse)).unwrap();
    assert!(s.undo() && s.undo());
    assert_eq!(edit::handles(&rect_of(&s, id)).len(), 4);
}

#[test]
fn rainure_et_trous_regulierement_repartis() {
    let mut p = Project::new();
    let ligne = p.add_shape("0", fablabcam_studio::shape::line((0.0, 0.0), (30.0, 0.0)).unwrap()).unwrap();
    let cercle = p.add_shape("0", fablabcam_studio::shape::circle((50.0, 50.0), 20.0).unwrap()).unwrap();
    let mut s = Studio::new(p);
    let rainure = s.edit(|p| p.open_offset_shapes(&[ligne], 3.0)).unwrap();
    assert!(fablabcam_studio::shape::is_closed(&s.project().shape(rainure[0]).unwrap().entity));
    let trous = s.edit(|p| p.divide_shapes(&[cercle], 8)).unwrap();
    assert_eq!(trous.len(), 8);
    assert!(s.edit(|p| p.divide_shapes(&[cercle], 0)).is_err());
    assert!(s.undo() && s.undo());
    assert_eq!(s.project().shapes.len(), 2, "annuler retire les ajouts");
}
