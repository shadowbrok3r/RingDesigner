//! One metadata-only template library for desktop and Android menus.
//! Ring data is parsed only after a choice; thumbnails are small bundled renders.
use std::sync::{
    Arc, LazyLock, Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
    mpsc::{self, TryRecvError},
};
use egui::{TextureHandle, Ui};
use ringdesign_core::{Alpha, AlphaLibrary, RingDesign};
use ringdesign_graph::{
    eval::{EvalReport, Evaluator},
    graph::Graph,
    registry::Registry,
    templates::{Step, TemplateGraph},
};

pub struct Template {
    pub name: &'static str,
    pub slug: &'static str,
    pub description: &'static str,
    source: Source,
}
/// `Design` names a bundled `.ring.json`; the document is decompressed only
/// when that template is chosen, not to list it in a menu.
#[derive(Clone, Copy)]
enum Source { Graph(&'static TemplateGraph), Document(&'static Graph), Starter(&'static ringdesign_core::templates::Template), Design(&'static str) }
impl Template {
    /// A template made from a graph document already read, opened like a bundled template graph.
    pub fn document(name: &'static str, slug: &'static str, graph: &'static Graph) -> Self {
        Template { name, slug, description: "", source: Source::Document(graph) }
    }

    /// The template's design, made on the calling thread with expression pins run.
    pub fn instantiate(&self, reg: &Registry, lib: &AlphaLibrary) -> anyhow::Result<RingDesign> {
        let lib = Arc::new(lib.clone());
        Ok(opened(self.source, reg, &lib, Arc::new(|_| {}), &Arc::default())?.design)
    }

    /// Opens this template on a thread of its own against `lib`, calling `wake` whenever it gets further; poll the [`Opening`] for the design.
    pub fn open(&'static self, reg: Arc<Registry>, lib: Arc<AlphaLibrary>, wake: impl Fn() + Send + Sync + 'static) -> Opening {
        open(self.name, self.source, reg, lib, wake)
    }

    /// What [`open`](Self::open) does on its thread, on the calling one: telling `progress` each stage and stopping once `cancel` is set.
    pub fn instantiate_with(&self, reg: &Registry, lib: &Arc<AlphaLibrary>, progress: &Arc<Progress>, cancel: &Arc<AtomicBool>) -> anyhow::Result<Opened> {
        let progress = progress.clone();
        opened(self.source, reg, lib, Arc::new(move |stage| progress.set(stage)), cancel)
    }
}

/// [`Template::open`] for a bundled template graph, opened as its graph even where a starter of its name exists.
pub fn open_graph(graph: &'static TemplateGraph, reg: Arc<Registry>, lib: Arc<AlphaLibrary>, wake: impl Fn() + Send + Sync + 'static) -> Opening {
    open(graph.name, Source::Graph(graph), reg, lib, wake)
}

fn open(name: &'static str, source: Source, reg: Arc<Registry>, lib: Arc<AlphaLibrary>, wake: impl Fn() + Send + Sync + 'static) -> Opening {
    let progress = Arc::new(Progress::default());
    let cancelled = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let (tx, answer) = mpsc::channel();
    let (at, stop, done) = (progress.clone(), cancelled.clone(), finished.clone());
    let wake = Arc::new(wake);
    let woken = wake.clone();
    let set: Arc<dyn Fn(Stage) + Send + Sync> = Arc::new(move |stage| {
        at.set(stage);
        woken();
    });
    let spawned = std::thread::Builder::new().name("template-open".into()).spawn(move || {
        let opened = opened(source, &reg, &lib, set, &stop);
        if !stop.load(Ordering::Relaxed) {
            let _ = tx.send(opened.map_err(|e| format!("{e:#}")));
        }
        done.store(true, Ordering::Relaxed);
        wake();
    });
    if let Err(e) = spawned {
        let (tx, failed) = mpsc::channel();
        let _ = tx.send(Err(format!("no thread to open it on: {e}")));
        finished.store(true, Ordering::Relaxed);
        return Opening { name, progress, answer: failed, cancelled, finished };
    }
    Opening { name, progress, answer, cancelled, finished }
}

