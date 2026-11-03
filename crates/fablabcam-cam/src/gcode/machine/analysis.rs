use std::collections::BTreeSet;
use super::{checks, motion, parse, state::State, Diagnostic, Profile, Report, Severity, Statistics, Status};

pub fn validate(source: &str, profile: &Profile) -> Report {
    let mut report = Report { schema_version: 1, analyzer_version: env!("CARGO_PKG_VERSION").into(), profile_id: profile.id.clone(), status: Status::Valid, limits_checked: false, diagnostics: Vec::new(), statistics: Statistics { complete: true, ..Default::default() } };
    if let Err(e) = profile.check() {
        report.error("profile",1,1,None,e); report.status = Status::Invalid; report.statistics.complete = false; return report;
    }
    configuration(profile,&mut report);
    let parsed = parse(source);
    report.diagnostics.extend(parsed.diagnostics);
    let mut state = State::new(profile);
    let mut seen = BTreeSet::new();
    let mut preamble_checked = false;
    let mut end_seen = false;
    for b in &parsed.blocks {
        if b.uncertain { state.poisoned = true; report.statistics.complete = false; }
        if state.ended && !b.words.is_empty() {
            report.partial("after_end", b.line, 1, None, "Bloc après la fin du programme, non interprété"); continue;
        }
        let diagnostics_before = report.diagnostics.len();
        checks::block(b,profile,&mut report,&mut state);
        state.modes(b,profile);
        checks::spindle(b,profile,&mut report,&state);
        for w in &b.words { seen.insert(w.code()); }
        if !preamble_checked && b.words.iter().any(|w| "XYZABCIJKR".contains(w.letter)) {
            preamble(profile,&seen,b.line,&mut report); preamble_checked = true;
        }
        if report.diagnostics[diagnostics_before..].iter().any(|d| d.severity == Severity::Error) {
            state.poisoned = true;
        }
        motion::execute(b,profile,&mut report,&mut state);
        if b.has("M2") || b.has("M30") {
            state.ended = true; end_seen = profile.format.end_codes.iter().any(|c| b.has(c));
        }
    }
    if !preamble_checked { preamble(profile,&seen,1,&mut report); }
    if !end_seen { report.error("end_code", parsed.blocks.last().map_or(1,|b| b.line),1,None,format!("Terminaison requise : {}",profile.format.end_codes.join(" ou "))); }
    if let Some(header) = &profile.format.header {
        if source.lines().find(|l| !l.trim().is_empty()).map(str::trim) != Some(header.as_str()) { report.error("header",1,1,None,format!("En-tête requis : {header}")); }
    }
    if ![report.statistics.cutting_length_mm,report.statistics.rapid_length_mm,report.statistics.estimated_seconds].iter().all(|v| v.is_finite()) {
        report.error("numeric_range",1,1,None,"Statistiques hors capacité numérique ; valeurs réinitialisées");
        report.statistics.cutting_length_mm = 0.; report.statistics.rapid_length_mm = 0.; report.statistics.estimated_seconds = 0.;
        report.statistics.complete = false;
    }
    report.statistics.tools.sort_unstable(); report.statistics.tools.dedup();
    report.statistics.complete &= !state.poisoned;
    report.status = if report.diagnostics.iter().any(|d| d.severity == Severity::Error) { Status::Invalid }
        else if !report.statistics.complete || !report.limits_checked { Status::Incomplete } else { Status::Valid };
    report.diagnostics.push(Diagnostic::new(Severity::Info,"scope",1,1,None,"Analyse statique : aucun contrôle de collision ni exécution de macro. Durée à vitesse constante, hors accélérations et latences."));
    report
}
fn preamble(p: &Profile, seen: &BTreeSet<String>, line: usize, r: &mut Report) {
    for code in &p.safety.required_preamble {
        if !seen.contains(code) { r.error("preamble",line,1,Some(code.clone()),format!("{code} requis avant le premier mouvement")); }
    }
}
fn configuration(p: &Profile, r: &mut Report) {
    r.limits_checked = ['X','Y','Z'].iter().all(|c| p.axes.get(c).is_some_and(|a| a.min.is_some() && a.max.is_some() && a.max_feed.is_some())) && p.max_feed_mm_min.is_some() && p.spindle.min_rpm.is_some() && p.spindle.max_rpm.is_some();
    if !r.limits_checked { r.partial("unconfigured_limits",1,1,None,"Configurer courses XYZ, avances maximales et limites de broche pour une analyse complète"); }
}
