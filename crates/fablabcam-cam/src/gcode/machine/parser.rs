use super::{Diagnostic, Severity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Word {
    pub letter: char,
    pub value: f64,
    pub text: String,
    pub column: usize,
}
impl Word {
    pub fn code(&self) -> String { format!("{}{}", self.letter, self.value) }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub line: usize,
    pub source: String,
    pub words: Vec<Word>,
    pub comments: Vec<(String, String)>,
    pub uncertain: bool,
}
impl Block {
    pub(crate) fn value(&self, c: char) -> Option<f64> { self.words.iter().find(|w| w.letter == c).map(|w| w.value) }
    pub(crate) fn has(&self, code: &str) -> bool { self.words.iter().any(|w| w.code() == code) }
    pub(crate) fn column(&self, c: char) -> usize { self.words.iter().find(|w| w.letter == c).map_or(1, |w| w.column) }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parsed {
    pub blocks: Vec<Block>,
    pub diagnostics: Vec<Diagnostic>,
}
/// Literal word parser, with recovery at each line. Never evaluates controller expressions.
pub fn parse(source: &str) -> Parsed {
    let mut result = Parsed { blocks: Vec::new(), diagnostics: Vec::new() };
    for (line, text) in source.lines().enumerate() {
        let mut b = Block { line: line + 1, source: text.into(), words: Vec::new(), comments: Vec::new(), uncertain: false };
        scan(&mut b, &mut result.diagnostics);
        result.blocks.push(b);
    }
    result
}
fn scan(b: &mut Block, diagnostics: &mut Vec<Diagnostic>) {
    let text = b.source.clone();
    let mut i = 0;
    while i < text.len() {
        let c = text[i..].chars().next().unwrap();
        if c.is_whitespace() { i += c.len_utf8(); continue; }
        let column = text[..i].chars().count() + 1;
        if c == ';' || text[i..].starts_with("//") || (c == '%' && text[..i].trim().is_empty()) {
            let style = if c == ';' { ";" } else if c == '%' { "%" } else { "//" };
            b.comments.push((style.into(), text[i..].into())); break;
        }
        if c == '(' {
            match text[i..].find(')') {
                Some(end) if !text[i+1..i+end].contains('(') => {
                    b.comments.push(("()".into(), text[i..=i+end].into())); i += end+1; continue;
                }
                _ => { diag(b, diagnostics, Severity::Error, "comment", column, "Commentaire imbriqué ou non terminé"); break; }
            }
        }
        if c == '/' || c == '#' || c == '[' || c == '=' {
            diag(b, diagnostics, Severity::Warning, "expression", column, "Bloc optionnel, paramètre ou expression non interprété"); break;
        }
        if !c.is_ascii_alphabetic() {
            diag(b, diagnostics, Severity::Error, "syntax", column, "Caractère inattendu"); i += c.len_utf8(); continue;
        }
        let start = i;
        i += 1;
        while text.as_bytes().get(i).is_some_and(u8::is_ascii_whitespace) { i += 1; }
        let number = i;
        if text.as_bytes().get(i).is_some_and(|v| b"+-".contains(v)) { i += 1; }
        while text.as_bytes().get(i).is_some_and(|v| v.is_ascii_digit() || *v == b'.') { i += 1; }
        let raw = &text[number..i];
        match raw.parse::<f64>() {
            Ok(value) if value.is_finite() => b.words.push(Word { letter: c.to_ascii_uppercase(), value, text: text[start..i].into(), column }),
            _ => diag(b, diagnostics, Severity::Error, "number", column, "Nombre littéral fini attendu après la lettre"),
        }
    }
}
fn diag(b: &mut Block, d: &mut Vec<Diagnostic>, s: Severity, code: &str, column: usize, message: &str) {
    b.uncertain = true;
    d.push(Diagnostic::new(s, code, b.line, column, None, message));
}
