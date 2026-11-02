use fablabcam_cli::run;

fn fx(n: &str) -> String {
    format!("{}/tests/fixtures/{n}", env!("CARGO_MANIFEST_DIR"))
}

fn go(args: &[&str]) -> Result<String, String> {
    run(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
}

#[test]
fn info_lists_layers() {
    let o = go(&["info", &fx("plaque.cb")]).unwrap();
    assert!(o.contains("calque « Contour » : 1 polyligne(s)"));
    assert!(o.contains("calque « Trous » : 0 polyligne(s), 2 cercle(s)"));
}

#[test]
fn styles_command_lists_library() {
    let o = go(&["styles", concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/styles-demo.xml")]).unwrap();
    assert_eq!(o.lines().count(), 7);
    assert!(o.contains("cutout+tabs"));
}

#[test]
fn outside_profile_gcode() {
    let o = go(&["gcode", &fx("plaque.cb"), "--op", "profile-outside", "--layer", "Contour", "--tool", "6", "--depth", "-3", "--step", "1"]).unwrap();
    assert!(o.starts_with("(FabLabCAM)\nG21 G90 G17 G94"));
    assert_eq!(o.matches("G3 ").count() + o.matches("G2 ").count(), 12);
    assert!(o.trim_end().ends_with("M30"));
}

#[test]
fn drill_uses_circle_centers() {
    let o = go(&["gcode", &fx("plaque.cb"), "--op", "drill", "--layer", "Trous", "--depth", "-4", "--step", "2"]).unwrap();
    assert!(o.contains("G83 X10 Y10 Z-4 R3 Q2 F300"), "{o}");
    assert!(o.contains("G83 X90 Y40 Z-4"));
}

#[test]
fn style_drives_parameters() {
    let o = go(&["gcode", &fx("plaque.cb"), "--op", "profile-on", "--layer", "Contour", "--styles", concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/styles-demo.xml"), "--style", "demo-aluminium"]).unwrap();
    assert!(o.contains("M3 S21000"));
    assert!(o.contains("(Outil D4)"));
}

#[test]
fn writes_file_with_o() {
    let out = std::env::temp_dir().join("fablabcam_cli_test.nc");
    let msg = go(&["gcode", &fx("plaque.cb"), "--op", "pocket", "--layer", "Contour", "--tool", "6", "--depth", "-2", "-o", out.to_str().unwrap()]).unwrap();
    assert!(msg.contains("écrit"));
    assert!(std::fs::read_to_string(&out).unwrap().contains("(Poche)"));
}

#[test]
fn errors_are_reported() {
    assert!(go(&["gcode", &fx("plaque.cb")]).unwrap_err().contains("--op"));
    assert!(go(&["gcode", &fx("plaque.cb"), "--op", "zzz"]).unwrap_err().contains("inconnue"));
    assert!(go(&["gcode", &fx("plaque.cb"), "--op", "pocket", "--layer", "nope"]).unwrap_err().contains("introuvable"));
    assert!(go(&["gcode", &fx("plaque.cb"), "--op", "profile-inside", "--layer", "Trous", "--tool", "10"]).unwrap_err().contains("trop gros"));
    assert!(go(&["gcode", "inexistant.cb", "--op", "pocket"]).is_err());
    assert!(go(&["gcode", &fx("plaque.cb"), "--op", "pocket", "--tool", "abc"]).unwrap_err().contains("pas un nombre"));
    assert!(go(&["nimportequoi"]).unwrap_err().contains("commande inconnue"));
}

#[test]
fn pocket_with_island_keeps_the_tool_away_from_it() {
    let o = go(&["gcode", &fx("poche_ilot.cb"), "--op", "pocket", "--tool", "6", "--depth", "-1", "--step", "0"]).unwrap();
    assert!(o.contains("(Poche, 1 îlot)"), "{o}"); // les parenthèses du commentaire sont retirées par le post
    // aucune coordonnée de coupe à moins de 11 mm (8 + 3) du centre (40, 30)
    let (mut x, mut y) = (0.0f64, 0.0f64);
    let mut min = f64::INFINITY;
    for l in o.lines().filter(|l| l.starts_with('G') || l.starts_with('X') || l.starts_with('Y')) {
        for w in l.split_whitespace() {
            if let Some(v) = w.strip_prefix('X') { x = v.parse().unwrap_or(x) }
            if let Some(v) = w.strip_prefix('Y') { y = v.parse().unwrap_or(y) }
        }
        if l.starts_with("G1") || l.starts_with('X') || l.starts_with("G2") || l.starts_with("G3") {
            min = min.min((x - 40.0).hypot(y - 30.0));
        }
    }
    assert!(min >= 11.0 - 1e-3, "{min}");
}

#[test]
fn tabs_and_leads_from_the_command_line() {
    let o = go(&[
        "gcode", &fx("plaque.cb"), "--op", "profile-outside", "--layer", "Contour", "--tool", "6", "--depth", "-3", "--step", "1",
        "--tabs", "auto", "--tab-height", "1", "--tab-width", "4", "--tab-distance", "0", "--tab-style", "square",
        "--lead-in", "tangent", "--lead-out", "tangent", "--radius", "5",
    ])
    .unwrap();
    assert!(o.contains("Z-2")); // sommet des tabs
    assert!(o.matches("G3 ").count() + o.matches("G2 ").count() > 12); // arcs de coin + arcs d'approche/sortie
    assert!(go(&["gcode", &fx("plaque.cb"), "--op", "profile-on", "--tabs", "wat"]).unwrap_err().contains("--tabs"));
    assert!(go(&["gcode", &fx("plaque.cb"), "--op", "profile-on", "--lead-in", "wat"]).unwrap_err().contains("--lead-in"));
}

#[test]
fn style_tabs_and_leads_apply_without_flags() {
    let o = go(&["gcode", &fx("plaque.cb"), "--op", "profile-outside", "--layer", "Contour", "--styles", concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/styles-demo.xml"), "--style", "cutout+tabs", "--tool", "6", "--depth", "-3", "--step", "1"]).unwrap();
    // spirale du style « cutout » : arcs hélicoïdaux avec Z
    assert!(o.lines().any(|l| (l.starts_with("G3") || l.starts_with("G2")) && l.contains(" Z")), "{o}");
}

#[test]
fn milling_direction_and_spindle_options() {
    let run = |extra: &[&str]| {
        let mut a = vec!["gcode", "PLAQUE", "--op", "profile-outside", "--layer", "Contour", "--tool", "6", "--depth", "-1"];
        a.extend_from_slice(extra);
        let f = fx("plaque.cb");
        let a: Vec<&str> = a.iter().map(|x| if *x == "PLAQUE" { f.as_str() } else { *x }).collect();
        go(&a).unwrap()
    };
    // extérieur en avalant (broche horaire) : horaire → G2 ; en opposition : G3 (comme le logiciel de référence)
    assert!(run(&[]).contains("G2 ") && !run(&[]).contains("G3 "));
    assert!(run(&["--milling", "conventional"]).contains("G3 "));
    // broche anti-horaire : M4, et le sens s'inverse pour rester en avalant
    let ccw = run(&["--spindle", "ccw"]);
    assert!(ccw.contains("M4 S") && ccw.contains("G3 "));
    assert!(go(&["gcode", &fx("plaque.cb"), "--op", "profile-on", "--milling", "wat"]).unwrap_err().contains("--milling"));
    assert!(go(&["gcode", &fx("plaque.cb"), "--op", "profile-on", "--spindle", "wat"]).unwrap_err().contains("--spindle"));
}

#[test]
fn overcut_and_final_step_options() {
    let o = go(&["gcode", &fx("plaque.cb"), "--op", "profile-inside", "--layer", "Contour", "--tool", "6", "--depth", "-3", "--step", "1", "--final-step", "0.2", "--overcut"]).unwrap();
    assert!(o.contains("Z-2.8"), "{o}");
    // coin (0,0) : le sommet du trajet est à (3,3), la surcoupe va à (2.121, 2.121)
    assert!(o.contains("X2.121 Y2.121"), "{o}");
}

#[test]
fn finish_and_crossover_options() {
    let rough = go(&["gcode", &fx("plaque.cb"), "--op", "pocket", "--layer", "Contour", "--tool", "6", "--depth", "-1", "--step", "0", "--cut-order", "level", "--crossover", "0.7"]).unwrap();
    let apart = go(&["gcode", &fx("plaque.cb"), "--op", "pocket", "--layer", "Contour", "--tool", "6", "--depth", "-1", "--step", "0", "--cut-order", "level", "--crossover", "0"]).unwrap();
    let plunges = |g: &str| g.lines().filter(|l| l.starts_with("G1 Z-1")).count();
    assert_eq!(plunges(&rough), 1, "{rough}");
    assert!(plunges(&apart) > 3, "{apart}");
    let fin = go(&["gcode", &fx("plaque.cb"), "--op", "pocket", "--layer", "Contour", "--tool", "6", "--depth", "-1", "--step", "0", "--finish", "1", "--crossover", "0"]).unwrap();
    // avec une finition de 1 mm, l'ébauche commence à 4 mm de la paroi : coin (96, 46) ; sans, les anneaux sont à 3 puis 5,4 mm
    assert!(fin.contains("X96 Y46"), "{fin}");
    assert!(!apart.contains("X96 Y46"));
    assert!(fin.contains("X97 Y47")); // et la finition passe sur la paroi (3 mm)
}

// ───────────── plot ─────────────

#[test]
fn plot_of_a_drawing_traces_what_the_machine_will_run() {
    let out = std::env::temp_dir().join("fablabcam_plot_test.svg");
    let msg = go(&["plot", &fx("plaque.cb"), "--op", "profile-outside", "--layer", "Contour", "--tool", "6", "--depth", "-3", "--step", "1", "--tabs", "auto", "--tab-style", "square", "--lead-in", "tangent", "--lead-out", "tangent", "--radius", "5", "-o", out.to_str().unwrap()]).unwrap();
    assert!(msg.contains("écrit"), "{msg}");
    let svg = std::fs::read_to_string(&out).unwrap();
    assert!(svg.contains("plaque.cb"), "titre = nom du fichier");
    assert!(svg.contains("<polygon"), "flèches");
    assert!(svg.contains("stroke-dasharray"), "rapides");
    assert!(svg.contains("3 niveau(x) de coupe") || svg.contains("niveau(x) de coupe"));
    assert!(svg.contains("Profil de profondeur"));
}

#[test]
fn plot_toggles_and_filters() {
    let base = ["plot", "PLAQUE", "--op", "profile-outside", "--layer", "Contour", "--tool", "6", "--depth", "-3", "--step", "1"];
    let run = |extra: &[&str]| {
        let f = fx("plaque.cb");
        let mut a: Vec<&str> = base.iter().map(|x| if *x == "PLAQUE" { f.as_str() } else { *x }).collect();
        a.extend_from_slice(extra);
        go(&a).unwrap()
    };
    let full = run(&[]);
    assert!(full.contains("stroke-dasharray") && full.contains("<polygon"));
    let bare = run(&["--no-rapids", "--no-arrows", "--no-plunges"]);
    assert!(!bare.contains("stroke-dasharray=\"4 3\" "), "rapides masqués dans le tracé");
    assert!(bare.len() < full.len());
    // filtre de profondeur : une seule passe
    let one = run(&["--z-range", "-3.1,-2.9", "--no-arrows"]);
    assert!(one.matches("<polyline").count() < run(&["--no-arrows"]).matches("<polyline").count());
    assert!(run(&["--color", "type"]).contains("rgb(30,90,200)") || run(&["--color", "type"]).contains("rgb(20,150,110)"));
    assert!(go(&["plot", &fx("plaque.cb"), "--op", "pocket", "--color", "wat"]).unwrap_err().contains("--color"));
    assert!(go(&["plot", &fx("plaque.cb"), "--op", "pocket", "--z-range", "x"]).unwrap_err().contains("--z-range"));
}

#[test]
fn plot_reads_a_gcode_file_and_reports_warnings() {
    let nc = std::env::temp_dir().join("fablabcam_plot_in.nc");
    std::fs::write(&nc, "G21 G90\nG0 Z3\nG0 X0 Y0\nG1 Z-1 F200\nG2 X10 Y0 I5 J0\nG65 P1\n").unwrap();
    let out = std::env::temp_dir().join("fablabcam_plot_nc.svg");
    let msg = go(&["plot", nc.to_str().unwrap(), "-o", out.to_str().unwrap()]).unwrap();
    // G0 X0 Y0 part déjà de (0,0) : mouvement nul, non enregistré → plongée + arc
    assert!(msg.contains("2 mouvements") && msg.contains("attention") && msg.contains("G65"), "{msg}");
    assert!(std::fs::read_to_string(&out).unwrap().contains("<svg"));
    // sans -o, le SVG est renvoyé tel quel
    assert!(go(&["plot", nc.to_str().unwrap()]).unwrap().starts_with("<svg"));
}

#[test]
fn plot_errors() {
    assert!(go(&["plot"]).unwrap_err().contains("manquant"));
    assert!(go(&["plot", &fx("plaque.cb")]).unwrap_err().contains("--op"));
    assert!(go(&["plot", "inexistant.nc"]).is_err());
}

#[test]
fn convert_dxf_svg_cb_et_build() {
    let dir = std::env::temp_dir().join(format!("fablabcam-conv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = |n: &str| dir.join(n).to_string_lossy().into_owned();
    let run = |v: &[&str]| fablabcam_cli::run(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    let plaque = format!("{}/tests/fixtures/plaque.cb", env!("CARGO_MANIFEST_DIR"));
    assert!(run(&["convert", &plaque, &p("a.dxf")]).unwrap().contains("formes"));
    assert!(run(&["convert", &p("a.dxf"), &p("a.svg")]).is_ok());
    assert!(run(&["convert", &p("a.svg"), &p("b.cb")]).is_ok());
    assert!(run(&["convert", &p("a.svg"), &p("b.xyz")]).is_err());
    assert!(run(&["build", &p("b.cb")]).unwrap_err().contains("aucune opération"));
    let demo = fablabcam_studio::persist::save(&fablabcam_studio::studio::demo_project());
    std::fs::write(p("demo.cb"), demo).unwrap();
    let out = run(&["build", &p("demo.cb"), "-o", &p("demo.nc")]).unwrap();
    assert!(out.contains("écrit"));
    assert!(std::fs::read_to_string(p("demo.nc")).unwrap().contains("M6 T2"));
}

#[test]
fn sim_et_web_studio() {
    let run = |v: &[&str]| fablabcam_cli::run(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    let out = run(&["sim", "demo", "--res", "0.5"]).unwrap();
    assert!(out.contains("volume retiré") && out.contains("aucune gouge"), "{out}");
    assert!(run(&["sim", "demo", "--res", "0.5", "--at", "0.5"]).unwrap().contains("50 %"));
    let path = std::env::temp_dir().join(format!("studio-{}.html", std::process::id())).to_string_lossy().into_owned();
    assert!(run(&["web", "demo", "-o", &path]).unwrap().contains("écrit"));
    let html = std::fs::read_to_string(&path).unwrap();
    assert!(html.contains("\"sim\":{") && html.contains("\"tools\":[") && !html.contains("__DATA__"));
    assert!(run(&["web", "demo"]).is_err());
}

#[test]
fn build_avec_post_processeur() {
    let dir = std::env::temp_dir().join(format!("fablabcam-post-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = |n: &str| dir.join(n).to_string_lossy().into_owned();
    let run = |v: &[&str]| fablabcam_cli::run(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    std::fs::write(p("demo.cb"), fablabcam_studio::persist::save(&fablabcam_studio::studio::demo_project())).unwrap();
    std::fs::write(
        p("mon.cbpp"),
        "<PostProcessor><PostFile>%\nO{$o}\n{$header}\n{$mops}\n{$footer}\n%</PostFile><Header>{$units} {$distancemode}</Header>\n<Footer>{$endrewind}</Footer>\n<Truc/><Machin/></PostProcessor>",
    )
    .unwrap();
    let out = run(&["build", &p("demo.cb"), "--post", &p("mon.cbpp"), "--macros", "$o=4321", "-o", &p("demo.nc")]).unwrap();
    assert!(out.contains("écrit") && out.contains("éléments sans effet : Truc, Machin"), "{out}");
    let g = std::fs::read_to_string(p("demo.nc")).unwrap();
    assert!(g.starts_with("%\nO4321\nG21 G90\n"), "{g}");
    assert!(g.ends_with("M30\n%\n"), "{g}");
    // macro absente : avertissement, fichier quand même écrit
    let out = run(&["build", &p("demo.cb"), "--post", &p("mon.cbpp")]).unwrap();
    assert!(out.contains("macro inconnue : {$o}"), "{}", &out[out.len().saturating_sub(200)..]);
    assert!(run(&["build", &p("demo.cb"), "--post", &p("absent.cbpp")]).is_err());
}

#[test]
fn build_post_numerotation_et_arcs_en_lignes() {
    let dir = std::env::temp_dir().join(format!("fablabcam-postln-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = |n: &str| dir.join(n).to_string_lossy().into_owned();
    let run = |v: &[&str]| fablabcam_cli::run(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    std::fs::write(p("demo.cb"), fablabcam_studio::persist::save(&fablabcam_studio::studio::demo_project())).unwrap();
    std::fs::write(p("ln.cbpp"), "<PostProcessor><AddLineNumbers>true</AddLineNumbers><LineNumberFormat>0000</LineNumberFormat><ArcOutput>ConvertToLines</ArcOutput></PostProcessor>").unwrap();
    run(&["build", &p("demo.cb"), "--post", &p("ln.cbpp"), "-o", &p("o.nc")]).unwrap();
    let g = std::fs::read_to_string(p("o.nc")).unwrap();
    assert!(g.contains("\nN0010 G21 G90 G17\n"), "{g}");
    let words = |g: &str| g.split_whitespace().map(str::to_string).collect::<Vec<_>>();
    assert!(!words(&g).iter().any(|w| w == "G2" || w == "G3" || w.starts_with('I')), "arcs restants");
    // sans conversion, la démo contient bien des arcs
    std::fs::write(p("n.cbpp"), "<PostProcessor/>").unwrap();
    run(&["build", &p("demo.cb"), "--post", &p("n.cbpp"), "-o", &p("n.nc")]).unwrap();
    assert!(words(&std::fs::read_to_string(p("n.nc")).unwrap()).iter().any(|w| w == "G2" || w == "G3"));
}

#[test]
fn script_en_ligne_de_commande() {
    let dir = std::env::temp_dir().join(format!("fablabcam-script-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = |n: &str| dir.join(n).to_string_lossy().into_owned();
    let run = |v: &[&str]| fablabcam_cli::run(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    std::fs::write(p("s.rhai"), "let c = circle(5, 5, 2); tool(\"F2\", 2); pocket(\"P\", c, \"F2\", -1); print(\"ok\");").unwrap();
    let out = run(&["script", &p("s.rhai"), "-o", &p("s.cb")]).unwrap();
    assert!(out.starts_with("ok\n") && out.contains("1 formes, 1 opérations"), "{out}");
    assert!(run(&["build", &p("s.cb")]).unwrap().contains("G1"));
    std::fs::write(p("bad.rhai"), "circle(0, 0, -1)").unwrap();
    assert!(run(&["script", &p("bad.rhai")]).is_err());
}

// ───────────── import des projets XML de référence ─────────────

fn build_fixture(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("fablabcam-real-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("o.nc").to_string_lossy().into_owned();
    go(&["build", &fx(&format!("reference/{name}.cb")), "-o", &out]).unwrap();
    std::fs::read_to_string(out).unwrap()
}

#[test]
fn profil_ferme_avec_ponts_respecte_la_geometrie_de_reference() {
    // Rectangle [-91,0] × [0,516], fraise de rayon 3 et surface à Z=0.
    // Ces invariants géométriques ne dépendent pas d'un G-code généré par un autre logiciel.
    let g = fablabcam_backplot::parse_gcode(&build_fixture("tirroire-haut"));
    let cuts: Vec<_> = g.segs.iter().filter(|s| s.kind != fablabcam_backplot::Kind::Rapid && s.from != s.to).collect();
    assert!(!cuts.is_empty());
    assert!(cuts.iter().any(|s| s.to[2] > 0.0), "ponts absents");
    assert!(cuts.iter().all(|s| s.to[2] >= -0.001), "coupe sous la surface");
    assert!(cuts.iter().any(|s| s.kind == fablabcam_backplot::Kind::Arc));
}

#[test]
fn lignes_ouvertes_en_trois_passes_respectent_la_profondeur() {
    let g = fablabcam_backplot::parse_gcode(&build_fixture("tablette-soussol"));
    let cuts: Vec<_> = g.segs.iter().filter(|s| s.kind != fablabcam_backplot::Kind::Rapid && s.from != s.to).collect();
    for depth in [-6.0, -12.0, -18.0] {
        assert!(cuts.iter().any(|s| (s.to[2] - depth).abs() < 0.001), "passe absente : {depth}");
    }
    assert!(cuts.iter().all(|s| s.to[2] >= -18.001), "profondeur excessive");
    assert!(g.segs.iter().any(|s| s.kind == fablabcam_backplot::Kind::Rapid && (s.to[2] + 11.0).abs() < 0.001), "approche de la troisième passe absente");
}

#[test]
fn projet_cb_lu_avec_ses_operations() {
    let o = go(&["info", &fx("reference/tablette-soussol.cb")]).unwrap();
    assert!(o.contains("6 polyligne(s)"), "{o}");
}