/// The design `source` makes with `lib` and its artwork baked in, telling `set` each stage; `Err` once `stop` is set.
fn opened(source: Source, reg: &Registry, lib: &Arc<AlphaLibrary>, set: Arc<dyn Fn(Stage) + Send + Sync>, stop: &Arc<AtomicBool>) -> anyhow::Result<Opened> {
    let stopped = || anyhow::anyhow!(ringdesign_graph::eval::CANCELLED);
    // A graph runs with expression pins attached, and its evaluation travels on as the first build's seed.
    let evaluated = |open: &dyn Fn(&mut Evaluator, Arc<dyn Fn(Step) + Send + Sync>) -> Result<ringdesign_graph::templates::OpenedGraph, ringdesign_graph::graph::GraphError>| {
        let mut evaluator = Evaluator::with_exprs(ringdesign_script::engine());
        let told = set.clone();
        let opened = open(&mut evaluator, Arc::new(move |s| told(Stage::from(s))))
            .map_err(|e| if e.message == ringdesign_graph::eval::CANCELLED { stopped() } else { anyhow::Error::from(e) })?;
        let after = opened.library.clone().unwrap_or_else(|| lib.clone());
        let seed = Seed { evaluator, design: opened.evaluated, report: opened.report, graph: opened.graph, base: lib.clone() };
        Ok::<_, anyhow::Error>(Opened { design: opened.design, before: lib.clone(), after, artwork: opened.artwork, seed: Some(seed) })
    };
    let design = match source {
        Source::Graph(graph) => return evaluated(&|ev, step| graph.open(reg, lib, ev, step, stop)),
        Source::Document(graph) => {
            set(Stage::Reading);
            return evaluated(&|ev, step| ringdesign_graph::templates::open_document(graph.clone(), reg, lib, ev, step, stop));
        }
        Source::Starter(template) => {
            set(Stage::Reading);
            template.design()
        }
        Source::Design(slug) => {
            set(Stage::Reading);
            let asset = ringdesign_assets::find(ringdesign_assets::DESIGNS, slug).ok_or_else(|| anyhow::anyhow!("{slug} is not bundled"))?;
            ringdesign_graph::templates::refine_sources(&serde_json::from_str(&asset.text())?)
        }
    };
    anyhow::ensure!(!stop.load(Ordering::Relaxed), stopped());
    let mut baked = (**lib).clone();
    let artwork = design.unpack_and_bake_observed(&mut baked, &|done, of| set(Stage::Baking { done, of }), stop).ok_or_else(stopped)?;
    let after = if baked.revision() == lib.revision() { lib.clone() } else { Arc::new(baked) };
    Ok(Opened { design, before: lib.clone(), after, artwork, seed: None })
}

/// How far a template being opened has got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Reading,
    Evaluating { done: usize, of: usize },
    Baking { done: usize, of: usize },
    /// Landed, and its ring is being built.
    Building,
}

impl From<Step> for Stage {
    fn from(step: Step) -> Self {
        match step {
            Step::Reading => Stage::Reading,
            Step::Evaluating { done, of } => Stage::Evaluating { done, of },
            Step::Baking { done, of } => Stage::Baking { done, of },
        }
    }
}

impl Stage {
    /// Share of the whole open done, 0 to 1, weighted by `template_open_probe`: reading is at most a tenth of an open,
    /// the graph up to a third, the bake up to a third and the first build the rest.
    pub fn fraction(self) -> f32 {
        let share = |done: usize, of: usize| done.min(of) as f32 / of.max(1) as f32;
        match self {
            Stage::Reading => 0.05,
            Stage::Evaluating { done, of } => 0.1 + 0.3 * share(done, of),
            Stage::Baking { done, of } => 0.4 + 0.35 * share(done, of),
            Stage::Building => 0.8,
        }
    }

    /// What the stage is doing, in words.
    pub fn words(self) -> String {
        match self {
            Stage::Reading => "reading it".into(),
            Stage::Evaluating { done, of } => format!("running its graph, node {} of {of}", (done + 1).min(of.max(1))),
            Stage::Baking { of: 0, .. } => "baking artwork".into(),
            Stage::Baking { done, of } => format!("baking artwork {} / {of}", done.min(of)),
            Stage::Building => "building the ring".into(),
        }
    }
}

/// How far an open has got, written by its thread and read every frame; the fraction only ever rises.
#[derive(Debug)]
pub struct Progress {
    stage: Mutex<Stage>,
    high: AtomicU32,
}

impl Default for Progress {
    fn default() -> Self {
        Self { stage: Mutex::new(Stage::Reading), high: AtomicU32::new(Stage::Reading.fraction().to_bits()) }
    }
}

impl Progress {
    /// Moves to `stage`; the fraction keeps the highest reached.
    pub fn set(&self, stage: Stage) {
        *self.stage.lock().unwrap_or_else(|e| e.into_inner()) = stage;
        self.high.fetch_max(stage.fraction().to_bits(), Ordering::Relaxed);
    }

    pub fn stage(&self) -> Stage {
        *self.stage.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Share done, 0 to 1, never lower than any read before.
    pub fn fraction(&self) -> f32 {
        f32::from_bits(self.high.load(Ordering::Relaxed))
    }

    /// "Opening Caiman — baking artwork 3 / 7".
    pub fn words(&self, name: &str) -> String {
        format!("Opening {name} — {}", self.stage().words())
    }
}

/// A template being opened off the UI thread; dropping it stops the thread between nodes or bakes and discards what it made.
pub struct Opening {
    /// The template's name.
    pub name: &'static str,
    progress: Arc<Progress>,
    answer: mpsc::Receiver<Result<Opened, String>>,
    cancelled: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
}

impl Opening {
    /// Where it has got.
    pub fn stage(&self) -> Stage {
        self.progress.stage()
    }

