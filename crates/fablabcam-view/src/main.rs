//! `fablabcam-view` : fenêtre interactive de backplot (egui).
//!
//! ```text
//! fablabcam-view programme.nc
//! fablabcam-view dessin.cb --op profile-outside --tool 6 --depth -3 --step 1 --tabs auto
//! ```
//!
//! Pour un dessin `.cb`, le G-code est d'abord généré avec les options de `fablabcam gcode` puis relu :
//! on voit exactement ce que la machine exécutera.

use fablabcam_backplot::{build_scene, parse_gcode, Backplot, ColorBy, Kind, Prim, Stats, ViewOptions};
use fablabcam_view::view_model::{bounds_xy, nearest_cut, solo_range, Camera, Player};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Sense, Shape, Stroke, Vec2};

const HELP: &str = "\
fablabcam-view — backplot interactif

USAGE
  fablabcam-view <programme.nc|.gcode|.tap>
  fablabcam-view <dessin.cb> --op <profile-outside|profile-inside|profile-on|pocket|drill> [options de `fablabcam gcode`]

OPTIONS
  --smoke <n>   ouvre la fenêtre, dessine n images puis quitte (test de bon fonctionnement)
  --at <0..1>   démarre en mode animation à cette fraction du trajet

COMMANDES
  glisser : déplacer    molette / pincer : zoom    double-clic ou F : cadrer
  Espace : lecture/pause    ← → : mouvement précédent/suivant    clic sur le profil Z : se placer
";

#[derive(Clone, PartialEq)]
struct SceneKey {
    flags: (bool, bool, bool, bool),
    z_range: Option<(i64, i64)>,
    upto: Option<i64>,
    scale_bucket: i32,
}

struct App {
    title: String,
    bp: Backplot,
    stats: Stats,
    total: f64,
    cam: Camera,
    fit_pending: bool,
    view: ViewOptions,
    z_filter: bool,
    z_lo: f64,
    z_hi: f64,
    animate: bool,
    player: Player,
    cache: Option<(SceneKey, Vec<Prim>)>,
    smoke: Option<u32>,
    frame: u32,
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

impl App {
    fn new(title: String, gcode: &str, smoke: Option<u32>) -> App {
        let bp = parse_gcode(gcode);
        let stats = bp.stats(5000.0);
        let (lo, hi) = stats.cut_z.unwrap_or((stats.z_min, stats.z_max));
        let total = bp.total_len();
        App {
            title,
            bp,
            stats,
            total,
            cam: Camera { center: [0.0, 0.0], scale: 1.0 },
            fit_pending: true,
            view: ViewOptions::default(),
            z_filter: false,
            z_lo: lo,
            z_hi: hi,
            animate: false,
            player: Player::default(),
            cache: None,
            smoke,
            frame: 0,
        }
    }

    fn z_range(&self) -> Option<(f64, f64)> {
        self.z_filter.then(|| (self.z_lo.min(self.z_hi), self.z_lo.max(self.z_hi)))
    }

