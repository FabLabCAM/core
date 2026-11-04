use super::*;
impl Cell {
    pub fn divide(&mut self, axis: Axis, count: usize) -> Result<(), String> {
        if !(1..=101).contains(&count) {
            return Err("Une division contient de 1 à 101 cases".into());
        }
        if self.division.is_some() || self.bin.is_some() {
            return Err(
                "Sélectionnez une case vide ; ses éléments existants sont conservés".into(),
            );
        }
        if count == 1 {
            return Ok(());
        }
        self.division = Some(Division {
            axis,
            mode: PositionMode::Equal,
            dividers: (1..count)
                .map(|i| Divider {
                    id: new_id(),
                    name: format!(
                        "{} {i}",
                        if axis == Axis::Horizontal {
                            "Tablette"
                        } else {
                            "Séparation"
                        }
                    ),
                    offset: 0.0,
                })
                .collect(),
            children: (1..=count)
                .map(|i| Cell::empty(format!("Case {i}")))
                .collect(),
        });
        Ok(())
    }
    /// Caller displays the descendant count before invoking this destructive edit.
    pub fn clear(&mut self) {
        self.division = None;
        self.bin = None;
    }
    pub fn copy_contents_from(&mut self, source: &Self) {
        let mut copy = source.clone();
        copy.renew_ids();
        self.division = copy.division;
        self.bin = copy.bin;
    }
    fn renew_ids(&mut self) {
        self.id = new_id();
        if let Some(b) = &mut self.bin {
            b.id = new_id();
        }
        if let Some(d) = &mut self.division {
            for p in &mut d.dividers {
                p.id = new_id();
            }
            for c in &mut d.children {
                c.renew_ids();
            }
        }
    }
}
impl Division {
    pub fn offsets(&self, length: f64, thickness: f64) -> Vec<f64> {
        if self.mode == PositionMode::Fixed {
            return self.dividers.iter().map(|p| p.offset).collect();
        }
        let gap = (length - self.dividers.len() as f64 * thickness) / self.children.len() as f64;
        (1..=self.dividers.len())
            .map(|i| i as f64 * gap + (i - 1) as f64 * thickness)
            .collect()
    }
    pub fn fix_positions(&mut self, length: f64, thickness: f64) {
        let offsets = self.offsets(length, thickness);
        for (p, offset) in self.dividers.iter_mut().zip(offsets) {
            p.offset = offset;
        }
        self.mode = PositionMode::Fixed;
    }
    pub fn duplicate_divider(
        &mut self,
        index: usize,
        length: f64,
        thickness: f64,
    ) -> Result<(), String> {
        if self.dividers.len() >= 100 || index >= self.dividers.len() {
            return Err("Division pleine ou élément inconnu".into());
        }
        self.fix_positions(length, thickness);
        let left = self.dividers[index].offset + thickness;
        let right = self.dividers.get(index + 1).map_or(length, |p| p.offset);
        if right - left <= thickness {
            return Err("Pas assez de place pour dupliquer la séparation".into());
        }
        let mut p = self.dividers[index].clone();
        p.id = new_id();
        p.name = format!("{} copie", p.name);
        p.offset = (left + right - thickness) / 2.0;
        self.dividers.insert(index + 1, p);
        self.children
            .insert(index + 2, Cell::empty("Nouvelle case"));
        Ok(())
    }
    /// Merges two adjacent spaces. The UI confirms removal of their descendants.
    pub fn remove_divider(&mut self, index: usize) -> Result<(), String> {
        if index >= self.dividers.len() {
            return Err("Séparation inconnue".into());
        }
        self.dividers.remove(index);
        self.children[index].clear();
        self.children.remove(index + 1);
        Ok(())
    }
}
