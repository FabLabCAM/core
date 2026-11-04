//! Corner orientation changes the continuous panel, preserving outer dimensions.
use super::*;
use crate::furniture::Panel;
pub(super) fn build(d: &Dimensions, c: &Composition) -> (Vec<Panel>, [f64; 3], [f64; 3]) {
    let thickness = |id: &str| c.panel_thickness.get(id).copied().unwrap_or(d.thickness);
    let (left, right, top, bottom, back) = (
        thickness("C1"),
        thickness("C2"),
        thickness("SUP"),
        thickness("INF"),
        thickness("DOS"),
    );
    let depth = d.depth - if d.back { back } else { 0.0 };
    let low = if c.bottom_covers_sides { bottom } else { 0.0 };
    let high = d.height - if c.top_covers_sides { top } else { 0.0 };
    let mut panels = vec![
        Panel::rectangular(
            "C1",
            "Côté gauche",
            [0.0, 0.0, low],
            [left, depth, high - low],
            0,
        ),
        Panel::rectangular(
            "C2",
            "Côté droit",
            [d.width - right, 0.0, low],
            [right, depth, high - low],
            0,
        ),
    ];
    for (id, name, z, t, covers) in [
        ("SUP", "Dessus", d.height - top, top, c.top_covers_sides),
        ("INF", "Dessous", 0.0, bottom, c.bottom_covers_sides),
    ] {
        let x = if covers { 0.0 } else { left };
        let w = if covers {
            d.width
        } else {
            d.width - left - right
        };
        panels.push(Panel::rectangular(id, name, [x, 0.0, z], [w, depth, t], 2));
    }
    if d.back {
        panels.push(Panel::rectangular(
            "DOS",
            "Dos rapporté",
            [0.0, depth, 0.0],
            [d.width, back, d.height],
            1,
        ));
    }
    (
        panels,
        [left, 0.0, bottom],
        [d.width - left - right, depth, d.height - top - bottom],
    )
}
