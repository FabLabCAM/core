use super::{new_id, Dimensions, Furniture, Template};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "document_type",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Document {
    Template(Template),
    Furniture(Box<Furniture>),
}
impl Document {
    pub fn check(&self) -> Result<(), String> {
        match self {
            Self::Template(t) => t.check(),
            Self::Furniture(f) => f.check(),
        }
    }
    pub fn id(&self) -> &str {
        match self {
            Self::Template(t) => &t.id,
            Self::Furniture(f) => &f.id,
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Self::Template(t) => &t.name,
            Self::Furniture(f) => &f.name,
        }
    }
    pub fn name_mut(&mut self) -> &mut String {
        match self {
            Self::Template(t) => &mut t.name,
            Self::Furniture(f) => &mut f.name,
        }
    }
    pub fn dimensions(&self) -> &Dimensions {
        match self {
            Self::Template(t) => &t.defaults,
            Self::Furniture(f) => &f.dimensions,
        }
    }
    pub fn dimensions_mut(&mut self) -> &mut Dimensions {
        match self {
            Self::Template(t) => &mut t.defaults,
            Self::Furniture(f) => &mut f.dimensions,
        }
    }
    pub fn template(&self) -> &Template {
        match self {
            Self::Template(t) => t,
            Self::Furniture(f) => &f.template,
        }
    }
    pub fn is_template(&self) -> bool {
        matches!(self, Self::Template(_))
    }
    pub fn extension(&self) -> &str {
        if self.is_template() {
            "fcm-template.json"
        } else {
            "fcm.json"
        }
    }
    pub fn duplicate(&self) -> Self {
        let mut doc = self.clone();
        match &mut doc {
            Self::Template(t) => {
                t.id = new_id();
                t.revision = 1;
            }
            Self::Furniture(f) => f.id = new_id(),
        }
        let mut name = self.name().to_owned();
        while name.len() > 188 {
            name.pop();
        }
        *doc.name_mut() = format!("{name} — copie");
        doc
    }
    /// Explicit save/export promotion; loading never writes back to the source.
    pub fn promote_schema(&mut self) {
        match self {
            Self::Template(t) => t.schema_version = super::SCHEMA_VERSION,
            Self::Furniture(f) => {
                f.schema_version = super::SCHEMA_VERSION;
                f.template.schema_version = super::SCHEMA_VERSION;
            }
        }
    }
    /// Explicit UI edits can remove a panel or change which panel is continuous.
    /// Keep compatible overrides by unordered panel pair; never clean an import silently.
    pub fn reconcile_edit(&mut self, previous: &Self) {
        if self.dimensions() == previous.dimensions() {
            return;
        }
        let Ok(next) = super::structure(self.dimensions()) else {
            return;
        };
        let links = super::connections(&next.panels);
        let old = super::structure(previous.dimensions())
            .map(|s| super::connections(&s.panels))
            .unwrap_or_default();
        let Some(c) = &mut self.dimensions_mut().composition else {
            return;
        };
        let overrides = std::mem::take(&mut c.joints);
        for (id, settings) in overrides {
            if links.iter().any(|l| l.id == id) {
                c.joints.insert(id, settings);
                continue;
            }
            if let Some(before) = old.iter().find(|l| l.id == id) {
                if let Some(after) = links.iter().find(|l| {
                    (l.male == before.male && l.female == before.female)
                        || (l.male == before.female && l.female == before.male)
                }) {
                    c.joints.insert(after.id.clone(), settings);
                }
            }
        }
        c.panel_names
            .retain(|id, _| id == "DOS" || next.panels.iter().any(|p| &p.id == id));
    }
    pub fn as_template(&self) -> Template {
        let mut t = self.template().clone();
        t.id = new_id();
        t.revision = 1;
        t.schema_version = super::SCHEMA_VERSION;
        let mut name = self.name().to_owned();
        while name.len() > "Modèle — ".len().abs_diff(200) {
            name.pop();
        }
        t.name = format!("Modèle — {name}");
        t.defaults = self.dimensions().clone();
        t
    }
    pub fn to_json(&self) -> Result<String, String> {
        self.check()?;
        let mut doc = self.clone();
        doc.promote_schema();
        serde_json::to_string_pretty(&doc)
            .map(|s| s + "\n")
            .map_err(|e| e.to_string())
    }
    pub fn from_json(source: &str) -> Result<Self, String> {
        if source.len() > 2_000_000 {
            return Err("Fichier natif trop volumineux (2 Mo maximum)".into());
        }
        let doc: Self =
            serde_json::from_str(source).map_err(|e| format!("Document meuble invalide : {e}"))?;
        doc.check()?;
        Ok(doc)
    }
}

/// The baseline survives undo/redo; a new/imported document remains dirty until saved.
pub struct Draft {
    pub document: Document,
    pub(super) baseline: Option<Document>,
    undo: Vec<Document>,
    redo: Vec<Document>,
    edit_baseline: Option<Document>,
    edit_key: Option<u64>,
}
impl Draft {
    pub fn new(document: Document) -> Self {
        Self {
            document,
            baseline: None,
            undo: Vec::new(),
            redo: Vec::new(),
            edit_baseline: None,
            edit_key: None,
        }
    }
    pub fn existing(document: Document) -> Self {
        Self {
            baseline: Some(document.clone()),
            ..Self::new(document)
        }
    }
    pub fn dirty(&self) -> bool {
        self.baseline.as_ref() != Some(&self.document)
    }
    pub fn record(&mut self, previous: Document) {
        if previous == self.document {
            return;
        }
        if self.undo.len() == 128 {
            self.undo.remove(0);
        }
        self.undo.push(previous);
        self.redo.clear();
    }
    /// Focus identifies a field/action; moving focus ends the preceding edit
    /// before the new action is added, even when a button retains keyboard focus.
    pub fn record_group_scoped(&mut self, previous: Document, editing: bool, key: Option<u64>) {
        if self.edit_key != key {
            if let Some(baseline) = self.edit_baseline.take() {
                if baseline != previous {
                    self.record(baseline);
                }
            }
        }
        self.edit_key = key;
        self.record_group(previous, editing);
    }
    pub fn record_group(&mut self, previous: Document, editing: bool) {
        if previous != self.document && self.edit_baseline.is_none() {
            self.edit_baseline = Some(previous);
        }
        if !editing {
            self.finish_edit();
        }
    }
    pub fn finish_edit(&mut self) {
        if let Some(previous) = self.edit_baseline.take() {
            self.record(previous);
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.edit_baseline.is_some()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo(&mut self) {
        self.finish_edit();
        if let Some(d) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.document, d));
        }
    }
    pub fn redo(&mut self) {
        self.finish_edit();
        if let Some(d) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.document, d));
        }
    }
}
