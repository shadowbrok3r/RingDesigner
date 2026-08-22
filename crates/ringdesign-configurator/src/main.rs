//! Build-a-Ring: the customer-facing configurator.
//!
//! A guided flow over `ringdesign-core` and nothing else — the preview is the
//! software rasterizer, so this binary carries no GL plumbing and the same
//! crate could later target a browser. Choices live in [`compose::Config`],
//! small serializable data; the finished order lands as a folder holding the
//! design file, the order JSON, the casting sheet, a GLB and a turntable — or,
//! in the browser, as one zip download of the same files.
//!
//! Web build: `trunk serve` in this crate's directory (`index.html` carries
//! the `--no-default-features` flag, so core runs serial). There is no
//! thread in a browser, so the build worker and the thumbnail renders run on
//! the UI thread, pumped from `poll` — a preview build is ~40 ms serial.

mod compose;
#[cfg(target_arch = "wasm32")]
mod web;

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

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window().and_then(|w| w.document()).expect("no document");
        let canvas = document
            .get_element_by_id("build_a_ring")
            .expect("no canvas #build_a_ring")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#build_a_ring is not a canvas");
        let started = eframe::WebRunner::new()
            .start(canvas, eframe::WebOptions::default(), Box::new(|cc| Ok(Box::new(App::new(cc)))))
            .await;
        if let Err(e) = started {
            log::error!("build-a-ring failed to start: {e:?}");
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
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

/// The build-and-render state one job advances; the same body runs on a
/// thread natively and inline on the UI thread in the browser.
struct Worker {
    lib: AlphaLibrary,
    mesh: Option<Arc<Mesh>>,
    field: Option<FieldReport>,
    grams: Vec<(String, f64)>,
    volume: f64,
}

impl Worker {
    fn new() -> Self {
        Self { lib: AlphaLibrary::builtin(), mesh: None, field: None, grams: Vec::new(), volume: 0.0 }
    }

    /// Keeps the newest pending job, never dropping a design change for a
    /// camera-only frame.
    fn coalesce(mut job: Job, rx: &Receiver<Job>) -> Job {
        while let Ok(newer) = rx.try_recv() {
            if job.design.is_some() && newer.design.is_none() {
                job = Job { design: job.design, ..newer };
            } else {
                job = newer;
            }
        }
        job
    }

    fn run(&mut self, mut job: Job) -> Option<Frame> {
        if let Some(d) = job.design.take() {
            d.bake_texts(&mut self.lib);
            let out = ringdesign_core::mesh::build(&d, &self.lib, PREVIEW);
            self.volume = out.report.volume_mm3;
            self.grams = out.report.metals.iter().map(|m| (m.metal.to_string(), m.grams)).collect();
            self.field =
                Some(ringdesign_core::castability::analyze_field(&d, &self.lib, &d.draft, 144, 96));
            self.mesh = Some(Arc::new(out.mesh));
        }
        let m = self.mesh.as_ref()?;
        let (edge, ss) = if job.fine { (VIEW_PX, 3) } else { (VIEW_PX / 2, 1) };
        let rgb = render::render_ss(m, job.yaw, job.pitch, edge, edge, ss, render::GOLD);
        Some(Frame {
            generation: job.generation,
            image: egui::ColorImage::from_rgb([edge, edge], &rgb),
            field: self.field.clone(),
            grams: self.grams.clone(),
            volume_mm3: self.volume,
        })
    }
}

/// Jobs in, frames out: a thread natively, `pump` on the UI thread in the
/// browser.
struct Engine {
    jobs: Sender<Job>,
    frames: Receiver<Frame>,
    #[cfg(target_arch = "wasm32")]
    inline: (Receiver<Job>, Sender<Frame>, Worker),
}

impl Engine {
    #[cfg(not(target_arch = "wasm32"))]
    fn new(ctx: egui::Context) -> Self {
        let (jobs_tx, jobs_rx) = channel::<Job>();
        let (frames_tx, frames_rx) = channel::<Frame>();
        std::thread::Builder::new()
            .name("compose-build".into())
            .spawn(move || {
                let mut worker = Worker::new();
                while let Ok(job) = jobs_rx.recv() {
                    let job = Worker::coalesce(job, &jobs_rx);
                    if let Some(frame) = worker.run(job) {
                        if frames_tx.send(frame).is_err() {
                            break;
                        }
                        ctx.request_repaint();
                    }
                }
            })
            .expect("spawn worker");
        Self { jobs: jobs_tx, frames: frames_rx }
    }

    #[cfg(target_arch = "wasm32")]
    fn new(_ctx: egui::Context) -> Self {
        let (jobs_tx, jobs_rx) = channel::<Job>();
        let (frames_tx, frames_rx) = channel::<Frame>();
        Self { jobs: jobs_tx, frames: frames_rx, inline: (jobs_rx, frames_tx, Worker::new()) }
    }

    fn send(&self, job: Job) {
        let _ = self.jobs.send(job);
    }

    /// Runs the pending jobs here where there is no worker thread.
    fn pump(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            let (rx, tx, worker) = &mut self.inline;
            while let Ok(job) = rx.try_recv() {
                let job = Worker::coalesce(job, rx);
                if let Some(frame) = worker.run(job) {
                    let _ = tx.send(frame);
                }
            }
        }
    }
}

