//! Build-a-Ring: the customer-facing configurator.
//!
//! A guided flow over `ringdesign-core` and nothing else — the preview is the
//! software rasterizer, so this binary carries no GL plumbing and the same
//! crate could later target a browser. Choices live in [`compose::Config`],
//! small serializable data; the finished order lands as a folder holding the
//! design file, the order JSON, the casting sheet, a GLB and a turntable.

mod compose;

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use compose::{compose, Base, Config, Stone, PATTERNS, SOLITAIRE_CUTS};
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{FieldReport, Verdict};
use ringdesign_core::mesh::{BuildParams, Mesh};
use ringdesign_core::{library, metal, render};

const PREVIEW: BuildParams = BuildParams {
    theta_steps: 320,
    profile_steps: 128,
    min_wall_mm: 0.5,
    adaptive: false,
    refine: None,
    soften_mm: 0.0,
};

/// Preview edge, px. Software-rastered, so modest and supersampled.
const VIEW_PX: usize = 640;

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_title("Build a Ring — Kings of Alchemy"),
        ..Default::default()
    };
    eframe::run_native(
        "build-a-ring",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

// --- Background build + render ----------------------------------------------

struct Job {
    generation: u64,
    /// A changed design, or `None` to re-render the cached mesh only.
    design: Option<ringdesign_core::RingDesign>,
    yaw: f64,
    pitch: f64,
    /// Cheap frame while dragging, supersampled once settled.
    fine: bool,
}

struct Frame {
    generation: u64,
    image: egui::ColorImage,
    field: Option<FieldReport>,
    grams: Vec<(String, f64)>,
    volume_mm3: f64,
}

fn spawn_worker(ctx: egui::Context) -> (Sender<Job>, Receiver<Frame>) {
    let (jobs_tx, jobs_rx) = channel::<Job>();
    let (frames_tx, frames_rx) = channel::<Frame>();
    std::thread::Builder::new()
        .name("compose-build".into())
        .spawn(move || {
            let mut lib = AlphaLibrary::builtin();
            let mut mesh: Option<Arc<Mesh>> = None;
            let mut field: Option<FieldReport> = None;
            let mut grams: Vec<(String, f64)> = Vec::new();
            let mut volume = 0.0;
            while let Ok(mut job) = jobs_rx.recv() {
                while let Ok(newer) = jobs_rx.try_recv() {
                    // Keep the newest, but never drop a design change for a
                    // camera-only frame.
                    if job.design.is_some() && newer.design.is_none() {
                        job = Job { design: job.design, ..newer };
                    } else {
                        job = newer;
                    }
                }
                if let Some(d) = job.design.take() {
                    d.bake_texts(&mut lib);
                    let out = ringdesign_core::mesh::build(&d, &lib, PREVIEW);
                    volume = out.report.volume_mm3;
                    grams = out
                        .report
                        .metals
                        .iter()
                        .map(|m| (m.metal.to_string(), m.grams))
                        .collect();
                    field = Some(ringdesign_core::castability::analyze_field(
                        &d, &lib, &d.draft, 144, 96,
                    ));
                    mesh = Some(Arc::new(out.mesh));
                }
                let Some(m) = mesh.as_ref() else { continue };
                let (edge, ss) = if job.fine { (VIEW_PX, 3) } else { (VIEW_PX / 2, 1) };
                let rgb = render::render_ss(m, job.yaw, job.pitch, edge, edge, ss, render::GOLD);
                let image = egui::ColorImage::from_rgb([edge, edge], &rgb);
                if frames_tx
                    .send(Frame {
                        generation: job.generation,
                        image,
                        field: field.clone(),
                        grams: grams.clone(),
                        volume_mm3: volume,
                    })
                    .is_err()
                {
                    break;
                }
                ctx.request_repaint();
            }
        })
        .expect("spawn worker");
    (jobs_tx, frames_rx)
}

/// One-shot worker: render every base once for the style step's cards.
fn spawn_thumbs(ctx: egui::Context) -> Receiver<(Base, egui::ColorImage)> {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("compose-thumbs".into())
        .spawn(move || {
            let lib = AlphaLibrary::builtin();
            for &base in Base::ALL {
                let cfg = Config { base, ..Config::default() };
                let d = compose(&cfg);
                let out = ringdesign_core::mesh::build(
                    &d,
                    &lib,
                    BuildParams { theta_steps: 192, profile_steps: 80, ..Default::default() },
                );
                let edge = 108usize;
                let rgb = render::render_ss(&out.mesh, 0.55, 1.12, edge, edge, 2, render::GOLD);
                if tx.send((base, egui::ColorImage::from_rgb([edge, edge], &rgb))).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        })
        .expect("spawn thumbs");
    rx
}

// --- The app -----------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Base,
    SizeMetal,
    Stone,
    Ornament,
    Review,
}

impl Step {
    const ALL: &'static [Step] =
        &[Step::Base, Step::SizeMetal, Step::Stone, Step::Ornament, Step::Review];

    fn label(self) -> &'static str {
        match self {
            Step::Base => "1 · Style",
            Step::SizeMetal => "2 · Size & metal",
            Step::Stone => "3 · Stones",
            Step::Ornament => "4 · Detail",
            Step::Review => "5 · Review",
        }
    }
}

