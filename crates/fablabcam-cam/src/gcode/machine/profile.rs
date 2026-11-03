use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Point = [f64; 3];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub location: Option<String>,
    pub machine_kind: String,
    /// Selects the meaning of codes; controller names alone never define semantics.
    pub semantics: DialectSemantics,
    pub controller: Controller,
    pub sources: Vec<String>,
    pub axes: BTreeMap<char, Axis>,
    pub max_feed_mm_min: Option<f64>,
    pub rapid_mm_min: Option<f64>,
    pub acceleration_mm_s2: Option<f64>,
    /// Machine coordinates in mm. None means the initial position is unknown.
    pub initial_position_mm: Option<Point>,
    pub positive_directions: String,
    /// Work origin relative to machine origin; keys are G54 through G59.
    pub work_offsets_mm: BTreeMap<String, Point>,
    pub spindle: Spindle,
    pub defaults: Defaults,
    pub gcodes: Codes,
    pub mcodes: Codes,
    pub format: Format,
    pub safety: Safety,
    pub tools: Tools,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DialectSemantics { Mach3Mill, Rs274Mill }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Controller {
    pub name: String,
    pub version: Option<String>,
    pub dialect: String,
    pub postprocessor: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Axis {
    pub kind: AxisKind,
    /// Linear limits are mm, rotary limits are degrees.
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub max_feed: Option<f64>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AxisKind { Linear, Rotary }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Spindle {
    pub min_rpm: Option<f64>,
    pub max_rpm: Option<f64>,
    pub directions: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    pub units: String,
    pub plane: String,
    pub distance: String,
    pub arc_distance: String,
    pub feed_mode: String,
    pub work_offset: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Codes {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}
impl Codes {
    pub fn permits(&self, code: &str) -> bool {
        self.allow.iter().any(|s| s == code) && !self.deny.iter().any(|s| s == code)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Format {
    pub line_numbers: LineNumbers,
    pub max_decimals: usize,
    pub comments: Vec<String>,
    pub ascii_only: bool,
    pub uppercase: bool,
    pub max_line_length: usize,
    pub header: Option<String>,
    pub end_codes: Vec<String>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LineNumbers { Optional, Required, Forbidden }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Safety {
    pub safe_z_mm: Option<f64>,
    pub park_position_mm: Option<Point>,
    pub tool_change_position_mm: Option<Point>,
    pub required_preamble: Vec<String>,
    pub allow_helical_arcs: bool,
    pub forbid_rapid_below_safe_z: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Tools {
    pub changer: String,
    pub positions: Option<u32>,
    pub m6_behavior: String,
    /// Set only after verifying M6 preserves position, offsets, modes, feed and spindle state.
    pub m6_preserves_state: bool,
    pub coolant: Vec<String>,
    pub table: BTreeMap<u32, Tool>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub diameter_mm: f64,
    pub length_mm: f64,
    pub kind: String,
}
impl Profile {
    pub fn mach3_mill() -> Self {
        serde_json::from_str(include_str!("../../../../../profiles/mach3-mill.json"))
            .expect("bundled Mach3 profile must be valid JSON")
    }
    pub fn from_json(source: &str) -> Result<Self, String> {
        let profile: Self = serde_json::from_str(source).map_err(|e| e.to_string())?;
        profile.check()?;
        Ok(profile)
    }
    pub fn to_json(&self) -> Result<String, String> {
        self.check()?;
        serde_json::to_string_pretty(self).map(|s| s + "\n").map_err(|e| e.to_string())
    }
}