/// One base rendered for the style step's cards.
fn thumb(base: Base, lib: &AlphaLibrary) -> egui::ColorImage {
    let cfg = Config { base, ..Config::default() };
    let d = compose(&cfg);
    let out = ringdesign_core::mesh::build(
        &d,
        lib,
        BuildParams { theta_steps: 192, profile_steps: 80, ..Default::default() },
    );
    let edge = 108usize;
    let rgb = render::render_ss(&out.mesh, 0.55, 1.12, edge, edge, 2, render::GOLD);
    egui::ColorImage::from_rgb([edge, edge], &rgb)
}

/// Every base's card, once: a one-shot thread natively, one base per frame
/// in the browser.
struct Thumbs {
    rx: Receiver<(Base, egui::ColorImage)>,
    #[cfg(target_arch = "wasm32")]
    inline: (Vec<Base>, Sender<(Base, egui::ColorImage)>, AlphaLibrary),
}

impl Thumbs {
    #[cfg(not(target_arch = "wasm32"))]
    fn new(ctx: egui::Context) -> Self {
        let (tx, rx) = channel();
        std::thread::Builder::new()
            .name("compose-thumbs".into())
            .spawn(move || {
                let lib = AlphaLibrary::builtin();
                for &base in Base::ALL {
                    if tx.send((base, thumb(base, &lib))).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("spawn thumbs");
        Self { rx }
    }

    #[cfg(target_arch = "wasm32")]
    fn new(_ctx: egui::Context) -> Self {
        let (tx, rx) = channel();
        let mut pending = Base::ALL.to_vec();
        pending.reverse();
        Self { rx, inline: (pending, tx, AlphaLibrary::builtin()) }
    }

    /// One more card where the thumbs run inline, asking for another frame
    /// while any remain.
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
    fn pump(&mut self, ctx: &egui::Context) {
        #[cfg(target_arch = "wasm32")]
        {
            let (pending, tx, lib) = &mut self.inline;
            if let Some(base) = pending.pop() {
                let _ = tx.send((base, thumb(base, lib)));
                if !pending.is_empty() {
                    ctx.request_repaint();
                }
            }
        }
    }
}

/// How an order is built: the full pass the app writes, or a small one for
/// tests.
struct OrderParams {
    build: BuildParams,
    hero_px: usize,
    gif_frames: usize,
    gif_px: usize,
}

impl OrderParams {
    const FULL: OrderParams = OrderParams {
        build: BuildParams { theta_steps: 768, profile_steps: 256, min_wall_mm: 0.5, adaptive: false, refine: None, soften_mm: 0.0 },
        hero_px: 1280,
        gif_frames: 36,
        gif_px: 480,
    };
}

/// The order's files — design, choices, sheet, GLB, hero, turntable — as
/// name and bytes.
fn order_files(cfg: &Config, p: &OrderParams) -> anyhow::Result<Vec<(&'static str, Vec<u8>)>> {
    let mut lib = AlphaLibrary::builtin();
    let d = compose(cfg);
    d.bake_texts(&mut lib);
    let out = ringdesign_core::mesh::build(&d, &lib, p.build);
    let field = ringdesign_core::castability::attributed_field_report(&d, &lib, &d.draft, 192, 128);
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
    Ok(vec![
        ("design.ring.json", library::design_json(&d)?.into_bytes()),
        ("choices.json", serde_json::to_string_pretty(cfg)?.into_bytes()),
        ("casting_sheet.html", sheet.into_bytes()),
        ("ring.glb", ringdesign_core::gltf::to_glb(&out.mesh, &d.name, render::GOLD)),
        ("hero.png", render::png_bytes(&out.mesh, 0.55, 1.12, p.hero_px, render::GOLD)?),
        ("turntable.gif", render::turntable_gif_bytes(&out.mesh, p.gif_frames, p.gif_px, render::GOLD)?),
    ])
}

/// The customer's name as a folder or file name.
fn order_slug(cfg: &Config) -> String {
    let base = if cfg.customer.trim().is_empty() { "order" } else { cfg.customer.trim() };
    base.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect()
}

/// Writes the order into one folder named for the customer.
#[cfg(not(target_arch = "wasm32"))]
fn deliver_order(slug: &str, files: Vec<(&'static str, Vec<u8>)>) -> anyhow::Result<String> {
    let dir = library::default_design_dir().with_file_name("orders").join(slug);
    std::fs::create_dir_all(&dir)?;
    for (name, bytes) in files {
        std::fs::write(dir.join(name), bytes)?;
    }
    Ok(dir.display().to_string())
}

/// Hands the order to the browser as one zip download.
#[cfg(target_arch = "wasm32")]
fn deliver_order(slug: &str, files: Vec<(&'static str, Vec<u8>)>) -> anyhow::Result<String> {
    use ringdesign_core::threemf::{zip_store, Entry};
    let entries: Vec<Entry> = files.into_iter().map(|(n, data)| Entry { name: n.to_string(), data }).collect();
    let name = format!("order-{slug}.zip");
    web::download(&name, "application/zip", &zip_store(&entries))?;
    Ok(format!("your downloads, as {name}"))
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
    engine: Engine,
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
    thumb_jobs: Thumbs,
    thumbs: std::collections::HashMap<Base, egui::TextureHandle>,
    prices: std::collections::HashMap<String, f64>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            cfg: Config::default(),
            step: Step::Base,
            engine: Engine::new(cc.egui_ctx.clone()),
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
            thumb_jobs: Thumbs::new(cc.egui_ctx.clone()),
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
        self.engine.send(Job {
            generation: self.generation,
            design: Some(compose(&self.cfg)),
            yaw: self.yaw,
            pitch: self.pitch,
            fine: true,
        });
    }

    fn rerender(&mut self, fine: bool) {
        self.generation += 1;
        self.engine.send(Job {
            generation: self.generation,
            design: None,
            yaw: self.yaw,
            pitch: self.pitch,
            fine,
        });
    }

    /// The whole order — design, choices, sheet, GLB, hero, turntable —
    /// into one folder named for the customer, or one zip in the browser.
    fn save_order(&mut self) {
        self.saving = true;
        let slug = order_slug(&self.cfg);
        let result = order_files(&self.cfg, &OrderParams::FULL).and_then(|files| deliver_order(&slug, files));
        self.saving = false;
        self.saved_to = Some(match result {
            Ok(p) => format!("Saved to {p}"),
            Err(e) => format!("Save failed: {e}"),
        });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        self.engine.pump();
        self.thumb_jobs.pump(ctx);
        while let Ok((base, img)) = self.thumb_jobs.rx.try_recv() {
            let tex = ctx.load_texture(
                format!("thumb-{}", base.label()),
                img,
                egui::TextureOptions::LINEAR,
            );
            self.thumbs.insert(base, tex);
        }
        while let Ok(f) = self.engine.frames.try_recv() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_order_is_six_files_and_a_slug() {
        let cfg = Config { customer: "Ada Lovelace!".into(), ..Config::default() };
        assert_eq!(order_slug(&cfg), "ada_lovelace_");
        assert_eq!(order_slug(&Config::default()), "order");
        let small = OrderParams {
            build: BuildParams { theta_steps: 96, profile_steps: 48, ..Default::default() },
            hero_px: 64,
            gif_frames: 4,
            gif_px: 32,
        };
        let files = order_files(&cfg, &small).unwrap();
        let names: Vec<&str> = files.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            ["design.ring.json", "choices.json", "casting_sheet.html", "ring.glb", "hero.png", "turntable.gif"]
        );
        assert!(files.iter().all(|(_, b)| !b.is_empty()));
        assert_eq!(&files[3].1[..4], b"glTF");
        assert_eq!(&files[4].1[..4], b"\x89PNG");
        assert_eq!(&files[5].1[..6], b"GIF89a");
        let design = ringdesign_core::library::load_design_str(std::str::from_utf8(&files[0].1).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(&design).unwrap(), serde_json::to_value(compose(&cfg)).unwrap());
        let back: Config = serde_json::from_slice(&files[1].1).unwrap();
        assert_eq!(back.customer, "Ada Lovelace!");
    }
}