    /// How far it has got, shared so a host can carry it on past the landing.
    pub fn progress(&self) -> Arc<Progress> {
        self.progress.clone()
    }

    /// The opened template once it has landed, or why it could not be opened.
    pub fn poll(&self) -> Option<Result<Opened, String>> {
        match self.answer.try_recv() {
            Ok(opened) => Some(opened),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("the thread opening it stopped without an answer".into())),
        }
    }

    /// Stops the thread at its next node or bake; nothing lands.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// The flag [`cancel`](Self::cancel) sets, for stopping it from elsewhere.
    pub fn cancel_handle(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }

    /// Whether its thread has returned.
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }

    /// A handle that says when the thread has returned, kept after the opening itself is dropped.
    pub fn finished(&self) -> Arc<AtomicBool> {
        self.finished.clone()
    }
}

impl Drop for Opening {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// The evaluation that made a graph template's design, for the first build to take instead of evaluating again.
pub struct Seed {
    /// Its cache, keyed by `base`'s revision.
    pub evaluator: Evaluator,
    pub design: Arc<RingDesign>,
    pub report: EvalReport,
    pub graph: Graph,
    /// The library it was evaluated against, before its artwork.
    pub base: Arc<AlphaLibrary>,
}

/// A template's design, and the library it was opened against with its artwork baked in.
pub struct Opened {
    pub design: RingDesign,
    before: Arc<AlphaLibrary>,
    after: Arc<AlphaLibrary>,
    artwork: Vec<Arc<Alpha>>,
    seed: Option<Seed>,
}

impl Opened {
    /// `current` with the template's artwork: the baked library itself while `current` is still the one it was opened against, else the artwork baked again onto `current`, which shares every raster the thread made.
    pub fn library_for(&self, current: &Arc<AlphaLibrary>) -> Arc<AlphaLibrary> {
        if Arc::ptr_eq(current, &self.before) {
            return self.after.clone();
        }
        let mut lib = (**current).clone();
        for alpha in &self.artwork {
            lib.insert_shared(alpha.clone());
        }
        self.design.bake_sdfs(&mut lib);
        if lib.revision() == current.revision() { current.clone() } else { Arc::new(lib) }
    }

    /// The design, the library it lands with over `current`, and the evaluation that made it while that library is the one it was made with.
    pub fn land(self, current: &Arc<AlphaLibrary>) -> (RingDesign, Arc<AlphaLibrary>, Option<Seed>) {
        let lib = self.library_for(current);
        let seed = self.seed.filter(|_| Arc::ptr_eq(current, &self.before));
        (self.design, lib, seed)
    }
}

/// The bar a template being opened shows: its name, how far it has got and what it is doing.
pub fn progress(ui: &mut Ui, name: &str, progress: &Progress) -> egui::Response {
    ui.add(egui::ProgressBar::new(progress.fraction()).text(progress.words(name)).animate(true))
}

/// A host's one template opening at a time, and the one that landed until a build shows it.
#[derive(Default)]
pub struct Slot {
    opening: Option<(Opening, bool)>,
    building: Option<(&'static str, u64, Arc<Progress>)>,
}

/// What a frame's poll of a [`Slot`] found.
pub enum Polled {
    Idle,
    /// Still opening, in words for the status line.
    Waiting(String),
    Landed(Landing),
    /// It could not be opened, in words.
    Failed(String),
}

/// A template that landed over the host's library.
pub struct Landing {
    pub name: &'static str,
    pub design: RingDesign,
    /// The host's library with the template's artwork baked in.
    pub lib: Arc<AlphaLibrary>,
    /// The evaluation that made the design, while it was made against that library.
    pub seed: Option<Seed>,
    /// What the host opened it with.
    pub flag: bool,
    progress: Arc<Progress>,
}

impl Slot {
    /// Opens `opening`'s template in place of any still opening, which stops; `flag` comes back with it.
    pub fn start(&mut self, opening: Opening, flag: bool) {
        self.opening = Some((opening, flag));
        self.building = None;
    }

    /// Stops the template opening; what it makes is dropped. Its name, when one was.
    pub fn cancel(&mut self) -> Option<&'static str> {
        self.opening.take().map(|(opening, _)| opening.name)
    }

    /// Whether a template is opening.
    pub fn is_opening(&self) -> bool {
        self.opening.is_some()
    }

    /// The flag that says when the opening thread has returned.
    pub fn finished(&self) -> Option<Arc<AtomicBool>> {
        self.opening.as_ref().map(|(opening, _)| opening.finished())
    }

