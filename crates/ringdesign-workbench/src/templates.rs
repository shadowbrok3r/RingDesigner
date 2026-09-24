//! One metadata-only template library for desktop and Android menus.
//! Ring data is parsed only after a choice; thumbnails are small bundled renders.
use std::sync::{
    Arc, LazyLock, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, TryRecvError},
};
use egui::{TextureHandle, Ui};
use ringdesign_core::{AlphaLibrary, RingDesign};
use ringdesign_graph::{registry::Registry, templates::{Step, TemplateGraph}};

pub struct Template {
    pub name: &'static str,
    pub slug: &'static str,
    pub description: &'static str,
    source: Source,
}
/// `Design` names a bundled `.ring.json`; the document is decompressed only
/// when that template is chosen, not to list it in a menu.
#[derive(Clone, Copy)]
enum Source { Graph(&'static TemplateGraph), Starter(&'static ringdesign_core::templates::Template), Design(&'static str) }
impl Template {
    pub fn instantiate(&self, reg: &Registry, lib: &AlphaLibrary) -> anyhow::Result<RingDesign> {
        match &self.source {
            Source::Graph(graph) => Ok(graph.instantiate(reg, lib)?),
            Source::Starter(template) => Ok(template.design()),
            Source::Design(slug) => {
                let asset = ringdesign_assets::find(ringdesign_assets::DESIGNS, slug).expect("bundled design");
                Ok(ringdesign_graph::templates::refine_sources(&serde_json::from_str(&asset.text())?))
            }
        }
    }
}

impl Template {
    /// Opens this template on a thread of its own against `lib`, calling `wake` whenever it gets further; poll the [`Opening`] for the design.
    pub fn open(&'static self, reg: Arc<Registry>, lib: Arc<AlphaLibrary>, wake: impl Fn() + Send + Sync + 'static) -> Opening {
        open(self.name, self.source, reg, lib, wake)
    }
}

/// [`Template::open`] for a bundled template graph, opened as its graph even where a starter of its name exists.
pub fn open_graph(graph: &'static TemplateGraph, reg: Arc<Registry>, lib: Arc<AlphaLibrary>, wake: impl Fn() + Send + Sync + 'static) -> Opening {
    open(graph.name, Source::Graph(graph), reg, lib, wake)
}

fn open(name: &'static str, source: Source, reg: Arc<Registry>, lib: Arc<AlphaLibrary>, wake: impl Fn() + Send + Sync + 'static) -> Opening {
    let stage = Arc::new(Mutex::new(Stage::Reading));
    let cancelled = Arc::new(AtomicBool::new(false));
    let (tx, answer) = mpsc::channel();
    let wake = Arc::new(wake);
    let (at, stop, woken) = (stage.clone(), cancelled.clone(), wake.clone());
    let set: Arc<dyn Fn(Stage) + Send + Sync> = Arc::new(move |next| {
        *at.lock().unwrap_or_else(|e| e.into_inner()) = next;
        woken();
    });
    let spawned = std::thread::Builder::new().name("template-open".into()).spawn(move || {
        let opened = opened(source, &reg, &lib, &set, &stop).map(|(design, after)| Opened { design, before: lib, after });
        if !stop.load(Ordering::Relaxed) {
            let _ = tx.send(opened.map_err(|e| format!("{e:#}")));
            wake();
        }
    });
    if let Err(e) = spawned {
        let (tx, failed) = mpsc::channel();
        let _ = tx.send(Err(format!("no thread to open it on: {e}")));
        return Opening { name, stage, answer: failed, cancelled };
    }
    Opening { name, stage, answer, cancelled }
}

/// The design `source` makes and `lib` with its artwork baked in, telling `set` each stage; `Err` once `stop` is set.
fn opened(source: Source, reg: &Registry, lib: &Arc<AlphaLibrary>, set: &Arc<dyn Fn(Stage) + Send + Sync>, stop: &AtomicBool) -> anyhow::Result<(RingDesign, Arc<AlphaLibrary>)> {
    let (design, baked) = match source {
        Source::Graph(graph) => {
            let told = set.clone();
            graph.open(reg, lib, Arc::new(move |step| told(Stage::from(step))))?
        }
        Source::Starter(template) => (template.design(), None),
        Source::Design(slug) => {
            set(Stage::Reading);
            let asset = ringdesign_assets::find(ringdesign_assets::DESIGNS, slug).ok_or_else(|| anyhow::anyhow!("{slug} is not bundled"))?;
            (ringdesign_graph::templates::refine_sources(&serde_json::from_str(&asset.text())?), None)
        }
    };
    anyhow::ensure!(!stop.load(Ordering::Relaxed), "stopped");
    let after = match baked {
        Some(baked) => baked,
        None => {
            set(Stage::Baking);
            let mut baked = (**lib).clone();
            design.unpack_embedded(&mut baked);
            design.bake_all(&mut baked);
            Arc::new(baked)
        }
    };
    Ok((design, after))
}

/// How far a template being opened has got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Reading,
    Evaluating { done: usize, of: usize },
    Baking,
    /// Landed, and its ring is being built.
    Building,
}

impl From<Step> for Stage {
    fn from(step: Step) -> Self {
        match step {
            Step::Reading => Stage::Reading,
            Step::Evaluating { done, of } => Stage::Evaluating { done, of },
            Step::Baking => Stage::Baking,
        }
    }
}

impl Stage {
    /// Share of the whole open done, 0 to 1.
    pub fn fraction(self) -> f32 {
        match self {
            Stage::Reading => 0.05,
            Stage::Evaluating { done, of } => 0.1 + 0.4 * done.min(of) as f32 / of.max(1) as f32,
            Stage::Baking => 0.55,
            Stage::Building => 0.8,
        }
    }