    /// Scène mise en cache : reconstruite quand les options, le zoom (par paliers) ou l'animation changent.
    fn scene(&mut self) -> &[Prim] {
        let scale = self.cam.scale.max(1e-6);
        let key = SceneKey {
            flags: (self.view.show_rapids, self.view.show_arrows, self.view.show_plunges, self.view.color_by == ColorBy::Z),
            z_range: self.z_range().map(|(a, b)| ((a * 1000.0) as i64, (b * 1000.0) as i64)),
            upto: self.animate.then_some((self.player.pos * 50.0) as i64),
            scale_bucket: (scale.log2() * 3.0).round() as i32,
        };
        if self.cache.as_ref().map(|(k, _)| k != &key).unwrap_or(true) {
            let opts = ViewOptions {
                arrow_spacing_mm: 44.0 / scale,
                max_err_mm: 0.4 / scale,
                z_range: self.z_range(),
                upto: self.animate.then_some(self.player.pos),
                ..self.view.clone()
            };
            self.cache = Some((key, build_scene(&self.bp, &opts)));
        }
        &self.cache.as_ref().unwrap().1
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading(&self.title);
        let st = &self.stats;
        ui.label(format!("{} mouvements", st.segments));
        ui.label(format!("Coupe : {:.1} mm", st.cut_len));
        ui.label(format!("Rapides : {:.1} mm", st.rapid_len));
        let t = st.time_s.round() as u64;
        ui.label(format!("Durée estimée : {} min {:02} s", t / 60, t % 60));
        ui.separator();

        ui.strong("Affichage");
        let mut changed = false;
        changed |= ui.checkbox(&mut self.view.show_rapids, "rapides (pointillés)").changed();
        changed |= ui.checkbox(&mut self.view.show_arrows, "flèches de sens").changed();
        changed |= ui.checkbox(&mut self.view.show_plunges, "points de plongée").changed();
        ui.horizontal(|ui| {
            ui.label("Couleur :");
            changed |= ui.radio_value(&mut self.view.color_by, ColorBy::Z, "profondeur").changed();
            changed |= ui.radio_value(&mut self.view.color_by, ColorBy::Type, "type").changed();
        });
        let _ = changed;
        if ui.button("Cadrer la vue (F)").clicked() {
            self.fit_pending = true;
        }
        ui.separator();

        ui.strong("Profondeur Z");
        if let Some((lo, hi)) = self.stats.cut_z {
            ui.checkbox(&mut self.z_filter, "n'afficher que les passes entre :");
            ui.add_enabled_ui(self.z_filter, |ui| {
                ui.add(egui::Slider::new(&mut self.z_lo, lo..=hi).text("bas").fixed_decimals(2));
                ui.add(egui::Slider::new(&mut self.z_hi, lo..=hi).text("haut").fixed_decimals(2));
            });
            ui.label("Niveaux de coupe (clic = isoler) :");
            let levels = self.stats.cut_levels.clone();
            ui.horizontal_wrapped(|ui| {
                if ui.button("tous").clicked() {
                    self.z_filter = false;
                }
                for (i, z) in levels.iter().enumerate().take(40) {
                    if ui.button(format!("{z:.2}")).clicked() {
                        if let Some((a, b)) = solo_range(&levels, i) {
                            self.z_filter = true;
                            self.z_lo = a;
                            self.z_hi = b;
                        }
                    }
                }
            });
        } else {
            ui.label("aucune coupe");
        }
        ui.separator();

        ui.strong("Animation");
        ui.checkbox(&mut self.animate, "suivre l'outil");
        ui.horizontal(|ui| {
            if ui.button("|◀").clicked() {
                self.animate = true;
                self.player.pos = 0.0;
                self.player.playing = false;
            }
            if ui.button("◀").clicked() {
                self.animate = true;
                self.player.step_segments(&self.bp, -1);
            }
            let label = if self.player.playing { "⏸ pause" } else { "▶ lecture" };
            if ui.button(label).clicked() {
                self.animate = true;
                if self.player.pos >= self.total {
                    self.player.pos = 0.0;
                }
                self.player.playing = !self.player.playing;
            }
            if ui.button("▶").clicked() {
                self.animate = true;
                self.player.step_segments(&self.bp, 1);
            }
            if ui.button("▶|").clicked() {
                self.animate = true;
                self.player.pos = self.total;
                self.player.playing = false;
            }
        });
        let mut pos = self.player.pos;
        if ui.add(egui::Slider::new(&mut pos, 0.0..=self.total.max(1e-6)).text("mm").fixed_decimals(1)).changed() {
            self.animate = true;
            self.player.pos = pos;
        }
        ui.add(egui::Slider::new(&mut self.player.speed, 5.0..=5000.0).logarithmic(true).text("mm/s"));
        if let Some((pt, i, dir)) = self.bp.position_at(self.player.pos) {
            let s = &self.bp.segs[i];
            ui.monospace(format!("X {:8.3}\nY {:8.3}\nZ {:8.3}", pt[0], pt[1], pt[2]));
            let way = match dir {
                Some(d) => format!("cap {:.0}°", d[1].atan2(d[0]).to_degrees()),
                None => "vertical".into(),
            };
            ui.label(format!("{:?}, F{:.0}, {way}", s.kind, s.feed));
            if let Some(src) = self.bp.source.get(s.line.saturating_sub(1)) {
                ui.monospace(format!("L{}: {}", s.line, src.trim()));
            }
        }
        if !self.bp.warnings.is_empty() {
            ui.separator();
            ui.strong("Avertissements");
            for w in &self.bp.warnings {
                ui.colored_label(Color32::from_rgb(180, 30, 30), format!("⚠ {w}"));
            }
        }
        ui.separator();
        ui.small("glisser : déplacer · molette : zoom · double-clic / F : cadrer\nEspace : lecture · ← → : pas à pas");
    }