    /// A template that landed, baked onto `current`, or where the one opening has got.
    pub fn poll(&mut self, current: &Arc<AlphaLibrary>) -> Polled {
        let Some((opening, _)) = &self.opening else { return Polled::Idle };
        let Some(answer) = opening.poll() else { return Polled::Waiting(opening.progress().words(opening.name)) };
        let Some((opening, flag)) = self.opening.take() else { return Polled::Idle };
        match answer {
            Ok(opened) => {
                let (design, lib, seed) = opened.land(current);
                Polled::Landed(Landing { name: opening.name, design, lib, seed, flag, progress: opening.progress() })
            }
            Err(e) => Polled::Failed(format!("could not open {}: {e}", opening.name)),
        }
    }

    /// `landing` is shown once a build after `generation` lands.
    pub fn building(&mut self, landing: &Landing, generation: u64) {
        landing.progress.set(Stage::Building);
        self.building = Some((landing.name, generation, landing.progress.clone()));
    }

    /// A build of `generation` landed or failed.
    pub fn built(&mut self, generation: u64) {
        if self.building.as_ref().is_some_and(|(_, g, _)| generation > *g) {
            self.building = None;
        }
    }

    /// The template the plate names and how far it has got, while one is opening or building.
    pub fn shown(&self) -> Option<(&'static str, Arc<Progress>)> {
        match (&self.opening, &self.building) {
            (Some((opening, _)), _) => Some((opening.name, opening.progress())),
            (None, Some((name, _, progress))) => Some((name, progress.clone())),
            (None, None) => None,
        }
    }

    /// The touch plate, centred under the top of `bounds`: the bar and, while it opens, a finger-sized Cancel that stops it. The name it stopped.
    pub fn plate(&mut self, ctx: &egui::Context, bounds: egui::Rect) -> Option<&'static str> {
        let (name, told) = self.shown()?;
        let building = told.stage() == Stage::Building;
        let width = (bounds.width() - 32.0).clamp(160.0, 420.0);
        let mut cancel = false;
        egui::Area::new(egui::Id::new("template-open"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::CENTER_TOP)
            .fixed_pos(bounds.center_top() + egui::vec2(0.0, 64.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(width);
                    ui.add_sized([width, 28.0], |ui: &mut Ui| progress(ui, name, &told));
                    if !building {
                        cancel = ui.add_sized([width, crate::touch::TARGET_PT], egui::Button::new("Cancel")).clicked();
                    }
                });
            });
        if cancel { self.cancel() } else { None }
    }
}

pub struct Collection { pub name: &'static str, pub templates: Vec<Template> }

pub fn collections() -> &'static [Collection] {
    static CATALOG: LazyLock<Vec<Collection>> = LazyLock::new(|| {
        let group = |name, slugs: &[&str], description| Collection {
            name,
            templates: slugs.iter().map(|slug| {
                let t = ringdesign_graph::templates::catalog().find(|t| t.slug == *slug).expect("catalogued graph");
                let source = ringdesign_core::templates::all().iter().find(|starter| starter.name == t.name)
                    .map(Source::Starter).unwrap_or(Source::Graph(t));
                Template { name: t.name, slug: t.slug, description, source }
            }).collect(),
        };
        vec![
            group("Reptilia collection", &["ecdysis-reptilia", "tessera-reptilia", "lorica-reptilia", "ophidian-reptilia", "varanus-reptilia"], "Sculpted reptile skins with editable artwork and geometry."),
            group("Stock masterworks", &["nocturne-imported", "solstice-imported", "aurelia-imported", "vesper-imported", "saurian-imported", "zenith-imported", "caiman-imported"], "Authored ornament on calibrated imported signet stock."),
            Collection { name: "Workshop collection", templates: vec![
                Template { name: "Aster — cushion seal", slug: "aster-workshop", description: "Editable workshop design with a nominal 18.2 mm bore.", source: Source::Design("aster-workshop") },
                Template { name: "Tide — twelve reeds", slug: "tide-workshop", description: "Editable workshop design with a nominal 18.2 mm bore.", source: Source::Design("tide-workshop") },
                Template { name: "Lantern — pierced octagonal signet", slug: "lantern-workshop", description: "Editable CAD assembly; use the CAD workspace for its feature history.", source: Source::Design("lantern-workshop") },
                Template { name: "Aureole — half-turn ribbon", slug: "aureole-workshop", description: "Editable CAD assembly; use the CAD workspace for its feature history.", source: Source::Design("aureole-workshop") },
            ] },
            {
                let mut atelier = group("Atelier designs", &["aster-atelier", "thalassa", "oriel"], "Complete authored designs with their artwork and settings.");
                atelier.templates.push(Template { name: "Aster — original botanical signet", slug: "aster-botanical", description: "The original botanical sand signet.", source: Source::Design("aster-botanical") });
                atelier
            },
            group("Original masterwork signets", &["nocturne", "solstice"], "Original sculpted designs, before the stock-based editions."),
            group("Starter bands", &["court-band", "braided-band", "wishbone-wave", "split-shank"], "A simple parametric starting point for a band."),
            group("Starter signets", &["heart-signet", "waved-hexagon-signet", "shouldered-cushion-signet"], "A parametric signet with an editable head and shank."),
            group("Stone settings", &["cathedral-solitaire-stock", "toi-et-moi"], "Starter rings with editable stone settings."),
        ]
    });
    &CATALOG
}

