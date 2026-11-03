use std::collections::BTreeSet;
use super::{state::{group, interpreted, State}, Block, LineNumbers, Profile, Report};

pub(crate) fn block(b: &Block, p: &Profile, r: &mut Report, s: &mut State) {
    format(b,p,r);
    let mut letters = BTreeSet::new();
    let mut groups = BTreeSet::new();
    for w in &b.words {
        let code = w.code();
        if w.letter != 'G' && w.letter != 'M' && !letters.insert(w.letter) {
            r.error("duplicate", b.line, w.column, Some(code.clone()), "Lettre répétée dans le bloc"); s.poisoned = true;
        }
        if let Some(g) = group(&code) {
            if !groups.insert(g) { r.error("modal_conflict", b.line, w.column, Some(code.clone()), "Deux codes du même groupe modal"); s.poisoned = true; }
        }
        if w.letter == 'G' || w.letter == 'M' {
            let list = if w.letter == 'G' { &p.gcodes } else { &p.mcodes };
            if !list.permits(&code) {
                r.error("unsupported_code", b.line, w.column, Some(code.clone()), "Code interdit ou absent de la liste autorisée"); s.poisoned = true;
            } else if !interpreted(&code) || (["G70","G71"].contains(&code.as_str()) && p.semantics != super::DialectSemantics::Mach3Mill) {
                r.partial("unmodeled_code", b.line, w.column, Some(code.clone()), "Code reconnu, sémantique non interprétée : statistiques partielles"); s.poisoned = true;
            }
        } else if "XYZABC".contains(w.letter) {
            if !p.axes.contains_key(&w.letter) { r.error("axis", b.line, w.column, Some(code.clone()), "Axe absent du profil"); s.poisoned = true; }
            if "ABC".contains(w.letter) { r.partial("rotary_axis", b.line, w.column, Some(code.clone()), "Cinématique rotative non interprétée"); s.poisoned = true; }
        } else if !"IJKRFSPTN".contains(w.letter) {
            r.partial("unmodeled_word", b.line, w.column, Some(code.clone()), "Mot non interprété"); s.poisoned = true;
        }
        if matches!(w.letter, 'N'|'T') && (w.value < 0. || w.value > 99999. || w.value.fract() != 0. || (w.letter == 'N' && w.text.chars().filter(char::is_ascii_digit).count() > 5)) {
            r.error("integer", b.line, w.column, Some(code.clone()), "Entier de 0 à 99999 attendu");
        }
        if w.letter == 'T' && (0. ..=99999.).contains(&w.value) && w.value.fract() == 0. {
            let t = w.value as u32;
            if p.tools.positions.is_some_and(|max| t > max) { r.error("tool", b.line, w.column, Some(code.clone()), "Outil hors du magasin"); }
            r.statistics.tools.push(t);
        }
        if w.letter == 'F' && w.value <= 0. { r.error("feed", b.line, w.column, Some(code), "Avance strictement positive requise"); }
        else if w.letter == 'S' && w.value < 0. { r.error("spindle", b.line, w.column, Some(code), "Vitesse de broche négative"); }
    }
    if b.words.iter().filter(|w| w.letter == 'M').count() > 4 { r.error("m_count", b.line, 1, None, "Au plus quatre M-codes par bloc"); }
    if (b.has("M9") && (b.has("M7") || b.has("M8"))) || b.words.iter().filter(|w| w.letter == 'M').map(|w| w.code()).collect::<BTreeSet<_>>().len() != b.words.iter().filter(|w| w.letter == 'M').count() {
        r.error("modal_conflict", b.line, 1, None, "M-codes répétés ou refroidissement contradictoire"); s.poisoned = true;
    }
    for c in ["M7","M8","M9"] {
        if b.has(c) && !p.tools.coolant.iter().any(|v| v == c) { r.error("coolant", b.line, 1, Some(c.into()), "Refroidissement non configuré"); }
    }
    if b.has("M6") && !p.tools.m6_preserves_position { r.partial("tool_macro", b.line, b.column('M'), Some("M6".into()), "La macro M6 peut déplacer les axes"); s.poisoned = true; }
    if b.has("M0") || b.has("M1") { r.partial("operator_stop", b.line, b.column('M'), None, "Durée de l'arrêt opérateur inconnue"); }
}
fn format(b: &Block, p: &Profile, r: &mut Report) {
    if b.source.chars().count() > p.format.max_line_length { r.error("line_length", b.line, 1, None, "Ligne trop longue"); }
    if p.format.ascii_only && !b.source.is_ascii() { r.error("charset", b.line, 1, None, "Le profil exige ASCII"); }
    for (style, _) in &b.comments {
        if !p.format.comments.contains(style) { r.error("comment_style", b.line, 1, None, format!("Commentaire {style} non autorisé")); }
    }
    let n = b.value('N');
    if (p.format.line_numbers == LineNumbers::Required && !b.words.is_empty() && n.is_none()) || (p.format.line_numbers == LineNumbers::Forbidden && n.is_some()) {
        r.error("line_number", b.line, 1, None, "Numérotation incompatible avec le profil");
    }
    if n.is_some() && b.words.first().is_some_and(|w| w.letter != 'N') { r.error("line_number", b.line, b.column('N'), None, "N doit précéder les autres mots"); }
    for w in &b.words {
        if w.text.split_once('.').is_some_and(|(_,tail)| tail.len() > p.format.max_decimals) { r.error("precision", b.line, w.column, Some(w.text.clone()), "Trop de décimales"); }
        if p.format.uppercase && w.text.chars().any(|c| c.is_ascii_lowercase()) { r.error("case", b.line, w.column, Some(w.text.clone()), "Majuscules requises"); }
    }
}
pub(crate) fn spindle(b: &Block, p: &Profile, r: &mut Report, s: &State) {
    if let Some(direction) = s.spindle {
        if !p.spindle.directions.iter().any(|v| v == direction) { r.error("spindle_direction", b.line, b.column('M'), None, "Sens de broche interdit"); }
        if s.speed <= 0. || p.spindle.min_rpm.is_some_and(|v| s.speed < v) || p.spindle.max_rpm.is_some_and(|v| s.speed > v) {
            r.error("spindle_speed", b.line, b.column('S'), None, format!("Broche hors limites : {} tr/min", s.speed));
        }
    } else if p.spindle.max_rpm.is_some_and(|v| s.speed > v) { r.error("spindle_speed", b.line, b.column('S'), None, "Consigne de broche supérieure au maximum"); }
}
