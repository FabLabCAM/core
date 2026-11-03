use super::Point;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity { Error, Warning, Info }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub line: usize,
    pub column: usize,
    pub word: Option<String>,
    pub message: String,
}
impl Diagnostic {
    pub(crate) fn new(severity: Severity, code: &str, line: usize, column: usize, word: Option<String>, message: impl Into<String>) -> Self {
        Self { severity, code: code.into(), line, column, word, message: message.into() }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status { Valid, Invalid, Incomplete }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Statistics {
    pub bounds_min_mm: Option<Point>,
    pub bounds_max_mm: Option<Point>,
    pub cutting_length_mm: f64,
    pub rapid_length_mm: f64,
    /// Constant speed estimate: excludes acceleration, controller latency and operator stops.
    pub estimated_seconds: f64,
    pub tools: Vec<u32>,
    pub complete: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: u32,
    pub analyzer_version: String,
    pub profile_id: String,
    pub status: Status,
    pub limits_checked: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub statistics: Statistics,
}
impl Report {
    pub(crate) fn error(&mut self, code: &str, line: usize, column: usize, word: Option<String>, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::new(Severity::Error, code, line, column, word, message));
    }
    pub(crate) fn partial(&mut self, code: &str, line: usize, column: usize, word: Option<String>, message: impl Into<String>) {
        self.statistics.complete = false;
        self.diagnostics.push(Diagnostic::new(Severity::Warning, code, line, column, word, message));
    }
}
