//! Bibliothèque de styles CAM (`styles/*.xml`, schéma observé dans `Standard-mm.xml`).
//!
//! Un style contient des éléments `<Nom state="Value|Auto">valeur</Nom>` et peut hériter d'un
//! autre via `<Style>parent</Style>`. Seuls les éléments en `state="Value"` sont définis ; `Auto`
//! signifie « déduit par le logiciel de référence » et est ignoré ici.

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct CamStyle {
    pub name: String,
    pub parent: Option<String>,
    values: HashMap<String, String>,
}

#[derive(Debug, Default)]
pub struct StyleLibrary {
    styles: Vec<CamStyle>,
}

impl StyleLibrary {
    pub fn parse(xml: &str) -> Result<Self, roxmltree::Error> {
        let xml = xml.trim_start_matches('\u{feff}');
        let doc = roxmltree::Document::parse(xml)?;
        let mut styles = Vec::new();
        for s in doc.root_element().children().filter(|n| n.has_tag_name("CAMStyle")) {
            let mut st = CamStyle { name: s.attribute("Name").unwrap_or("").to_string(), ..Default::default() };
            for e in s.children().filter(|n| n.is_element()) {
                let tag = e.tag_name().name();
                if tag == "Style" {
                    st.parent = e.text().map(str::trim).filter(|t| !t.is_empty()).map(String::from);
                } else if e.attribute("state") == Some("Value") {
                    if e.has_children_elements() {
                        // propriété composite : « HoldingTabs.Width », « LeadInMove.TangentRadius »…
                        for c in e.children().filter(|c| c.is_element()) {
                            st.values.insert(format!("{tag}.{}", c.tag_name().name()), c.text().unwrap_or("").trim().to_string());
                        }
                    } else {
                        st.values.insert(tag.to_string(), e.text().unwrap_or("").trim().to_string());
                    }
                }
            }
            styles.push(st);
        }
        Ok(StyleLibrary { styles })
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.styles.iter().map(|s| s.name.as_str())
    }

    /// Style résolu : valeurs du parent (récursivement) surchargées par celles du style.
    pub fn resolve(&self, name: &str) -> Option<CamStyle> {
        let mut chain = Vec::new();
        let mut cur = self.styles.iter().find(|s| s.name == name)?;
        loop {
            if chain.len() > 32 {
                return None; // cycle d'héritage
            }
            chain.push(cur);
            match &cur.parent {
                Some(p) => cur = self.styles.iter().find(|s| &s.name == p)?,
                None => break,
            }
        }
        // `<Style />` vide = hérite du style de base nommé "" (inféré : c'est lui qui porte les valeurs par défaut)
        if chain.last().map(|s| !s.name.is_empty()).unwrap_or(false) {
            if let Some(base) = self.styles.iter().find(|s| s.name.is_empty()) {
                chain.push(base);
            }
        }
        let mut out = CamStyle { name: name.to_string(), ..Default::default() };
        for s in chain.iter().rev() {
            out.values.extend(s.values.clone());
        }
        Some(out)
    }
}

trait HasChildElements {
    fn has_children_elements(&self) -> bool;
}
impl HasChildElements for roxmltree::Node<'_, '_> {
    fn has_children_elements(&self) -> bool {
        self.children().any(|c| c.is_element())
    }
}

impl CamStyle {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn f64(&self, key: &str) -> Option<f64> {
        self.get(key)?.replace(',', ".").parse().ok()
    }

    pub fn set(&mut self, key: &str, value: impl ToString) {
        self.values.insert(key.to_string(), value.to_string());
    }
}
