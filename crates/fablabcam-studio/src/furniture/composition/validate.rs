use super::*;
use crate::furniture::data::{identity, text};
use std::collections::HashSet;
pub(super) fn check(c: &Composition) -> Result<(), String> {
    let mut ids = HashSet::new();
    visit(&c.root, 0, &mut ids)?;
    c.joint_default.check()?;
    if c.joints.len() > 4096 {
        return Err("Trop de liaisons (maximum 4096)".into());
    }
    for (id, settings) in &c.joints {
        text(id, "Liaison")?;
        settings.check()?;
    }
    for (id, name) in &c.panel_names {
        identity(id, name)?;
    }
    for (id, t) in &c.panel_thickness {
        if !["C1", "C2", "SUP", "INF", "DOS"].contains(&id.as_str()) {
            return Err(format!("Épaisseur d’un panneau inconnu : {id}"));
        }
        thickness(*t)?;
    }
    Ok(())
}
fn thickness(t: f64) -> Result<(), String> {
    if !t.is_finite() || !(3.0..=200.0).contains(&t) {
        return Err("Épaisseur de panneau hors limites : 3 à 200 mm".into());
    }
    Ok(())
}
fn insert(id: &str, name: &str, ids: &mut HashSet<String>) -> Result<(), String> {
    identity(id, name)?;
    if !ids.insert(id.into()) {
        return Err(format!("Identifiant d’élément en double : {id}"));
    }
    if ids.len() > 1024 {
        return Err("Composition trop grande (1024 éléments maximum)".into());
    }
    Ok(())
}
fn visit(c: &Cell, depth: usize, ids: &mut HashSet<String>) -> Result<(), String> {
    if depth > 24 {
        return Err("Composition trop profonde (24 niveaux maximum)".into());
    }
    insert(&c.id, &c.name, ids)?;
    if c.bin.is_some() && c.division.is_some() {
        return Err(format!("{} : bac et division incompatibles", c.name));
    }
    if let Some(b) = &c.bin {
        insert(&b.id, &b.name, ids)?;
        for v in [b.depth, b.thickness, b.side_gap, b.top_gap, b.back_gap] {
            if !v.is_finite() || !(0.0..=100_000.0).contains(&v) {
                return Err(format!("{} : dimensions/jeux invalides", b.name));
            }
        }
        if b.thickness < 3.0 || b.depth <= 2.0 * b.thickness {
            return Err(format!("{} : épaisseur/profondeur impossible", b.name));
        }
    }
    if let Some(d) = &c.division {
        if d.children.len() != d.dividers.len() + 1 || d.dividers.len() > 100 {
            return Err(format!("{} : division incohérente", c.name));
        }
        for p in &d.dividers {
            insert(&p.id, &p.name, ids)?;
            if let Some(t) = p.thickness {
                thickness(t)?;
            }
            if p.depth
                .is_some_and(|v| !v.is_finite() || !(0.01..=100_000.0).contains(&v))
            {
                return Err(format!("{} : profondeur invalide", p.name));
            }
            if !p.offset.is_finite() || p.offset < 0.0 {
                return Err(format!("{} : position invalide", p.name));
            }
        }
        for child in &d.children {
            visit(child, depth + 1, ids)?;
        }
    }
    Ok(())
}
