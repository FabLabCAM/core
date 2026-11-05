use fablabcam_cam::*;

fn tool(index: usize, d: f64) -> ToolInfo {
    ToolInfo { index, name: format!("T{index}"), diameter: d, comment: String::new(), change: String::new() }
}

fn mop<'a>(name: &str, tool: ToolInfo, path: &'a Toolpath, params: &'a Params) -> Mop<'a> {
    Mop { name: name.into(), header: String::new(), footer: String::new(), tool, path, params }
}

fn prog(mops: Vec<Mop<'_>>) -> Program<'_> {
    Program { name: "piece".into(), date: "2026-01-02".into(), file_header: String::new(), file_footer: String::new(), inches: false, mops }
}

fn lines(o: &PostOutput) -> Vec<&str> {
    o.gcode.lines().collect()
}

fn square_path() -> Toolpath {
    Toolpath {
        moves: vec![
            Move::Rapid { x: Some(0.0), y: Some(0.0), z: None },
            Move::Rapid { x: None, y: None, z: Some(0.0) },
            Move::Feed { x: None, y: None, z: Some(-1.0), f: 300.0 },
            Move::Feed { x: Some(10.0), y: Some(0.0), z: None, f: 800.0 },
            Move::Feed { x: Some(10.0), y: Some(10.0), z: None, f: 800.0 },
            Move::Feed { x: Some(10.0), y: Some(20.0), z: None, f: 800.0 },
            Move::Rapid { x: None, y: None, z: Some(3.0) },
        ],
    }
}

#[test]
fn default_definition_produces_a_complete_program() {
    let (path, p) = (square_path(), Params::default());
    let out = render_post(&PostDef::default(), &prog(vec![mop("Découpe", tool(1, 6.0), &path, &p)]));
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    assert_eq!(
        lines(&out),
        [
            "( piece )", "( T1 : 6 )", "G21 G90 G17 G94 G40 G49 G80 G91.1", "G0 Z3", "( T1 : 6 )", "T1 M6", "( Découpe )", "M3 S1000", "G0 X0 Y0", "G0 Z0", "G1 F300 Z-1", "F800 X10", "Y10", "Y20",
            "G0 Z3", "M5", "M30"
        ]
    );
}

#[test]
fn modal_words_are_written_once_and_plain_ones_always() {
    let (path, p) = (square_path(), Params::default());
    let mut d = PostDef { feed_move: "{$g1} {$_f} {$_x} {$_y} {$_z}".into(), ..PostDef::default() }; // G1 non modal
    let out = render_post(&d, &prog(vec![mop("a", tool(1, 3.0), &path, &p)]));
    let l = lines(&out);
    assert!(l.contains(&"G1 Y10") && l.contains(&"G1 Y20"), "{l:?}");
    // X10 et F800 ne sont pas répétés
    assert_eq!(out.gcode.matches("X10").count(), 1);
    assert_eq!(out.gcode.matches("F800").count(), 1);
    // sans souligner : toujours écrits
    d.feed_move = "{$g1} {$f} {$x} {$y} {$z}".into();
    let out = render_post(&d, &prog(vec![mop("a", tool(1, 3.0), &path, &p)]));
    assert_eq!(out.gcode.matches("F800").count(), 3);
}

#[test]
fn custom_file_structure_with_user_macros_and_pipes_of_a_cbpp() {
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<PostProcessor Version="0.9.8.0">
  <PostFile>%
O{$o}
{$comment} MON POST {$endcomment}
{$header}
{$mops}
{$footer}
%</PostFile>
  <Header>{$units} {$distancemode}</Header>
  <Footer>{$spindle(off)}
{$endrewind}</Footer>
  <MOP>{$comment} {$mop.name} {$endcomment}
{$blocks}</MOP>
  <Rapid>{$g0} {$_x} {$_y} {$_z}</Rapid>
  <Comment>;</Comment>
  <EndComment></EndComment>
  <EndOfLine>\r\n</EndOfLine>
  <NumberFormat>0.00</NumberFormat>
  <UnitsMetric>G71</UnitsMetric>
  <InvertArcs>true</InvertArcs>
  <PostProcessorMacros>$o=1234
$stock=5</PostProcessorMacros>
  <Truc>x</Truc>
</PostProcessor>"#;
    let (d, ignored) = PostDef::parse(xml).unwrap();
    assert_eq!(ignored, vec!["InvertArcs", "Truc"]);
    let (path, p) = (square_path(), Params::default());
    let out = render_post(&d, &prog(vec![mop("Coupe", tool(1, 6.0), &path, &p)]));
    assert!(out.gcode.starts_with("%\r\nO1234\r\n; MON POST\r\nG71 G90\r\n; Coupe\r\nG0 X0.00 Y0.00\r\nG0 Z0.00\r\n"), "{:?}", out.gcode);
    assert!(out.gcode.ends_with("M5\r\nM30\r\n%\r\n"), "{:?}", out.gcode);
}

#[test]
fn bad_definitions_are_rejected() {
    assert!(PostDef::parse("<Autre/>").unwrap_err().contains("PostProcessor"));
    assert!(PostDef::parse("<PostProcessor>").unwrap_err().contains("XML"));
}

#[test]
fn tool_change_only_when_the_tool_changes() {
    let (path, p) = (square_path(), Params::default());
    let out = render_post(
        &PostDef::default(),
        &prog(vec![mop("a", tool(1, 6.0), &path, &p), mop("b", tool(1, 6.0), &path, &p), mop("c", tool(2, 3.0), &path, &p)]),
    );
    let changes: Vec<&str> = lines(&out).into_iter().filter(|l| l.ends_with("M6")).collect();
    assert_eq!(changes, ["T1 M6", "T2 M6"], "{}", out.gcode);
    // table d'outils d'en-tête : un élément par outil distinct
    assert_eq!(lines(&out).iter().filter(|l| **l == "( T1 : 6 )").count(), 2); // table + changement
    assert_eq!(lines(&out).iter().filter(|l| **l == "( T2 : 3 )").count(), 2);
}

#[test]
fn arcs_follow_the_arc_center_mode_and_limits() {
    let arc = Toolpath { moves: vec![Move::Rapid { x: Some(10.0), y: Some(0.0), z: None }, Move::Arc { ccw: true, x: 0.0, y: 10.0, z: None, i: -10.0, j: 0.0, f: 500.0 }] };
    let p = Params::default();
    let run = |d: &PostDef| render_post(d, &prog(vec![mop("a", tool(1, 3.0), &arc, &p)])).gcode;
    let mut d = PostDef::default();
    assert!(run(&d).contains("G3 F500 X0 Y10 I-10 J0"), "{}", run(&d));
    d.arc_center = ArcCenter::Absolute;
    assert!(run(&d).contains("G3 F500 X0 Y10 I0 J0"), "{}", run(&d));
    d.arc_center = ArcCenter::IncrementalToStart;
    assert!(run(&d).contains("I10 J-0") || run(&d).contains("I10 J0"), "{}", run(&d));
    // rayon max : l'arc sort en ligne droite
    d.arc_center = ArcCenter::IncrementalFromStart;
    d.max_arc_radius = 5.0;
    let g = run(&d);
    assert!(g.contains("G1 F500 X0 Y10") && !g.contains("G3"), "{g}");
    // longueur min : idem pour un très petit arc
    d.max_arc_radius = 0.0;
    d.min_arc_length = 100.0;
    assert!(!run(&d).contains("G3"));
}

#[test]
fn drills_use_canned_cycles_with_peck() {
    let t = Toolpath {
        moves: vec![
            Move::Drill { x: 5.0, y: 5.0, z: -4.0, retract: 3.0, f: 100.0, peck: None, dwell: 0.0 },
            Move::Drill { x: 15.0, y: 5.0, z: -4.0, retract: 3.0, f: 100.0, peck: Some(1.0), dwell: 0.0 },
        ],
    };
    let p = Params::default();
    let g = render_post(&PostDef::default(), &prog(vec![mop("a", tool(1, 3.0), &t, &p)])).gcode;
    assert!(g.contains("G81 X5 Y5 Z-4 R3 F100"), "{g}");
    assert!(g.contains("G83 X15 Q1"), "{g}"); // Y, Z, R et F modaux
    // temporisation : G82 avec P
    let t = Toolpath { moves: vec![Move::Drill { x: 5.0, y: 5.0, z: -4.0, retract: 3.0, f: 100.0, peck: None, dwell: 0.5 }] };
    let g = render_post(&PostDef::default(), &prog(vec![mop("a", tool(1, 3.0), &t, &p)])).gcode;
    assert!(g.contains("G82 X5 Y5 Z-4 P0.5 R3 F100"), "{g}");
}

#[test]
fn spindle_direction_header_footer_and_unknown_macros() {
    let (path, p) = (square_path(), Params { spindle_cw: false, spindle_speed: 18000.0, ..Params::default() });
    let d = PostDef { mop: "{$mop.header}\n{$spindle} {$s}\n{$truc}\n{$blocks}\n{$mop.footer}".into(), ..PostDef::default() };
    let mut m = mop("a", tool(1, 3.0), &path, &p);
    m.header = "M8".into();
    m.footer = "M9".into();
    let out = render_post(&d, &prog(vec![m]));
    let l = lines(&out);
    let at = l.iter().position(|x| *x == "M8").unwrap();
    assert_eq!(&l[at..at + 2], ["M8", "M4 S18000"]);
    assert!(l.contains(&"M9"));
    assert_eq!(out.warnings, vec!["macro inconnue : {$truc}"]);
}

#[test]
fn recursive_templates_do_not_loop() {
    let (path, p) = (square_path(), Params::default());
    let d = PostDef { header: "{$header}".into(), ..PostDef::default() };
    let out = render_post(&d, &prog(vec![mop("a", tool(1, 3.0), &path, &p)]));
    assert!(out.warnings.iter().any(|w| w.contains("imbriqués")), "{:?}", out.warnings);
}

// ───────────── numérotation de lignes ─────────────

fn numbered(ln: LineNumbering) -> PostDef {
    PostDef { line_numbers: LineNumbering { enabled: true, ..ln }, ..PostDef::default() }
}

#[test]
fn line_numbers_skip_comments_and_follow_increment() {
    let (path, p) = (square_path(), Params::default());
    let run = |d: &PostDef| render_post(d, &prog(vec![mop("a", tool(1, 3.0), &path, &p)])).gcode;
    let g = run(&numbered(LineNumbering::default()));
    let l: Vec<&str> = g.lines().collect();
    // commentaires non numérotés et sans consommer de numéro
    assert_eq!(l[0], "( piece )");
    assert_eq!(l[2], "N10 G21 G90 G17 G94 G40 G49 G80 G91.1");
    assert_eq!(l[3], "N20 G0 Z3");
    assert!(l.last().unwrap().starts_with('N') && l.last().unwrap().ends_with("M30"));
    // numéros strictement croissants de 10 en 10
    let nums: Vec<i64> = l.iter().filter_map(|x| x.strip_prefix('N')?.split(' ').next()?.parse().ok()).collect();
    assert!(nums.windows(2).all(|w| w[1] - w[0] == 10), "{nums:?}");
    // désactivé : aucun numéro
    assert!(!run(&PostDef::default()).contains("\nN"));
}

#[test]
fn line_number_format_prefix_start_and_space() {
    let (path, p) = (square_path(), Params::default());
    let run = |ln: LineNumbering| render_post(&numbered(ln), &prog(vec![mop("a", tool(1, 3.0), &path, &p)])).gcode;
    let g = run(LineNumbering { format: "0000".into(), start: 5, increment: 1, ..LineNumbering::default() });
    assert!(g.contains("\nN0005 G21 G90 G17 G94 G40 G49 G80 G91.1\nN0006 G0 Z3\n"), "{g}");
    let g = run(LineNumbering { format: "####".into(), start: 98, increment: 1, ..LineNumbering::default() });
    assert!(g.contains("\nN  98 G21") && g.contains("\nN  99 G0") && g.contains("\nN 100 "), "{g}");
    let g = run(LineNumbering { format: "###0".into(), start: 3, space_after: false, prefix: "L".into(), skip: "(".into(), ..LineNumbering::default() });
    assert!(g.contains("\nL   3G21 G90 G17"), "{g}"); // # = espace de remplissage
    // % non listé dans skip : numéroté
    let d = numbered(LineNumbering { skip: "(".into(), ..LineNumbering::default() });
    let xml = "<PostProcessor><PostFile>%\n{$header}</PostFile><Header>G21</Header></PostProcessor>";
    let (mut parsed, _) = PostDef::parse(xml).unwrap();
    parsed.line_numbers = d.line_numbers;
    assert!(render_post(&parsed, &prog(vec![mop("a", tool(1, 3.0), &path, &p)])).gcode.starts_with("N10 %\nN20 G21\n"));
}

#[test]
fn line_numbers_and_arc_options_are_read_from_cbpp() {
    let xml = "<PostProcessor><AddLineNumbers>true</AddLineNumbers><LineNumberStart>100</LineNumberStart><LineNumberIncrement>5</LineNumberIncrement>\
        <LineNumberPrefix>N</LineNumberPrefix><LineNumberFormat>0000</LineNumberFormat><LineNumberSpaceAfter>false</LineNumberSpaceAfter>\
        <LineNumberSkip>(</LineNumberSkip><ArcOutput>HelixConvertToLines</ArcOutput><ArcToLinesTolerance>0.02</ArcToLinesTolerance></PostProcessor>";
    let (d, ignored) = PostDef::parse(xml).unwrap();
    assert!(ignored.is_empty(), "{ignored:?}");
    assert_eq!(d.line_numbers, LineNumbering { enabled: true, start: 100, increment: 5, prefix: "N".into(), format: "0000".into(), skip: "(".into(), space_after: false });
    assert_eq!((d.arc_output, d.arc_tolerance), (ArcOutput::HelixConvertToLines, 0.02));
}

// ───────────── conversion d'arcs en segments ─────────────

/// Points XY des lignes `G1` / modales d'un programme (état modal reconstruit).
fn xy_points(g: &str) -> Vec<(f64, f64)> {
    let (mut x, mut y, mut out) = (0.0, 0.0, Vec::new());
    for l in g.lines() {
        let mut moved = false;
        for w in l.split(' ') {
            if let Some(v) = w.strip_prefix('X').and_then(|v| v.parse().ok()) {
                x = v;
                moved = true;
            }
            if let Some(v) = w.strip_prefix('Y').and_then(|v| v.parse().ok()) {
                y = v;
                moved = true;
            }
        }
        if moved && !l.starts_with("G0") {
            out.push((x, y));
        }
    }
    out
}

fn quarter_arc() -> Toolpath {
    Toolpath { moves: vec![Move::Rapid { x: Some(10.0), y: Some(0.0), z: None }, Move::Arc { ccw: true, x: 0.0, y: 10.0, z: None, i: -10.0, j: 0.0, f: 500.0 }] }
}

#[test]
fn arcs_converted_to_lines_stay_within_the_tolerance() {
    let (path, p) = (quarter_arc(), Params::default());
    for tol in [0.1, 0.01, 0.001] {
        let d = PostDef { arc_output: ArcOutput::ConvertToLines, arc_tolerance: tol, ..PostDef::default() };
        let g = render_post(&d, &prog(vec![mop("a", tool(1, 3.0), &path, &p)])).gcode;
        assert!(!g.contains("G3") && !g.contains(" I"), "{g}");
        let pts = xy_points(&g);
        assert!(pts.len() >= 3, "{tol}: {pts:?}");
        // sommets sur le cercle, arrivée exacte, écart des cordes ≤ tolérance
        assert!(pts.iter().all(|(x, y)| (x.hypot(*y) - 10.0).abs() < 2e-3), "{pts:?}");
        assert_eq!(*pts.last().unwrap(), (0.0, 10.0));
        let mut prev = (10.0, 0.0);
        for &(x, y) in &pts {
            let mid = ((prev.0 + x) / 2.0, (prev.1 + y) / 2.0);
            assert!(10.0 - mid.0.hypot(mid.1) <= tol + 2e-3, "corde trop lâche à tol {tol}: {prev:?} → {:?}", (x, y));
            prev = (x, y);
        }
    }
    // plus la tolérance est fine, plus il y a de segments
    let count = |tol: f64| xy_points(&render_post(&PostDef { arc_output: ArcOutput::ConvertToLines, arc_tolerance: tol, ..PostDef::default() }, &prog(vec![mop("a", tool(1, 3.0), &path, &p)])).gcode).len();
    assert!(count(0.001) > count(0.1));
}

#[test]
fn clockwise_arc_and_full_circle_convert_correctly() {
    let p = Params::default();
    let d = PostDef { arc_output: ArcOutput::ConvertToLines, arc_tolerance: 0.01, ..PostDef::default() };
    // quart de tour horaire de (0,10) à (10,0)
    let cw = Toolpath { moves: vec![Move::Rapid { x: Some(0.0), y: Some(10.0), z: None }, Move::Arc { ccw: false, x: 10.0, y: 0.0, z: None, i: 0.0, j: -10.0, f: 500.0 }] };
    let pts = xy_points(&render_post(&d, &prog(vec![mop("a", tool(1, 3.0), &cw, &p)])).gcode);
    assert!(pts.iter().all(|(x, y)| (x.hypot(*y) - 10.0).abs() < 2e-3) && pts.iter().all(|(x, y)| *x >= -1e-6 && *y >= -1e-3), "{pts:?}");
    assert_eq!(*pts.last().unwrap(), (10.0, 0.0));
    // cercle complet : départ = arrivée, 360° balayés
    let full = Toolpath { moves: vec![Move::Rapid { x: Some(10.0), y: Some(0.0), z: None }, Move::Arc { ccw: true, x: 10.0, y: 0.0, z: None, i: -10.0, j: 0.0, f: 500.0 }] };
    let pts = xy_points(&render_post(&d, &prog(vec![mop("a", tool(1, 3.0), &full, &p)])).gcode);
    assert!(pts.len() > 20 && pts.iter().any(|(_, y)| *y < -9.9) && pts.iter().any(|(x, _)| *x < -9.9), "{} points", pts.len());
    assert_eq!(*pts.last().unwrap(), (10.0, 0.0));
}

#[test]
fn helix_mode_only_converts_arcs_with_varying_z() {
    let p = Params::default();
    let helix = Toolpath {
        moves: vec![
            Move::Rapid { x: Some(10.0), y: Some(0.0), z: Some(0.0) },
            Move::Arc { ccw: true, x: 0.0, y: 10.0, z: Some(-1.0), i: -10.0, j: 0.0, f: 500.0 }, // hélice
            Move::Arc { ccw: true, x: -10.0, y: 0.0, z: None, i: 0.0, j: -10.0, f: 500.0 },       // plan
        ],
    };
    let d = PostDef { arc_output: ArcOutput::HelixConvertToLines, arc_tolerance: 0.05, ..PostDef::default() };
    let g = render_post(&d, &prog(vec![mop("a", tool(1, 3.0), &helix, &p)])).gcode;
    assert_eq!(g.matches("G3").count(), 1, "{g}"); // seul l'arc plan reste
    // Z descend de façon monotone jusqu'à -1 pendant l'hélice
    let zs: Vec<f64> = g.lines().flat_map(|l| l.split(' ').filter_map(|w| w.strip_prefix('Z')?.parse().ok()).collect::<Vec<f64>>()).collect();
    let hel: Vec<f64> = zs.iter().copied().skip_while(|z| *z != 0.0).take_while(|z| *z <= 0.0).collect();
    assert!(hel.windows(2).all(|w| w[1] <= w[0] + 1e-9) && (hel.last().unwrap() + 1.0).abs() < 1e-9, "{zs:?}");
}
