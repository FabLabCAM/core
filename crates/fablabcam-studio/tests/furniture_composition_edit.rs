use fablabcam_studio::furniture::*;
fn document() -> Document {
    let mut f = Furniture::from_template(Template::cabinet());
    f.dimensions.composition = Some(Box::new(
        Composition::from_dimensions(&f.dimensions).unwrap(),
    ));
    Document::Furniture(Box::new(f))
}
#[test]
fn corner_orientation_changes_partners_preserving_outer_and_inner_dimensions() {
    let mut doc = document();
    let before = doc.clone();
    let scene = structure(doc.dimensions()).unwrap();
    let link = connections(&scene.panels)
        .into_iter()
        .find(|l| l.male == "SUP" && l.female == "C1")
        .unwrap();
    let c = doc.dimensions_mut().composition.as_mut().unwrap();
    c.joints.insert(
        link.id,
        JointSettings {
            kind: JointKind::Tabs,
            ..Default::default()
        },
    );
    c.top_covers_sides = true;
    c.bottom_covers_sides = true;
    doc.reconcile_edit(&before);
    let after = structure(doc.dimensions()).unwrap();
    assert_eq!(scene.cells[0].origin, after.cells[0].origin);
    assert_eq!(scene.cells[0].size, after.cells[0].size);
    let links = connections(&after.panels);
    let changed = links
        .iter()
        .find(|l| l.male == "C1" && l.female == "SUP")
        .unwrap();
    assert_eq!(
        doc.dimensions().composition.as_ref().unwrap().joints[&changed.id].kind,
        JointKind::Tabs
    );
    assert_eq!(
        after.panels.iter().find(|p| p.id == "SUP").unwrap().size[0],
        600.0
    );
    assert_eq!(
        after.panels.iter().find(|p| p.id == "C1").unwrap().size[2],
        760.0
    );
    assert_eq!(Document::from_json(&doc.to_json().unwrap()).unwrap(), doc);
}
#[test]
fn individual_thickness_recalculates_shared_spaces_and_mixed_stock() {
    let mut doc = document();
    let c = doc.dimensions_mut().composition.as_mut().unwrap();
    c.panel_thickness.insert("C1".into(), 24.0);
    let divider = &mut c.root.division.as_mut().unwrap().dividers[0];
    divider.thickness = Some(12.0);
    divider.depth = Some(200.0);
    let id = divider.id.clone();
    let s = structure(doc.dimensions()).unwrap();
    assert_eq!(s.cells[0].size[0], 556.0);
    assert_eq!(s.cells[1].size[2], 374.0);
    let p = s.panels.iter().find(|p| p.id == id).unwrap();
    assert_eq!(p.thickness, 12.0);
    assert_eq!(p.size[1], 200.0);
    doc.check().unwrap();
}
#[test]
fn removal_of_back_cleans_only_related_overrides_on_explicit_edit() {
    let mut doc = document();
    let scene = structure(doc.dimensions()).unwrap();
    for l in connections(&scene.panels) {
        doc.dimensions_mut()
            .composition
            .as_mut()
            .unwrap()
            .joints
            .insert(l.id, JointSettings::default());
    }
    let previous = doc.clone();
    doc.dimensions_mut().back = false;
    assert!(doc.check().is_err()); // Imported invalid references must not be silently discarded.
    doc.reconcile_edit(&previous);
    doc.check().unwrap();
    let c = doc.dimensions().composition.as_ref().unwrap();
    assert!(!c.joints.is_empty());
    assert!(c.joints.keys().all(|id| !id.contains("DOS")));
}
#[test]
fn copy_bin_preserves_custom_joint_settings_with_new_panel_ids() {
    let mut doc = document();
    let d = doc.dimensions_mut();
    let c = d.composition.as_mut().unwrap();
    let children = &mut c.root.division.as_mut().unwrap().children;
    let source = children[0].id.clone();
    let target = children[1].id.clone();
    let b = Bin {
        bottom_covers_sides: true,
        ..Default::default()
    };
    let bid = b.id.clone();
    children[0].bin = Some(b);
    let links = connections(&structure(d).unwrap().panels);
    let link = links.iter().find(|l| l.male.starts_with(&bid)).unwrap();
    let value = JointSettings {
        kind: JointKind::Tabs,
        count: 2,
        width: 15.0,
        ..Default::default()
    };
    d.composition
        .as_mut()
        .unwrap()
        .joints
        .insert(link.id.clone(), value.clone());
    let dimensions = d.clone();
    let c = d.composition.as_mut().unwrap();
    let clipboard = CellClipboard::capture(&dimensions, c, &source).unwrap();
    clipboard.paste(&dimensions, c, &target, 6.0).unwrap();
    assert_eq!(c.joints.len(), 2);
    assert!(c.joints.values().all(|v| v == &value));
    assert_ne!(
        c.root.find(&source).unwrap().bin.as_ref().unwrap().id,
        c.root.find(&target).unwrap().bin.as_ref().unwrap().id
    );
    doc.check().unwrap();
}
#[test]
fn invalid_legacy_count_returns_error_without_panic() {
    let d = Dimensions {
        shelves: 101,
        ..Default::default()
    };
    assert!(Composition::from_dimensions(&d).is_err());
    assert!(Composition::grid(32, 32).is_err());
}

#[test]
fn bin_repeat_rejects_tight_joinery_atomically() {
    let mut d = Dimensions {
        width: 600.0,
        composition: Some(Box::new(Composition::grid(2, 1).unwrap())),
        ..Default::default()
    };
    let c = d.composition.as_mut().unwrap();
    let division = c.root.division.as_mut().unwrap();
    division.mode = PositionMode::Fixed;
    division.dividers[0].offset = 470.0; // right opening is only 70 mm
    let source = division.children[0].id.clone();
    let target = division.children[1].id.clone();
    let bin = Bin::default();
    let bid = bin.id.clone();
    division.children[0].bin = Some(bin);
    let links = connections(&structure(&d).unwrap().panels);
    let link = links
        .iter()
        .find(|l| l.male == format!("{bid}-B") && l.female == format!("{bid}-F"))
        .unwrap();
    d.composition.as_mut().unwrap().joints.insert(
        link.id.clone(),
        JointSettings {
            kind: JointKind::Tabs,
            ..Default::default()
        },
    );
    let dimensions = d.clone();
    let c = d.composition.as_mut().unwrap();
    let before = c.clone();
    let clipboard = CellClipboard::capture(&dimensions, c, &source).unwrap();
    assert!(clipboard.paste(&dimensions, c, &target, 6.0).is_err());
    assert_eq!(*c, before);
}

#[test]
fn history_separates_numeric_field_edit_from_following_button_action() {
    let original = document();
    let mut draft = Draft::existing(original.clone());
    draft.document.dimensions_mut().width = 700.0;
    draft.record_group_scoped(original.clone(), true, Some(1));
    let numeric = draft.document.clone();
    draft
        .document
        .dimensions_mut()
        .composition
        .as_mut()
        .unwrap()
        .top_covers_sides = true;
    draft.record_group_scoped(numeric.clone(), true, Some(2));
    draft.undo();
    assert_eq!(draft.document, numeric);
    draft.undo();
    assert_eq!(draft.document, original);
    draft.redo();
    assert_eq!(draft.document, numeric);
}