pub fn preview_bytes(slug: &str) -> Option<&'static [u8]> {
    Some(match slug {
        "court-band" => include_bytes!("../assets/templates/court-band.png"),
        "heart-signet" => include_bytes!("../assets/templates/heart-signet.png"),
        "waved-hexagon-signet" => include_bytes!("../assets/templates/waved-hexagon-signet.png"),
        "shouldered-cushion-signet" => include_bytes!("../assets/templates/shouldered-cushion-signet.png"),
        "braided-band" => include_bytes!("../assets/templates/braided-band.png"),
        "cathedral-solitaire-stock" => include_bytes!("../assets/templates/cathedral-solitaire-stock.png"),
        "wishbone-wave" => include_bytes!("../assets/templates/wishbone-wave.png"),
        "split-shank" => include_bytes!("../assets/templates/split-shank.png"),
        "toi-et-moi" => include_bytes!("../assets/templates/toi-et-moi.png"),
        "nocturne" => include_bytes!("../assets/templates/nocturne.png"),
        "solstice" => include_bytes!("../assets/templates/solstice.png"),
        "aster-atelier" => include_bytes!("../assets/templates/aster-atelier.png"),
        "thalassa" => include_bytes!("../assets/templates/thalassa.png"),
        "oriel" => include_bytes!("../assets/templates/oriel.png"),
        "nocturne-imported" => include_bytes!("../assets/templates/nocturne-imported.png"),
        "solstice-imported" => include_bytes!("../assets/templates/solstice-imported.png"),
        "aurelia-imported" => include_bytes!("../assets/templates/aurelia-imported.png"),
        "vesper-imported" => include_bytes!("../assets/templates/vesper-imported.png"),
        "saurian-imported" => include_bytes!("../assets/templates/saurian-imported.png"),
        "zenith-imported" => include_bytes!("../assets/templates/zenith-imported.png"),
        "caiman-imported" => include_bytes!("../assets/templates/caiman-imported.png"),
        "ecdysis-reptilia" => include_bytes!("../assets/templates/ecdysis-reptilia.png"),
        "tessera-reptilia" => include_bytes!("../assets/templates/tessera-reptilia.png"),
        "lorica-reptilia" => include_bytes!("../assets/templates/lorica-reptilia.png"),
        "ophidian-reptilia" => include_bytes!("../assets/templates/ophidian-reptilia.png"),
        "varanus-reptilia" => include_bytes!("../assets/templates/varanus-reptilia.png"),
        "aster-botanical" => include_bytes!("../assets/templates/aster-botanical.png"),
        "aster-workshop" => include_bytes!("../assets/templates/aster-workshop.png"),
        "tide-workshop" => include_bytes!("../assets/templates/tide-workshop.png"),
        "lantern-workshop" => include_bytes!("../assets/templates/lantern-workshop.png"),
        "aureole-workshop" => include_bytes!("../assets/templates/aureole-workshop.png"),
        _ => return None,
    })
}

fn thumbnail(ui: &Ui, template: &Template) -> TextureHandle {
    let id = egui::Id::new(("template-thumbnail", template.slug));
    if let Some(texture) = ui.ctx().data(|data| data.get_temp::<TextureHandle>(id)) { return texture; }
    let image = image::load_from_memory(preview_bytes(template.slug).expect("template preview")).expect("valid bundled preview").into_rgba8();
    let texture = ui.ctx().load_texture(template.slug, egui::ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], &image), egui::TextureOptions::LINEAR);
    ui.ctx().data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}