    fn profile_panel(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let r = resp.rect;
        painter.rect_filled(r, 0.0, Color32::from_rgb(250, 250, 250));
        painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, Color32::from_gray(200)));
        if self.total <= 0.0 {
            return;
        }
        let (zmin, zmax) = (self.stats.z_min, self.stats.z_max);
        let zr = (zmax - zmin).max(1e-9);
        let to = |d: f64, z: f64| Pos2::new(r.left() + (d / self.total) as f32 * r.width(), r.bottom() - 6.0 - ((z - zmin) / zr) as f32 * (r.height() - 12.0));
        let prof = self.bp.z_profile();
        let step = (prof.len() / 6000).max(1);
        let pts: Vec<Pos2> = prof.iter().step_by(step).map(|(d, z)| to(*d, *z)).collect();
        painter.add(Shape::line(pts, Stroke::new(1.2_f32, Color32::from_rgb(43, 95, 180))));
        painter.text(r.left_top() + Vec2::new(4.0, 2.0), Align2::LEFT_TOP, format!("Z {zmax:.1}"), FontId::monospace(10.0), Color32::GRAY);
        painter.text(r.left_bottom() + Vec2::new(4.0, -2.0), Align2::LEFT_BOTTOM, format!("Z {zmin:.1}"), FontId::monospace(10.0), Color32::GRAY);
        painter.text(r.right_bottom() - Vec2::new(4.0, 2.0), Align2::RIGHT_BOTTOM, format!("{:.0} mm", self.total), FontId::monospace(10.0), Color32::GRAY);
        if self.animate {
            let x = r.left() + (self.player.pos / self.total) as f32 * r.width();
            painter.line_segment([Pos2::new(x, r.top()), Pos2::new(x, r.bottom())], Stroke::new(2.0_f32, Color32::from_rgb(255, 60, 0)));
        }
        if resp.clicked() || resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.animate = true;
                self.player.playing = false;
                self.player.pos = (((p.x - r.left()) / r.width()) as f64).clamp(0.0, 1.0) * self.total;
            }
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        let size = (rect.width() as f64, rect.height() as f64);
        if size.0 < 10.0 || size.1 < 10.0 {
            return;
        }
        if self.fit_pending || resp.double_clicked() {
            if let Some((lo, hi)) = bounds_xy(&self.bp) {
                self.cam = Camera::fit(lo, hi, size.0, size.1, 40.0);
            }
            self.fit_pending = false;
        }
        if resp.dragged() {
            let d = resp.drag_delta();
            self.cam.pan_px(d.x as f64, d.y as f64);
        }
        if resp.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.scroll_delta.y, i.zoom_delta()));
            let factor = (scroll as f64 * 0.01).exp() * zoom as f64;
            if (factor - 1.0).abs() > 1e-6 {
                if let Some(p) = resp.hover_pos() {
                    self.cam.zoom_at(factor, ((p.x - rect.left()) as f64, (p.y - rect.top()) as f64), size);
                }
            }
        }
        let cam = self.cam;
        let to = move |p: [f64; 2]| {
            let (x, y) = cam.to_screen(p, size);
            Pos2::new(rect.left() + x as f32, rect.top() + y as f32)
        };
        let painter = painter.with_clip_rect(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(250, 250, 250));

        // grille adaptée au zoom
        let step = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0].into_iter().find(|s| s * cam.scale >= 40.0).unwrap_or(1000.0);
        let (w0, w1) = (cam.to_world((0.0, size.1), size), cam.to_world((size.0, 0.0), size));
        let grid = Stroke::new(1.0_f32, Color32::from_gray(232));
        let mut x = (w0[0] / step).floor() * step;
        while x <= w1[0] {
            painter.line_segment([to([x, w0[1]]), to([x, w1[1]])], grid);
            x += step;
        }
        let mut y = (w0[1] / step).floor() * step;
        while y <= w1[1] {
            painter.line_segment([to([w0[0], y]), to([w1[0], y])], grid);
            y += step;
        }
        let axes = Stroke::new(1.0_f32, Color32::from_gray(190));
        painter.line_segment([to([0.0, w0[1]]), to([0.0, w1[1]])], axes);
        painter.line_segment([to([w0[0], 0.0]), to([w1[0], 0.0])], axes);
        painter.text(rect.right_bottom() - Vec2::new(6.0, 4.0), Align2::RIGHT_BOTTOM, format!("grille {step} mm"), FontId::monospace(11.0), Color32::GRAY);

        // trajet
        let prims: Vec<Prim> = self.scene().to_vec();
        for p in &prims {
            match p {
                Prim::Line { pts, rgb: c, width, dashed } => {
                    let pts: Vec<Pos2> = pts.iter().map(|q| to(*q)).collect();
                    let stroke = Stroke::new(*width, rgb(*c));
                    if *dashed {
                        painter.extend(Shape::dashed_line(&pts, stroke, 5.0, 4.0));
                    } else {
                        painter.add(Shape::line(pts, stroke));
                    }
                }
                Prim::Arrow { pos, dir, rgb: c } => {
                    let o = to(*pos);
                    let d = Vec2::new(dir[0] as f32, -dir[1] as f32);
                    let n = Vec2::new(-d.y, d.x);
                    let (l, b) = (6.0, 3.2);
                    painter.add(Shape::convex_polygon(vec![o + d * l, o - d * l * 0.6 + n * b, o - d * l * 0.6 - n * b], rgb(*c), Stroke::NONE));
                }
                Prim::Dot { pos, rgb: c, radius, ring } => {
                    let o = to(*pos);
                    if *ring {
                        painter.circle_stroke(o, *radius, Stroke::new(2.5_f32, rgb(*c)));
                    } else {
                        painter.circle_filled(o, *radius, rgb(*c));
                        painter.circle_stroke(o, *radius, Stroke::new(1.0_f32, Color32::WHITE));
                    }
                }
            }
        }

        // survol : coordonnées, mouvement le plus proche, ligne de G-code
        if let Some(h) = resp.hover_pos() {
            let world = cam.to_world(((h.x - rect.left()) as f64, (h.y - rect.top()) as f64), size);
            let mut info = format!("X {:.2}  Y {:.2}", world[0], world[1]);
            if let Some((i, d, _)) = nearest_cut(&self.bp, world, 7.0 / cam.scale) {
                let s = &self.bp.segs[i];
                let pts: Vec<Pos2> = s.polyline(0.4 / cam.scale).iter().map(|p| to([p[0], p[1]])).collect();
                painter.add(Shape::line(pts, Stroke::new(4.5_f32, Color32::from_rgba_unmultiplied(255, 140, 0, 150))));
                let kind = match s.kind {
                    Kind::Arc => if s.ccw { "arc G3" } else { "arc G2" },
                    Kind::Feed => "coupe G1",
                    Kind::Plunge => "plongée",
                    Kind::Rapid => "rapide",
                };
                let z = if (s.from[2] - s.to[2]).abs() < 1e-9 { format!("Z {:.2}", s.from[2]) } else { format!("Z {:.2}→{:.2}", s.from[2], s.to[2]) };
                info += &format!("   {kind}  {z}  F{:.0}  (à {d:.2} mm)", s.feed);
                if let Some(src) = self.bp.source.get(s.line.saturating_sub(1)) {
                    info += &format!("\nL{}: {}", s.line, src.trim());
                }
            }
            let galley_pos = rect.left_bottom() + Vec2::new(8.0, -8.0);
            let r = painter.text(galley_pos, Align2::LEFT_BOTTOM, &info, FontId::monospace(12.0), Color32::BLACK);
            painter.rect_filled(r.expand(3.0), 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 215));
            painter.text(galley_pos, Align2::LEFT_BOTTOM, info, FontId::monospace(12.0), Color32::BLACK);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame += 1;
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1) as f64;
        if self.animate {
            self.player.advance(dt, self.total);
            if self.player.playing {
                ctx.request_repaint();
            }
        }
        ctx.input(|i| {
            if i.key_pressed(egui::Key::F) {
                self.fit_pending = true;
            }
            if i.key_pressed(egui::Key::Space) {
                self.animate = true;
                if self.player.pos >= self.total {
                    self.player.pos = 0.0;
                }
                self.player.playing = !self.player.playing;
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                self.animate = true;
                self.player.step_segments(&self.bp, 1);
            }
            if i.key_pressed(egui::Key::ArrowLeft) {
                self.animate = true;
                self.player.step_segments(&self.bp, -1);
            }
        });

        egui::SidePanel::left("side").default_width(300.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.side_panel(ui));
        });
        egui::TopBottomPanel::bottom("profile").exact_height(130.0).show(ctx, |ui| self.profile_panel(ui));
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| self.canvas(ui));

        if let Some(n) = self.smoke {
            ctx.request_repaint();
            if self.frame >= n {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

fn load(args: &[String]) -> Result<(String, String), String> {
    let file = args.iter().find(|a| !a.starts_with('-')).ok_or("fichier manquant")?;
    let name = std::path::Path::new(file).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| file.clone());
    if file.ends_with(".cb") {
        let mut a = vec!["gcode".to_string()];
        a.extend(args.iter().cloned());
        Ok((name, fablabcam_cli::run(&a)?))
    } else {
        Ok((name, std::fs::read_to_string(file).map_err(|e| format!("{file} : {e}"))?))
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{HELP}");
        std::process::exit(if args.is_empty() { 2 } else { 0 });
    }
    let mut smoke = None;
    if let Some(i) = args.iter().position(|a| a == "--smoke") {
        smoke = args.get(i + 1).and_then(|n| n.parse::<u32>().ok()).or(Some(30));
        let end = (i + 2).min(args.len());
        args.drain(i..end);
    }
    let mut at = None;
    if let Some(i) = args.iter().position(|a| a == "--at") {
        at = args.get(i + 1).and_then(|n| n.replace(',', ".").parse::<f64>().ok());
        let end = (i + 2).min(args.len());
        args.drain(i..end);
    }
    let (name, gcode) = match load(&args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("erreur : {e}");
            std::process::exit(1);
        }
    };
    let mut app = App::new(name.clone(), &gcode, smoke);
    if let Some(f) = at {
        app.animate = true;
        app.player.pos = f.clamp(0.0, 1.0) * app.total;
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1360.0, 860.0]).with_title(format!("fablabcam-view — {name}")),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native("fablabcam-view", options, Box::new(move |_cc| Box::new(app))) {
        eprintln!("erreur d'affichage : {e}");
        std::process::exit(1);
    }
}
