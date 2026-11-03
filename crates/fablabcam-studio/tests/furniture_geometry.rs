use fablabcam_studio::furniture::*;
use std::collections::HashSet;

fn job() -> Furniture {
    Furniture::from_template(Template::cabinet())
}

#[test]
fn reference_cabinet_has_six_exact_parts_on_three_sheets() {
    let f = job();
    let p = panels(&f.template, &f.dimensions).unwrap();
    assert_eq!(p.len(), 6);
    assert_eq!(
        p.iter()
            .filter(|p| (p.width, p.height) == (800.0, 580.0))
            .count(),
        2
    );
    assert_eq!(
        p.iter()
            .filter(|p| (p.width, p.height) == (560.0, 580.0))
            .count(),
        3
    );
    let back = p.iter().find(|p| p.id == "DOS").unwrap();
    assert_eq!((back.width, back.height), (600.0, 800.0));
    for p in &p {
        for i in 0..3 {
            assert!(p.origin[i] >= 0.0);
            assert!(p.origin[i] + p.size[i] <= [600.0, 600.0, 800.0][i]);
        }
    }
    let layout = place(&f).unwrap();
    assert_eq!(layout.sheets.len(), 3);
    assert_eq!(
        layout
            .sheets
            .iter()
            .map(|s| s.placements.len())
            .collect::<Vec<_>>(),
        vec![2, 2, 2]
    );
    verify_layout(&f, &layout);
}

fn verify_layout(f: &Furniture, layout: &Layout) {
    let mut ids = HashSet::new();
    for s in &layout.sheets {
        let mut area = 0.0;
        for (i, p) in s.placements.iter().enumerate() {
            assert!(ids.insert(p.panel.id.clone()));
            assert!(p.x >= f.setup.margin && p.y >= f.setup.margin);
            assert!(p.x + p.width <= f.setup.sheet_size[0] - f.setup.margin + 1e-8);
            assert!(p.y + p.height <= f.setup.sheet_size[1] - f.setup.margin + 1e-8);
            let expected = if p.rotated {
                (p.panel.height, p.panel.width)
            } else {
                (p.panel.width, p.panel.height)
            };
            assert_eq!((p.width, p.height), expected);
            for other in s.placements.iter().skip(i + 1) {
                let gap = f.setup.spacing - 1e-8;
                assert!(
                    p.x + p.width + gap <= other.x
                        || other.x + other.width + gap <= p.x
                        || p.y + p.height + gap <= other.y
                        || other.y + other.height + gap <= p.y
                );
            }
            area += p.width * p.height;
        }
        assert!(
            (s.occupied_ratio - area / (f.setup.sheet_size[0] * f.setup.sheet_size[1])).abs()
                < 1e-12
        );
    }
    assert_eq!(ids.len(), panels(&f.template, &f.dimensions).unwrap().len());
}

#[test]
fn variations_preserve_dimensions_and_unique_nonoverlapping_placements() {
    for width in [300.0, 600.0, 700.0, 900.0] {
        for thickness in [12.0, 20.0, 30.0] {
            for back in [false, true] {
                for shelves in [0, 1, 4] {
                    let mut f = job();
                    f.dimensions = Dimensions {
                        width,
                        thickness,
                        back,
                        shelves,
                        ..Dimensions::default()
                    };
                    let p = panels(&f.template, &f.dimensions).unwrap();
                    assert_eq!(
                        p.iter().find(|p| p.id == "SUP").unwrap().width,
                        width - 2.0 * thickness
                    );
                    assert_eq!(
                        p.iter().find(|p| p.id == "C1").unwrap().height,
                        600.0 - if back { thickness } else { 0.0 }
                    );
                    assert_eq!(p.len(), 4 + shelves as usize + usize::from(back));
                    if width == 900.0 && back {
                        assert!(place(&f).is_err());
                    } else {
                        verify_layout(&f, &place(&f).unwrap());
                    }
                }
            }
        }
    }
}

#[test]
fn rejects_impossible_geometry_nonfinite_settings_and_unplaceable_parts() {
    let mut f = job();
    f.dimensions.width = 30.0;
    assert!(f.check().is_err());
    f = job();
    f.dimensions.height = 1600.0;
    assert!(place(&f).unwrap_err().contains("non plaçable"));
    f = job();
    f.setup.sheet_size[0] = 1600.0;
    assert!(place(&f).unwrap_err().contains("course"));
    for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        f = job();
        f.setup.cutter = value;
        assert!(place(&f).is_err());
    }
    f = job();
    f.setup.margin = 5.0;
    assert!(place(&f).is_err());
    f = job();
    f.setup.spacing = 5.0;
    assert!(place(&f).is_err());
    f = job();
    f.template.limits.width.min = 700.0;
    assert!(f.check().is_err());
    f = job();
    f.dimensions.shelves = 100;
    assert!(f.check().is_err());
}

#[test]
fn rotations_and_configured_machine_limits_are_enforced() {
    let mut f = job();
    f.setup.rotate = false;
    assert!(place(&f).is_err()); // 600 × 800 back cannot fit the 1480 × 780 rectangle without rotation.
    f.setup.rotate = true;
    assert!(place(&f).is_ok());
    let axis = f.setup.machine.axes.get_mut(&'X').unwrap();
    axis.min = Some(0.0);
    axis.max = Some(1200.0);
    assert!(place(&f).unwrap_err().contains("limites du profil"));
}
