use super::*;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversion {
    pub output: Option<String>,
    pub source_report: Report,
    pub target_report: Option<Report>,
    pub unsupported: Vec<Diagnostic>,
}
/// Convert only semantics-preserving lexical differences. Unsupported commands
/// are reported; no output is returned for partially interpreted input.
pub fn convert(source: &str, from: &Profile, to: &Profile) -> Conversion {
    let source_report = validate(source,from);
    let mut out = Conversion { output: None, source_report, target_report: None, unsupported: Vec::new() };
    if let Err(e) = to.check() { out.unsupported.push(Diagnostic::new(Severity::Error,"target_profile",1,1,None,e)); return out; }
    if out.source_report.status != Status::Valid {
        out.unsupported.extend(out.source_report.diagnostics.iter().filter(|d| d.severity != Severity::Info).cloned()); return out;
    }
    // No implicit coordinate transformation: both snapshots describe the same frame.
    if from.initial_position_mm != to.initial_position_mm || from.work_offsets_mm != to.work_offsets_mm {
        out.unsupported.push(Diagnostic::new(Severity::Error,"coordinate_transform",1,1,None,"Positions initiales et origines doivent correspondre ; transformation de repère non prise en charge")); return out;
    }
    let parsed = parse(source);
    let mut lines = Vec::new();
    if let Some(h) = &to.format.header { lines.push(h.clone()); }
    // Make all initial modal assumptions explicit, independent of target defaults.
    let d = &from.defaults;
    let modes = [&d.units,&d.plane,&d.distance,&d.arc_distance,&d.feed_mode,&d.work_offset];
    let mut number = 1;
    lines.push(numbered(modes.into_iter().cloned().collect::<Vec<_>>().join(" "),to,&mut number));
    for b in &parsed.blocks {
        let mut words = Vec::new();
        for w in &b.words {
            if w.letter == 'N' { continue; }
            let code = match w.code().as_str() { "G70" => "G20".into(), "G71" => "G21".into(), _ => w.code() };
            let text = if "GMT".contains(w.letter) { code } else {
                let mut n = format!("{:.*}",to.format.max_decimals,w.value);
                if n.contains('.') { n = n.trim_end_matches('0').trim_end_matches('.').into(); }
                if n.parse::<f64>().ok() != Some(w.value) {
                    out.unsupported.push(Diagnostic::new(Severity::Error,"precision_loss",b.line,w.column,Some(w.text.clone()),"La précision cible modifierait une valeur"));
                }
                format!("{}{n}",w.letter)
            };
            words.push(text);
        }
        if !words.is_empty() { lines.push(numbered(words.join(" "),to,&mut number)); }
        for (style,text) in &b.comments {
            if style == "%" && text.trim() == "%" { continue; }
            let body = if style == "()" { &text[1..text.len()-1] } else { &text[style.len()..] };
            // MSG comments are controller behavior, not merely annotation.
            if body.trim().to_ascii_uppercase().starts_with("MSG,") {
                out.unsupported.push(Diagnostic::new(Severity::Error,"operator_message",b.line,1,None,"Message opérateur dépendant du dialecte")); continue;
            }
            if to.format.comments.iter().any(|s| s == "()") && !body.contains(['(',')']) { lines.push(format!("({body})")); }
            else if to.format.comments.iter().any(|s| s == ";") { lines.push(format!(";{body}")); }
            else if to.format.comments.iter().any(|s| s == "//") { lines.push(format!("//{body}")); }
            else { out.unsupported.push(Diagnostic::new(Severity::Error,"comment_conversion",b.line,1,None,"Commentaire impossible à conserver dans le format cible")); }
        }
    }
    let output = lines.join("\n")+"\n";
    let target = validate(&output,to);
    if target.status != Status::Valid { out.unsupported.extend(target.diagnostics.iter().filter(|d| d.severity != Severity::Info).cloned()); }
    if out.unsupported.is_empty() { out.output = Some(output); }
    out.target_report = Some(target);
    out
}
fn numbered(line: String, p: &Profile, n: &mut usize) -> String {
    if p.format.line_numbers != LineNumbers::Required { return line; }
    let line = format!("N{n} {line}"); *n += 1; line
}
