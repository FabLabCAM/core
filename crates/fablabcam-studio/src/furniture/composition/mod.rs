//! A partition tree owns each shared panel exactly once. IDs survive resizing.
mod edit;
mod resolve;
mod validate;

use super::{new_id, Dimensions, JointSettings};
pub use resolve::{structure, CellBounds, Structure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Horizontal,
    Vertical,
}
impl Axis {
    pub fn coordinate(self) -> usize {
        if self == Self::Horizontal {
            2
        } else {
            0
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PositionMode {
    Equal,
    Fixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Divider {
    pub id: String,
    pub name: String,
    /// Lower edge measured from the containing cell's lower/left inner edge.
    pub offset: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Division {
    pub axis: Axis,
    pub mode: PositionMode,
    pub dividers: Vec<Divider>,
    pub children: Vec<Cell>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Bin {
    pub id: String,
    pub name: String,
    pub depth: f64,
    pub thickness: f64,
    /// Clearance on EACH side, independent of the total joint fit clearance.
    pub side_gap: f64,
    pub top_gap: f64,
    pub back_gap: f64,
}
impl Default for Bin {
    fn default() -> Self {
        Self {
            id: new_id(),
            name: "Bac amovible".into(),
            depth: 200.0,
            thickness: 12.0,
            side_gap: 2.0,
            top_gap: 3.0,
            back_gap: 2.0,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    pub id: String,
    pub name: String,
    pub division: Option<Division>,
    pub bin: Option<Bin>,
}
impl Cell {
    pub fn empty(name: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            name: name.into(),
            division: None,
            bin: None,
        }
    }
    pub fn find(&self, id: &str) -> Option<&Self> {
        if self.id == id {
            return Some(self);
        }
        self.division
            .as_ref()?
            .children
            .iter()
            .find_map(|c| c.find(id))
    }
    pub fn find_mut(&mut self, id: &str) -> Option<&mut Self> {
        if self.id == id {
            return Some(self);
        }
        self.division
            .as_mut()?
            .children
            .iter_mut()
            .find_map(|c| c.find_mut(id))
    }
    pub fn descendants(&self) -> usize {
        self.division.as_ref().map_or(0, |d| {
            d.dividers.len()
                + d.children
                    .iter()
                    .map(|c| 1 + c.descendants())
                    .sum::<usize>()
        }) + usize::from(self.bin.is_some())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Composition {
    pub root: Cell,
    pub joint_default: JointSettings,
    pub joints: BTreeMap<String, JointSettings>,
    /// Optional user names for shell panels, identified by stable legacy IDs.
    pub panel_names: BTreeMap<String, String>,
}
impl Composition {
    pub fn from_dimensions(d: &Dimensions) -> Self {
        let mut root = Cell::empty("Intérieur du meuble");
        if d.shelves > 0 {
            root.divide(Axis::Horizontal, d.shelves as usize + 1)
                .expect("bounded legacy shelves");
            for (i, divider) in root
                .division
                .as_mut()
                .unwrap()
                .dividers
                .iter_mut()
                .enumerate()
            {
                divider.id = format!("T{:02}", i + 1);
            }
        }
        Self {
            root,
            joint_default: JointSettings::default(),
            joints: BTreeMap::new(),
            panel_names: BTreeMap::new(),
        }
    }
    pub fn grid(columns: usize, rows: usize) -> Result<Self, String> {
        let mut root = Cell::empty("Grille de rangement");
        root.divide(Axis::Horizontal, rows)?;
        if let Some(d) = &mut root.division {
            for c in &mut d.children {
                c.divide(Axis::Vertical, columns)?;
            }
        } else {
            root.divide(Axis::Vertical, columns)?;
        }
        Ok(Self {
            root,
            joint_default: JointSettings::default(),
            joints: BTreeMap::new(),
            panel_names: BTreeMap::new(),
        })
    }
    pub fn check(&self) -> Result<(), String> {
        validate::check(self)
    }
}