struct App {
    cfg: Config,
    step: Step,
    jobs: Sender<Job>,
    frames: Receiver<Frame>,
    generation: u64,
    view: Option<egui::TextureHandle>,
    field: Option<FieldReport>,
    grams: Vec<(String, f64)>,
    volume_mm3: f64,
    yaw: f64,
    pitch: f64,
    dragging: bool,
    saved_to: Option<String>,
    saving: bool,
    thumbs_rx: Receiver<(Base, egui::ColorImage)>,
    thumbs: std::collections::HashMap<Base, egui::TextureHandle>,
    prices: std::collections::HashMap<String, f64>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (jobs, frames) = spawn_worker(cc.egui_ctx.clone());
        let thumbs_rx = spawn_thumbs(cc.egui_ctx.clone());
        let mut app = Self {
            cfg: Config::default(),
            step: Step::Base,
            jobs,
            frames,
            generation: 0,
            view: None,
            field: None,
            grams: Vec::new(),
            volume_mm3: 0.0,
            yaw: 0.55,
            pitch: 1.12,
            dragging: false,
            saved_to: None,
            saving: false,
            thumbs_rx,
            thumbs: std::collections::HashMap::new(),
            prices: compose::load_prices(),
        };
        app.rebuild();
        app
    }

    fn rebuild(&mut self) {
        self.cfg.reconcile();
        self.generation += 1;
        self.saved_to = None;
        let _ = self.jobs.send(Job {
            generation: self.generation,
            design: Some(compose(&self.cfg)),
            yaw: self.yaw,
            pitch: self.pitch,
            fine: true,
        });
    }

    fn rerender(&mut self, fine: bool) {
        self.generation += 1;
        let _ = self.jobs.send(Job {
            generation: self.generation,
            design: None,
            yaw: self.yaw,
            pitch: self.pitch,
            fine,
        });
    }

    /// Write the whole order — design, choices, sheet, GLB, turntable — into
    /// one folder named for the customer.
    fn save_order(&mut self) {
        self.saving = true;
        let cfg = self.cfg.clone();
        let slug: String = {
            let base = if cfg.customer.trim().is_empty() { "order" } else { cfg.customer.trim() };
            base.chars()
                .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
                .collect()
        };
        let dir = library::default_design_dir().with_file_name("orders").join(&slug);
        let result = (|| -> anyhow::Result<String> {
            std::fs::create_dir_all(&dir)?;
            let mut lib = AlphaLibrary::builtin();
            let d = compose(&cfg);
            d.bake_texts(&mut lib);

            library::save_design(dir.join("design.ring.json"), &d)?;
            std::fs::write(dir.join("choices.json"), serde_json::to_string_pretty(&cfg)?)?;

            let out = ringdesign_core::mesh::build(
                &d,
                &lib,
                BuildParams { theta_steps: 768, profile_steps: 256, ..Default::default() },
            );
            let field = ringdesign_core::castability::attributed_field_report(
                &d, &lib, &d.draft, 192, 128,
            );
            let stones = ringdesign_core::stones::report(&d, field.parting_z_mm);
            let dfm = ringdesign_core::dfm::findings(&d);
            let sheet = ringdesign_core::spec::html(
                &d,
                &out.report,
                &field,
                stones.as_ref(),
                &dfm,
                concat!("Build-a-Ring ", env!("CARGO_PKG_VERSION")),
            );
            std::fs::write(dir.join("casting_sheet.html"), sheet)?;
            ringdesign_core::gltf::write_glb(dir.join("ring.glb"), &out.mesh, &d.name, render::GOLD)?;
            render::write_png(dir.join("hero.png"), &out.mesh, 0.55, 1.12, 1280, render::GOLD)?;
            render::write_turntable_gif(dir.join("turntable.gif"), &out.mesh, 36, 480, render::GOLD)?;
            Ok(dir.display().to_string())
        })();
        self.saving = false;
        self.saved_to = Some(match result {
            Ok(p) => format!("Saved to {p}"),
            Err(e) => format!("Save failed: {e}"),
        });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((base, img)) = self.thumbs_rx.try_recv() {
            let tex = ctx.load_texture(
                format!("thumb-{}", base.label()),
                img,
                egui::TextureOptions::LINEAR,
            );
            self.thumbs.insert(base, tex);
        }
        while let Ok(f) = self.frames.try_recv() {
            if f.generation != self.generation {
                continue;
            }
            self.view = Some(ctx.load_texture("ring-view", f.image, egui::TextureOptions::LINEAR));
            if f.field.is_some() {
                self.field = f.field;
                self.grams = f.grams;
                self.volume_mm3 = f.volume_mm3;
            }
        }
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        let side = ui.available_width().min(ui.available_height());
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(side, side),
            egui::Sense::click_and_drag(),
        );
        ui.painter().rect_filled(rect, 8.0, egui::Color32::from_rgb(18, 18, 20));
        if let Some(tex) = self.view.as_ref() {
            ui.painter().image(
                tex.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "building…",
                egui::FontId::proportional(16.0),
                egui::Color32::GRAY,
            );
        }
        if response.dragged() {
            let d = response.drag_delta();
            self.yaw += f64::from(d.x) * 0.01;
            self.pitch = (self.pitch + f64::from(d.y) * 0.01).clamp(0.05, 1.55);
            self.dragging = true;
            self.rerender(false);
        } else if self.dragging && !response.dragged() {
            self.dragging = false;
            self.rerender(true);
        }
        ui.label(
            egui::RichText::new("drag to turn the ring")
                .small()
                .color(egui::Color32::from_gray(120)),
        );
    }

    fn step_base(&mut self, ui: &mut egui::Ui) {
        ui.heading("Choose a style");
        ui.add_space(6.0);
        let mut changed = false;
        for &b in Base::ALL {
            let selected = self.cfg.base == b;
            let r = ui
                .push_id(b.label(), |ui| {
                    let h = 64.0;
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), h),
                        egui::Sense::click(),
                    );
                    let bg = if selected {
                        ui.visuals().selection.bg_fill
                    } else if resp.hovered() {
                        ui.visuals().widgets.hovered.bg_fill
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };
                    ui.painter().rect_filled(rect, 6.0, bg);
                    let pad = 6.0;
                    let img_side = h - 2.0 * pad;
                    let img_rect = egui::Rect::from_min_size(
                        rect.min + egui::vec2(pad, pad),
                        egui::vec2(img_side, img_side),
                    );
                    if let Some(tex) = self.thumbs.get(&b) {
                        ui.painter().image(
                            tex.id(),
                            img_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    } else {
                        ui.painter().rect_filled(img_rect, 4.0, egui::Color32::from_gray(30));
                    }
                    let text_at = egui::pos2(img_rect.right() + 10.0, rect.min.y + 12.0);
                    ui.painter().text(
                        text_at,
                        egui::Align2::LEFT_TOP,
                        b.label(),
                        egui::FontId::proportional(15.0),
                        ui.visuals().strong_text_color(),
                    );
                    ui.painter().text(
                        text_at + egui::vec2(0.0, 22.0),
                        egui::Align2::LEFT_TOP,
                        b.blurb(),
                        egui::FontId::proportional(11.5),
                        ui.visuals().weak_text_color(),
                    );
                    resp
                })
                .inner;
            ui.add_space(4.0);
            if r.clicked() && !selected {
                self.cfg.base = b;
                changed = true;
            }
        }
        if changed {
            self.rebuild();
        }
    }

    fn step_size_metal(&mut self, ui: &mut egui::Ui) {
        ui.heading("Size and metal");
        ui.add_space(6.0);
        let mut changed = false;
        changed |= ui
            .add(egui::Slider::new(&mut self.cfg.size, 3.0..=13.0).step_by(0.25).text("US size"))
            .changed();
        ui.label(
            egui::RichText::new(format!(
                "inside diameter {:.2} mm",
                ringdesign_core::RingSize::new(self.cfg.size).inner_diameter_mm()
            ))
            .small(),
        );
        ui.add_space(8.0);
        for (i, m) in metal::METALS.iter().enumerate() {
            let g = self
                .grams
                .iter()
                .find(|(n, _)| n == m.name)
                .map(|(_, g)| *g);
            let price = g.and_then(|g| self.prices.get(m.name).map(|p| p * g));
            let label = match (g, price) {
                (Some(g), Some(p)) => format!("{} — {:.1} g · about ${:.0} in metal", m.name, g, p),
                (Some(g), None) => format!("{} — {:.1} g", m.name, g),
                _ => m.name.to_string(),
            };
            if ui.radio(self.cfg.metal == i, label).clicked() && self.cfg.metal != i {
                self.cfg.metal = i;
                changed = true;
            }
        }
        if self.prices.is_empty() {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "Metal prices show here when a prices.json sits beside the designs                      folder: {\"Silver 925\": 1.2, \"Gold 14k\": 55.0} per gram.",
                )
                .small()
                .color(egui::Color32::from_gray(120)),
            );
        }
        if changed {
            self.rebuild();
        }
    }

    fn step_stone(&mut self, ui: &mut egui::Ui) {
        ui.heading("Stones");
        ui.add_space(6.0);
        if !self.cfg.base.takes_stones() {
            ui.label("A signet carries its engraved face instead of stones.");
            return;
        }
        let mut changed = false;
        let is_none = matches!(self.cfg.stone, Stone::None);
        if ui.radio(is_none, "No stone").clicked() && !is_none {
            self.cfg.stone = Stone::None;
            changed = true;
        }
        let (is_sol, sol_cut, sol_mm) = match self.cfg.stone {
            Stone::Solitaire { cut, mm } => (true, cut, mm),
            _ => (false, ringdesign_core::gem::GemCut::Round, 5.0),
        };
        if ui.radio(is_sol, "Solitaire — one center stone").clicked() && !is_sol {
            self.cfg.stone = Stone::Solitaire { cut: sol_cut, mm: sol_mm };
            changed = true;
        }
        if is_sol {
            let mut mm = sol_mm;
            ui.horizontal_wrapped(|ui| {
                for &cut in SOLITAIRE_CUTS {
                    if ui.selectable_label(sol_cut == cut, cut.label()).clicked() && sol_cut != cut
                    {
                        self.cfg.stone = Stone::Solitaire { cut, mm };
                        changed = true;
                    }
                }
            });
            if ui
                .add(egui::Slider::new(&mut mm, 3.0..=8.0).step_by(0.5).text("stone mm"))
                .changed()
            {
                self.cfg.stone = Stone::Solitaire { cut: sol_cut, mm };
                changed = true;
            }
            let ct = ringdesign_core::gem::Gem::calibrated(sol_cut, mm).carats();
            ui.label(egui::RichText::new(format!("about {ct:.2} ct")).small());
        }
        let (is_et, et_mm) = match self.cfg.stone {
            Stone::Eternity { mm } => (true, mm),
            _ => (false, 1.5),
        };
        if ui.radio(is_et, "Eternity — stones all the way round").clicked() && !is_et {
            self.cfg.stone = Stone::Eternity { mm: et_mm };
            changed = true;
        }
        if is_et {
            let mut mm = et_mm;
            if ui
                .add(egui::Slider::new(&mut mm, 1.0..=2.5).step_by(0.25).text("stone mm"))
                .changed()
            {
                self.cfg.stone = Stone::Eternity { mm };
                changed = true;
            }
        }
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(
                "The ring is cast with seats and prong stock; your stones are set by hand at \
                 the bench.",
            )
            .small()
            .color(egui::Color32::from_gray(140)),
        );
        if changed {
            self.rebuild();
        }
    }

    fn step_ornament(&mut self, ui: &mut egui::Ui) {
        ui.heading("Detail");
        ui.add_space(6.0);
        let mut changed = false;
        if self.cfg.base.has_sides() {
            ui.label("Pattern on the sides");
            let none = self.cfg.pattern.is_none();
            if ui.radio(none, "Plain").clicked() && !none {
                self.cfg.pattern = None;
                changed = true;
            }
            for (i, (name, _)) in PATTERNS.iter().enumerate() {
                let sel = self.cfg.pattern == Some(i);
                if ui.radio(sel, *name).clicked() && !sel {
                    self.cfg.pattern = Some(i);
                    self.cfg.engraving.clear();
                    changed = true;
                }
            }
            ui.add_space(8.0);
            ui.label("Or engraved on the side instead");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut self.cfg.engraving)
                        .hint_text("a name, a date… (up to 24 letters)")
                        .char_limit(24),
                )
                .changed()
            {
                if !self.cfg.engraving.trim().is_empty() {
                    self.cfg.pattern = None;
                }
                changed = true;
            }
            if !self.cfg.engraving.trim().is_empty()
                && ui.checkbox(&mut self.cfg.script_font, "script lettering").changed()
            {
                changed = true;
            }
        } else {
            ui.label(
                "This style is all curves — patterns and side engraving need the wide flat \
                 sides of the Wide band, Split shank or a signet.",
            );
        }
        ui.add_space(8.0);
        if self.cfg.base.crest_is_stationary() {
            if ui.checkbox(&mut self.cfg.milgrain, "Milgrain beaded edge").changed() {
                changed = true;
            }
        } else {
            ui.label(
                egui::RichText::new(
                    "Milgrain needs a still crest line — the wave and twist carry their                      sparkle in the shape itself.",
                )
                .small()
                .color(egui::Color32::from_gray(140)),
            );
        }
        if changed {
            self.rebuild();
        }
    }

    fn step_review(&mut self, ui: &mut egui::Ui) {
        ui.heading("Your ring");
        ui.add_space(6.0);
        ui.label("Your name (for the order)");
        if ui.text_edit_singleline(&mut self.cfg.customer).changed() {
            self.rebuild();
        }
        ui.add_space(6.0);

        let metal = metal::METALS.get(self.cfg.metal);
        let grams = metal.and_then(|m| {
            self.grams.iter().find(|(n, _)| n == m.name).map(|(_, g)| *g)
        });
        ui.label(format!("Style: {}", self.cfg.base.label()));
        ui.label(format!("Size: US {:.2}", self.cfg.size));
        if let (Some(m), Some(g)) = (metal, grams) {
            match self.prices.get(m.name) {
                Some(p) => ui.label(format!(
                    "Metal: {} — about {:.1} g cast, about ${:.0} in metal",
                    m.name,
                    g,
                    p * g
                )),
                None => ui.label(format!("Metal: {} — about {:.1} g cast", m.name, g)),
            };
        }
        ui.label(format!("Stones: {}", self.cfg.stone.label()));
        if let Some(p) = self.cfg.pattern.and_then(|i| PATTERNS.get(i)) {
            ui.label(format!("Pattern: {}", p.0));
        }
        if !self.cfg.engraving.trim().is_empty() {
            ui.label(format!("Engraving: \u{201c}{}\u{201d}", self.cfg.engraving.trim()));
            ui.label(
                egui::RichText::new(
                    "Cast as raised lettering, then crisped by hand at the bench.",
                )
                .small()
                .color(egui::Color32::from_gray(140)),
            );
        }
        if self.cfg.milgrain {
            ui.label("Milgrain edge");
        }

        ui.add_space(6.0);
        if let Some(f) = self.field.as_ref() {
            let (color, text) = match f.verdict {
                Verdict::Castable => (egui::Color32::from_rgb(120, 200, 130), "Ready to cast"),
                Verdict::Marginal => (
                    egui::Color32::from_rgb(230, 190, 90),
                    "Castable — the jeweler will review the marked spots",
                ),
                Verdict::NotCastable => (
                    egui::Color32::from_rgb(230, 110, 110),
                    "Needs adjustment — the jeweler will follow up",
                ),
            };
            ui.colored_label(color, text);
        }

        ui.add_space(10.0);
        let save = egui::Button::new(
            egui::RichText::new(if self.saving { "Saving…" } else { "Save my ring" }).size(16.0),
        );
        if ui.add_sized([ui.available_width(), 40.0], save).clicked() && !self.saving {
            self.save_order();
        }
        if let Some(s) = self.saved_to.as_ref() {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(s).small());
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);

        egui::Panel::top(egui::Id::new("steps")).show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("Build a Ring");
                ui.separator();
                for &s in Step::ALL {
                    if ui.selectable_label(self.step == s, s.label()).clicked() {
                        self.step = s;
                    }
                }
            });
            ui.add_space(4.0);
        });

        egui::Panel::left(egui::Id::new("choices"))
            .default_size(360.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.add_space(6.0);
                    match self.step {
                        Step::Base => self.step_base(ui),
                        Step::SizeMetal => self.step_size_metal(ui),
                        Step::Stone => self.step_stone(ui),
                        Step::Ornament => self.step_ornament(ui),
                        Step::Review => self.step_review(ui),
                    }
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let i = Step::ALL.iter().position(|&s| s == self.step).unwrap_or(0);
                        if i > 0 && ui.button("< Back").clicked() {
                            self.step = Step::ALL[i - 1];
                        }
                        if i + 1 < Step::ALL.len() && ui.button("Next >").clicked() {
                            self.step = Step::ALL[i + 1];
                        }
                    });
                });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.vertical_centered(|ui| self.preview(ui));
        });
    }
}