    /// What the stage is doing, in words.
    pub fn words(self) -> String {
        match self {
            Stage::Reading => "reading it".into(),
            Stage::Evaluating { done, of } => format!("running its graph, node {} of {of}", (done + 1).min(of.max(1))),
            Stage::Baking => "baking its artwork".into(),
            Stage::Building => "building the ring".into(),
        }
    }
}

/// A template being opened off the UI thread; dropping it stops the thread before its bake and discards what it made.
pub struct Opening {
    /// The template's name.
    pub name: &'static str,
    stage: Arc<Mutex<Stage>>,
    answer: mpsc::Receiver<Result<Opened, String>>,
    cancelled: Arc<AtomicBool>,
}

impl Opening {
    /// Where it has got.
    pub fn stage(&self) -> Stage {
        *self.stage.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The opened template once it has landed, or why it could not be opened.
    pub fn poll(&self) -> Option<Result<Opened, String>> {
        match self.answer.try_recv() {
            Ok(opened) => Some(opened),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("the thread opening it stopped without an answer".into())),
        }
    }
}

impl Drop for Opening {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

/// A template's design, and the library it was opened against with its artwork baked in.
pub struct Opened {
    pub design: RingDesign,
    before: Arc<AlphaLibrary>,
    after: Arc<AlphaLibrary>,
}

impl Opened {
    /// `current` with the artwork the template baked: the baked library itself while `current` is still the one it was opened against, else `current` with the alphas the bake inserted, shared.
    pub fn library_for(&self, current: &Arc<AlphaLibrary>) -> Arc<AlphaLibrary> {
        if Arc::ptr_eq(current, &self.before) {
            return self.after.clone();
        }
        let mut lib = (**current).clone();
        for alpha in self.after.changed_since(&self.before) {
            lib.insert_shared(alpha.clone());
        }
        Arc::new(lib)
    }
}

/// The bar a template being opened shows: its name and how far it has got.
pub fn progress(ui: &mut Ui, name: &str, stage: Stage) -> egui::Response {
    ui.add(egui::ProgressBar::new(stage.fraction()).text(format!("Opening {name}: {}", stage.words())).animate(true))
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
            assert_eq!(serde_json::to_value(&opened.design).unwrap(), serde_json::to_value(template.instantiate(&reg, &lib).unwrap()).unwrap(), "{slug}");
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
            let moved = Arc::new(moved);
            let merged = opened.library_for(&moved);
            assert!(merged.get("mine").is_some());
            for alpha in &inserted {
                assert!(std::ptr::eq(merged.get(&alpha.name).unwrap(), opened.after.get(&alpha.name).unwrap()), "{slug}: {}", alpha.name);
            }
        }
    }

    #[test]
    fn an_opening_moves_forward_through_its_stages() {
        let stages = [Stage::Reading, Stage::Evaluating { done: 0, of: 40 }, Stage::Evaluating { done: 39, of: 40 }, Stage::Baking, Stage::Building];
        assert!(stages.windows(2).all(|w| w[0].fraction() < w[1].fraction()), "{stages:?}");
        assert_eq!(Stage::Evaluating { done: 39, of: 40 }.words(), "running its graph, node 40 of 40");
        assert_eq!(Stage::Evaluating { done: 0, of: 0 }.fraction(), 0.1);
    }
}
