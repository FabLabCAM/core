//! Copy element IDs and their joint overrides together; validate before committing.
use super::*;
use crate::furniture::{connections, Connection};

#[derive(Clone)]
pub struct CellClipboard {
    cell: Cell,
    joints: Vec<(Connection, JointSettings)>,
}
impl CellClipboard {
    pub fn capture(d: &Dimensions, c: &Composition, id: &str) -> Result<Self, String> {
        let cell = c.root.find(id).ok_or("Case inconnue")?.clone();
        let mut source = d.clone();
        source.composition = Some(Box::new(c.clone()));
        let joints = connections(&structure(&source)?.panels)
            .into_iter()
            .filter_map(|l| c.joints.get(&l.id).map(|s| (l.clone(), s.clone())))
            .collect();
        Ok(Self { cell, joints })
    }
    pub fn paste(
        &self,
        d: &Dimensions,
        c: &mut Composition,
        target: &str,
        cutter: f64,
    ) -> Result<(), String> {
        let mut next = c.clone();
        let cell = next.root.find_mut(target).ok_or("Case inconnue")?;
        if cell.bin.is_some() || cell.division.is_some() {
            return Err("Sélectionnez une case vide".into());
        }
        let ids = cell.copy_contents_from(&self.cell);
        let mut dimensions = d.clone();
        dimensions.composition = Some(Box::new(next.clone()));
        let links = connections(&structure(&dimensions)?.panels);
        for (old, settings) in &self.joints {
            let male = ids.get(&old.male);
            let female = ids.get(&old.female);
            // A copied boundary joint follows its new surrounding panel. Internal
            // joints preserve both mapped endpoints. Outside-only joints stay outside.
            let found = links.iter().find(|l| match (male, female) {
                (Some(m), Some(f)) => &l.male == m && &l.female == f,
                (Some(m), None) => &l.male == m && l.axis == old.axis && l.high_end == old.high_end,
                (None, Some(f)) => {
                    &l.female == f && l.axis == old.axis && l.female_face == old.female_face
                }
                _ => false,
            });
            if let Some(link) = found {
                next.joints.insert(link.id.clone(), settings.clone());
            }
        }
        dimensions.composition = Some(Box::new(next.clone()));
        crate::furniture::machining_preview(structure(&dimensions)?.panels, &next, cutter)?;
        *c = next;
        Ok(())
    }
}
