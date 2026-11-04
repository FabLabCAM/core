use fablabcam_studio::furniture::*;
fn grid() -> Dimensions {
    Dimensions {
        width: 1000.0,
        depth: 350.0,
        height: 1200.0,
        thickness: 18.0,
        shelves: 0,
        composition: Some(Box::new(Composition::grid(5, 6).unwrap())),
        ..Dimensions::default()
    }
}
#[test]
fn thirty_independent_openings_and_unique_panels() {
    let d = grid();
    let s = structure(&d).unwrap();
    let leaves = s.cells.iter().filter(|c| c.leaf).collect::<Vec<_>>();
    assert_eq!(leaves.len(), 30);
    for c in &leaves {
        assert!((c.size[0] - 178.4).abs() < 1e-9);
        assert!((c.size[2] - 179.0).abs() < 1e-9);
    }
    assert_eq!(s.panels.len(), 34); // 5 shell + 5 continuous shelves + 24 dividers.
    for (i, a) in s.panels.iter().enumerate() {
        for b in &s.panels[i + 1..] {
            assert_ne!(a.id, b.id);
            assert!(
                (0..3).any(|k| a.origin[k] + a.size[k] <= b.origin[k] + 1e-8
                    || b.origin[k] + b.size[k] <= a.origin[k] + 1e-8),
                "overlap {} {}",
                a.id,
                b.id
            );
        }
    }
}
#[test]
fn legacy_conversion_keeps_coordinates_and_labels() {
    let t = Template::cabinet();
    let mut d = t.defaults.clone();
    let original = panels(&t, &d).unwrap();
    d.composition = Some(Box::new(Composition::from_dimensions(&d).unwrap()));
    assert_eq!(original, panels(&t, &d).unwrap());
}
#[test]
fn fixed_and_equal_resize_behave_differently() {
    let mut d = grid();
    let length = d.height - 2.0 * d.thickness;
    let division = d
        .composition
        .as_mut()
        .unwrap()
        .root
        .division
        .as_mut()
        .unwrap();
    division.fix_positions(length, d.thickness);
    let first = division.dividers[0].id.clone();
    let z = structure(&d)
        .unwrap()
        .panels
        .iter()
        .find(|p| p.id == first)
        .unwrap()
        .origin[2];
    d.height += 100.0;
    assert_eq!(
        structure(&d)
            .unwrap()
            .panels
            .iter()
            .find(|p| p.id == first)
            .unwrap()
            .origin[2],
        z
    );
    d.composition
        .as_mut()
        .unwrap()
        .root
        .division
        .as_mut()
        .unwrap()
        .mode = PositionMode::Equal;
    assert!(
        structure(&d)
            .unwrap()
            .panels
            .iter()
            .find(|p| p.id == first)
            .unwrap()
            .origin[2]
            > z
    );
    d.height = 100.0;
    assert!(structure(&d).is_err());
}
#[test]
fn bins_have_clearance_and_five_distinct_parts() {
    let mut d = grid();
    let id = structure(&d)
        .unwrap()
        .cells
        .iter()
        .find(|c| c.leaf)
        .unwrap()
        .id
        .clone();
    let bin = Bin::default();
    let bid = bin.id.clone();
    d.composition
        .as_mut()
        .unwrap()
        .root
        .find_mut(&id)
        .unwrap()
        .bin = Some(bin);
    let s = structure(&d).unwrap();
    assert_eq!(s.panels.len(), 39);
    let left = s
        .panels
        .iter()
        .find(|p| p.id == format!("{bid}-L"))
        .unwrap();
    assert_eq!(left.size, [12.0, 200.0, 176.0]);
    d.composition
        .as_mut()
        .unwrap()
        .root
        .find_mut(&id)
        .unwrap()
        .bin
        .as_mut()
        .unwrap()
        .depth = 400.0;
    assert!(structure(&d).unwrap_err().contains("bac trop grand"));
}
#[test]
fn native_templates_projects_history_and_invalid_references() {
    let mut t = Template::cabinet();
    t.defaults = grid();
    let original = Document::Furniture(Box::new(Furniture::from_template(t)));
    assert_eq!(
        Document::from_json(&original.to_json().unwrap()).unwrap(),
        original
    );
    let mut draft = Draft::existing(original.clone());
    draft
        .document
        .dimensions_mut()
        .composition
        .as_mut()
        .unwrap()
        .root
        .name = "Rangement labo".into();
    draft.record(original.clone());
    assert!(draft.dirty());
    draft.undo();
    assert_eq!(draft.document, original);
    draft.redo();
    assert!(draft.dirty());
    let c = draft
        .document
        .dimensions_mut()
        .composition
        .as_mut()
        .unwrap();
    c.root.division.as_mut().unwrap().children[0].id = c.root.id.clone();
    assert!(draft.document.check().unwrap_err().contains("double"));
}
