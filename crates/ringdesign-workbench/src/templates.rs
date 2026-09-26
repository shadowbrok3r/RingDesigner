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
    pub badge: Option<&'static str>,
    pub family: Option<&'static str>,
    pub description: &'static str,
    source: Source,
}
/// `Design` names a bundled `.ring.json`; the document is decompressed only
/// when that template is chosen, not to list it in a menu. `File` is a design file on disk.
#[derive(Clone)]
enum Source { Graph(&'static TemplateGraph), Document(&'static Graph), Starter(&'static ringdesign_core::templates::Template), Design(&'static str), File(std::path::PathBuf), Stock(&'static ringdesign_core::imported_base::Preset) }

/// What an open is called: a template's name, or a file's.
pub type Name = std::borrow::Cow<'static, str>;

impl Template {
    /// A template made from a graph document already read, opened like a bundled template graph.
    pub fn document(name: &'static str, slug: &'static str, graph: &'static Graph) -> Self {
        Template { name, slug, badge: None, family: None, description: "", source: Source::Document(graph) }
    }

    /// The template's design, made on the calling thread with expression pins run.
    pub fn instantiate(&self, reg: &Registry, lib: &AlphaLibrary) -> anyhow::Result<RingDesign> {
        let lib = Arc::new(lib.clone());
        Ok(opened(self.source.clone(), reg, &lib, Arc::new(|_| {}), &Arc::default())?.design)
    }

    /// Opens this template on a thread of its own against `lib`, calling `wake` whenever it gets further; poll the [`Opening`] for the design.
    pub fn open(&'static self, reg: Arc<Registry>, lib: Arc<AlphaLibrary>, wake: impl Fn() + Send + Sync + 'static) -> Opening {
        open(self.name.into(), self.source.clone(), reg, lib, false, wake)
    }

    /// [`open`](Self::open), measuring the design's detail findings on the thread too, for a host that reads them on its UI thread.
    pub fn open_measured(&'static self, reg: Arc<Registry>, lib: Arc<AlphaLibrary>, wake: impl Fn() + Send + Sync + 'static) -> Opening {
        open(self.name.into(), self.source.clone(), reg, lib, true, wake)
    }

    /// What [`open`](Self::open) does on its thread, on the calling one: telling `progress` each stage and stopping once `cancel` is set.
    pub fn instantiate_with(&self, reg: &Registry, lib: &Arc<AlphaLibrary>, progress: &Arc<Progress>, cancel: &Arc<AtomicBool>) -> anyhow::Result<Opened> {
        let progress = progress.clone();
        opened(self.source.clone(), reg, lib, Arc::new(move |stage| progress.set(stage)), cancel)
    }
}

/// [`Template::open`] for a bundled template graph, opened as its graph even where a starter of its name exists.
pub fn open_graph(graph: &'static TemplateGraph, reg: Arc<Registry>, lib: Arc<AlphaLibrary>, wake: impl Fn() + Send + Sync + 'static) -> Opening {
    open(graph.name.into(), Source::Graph(graph), reg, lib, false, wake)
}

/// A design file read, migrated and its artwork baked onto `lib` on a thread of its own, named by its file; `measured` as in [`Template::open_measured`].
pub fn open_file(path: std::path::PathBuf, lib: Arc<AlphaLibrary>, measured: bool, wake: impl Fn() + Send + Sync + 'static) -> Opening {
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    open(name.into(), Source::File(path), Arc::new(Registry::empty()), lib, measured, wake)
}

fn open(name: Name, source: Source, reg: Arc<Registry>, lib: Arc<AlphaLibrary>, measured: bool, wake: impl Fn() + Send + Sync + 'static) -> Opening {
    let progress = Arc::new(Progress::default());
    let cancelled = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let (tx, answer) = mpsc::channel();
    let (at, stop, done) = (progress.clone(), cancelled.clone(), finished.clone());
    let wake = Arc::new(wake);
    // The host is woken from a thread of its own, never from the pool threads a bake tells its stages on.
    let (signal, signalled) = mpsc::sync_channel::<()>(1);
    let woken = wake.clone();
    let _ = std::thread::Builder::new().name("template-wake".into()).spawn(move || {
        while signalled.recv().is_ok() {
            woken();
        }
    });
    let set: Arc<dyn Fn(Stage) + Send + Sync> = Arc::new(move |stage| {
        at.set(stage);
        let _ = signal.try_send(());
    });
    let spawned = std::thread::Builder::new().name("template-open".into()).spawn(move || {
        let opened = opened(source, &reg, &lib, set.clone(), &stop).and_then(|o| {
            if measured {
                anyhow::ensure!(!stop.load(Ordering::Relaxed), ringdesign_graph::eval::CANCELLED);
                set(Stage::Measuring);
                // Fills the detail measure's content cache before the design lands, given up once cancelled.
                ringdesign_core::alpha::measuring_until(&stop, || ringdesign_core::dfm::findings_in(&o.design, &o.after));
                anyhow::ensure!(!stop.load(Ordering::Relaxed), ringdesign_graph::eval::CANCELLED);
            }
            Ok(o)
        });
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
        let graph = Arc::new(opened.graph);
        let seed = Seed { evaluator, design: opened.evaluated, report: opened.report, graph: graph.clone(), base: lib.clone() };
        Ok::<_, anyhow::Error>(Opened { design: opened.design, before: lib.clone(), after, artwork: opened.artwork, seed: Some(seed), graph: Some(graph) })
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
        Source::Stock(preset) => {
            set(Stage::Reading);
            ringdesign_core::templates::stock(preset)?
        }
        Source::Design(slug) => {
            set(Stage::Reading);
            let asset = ringdesign_assets::find(ringdesign_assets::DESIGNS, slug).ok_or_else(|| anyhow::anyhow!("{slug} is not bundled"))?;
            ringdesign_graph::templates::refine_sources(&serde_json::from_str(&asset.text())?)
        }
        Source::File(path) => {
            set(Stage::Reading);
            ringdesign_core::library::load_design(&path)?
        }
    };
    anyhow::ensure!(!stop.load(Ordering::Relaxed), stopped());
    let mut baked = (**lib).clone();
    let artwork = design.unpack_and_bake_observed(&mut baked, &|done, of| set(Stage::Baking { done, of }), stop).ok_or_else(stopped)?;
    let after = if baked.revision() == lib.revision() { lib.clone() } else { Arc::new(baked) };
    anyhow::ensure!(!stop.load(Ordering::Relaxed), stopped());
    let graph = design.graph.as_ref().and_then(|j| <Graph as serde::Deserialize>::deserialize(j).ok()).map(Arc::new);
    Ok(Opened { design, before: lib.clone(), after, artwork, seed: None, graph })
}

/// How far a template being opened has got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Reading,
    Evaluating { done: usize, of: usize },
    Baking { done: usize, of: usize },
    /// Measuring the design's detail against the sand's floor.
    Measuring,
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
    /// Share of the whole open done, 0 to 1, weighted by `template_open_probe`'s phases.
    pub fn fraction(self) -> f32 {
        let share = |done: usize, of: usize| done.min(of) as f32 / of.max(1) as f32;
        match self {
            Stage::Reading => 0.05,
            Stage::Evaluating { done, of } => 0.1 + 0.3 * share(done, of),
            Stage::Baking { done, of } => 0.4 + 0.35 * share(done, of),
            Stage::Measuring => 0.78,
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
            Stage::Measuring => "measuring its detail".into(),
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
    /// The template's name, or the file's.
    pub name: Name,
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
    pub graph: Arc<Graph>,
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
    graph: Option<Arc<Graph>>,
}

impl Opened {
    /// The design's graph, read on the opening thread; `None` for a design without one, or one that does not read.
    pub fn graph(&self) -> Option<&Arc<Graph>> {
        self.graph.as_ref()
    }

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

/// A host's one template or file opening at a time, and the one that landed until a build shows it; `T` is where the host said it lands.
pub struct Slot<T = bool> {
    opening: Option<(Opening, T)>,
    building: Option<(Name, u64, Arc<Progress>)>,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Slot { opening: None, building: None }
    }
}

/// What a frame's poll of a [`Slot`] found.
pub enum Polled<T = bool> {
    Idle,
    /// Still opening, in words for the status line.
    Waiting(String),
    Landed(Landing<T>),
    /// It could not be opened, in words.
    Failed(String),
}

/// A template or file that landed over the host's library.
pub struct Landing<T = bool> {
    pub name: Name,
    pub design: RingDesign,
    /// The host's library with the design's artwork baked in.
    pub lib: Arc<AlphaLibrary>,
    /// The evaluation that made the design, while it was made against that library.
    pub seed: Option<Seed>,
    /// The design's graph, read on the opening thread.
    pub graph: Option<Arc<Graph>>,
    /// Where the host said it lands.
    pub lands: T,
    progress: Arc<Progress>,
}

impl<T> Slot<T> {
    /// Opens `opening` in place of any still opening, which stops; `lands` comes back with it.
    pub fn start(&mut self, opening: Opening, lands: T) {
        self.opening = Some((opening, lands));
        self.building = None;
    }

    /// Stops what is opening, dropping what it makes, and the plate of one that landed. Its name, when one was opening.
    pub fn cancel(&mut self) -> Option<Name> {
        self.building = None;
        self.opening.take().map(|(opening, _)| opening.name.clone())
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
    pub fn poll(&mut self, current: &Arc<AlphaLibrary>) -> Polled<T> {
        let Some((opening, _)) = &self.opening else { return Polled::Idle };
        let Some(answer) = opening.poll() else { return Polled::Waiting(opening.progress().words(&opening.name)) };
        let Some((opening, lands)) = self.opening.take() else { return Polled::Idle };
        match answer {
            Ok(opened) => {
                let graph = opened.graph().cloned();
                let (design, lib, seed) = opened.land(current);
                Polled::Landed(Landing { name: opening.name.clone(), design, lib, seed, graph, lands, progress: opening.progress() })
            }
            Err(e) => Polled::Failed(format!("could not open {}: {e}", opening.name)),
        }
    }

    /// `landing` is shown once a build after `generation` lands.
    pub fn building(&mut self, landing: &Landing<T>, generation: u64) {
        landing.progress.set(Stage::Building);
        self.building = Some((landing.name.clone(), generation, landing.progress.clone()));
    }

    /// A build of `generation` landed or failed.
    pub fn built(&mut self, generation: u64) {
        if self.building.as_ref().is_some_and(|(_, g, _)| generation > *g) {
            self.building = None;
        }
    }

    /// The template the plate names and how far it has got, while one is opening or building.
    pub fn shown(&self) -> Option<(Name, Arc<Progress>)> {
        match (&self.opening, &self.building) {
            (Some((opening, _)), _) => Some((opening.name.clone(), opening.progress())),
            (None, Some((name, _, progress))) => Some((name.clone(), progress.clone())),
            (None, None) => None,
        }
    }

    /// The touch plate, centred under the top of `bounds`: the bar and, while it opens, a finger-sized Cancel that stops it. The name it stopped.
    pub fn plate(&mut self, ctx: &egui::Context, bounds: egui::Rect) -> Option<Name> {
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
                    ui.add_sized([width, 28.0], |ui: &mut Ui| progress(ui, &name, &told));
                    if !building {
                        cancel = ui.add_sized([width, crate::touch::TARGET_PT], egui::Button::new("Cancel")).clicked();
                    }
                });
            });
        if cancel { self.cancel() } else { None }
    }
}

pub struct Collection { pub name: &'static str, pub templates: Vec<Template> }

struct StockText {
    preset: &'static ringdesign_core::imported_base::Preset,
    name: String,
    slug: String,
    description: String,
    family: &'static str,
}

fn stock_templates() -> Vec<Template> {
    static STOCKS: LazyLock<Vec<StockText>> = LazyLock::new(|| {
        [
            ("Round and square", &["013", "012", "001", "017", "006", "015"][..]),
            ("Shields", &["004", "014", "020", "011", "019"][..]),
            ("Lobed", &["003", "007", "005", "016", "018", "008"][..]),
            ("Pointed", &["002", "009", "010"][..]),
        ].into_iter().flat_map(|(family, ids)| ids.iter().map(move |id| {
            let preset = ringdesign_core::imported_base::PRESETS.iter().find(|p| p.id == *id).expect("factory stock");
            StockText {
                preset, family,
                name: ringdesign_core::templates::stock_name(preset),
                slug: format!("stock-{}-{}", preset.id, preset.name.to_ascii_lowercase()),
                description: format!("{} · factory stock, hard angles where wall meets face; bare, ready for a theme.{}", preset.label(), ringdesign_core::templates::stock_process_note(preset).map_or(String::new(), |note| format!(" {note}"))),
            }
        })).collect()
    });
    STOCKS.iter().map(|s| Template {
        name: &s.name, slug: &s.slug, description: &s.description, family: Some(s.family),
        badge: Some(if ringdesign_core::templates::stock_sand_ready(s.preset) { "sand-safe plan" } else if s.preset.sand_safe() { "sand trial failed · lost wax" } else { "upright · lost wax" }),
        source: Source::Stock(s.preset),
    }).collect()
}

pub fn collections() -> &'static [Collection] {
    static CATALOG: LazyLock<Vec<Collection>> = LazyLock::new(|| {
        let group = |name, slugs: &[&str], description| Collection {
            name,
            templates: slugs.iter().map(|slug| {
                let t = ringdesign_graph::templates::catalog().find(|t| t.slug == *slug).expect("catalogued graph");
                let source = ringdesign_core::templates::all().iter().find(|starter| starter.name == t.name)
                    .map(Source::Starter).unwrap_or(Source::Graph(t));
                Template { name: t.name, slug: t.slug, badge: None, family: None, description: ringdesign_core::templates::all().iter().find(|s| s.name == t.name).map_or(description, |s| s.blurb), source }
            }).collect(),
        };
        vec![
            group("Starter bands", &["court-band", "braided-band", "split-shank", "split-gallery"], "Simple bands with editable sections and open rails."),
            {
                let mut signets = group("Starter signets", &["shouldered-cushion-signet"], "A blank table, ready for a theme.");
                signets.templates.extend(stock_templates());
                signets
            },
            group("Stone settings", &["cathedral-solitaire", "bezel-solitaire", "halo", "trilogy", "toi-et-moi", "split-shank-basket", "half-eternity", "gypsy-trio"], "Eight made settings · three pour in sand, five in lost wax"),
            Collection { name: "Workshop collection", templates: vec![
                Template { name: "Aster — cushion seal", slug: "aster-workshop", description: "Editable workshop design with a nominal 18.2 mm bore.", badge: None, family: None, source: Source::Design("aster-workshop") },
                Template { name: "Tide — twelve reeds", slug: "tide-workshop", description: "Editable workshop design with a nominal 18.2 mm bore.", badge: None, family: None, source: Source::Design("tide-workshop") },
                Template { name: "Lantern — pierced octagonal signet", slug: "lantern-workshop", description: "Editable CAD assembly; use the CAD workspace for its feature history.", badge: None, family: None, source: Source::Design("lantern-workshop") },
                Template { name: "Aureole — half-turn ribbon", slug: "aureole-workshop", description: "Editable CAD assembly; use the CAD workspace for its feature history.", badge: None, family: None, source: Source::Design("aureole-workshop") },
            ] },
            group("Reptilia collection", &["ecdysis-reptilia", "tessera-reptilia", "lorica-reptilia", "ophidian-reptilia", "varanus-reptilia"], "Sculpted reptile skins with editable artwork and geometry."),
            group("Stock masterworks", &["nocturne-imported", "solstice-imported", "aurelia-imported", "vesper-imported", "saurian-imported", "zenith-imported", "caiman-imported"], "Authored ornament on calibrated imported signet stock."),
            {
                let mut atelier = group("Atelier designs", &["aster-atelier", "thalassa", "oriel"], "Complete authored designs with their artwork and settings.");
                atelier.templates.push(Template { name: "Aster — original botanical signet", slug: "aster-botanical", description: "The original botanical sand signet.", badge: None, family: None, source: Source::Design("aster-botanical") });
                atelier
            },
            group("Original masterwork signets", &["nocturne", "solstice"], "Original sculpted designs, before the stock-based editions."),
        ]
    });
    &CATALOG
}

macro_rules! thumbnails { ($($slug:literal),* $(,)?) => {
    pub const SLUGS: &[&str] = &[$($slug),*];
    pub fn preview_bytes(slug: &str) -> Option<&'static [u8]> {
        Some(match slug { $($slug => include_bytes!(concat!("../assets/templates/", $slug, ".png")),)* _ => return None })
    }
}}
thumbnails! {
    "court-band",
    "braided-band",
    "split-shank",
    "split-gallery",
    "shouldered-cushion-signet",
    "cathedral-solitaire",
    "bezel-solitaire",
    "halo",
    "trilogy",
    "toi-et-moi",
    "split-shank-basket",
    "half-eternity",
    "gypsy-trio",
    "nocturne",
    "solstice",
    "aster-atelier",
    "thalassa",
    "oriel",
    "nocturne-imported",
    "solstice-imported",
    "aurelia-imported",
    "vesper-imported",
    "saurian-imported",
    "zenith-imported",
    "caiman-imported",
    "ecdysis-reptilia",
    "tessera-reptilia",
    "lorica-reptilia",
    "ophidian-reptilia",
    "varanus-reptilia",
    "aster-botanical",
    "aster-workshop",
    "tide-workshop",
    "lantern-workshop",
    "aureole-workshop",
    "stock-001-cushion",
    "stock-002-kite",
    "stock-003-clover",
    "stock-004-shield",
    "stock-005-rosette",
    "stock-006-square",
    "stock-007-quatrefoil",
    "stock-008-heart",
    "stock-009-drop",
    "stock-010-trillion",
    "stock-011-badge",
    "stock-012-cushion",
    "stock-013-round",
    "stock-014-heater",
    "stock-015-octagon",
    "stock-016-star",
    "stock-017-tonneau",
    "stock-018-butterfly",
    "stock-019-jewel",
    "stock-020-escutcheon",
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
                let mut family = None;
                for template in &collection.templates {
                    if template.family != family {
                        family = template.family;
                        if let Some(name) = family { ui.weak(name); }
                    }
                    let thumb = thumbnail(ui, template);
                    let response = ui.horizontal(|ui| {
                        let response = ui.add(egui::Button::new((egui::Image::new((thumb.id(), egui::vec2(size, size))), template.name)).image_tint_follows_text_color(false));
                        if let Some(badge) = template.badge { ui.weak(badge); }
                        response
                    }).inner;
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
        assert_eq!(slugs.len(), 55);
        assert_eq!(slugs, SLUGS.iter().copied().collect(), "thumbnail table and menu agree");
    }

    #[test]
    fn the_starter_groups_lead_the_menu_and_each_stock_is_badged_by_its_process() {
        let names: Vec<_> = collections().iter().map(|c| c.name).collect();
        assert_eq!(names[..3], ["Starter bands", "Starter signets", "Stone settings"]);
        assert_eq!(collections()[..3].iter().map(|c| c.templates.len()).collect::<Vec<_>>(), [4, 21, 8]);
        let signets = &collections()[1].templates;
        assert_eq!(signets[0].slug, "shouldered-cushion-signet");
        let mut families = Vec::new();
        let mut badges = std::collections::BTreeMap::new();
        for t in &signets[1..] {
            let Source::Stock(preset) = &t.source else { panic!("{} is not a stock", t.name) };
            if families.last() != t.family.as_ref() { families.push(t.family.unwrap()); }
            let badge = t.badge.expect("a stock names its process");
            *badges.entry(badge).or_insert(0) += 1;
            assert_eq!(badge == "sand-safe plan", ringdesign_core::templates::stock_sand_ready(preset), "{}", t.name);
            assert_eq!(badge == "upright · lost wax", !preset.sand_safe(), "{}", t.name);
            assert!(t.description.starts_with(&preset.label()) && !t.description.ends_with(' '), "{:?}", t.description);
        }
        assert_eq!(families, ["Round and square", "Shields", "Lobed", "Pointed"]);
        assert_eq!(badges, std::collections::BTreeMap::from([("sand trial failed · lost wax", 7), ("sand-safe plan", 4), ("upright · lost wax", 9)]));
    }

    #[test]
    fn a_template_opens_off_the_ui_thread_as_it_instantiates_with_its_artwork_baked_as_the_ui_thread_baked_it() {
        let reg = Arc::new(ringdesign_script::registry());
        let lib = Arc::new(AlphaLibrary::builtin());
        let find = |slug: &str| collections().iter().flat_map(|c| &c.templates).find(|t| t.slug == slug).unwrap();
        // A graph carrying artwork, a starter, a factory stock and a bundled design.
        for slug in ["aster-atelier", "court-band", "stock-017-tonneau", "aster-botanical"] {
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

    /// A wake that holds the opening thread at its first stage until the returned release is called.
    fn held() -> (impl Fn() + Send + Sync + 'static, impl Fn()) {
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let wait = gate.clone();
        let hold = move || {
            let (open, turned) = &*wait;
            let mut open = open.lock().unwrap_or_else(|e| e.into_inner());
            let started = std::time::Instant::now();
            while !*open && started.elapsed().as_secs() < 60 {
                open = turned.wait_timeout(open, std::time::Duration::from_millis(100)).unwrap_or_else(|e| e.into_inner()).0;
            }
        };
        let release = move || {
            *gate.0.lock().unwrap_or_else(|e| e.into_inner()) = true;
            gate.1.notify_all();
        };
        (hold, release)
    }

    /// The phone's flow through a slot: the second choice stops the first before its first node, the plate says how far it has got, the landing is baked onto the library as it stands.
    #[test]
    fn a_slot_lands_the_last_choice_and_holds_its_plate_until_a_later_build() {
        let reg = Arc::new(ringdesign_script::registry());
        let lib = Arc::new(AlphaLibrary::builtin());
        let find = |slug: &str| collections().iter().flat_map(|c| &c.templates).find(|t| t.slug == slug).unwrap();
        let ctx = egui::Context::default();
        let mut slot = Slot::default();
        assert!(matches!(slot.poll(&lib), Polled::Idle));
        let (hold, release) = held();
        let first = find("caiman-imported").open(reg.clone(), lib.clone(), hold);
        let (first_progress, first_done) = (first.progress(), first.finished());
        slot.start(first, false);
        slot.start(find("nocturne").open(reg.clone(), lib.clone(), || {}), true);
        release();
        let started = std::time::Instant::now();
        while !first_done.load(Ordering::Relaxed) {
            assert!(started.elapsed().as_secs() < 60, "the replaced open never stopped");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(first_progress.stage(), Stage::Reading, "the replaced open stopped before its first node");
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
        assert!(read.windows(2).all(|w| w[0] <= w[1]), "{read:?}");
        assert!(landing.lands && landing.name == "Nocturne — original night garden");
        assert!(landing.design.name.starts_with("Nocturne") && landing.lib.get("Palmette").is_some() && landing.seed.is_some());
        assert!(!slot.is_opening() && slot.shown().is_none());
        slot.building(&landing, 7);
        let (_, progress) = slot.shown().expect("the plate stays up");
        assert_eq!((progress.stage(), progress.fraction()), (Stage::Building, 0.8));
        slot.built(7);
        assert!(slot.shown().is_some(), "a build dispatched before the landing does not show it");
        slot.built(8);
        assert!(slot.shown().is_none());
        // A document replaced before the landing's build shows takes the plate down with it.
        slot.building(&landing, 9);
        assert_eq!(slot.cancel(), None);
        assert!(slot.shown().is_none());
    }

    /// A design file whose one tiling reads a mask no other test measures.
    fn file_with_a_fresh_mask(dir: &std::path::Path) -> std::path::PathBuf {
        let n = 256;
        let ink = 0.5 + (std::process::id() % 1000) as f32 * 1e-4;
        let mask = Alpha::new("fresh-mask", n, n, (0..n * n).map(|i| if (i % n) % 11 < 3 || (i / n) % 13 < 2 { ink } else { 0.0 }).collect());
        let mut design = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let tiling = ringdesign_core::tiling::TilingLayer::default_for("fresh-mask", &design.field_context());
        design.layers.layers.push(ringdesign_core::field::LayerEntry::new("Fresh", ringdesign_core::field::Layer::Tiling(tiling)));
        let mut lib = AlphaLibrary::builtin();
        lib.insert(mask);
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("fresh.ring.json");
        ringdesign_core::library::save_design_embedded(&path, &design, &lib).unwrap();
        path
    }

    #[test]
    fn a_cancel_while_measuring_gives_the_measure_up_and_keeps_none_of_it() {
        let dir = std::env::temp_dir().join(format!("measure-cancel-{}", std::process::id()));
        let path = file_with_a_fresh_mask(&dir);
        let lib = Arc::new(AlphaLibrary::builtin());
        let handles: Arc<std::sync::OnceLock<(Arc<Progress>, Arc<AtomicBool>)>> = Arc::default();
        let measuring: Arc<Mutex<Option<std::time::Instant>>> = Arc::default();
        let (seen, at) = (handles.clone(), measuring.clone());
        let opening = open_file(path.clone(), lib.clone(), true, move || {
            if let Some((progress, stop)) = seen.get() {
                if progress.stage() == Stage::Measuring {
                    at.lock().unwrap().get_or_insert_with(std::time::Instant::now);
                    stop.store(true, Ordering::Relaxed);
                }
            }
        });
        handles.set((opening.progress(), opening.cancel_handle())).ok().expect("set once");
        let started = std::time::Instant::now();
        while !opening.is_finished() {
            assert!(started.elapsed().as_secs() < 120, "the thread never stopped");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let given_up = measuring.lock().unwrap().expect("it reached the measure").elapsed();
        assert!(!matches!(opening.poll(), Some(Ok(_))), "nothing lands");
        // The same measure made whole, which it could not skip had the cancelled one kept anything.
        let design = ringdesign_core::library::load_design(&path).unwrap();
        let mut baked = (*lib).clone();
        design.unpack_embedded(&mut baked);
        design.bake_all(&mut baked);
        let t = std::time::Instant::now();
        ringdesign_core::dfm::findings_in(&design, &baked);
        let whole = t.elapsed();
        assert!(given_up * 3 < whole, "given up in {given_up:?} against {whole:?} for the whole measure");
        let _ = std::fs::remove_dir_all(&dir);
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
            |ui, (slot, stopped): &mut (Slot, Option<Name>)| {
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
        assert_eq!(h.state().1, Some("Caiman — armoured hide".into()));
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
        let stages = [Stage::Reading, Stage::Evaluating { done: 0, of: 40 }, Stage::Evaluating { done: 39, of: 40 }, Stage::Baking { done: 0, of: 7 }, Stage::Baking { done: 7, of: 7 }, Stage::Measuring, Stage::Building];
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
        // Dropped, as choosing another template drops it, a running open stops at its next check.
        let (hold, release) = held();
        let opening = find("caiman-imported").open(reg, lib, hold);
        let (progress, finished) = (opening.progress(), opening.finished());
        drop(opening);
        release();
        while !finished.load(Ordering::Relaxed) {
            assert!(started.elapsed().as_secs() < 60, "the thread never stopped");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(progress.stage(), Stage::Reading, "no node ran after the drop");
    }
}