/// Nested collection menus keep all authored templates reachable without parsing designs per frame.
pub fn menu(ui: &mut Ui) -> Option<&'static Template> {
    let mut chosen = None;
    for collection in collections() {
        let thumb = thumbnail(ui, &collection.templates[0]);
        let size = (ui.spacing().interact_size.y - ui.spacing().button_padding.y * 2.).min(24.);
        ui.menu_button((egui::Image::new((thumb.id(), egui::vec2(size, size))), collection.name), |ui| {
            egui::ScrollArea::vertical().max_height((ui.ctx().content_rect().height() * 0.7).min(520.)).show(ui, |ui| {
                for template in &collection.templates {
                    let thumb = thumbnail(ui, template);
                    let response = ui.add(egui::Button::new((egui::Image::new((thumb.id(), egui::vec2(size, size))), template.name)).image_tint_follows_text_color(false));
                    if response.clicked() { chosen = Some(template); ui.close(); }
                    response.on_hover_ui(|ui| {
                        ui.image((thumb.id(), egui::vec2(160., 160.)));
                        ui.label(template.description);
                    });
                }
            });
        });
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_authored_graph_and_collection_has_a_real_preview() {
        let entries: Vec<_> = collections().iter().flat_map(|c| &c.templates).collect();
        let slugs: std::collections::HashSet<_> = entries.iter().map(|t| t.slug).collect();
        assert_eq!(entries.len(), slugs.len(), "templates appear once");
        for graph in ringdesign_graph::templates::catalog() { assert!(slugs.contains(graph.slug), "{} absent from menu", graph.slug); }
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../graphs/templates");
        for file in std::fs::read_dir(directory).unwrap() {
            let file = file.unwrap().file_name().to_string_lossy().into_owned();
            if let Some(slug) = file.strip_suffix(".graph.json") { assert!(slugs.contains(slug), "{slug} is authored but unavailable"); }
        }
        for entry in entries {
            let png = image::load_from_memory(preview_bytes(entry.slug).expect(entry.slug)).unwrap().to_rgb8();
            assert_eq!((png.width(), png.height()), (160, 160));
            assert!(png.pixels().any(|p| p.0.iter().copied().max().unwrap() > 100), "{} preview is blank", entry.slug);
        }
        assert_eq!(slugs.len(), 31);
    }

    #[test]
    fn a_template_opens_off_the_ui_thread_as_it_instantiates_with_its_artwork_baked_as_the_ui_thread_baked_it() {
        let reg = Arc::new(ringdesign_script::registry());
        let lib = Arc::new(AlphaLibrary::builtin());
        let find = |slug: &str| collections().iter().flat_map(|c| &c.templates).find(|t| t.slug == slug).unwrap();
        // A graph carrying artwork, a starter, and a bundled design.
        for slug in ["aster-atelier", "court-band", "aster-workshop"] {
            let template = find(slug);
            let wakes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let woken = wakes.clone();
            let opening = template.open(reg.clone(), lib.clone(), move || {
                woken.fetch_add(1, Ordering::Relaxed);
            });
            let started = std::time::Instant::now();
            let opened = loop {
                if let Some(opened) = opening.poll() {
                    break opened.unwrap();
                }
                assert!(started.elapsed().as_secs() < 60, "{slug} never landed at {:?}", opening.stage());
                std::thread::sleep(std::time::Duration::from_millis(2));
            };
            assert!(wakes.load(Ordering::Relaxed) >= 2, "{slug} wakes the UI as it goes and when it lands");
            assert!(opening.is_finished() || { std::thread::sleep(std::time::Duration::from_millis(50)); opening.is_finished() });
            assert_eq!(serde_json::to_value(&opened.design).unwrap(), serde_json::to_value(template.instantiate(&reg, &lib).unwrap()).unwrap(), "{slug}");
            assert_eq!(opened.seed.is_some(), slug == "aster-atelier", "{slug}: only a graph carries its evaluation on");
            // What the UI thread used to bake, the thread baked: every alpha it inserted, texel for texel.
            let mut old = (*lib).clone();
            opened.design.unpack_embedded(&mut old);
            opened.design.bake_all(&mut old);
            let landed = opened.library_for(&lib);
            assert!(Arc::ptr_eq(&landed, &opened.after));
            let inserted: Vec<&Arc<ringdesign_core::alpha::Alpha>> = old.changed_since(&lib).collect();
            assert_eq!(inserted.len(), opened.after.changed_since(&lib).count(), "{slug}");
            for alpha in &inserted {
                assert_eq!(landed.get(&alpha.name).map(|a| &a.data), Some(&alpha.data), "{slug}: {}", alpha.name);
            }
            // A library changed while it opened keeps its own entries and gains the template's, shared.
            let mut moved = (*lib).clone();
            moved.insert(ringdesign_core::alpha::Alpha::new("mine", 2, 2, vec![0.5; 4]));
            // An alpha the old design redrew under one of the template's own names while it opened.
            if let Some(first) = opened.artwork.first() {
                moved.insert(ringdesign_core::alpha::Alpha::new(first.name.clone(), 2, 2, vec![0.9; 4]));
            }
            let moved = Arc::new(moved);
            let merged = opened.library_for(&moved);
            assert!(merged.get("mine").is_some());
            for alpha in &inserted {
                assert!(std::ptr::eq(merged.get(&alpha.name).unwrap(), opened.after.get(&alpha.name).unwrap()), "{slug}: {}", alpha.name);
            }
            let (_, lib_after, seed) = opened.land(&moved);
            assert!(lib_after.get("mine").is_some());
            assert!(seed.is_none(), "{slug}: an evaluation against a library that has moved is not carried on");
        }
    }

    /// The phone's flow through a slot: the second choice stops the first, the plate says how far it has got, the landing is baked onto the library as it stands.
    #[test]
    fn a_slot_lands_the_last_choice_and_holds_its_plate_until_a_later_build() {
        let reg = Arc::new(ringdesign_script::registry());
        let lib = Arc::new(AlphaLibrary::builtin());
        let find = |slug: &str| collections().iter().flat_map(|c| &c.templates).find(|t| t.slug == slug).unwrap();
        let ctx = egui::Context::default();
        let mut slot = Slot::default();
        assert!(matches!(slot.poll(&lib), Polled::Idle));
        slot.start(find("caiman-imported").open(reg.clone(), lib.clone(), || {}), false);
        let first = slot.finished().expect("opening");
        slot.start(find("nocturne").open(reg.clone(), lib.clone(), || {}), true);
        let started = std::time::Instant::now();
        let mut read = Vec::new();
        let landing = loop {
            ctx.run_ui(egui::RawInput::default(), |ui| assert_eq!(slot.plate(ui.ctx(), ui.ctx().content_rect()), None)).textures_delta.clear();
            read.extend(slot.shown().map(|(_, p)| p.fraction()));
            match slot.poll(&lib) {
                Polled::Landed(landing) => break landing,
                Polled::Waiting(words) => assert!(words.starts_with("Opening Nocturne — original night garden — "), "{words}"),
                Polled::Idle => panic!("idle while opening"),
                Polled::Failed(why) => panic!("{why}"),
            }
            assert!(started.elapsed().as_secs() < 60);
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        assert!(first.load(Ordering::Relaxed), "the replaced open stopped");
        assert!(read.windows(2).all(|w| w[0] <= w[1]), "{read:?}");
        assert!(landing.flag && landing.name == "Nocturne — original night garden");
        assert!(landing.design.name.starts_with("Nocturne") && landing.lib.get("Palmette").is_some() && landing.seed.is_some());
        assert!(!slot.is_opening() && slot.shown().is_none());
        slot.building(&landing, 7);
        let (_, progress) = slot.shown().expect("the plate stays up");
        assert_eq!((progress.stage(), progress.fraction()), (Stage::Building, 0.8));
        slot.built(7);
        assert!(slot.shown().is_some(), "a build dispatched before the landing does not show it");
        slot.built(8);
        assert!(slot.shown().is_none());
    }

    #[test]
    fn the_touch_plates_cancel_stops_the_open() {
        use egui_kittest::kittest::{NodeT, Queryable};
        let reg = Arc::new(ringdesign_script::registry());
        let lib = Arc::new(AlphaLibrary::builtin());
        let template = collections().iter().flat_map(|c| &c.templates).find(|t| t.slug == "caiman-imported").unwrap();
        let mut slot = Slot::default();
        slot.start(template.open(reg, lib, || {}), false);
        let finished = slot.finished().unwrap();
        let mut h = egui_kittest::Harness::builder().with_size([400.0, 800.0]).build_ui_state(
            |ui, (slot, stopped): &mut (Slot, Option<&'static str>)| {
                let rect = ui.ctx().content_rect();
                if let Some(name) = slot.plate(ui.ctx(), rect) {
                    *stopped = Some(name);
                }
            },
            (slot, None),
        );
        h.run_steps(2);
        let bar = h.query_all_by_label_contains("Opening Caiman — armoured hide — ").find(|n| n.accesskit_node().role() == egui::accesskit::Role::ProgressIndicator).expect("the bar").rect();
        let cancel = h.get_by_label("Cancel");
        assert!(cancel.rect().height() >= crate::touch::TARGET_PT - 0.5 && cancel.rect().top() >= bar.bottom(), "a finger-sized Cancel under the bar");
        cancel.click();
        h.run_steps(2);
        assert_eq!(h.state().1, Some("Caiman — armoured hide"));
        assert!(!h.state().0.is_opening() && h.query_by_label("Cancel").is_none());
        let started = std::time::Instant::now();
        while !finished.load(Ordering::Relaxed) {
            assert!(started.elapsed().as_secs() < 60, "the open never stopped");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// Court band with its width an expression.
    fn expression_template() -> &'static Template {
        let mut g = ringdesign_graph::templates::graph("Court band").unwrap();
        g.set_input(ringdesign_graph::graph::NodeId(1), "width_mm", ringdesign_graph::value::Literal::expr("2.5 + 2.0")).unwrap();
        Box::leak(Box::new(Template::document("Court band by expression", "court-band-expression", Box::leak(Box::new(g)))))
    }

    #[test]
    fn a_template_with_an_expression_pin_opens_and_carries_its_evaluation() {
        let reg = Arc::new(ringdesign_script::registry());
        let lib = Arc::new(AlphaLibrary::builtin());
        let template = expression_template();
        let Source::Document(g) = template.source else { unreachable!() };
        let bare = ringdesign_graph::templates::open_document(g.clone(), &reg, &lib, &mut Evaluator::new(), Arc::new(|_| {}), &Arc::default())
            .err()
            .expect("without an engine the pin fails");
        assert!(bare.message.contains("no expression engine"), "{bare}");
        let opening = template.open(reg.clone(), lib.clone(), || {});
        let started = std::time::Instant::now();
        let opened = loop {
            if let Some(opened) = opening.poll() {
                break opened.expect("opens with the script engine attached");
            }
            assert!(started.elapsed().as_secs() < 60);
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        assert_eq!(opened.design.profile.width_mm, 4.5);
        assert_eq!(template.instantiate(&reg, &lib).unwrap().profile.width_mm, 4.5);
        let seed = opened.seed.as_ref().expect("its evaluation");
        assert_eq!(seed.design.profile.width_mm, 4.5);
        assert_eq!(seed.evaluator.cached_nodes(), 3);
        assert!(Arc::ptr_eq(&seed.base, &lib));
    }

    #[test]
    fn the_fraction_never_falls_and_the_words_say_where_it_is() {
        let stages = [Stage::Reading, Stage::Evaluating { done: 0, of: 40 }, Stage::Evaluating { done: 39, of: 40 }, Stage::Baking { done: 0, of: 7 }, Stage::Baking { done: 7, of: 7 }, Stage::Building];
        assert!(stages.windows(2).all(|w| w[0].fraction() < w[1].fraction()), "{stages:?}");
        assert_eq!(Stage::Evaluating { done: 39, of: 40 }.words(), "running its graph, node 40 of 40");
        assert_eq!(Stage::Evaluating { done: 0, of: 0 }.fraction(), 0.1);
        let progress = Progress::default();
        progress.set(Stage::Baking { done: 3, of: 7 });
        assert_eq!(progress.words("Caiman"), "Opening Caiman — baking artwork 3 / 7");
        let high = progress.fraction();
        progress.set(Stage::Evaluating { done: 1, of: 40 });
        assert_eq!(progress.fraction(), high, "a stage told late never pulls the bar back");
        progress.set(Stage::Building);
        assert_eq!(progress.fraction(), 0.8);
        // An open with a graph and artwork, read as often as a frame would read it.
        let reg = Arc::new(ringdesign_script::registry());
        let lib = Arc::new(AlphaLibrary::builtin());
        let template = collections().iter().flat_map(|c| &c.templates).find(|t| t.slug == "nocturne").unwrap();
        let opening = template.open(reg, lib, || {});
        let progress = opening.progress();
        let mut read = vec![progress.fraction()];
        let started = std::time::Instant::now();
        while opening.poll().is_none() {
            read.push(progress.fraction());
            assert!(started.elapsed().as_secs() < 60);
        }
        read.push(progress.fraction());
        assert!(read.windows(2).all(|w| w[0] <= w[1]), "{read:?}");
        assert_eq!(*read.last().unwrap(), Stage::Baking { done: 1, of: 1 }.fraction());
    }

    #[test]
    fn a_cancelled_open_stops_and_lands_nothing() {
        let reg = Arc::new(ringdesign_script::registry());
        let lib = Arc::new(AlphaLibrary::builtin());
        let find = |slug: &str| collections().iter().flat_map(|c| &c.templates).find(|t| t.slug == slug).unwrap();
        // Cancelled before it starts, the calling thread's open says so and bakes nothing.
        let stop = Arc::new(AtomicBool::new(true));
        let progress = Arc::new(Progress::default());
        let err = find("caiman-imported").instantiate_with(&reg, &lib, &progress, &stop).err().expect("cancelled");
        assert_eq!(err.to_string(), ringdesign_graph::eval::CANCELLED);
        assert!(!matches!(progress.stage(), Stage::Baking { .. }));
        // Cancelled at its third node, from the thread itself, the next node never runs and nothing lands.
        let handles: Arc<std::sync::OnceLock<(Arc<Progress>, Arc<AtomicBool>)>> = Arc::default();
        let seen = handles.clone();
        let opening = find("caiman-imported").open(reg.clone(), lib.clone(), move || {
            if let Some((progress, stop)) = seen.get() {
                if matches!(progress.stage(), Stage::Evaluating { done, .. } if done >= 2) {
                    stop.store(true, Ordering::Relaxed);
                }
            }
        });
        handles.set((opening.progress(), opening.cancel_handle())).ok().expect("set once");
        let started = std::time::Instant::now();
        while !opening.is_finished() {
            assert!(started.elapsed().as_secs() < 60, "the thread never stopped");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(matches!(opening.stage(), Stage::Evaluating { done: 2, .. }), "{:?}", opening.stage());
        assert!(!matches!(opening.poll(), Some(Ok(_))));
        // Dropped, as choosing another template drops it, a running open stops too.
        let opening = find("caiman-imported").open(reg, lib, || {});
        let finished = opening.finished();
        drop(opening);
        while !finished.load(Ordering::Relaxed) {
            assert!(started.elapsed().as_secs() < 60, "the thread never stopped");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
