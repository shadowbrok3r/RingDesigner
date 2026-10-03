//! The File-menu starters as graphs.
//!
//! Each template in `ringdesign_core::templates` is re-expressed here with
//! the same nodes a user would wire, and the result is committed under
//! `graphs/templates/` and compiled in through `ringdesign-assets`. The golden test
//! evaluates every bundled graph and holds the design it produces to the
//! code template byte for byte, and holds the committed file to what the
//! builder produces — so neither the registry nor the files can drift.

use ringdesign_core::profile::TOP_DEG;

use crate::eval::{OUTPUT_DESIGN_PIN, OUTPUT_KIND};
use crate::graph::{Graph, GraphError, Mode, NodeId};
use crate::value::Literal;

/// A bundled template graph: the template's name and its file.
pub struct TemplateGraph {
    pub name: &'static str,
    pub slug: &'static str,
}

/// How far [`TemplateGraph::open`] has got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Decompressing and parsing the template's document.
    Reading,
    /// Running its nodes: `done` of the `of` its design needs.
    Evaluating { done: usize, of: usize },
    /// Baking the design's artwork into the library: `done` of `of` sources and distance fields.
    Baking { done: usize, of: usize },
}

/// A template graph opened: the design, the evaluation that made it, and its artwork baked.
pub struct OpenedGraph {
    /// The design, carrying its graph.
    pub design: ringdesign_core::RingDesign,
    /// The design as the evaluation produced it, without the graph.
    pub evaluated: std::sync::Arc<ringdesign_core::RingDesign>,
    pub report: crate::eval::EvalReport,
    pub graph: Graph,
    /// Every artwork raster the bake inserted, in order.
    pub artwork: Vec<std::sync::Arc<ringdesign_core::Alpha>>,
    /// The library with the artwork baked in; `None` when that left it as it was.
    pub library: Option<std::sync::Arc<ringdesign_core::AlphaLibrary>>,
}

impl TemplateGraph {
    /// The template's document, decompressed out of the bundle on demand.
    /// The artwork these carry as base64 is 99 MB across the catalogue and
    /// 27 MB deflated, so it is decoded for the one template chosen and not
    /// held resident for the menu that lists them all.
    pub fn json(&self) -> std::borrow::Cow<'static, str> {
        ringdesign_assets::find(ringdesign_assets::GRAPHS, self.slug)
            .expect("every catalogued template is bundled")
            .text()
    }

    pub fn load(&self) -> Graph {
        crate::file::load_graph_str(&self.json(), None).expect("bundled graph parses")
    }

    /// Start a project from the whole template, including its title and
    /// manufacturing setup. Attaching just the graph to the old project
    /// lets the hosts' metadata-preserving rebuild overwrite those fields.
    /// Expression pins need an engine this crate does not carry: [`open`](Self::open) takes an evaluator with one attached.
    pub fn instantiate(&self, reg: &crate::registry::Registry, lib: &ringdesign_core::AlphaLibrary) -> Result<ringdesign_core::RingDesign, GraphError> {
        let never = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.open(reg, lib, &mut crate::eval::Evaluator::new(), std::sync::Arc::new(|_| {}), &never).map(|o| o.design)
    }

    /// [`instantiate`](Self::instantiate) with `ev` keyed by `lib`'s revision, telling `step` each step and stopping between nodes and bakes once `cancel` is set.
    pub fn open(
        &self,
        reg: &crate::registry::Registry,
        lib: &ringdesign_core::AlphaLibrary,
        ev: &mut crate::eval::Evaluator,
        step: std::sync::Arc<dyn Fn(Step) + Send + Sync>,
        cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<OpenedGraph, GraphError> {
        step(Step::Reading);
        open_document(self.load(), reg, lib, ev, step, cancel)
    }
}

/// [`TemplateGraph::open`] for a graph already read.
pub fn open_document(
    graph: Graph,
    reg: &crate::registry::Registry,
    lib: &ringdesign_core::AlphaLibrary,
    ev: &mut crate::eval::Evaluator,
    step: std::sync::Arc<dyn Fn(Step) + Send + Sync>,
    cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<OpenedGraph, GraphError> {
    use std::sync::atomic::Ordering;
    let cancelled = || GraphError::global(crate::eval::CANCELLED);
    if cancel.load(Ordering::Relaxed) {
        return Err(cancelled());
    }
    let evaluating = step.clone();
    ev.progress = Some(std::sync::Arc::new(move |done, of| evaluating(Step::Evaluating { done, of })));
    ev.cancel = Some(cancel.clone());
    let evaluated = crate::eval::design_of(ev, &graph, reg, lib, lib.revision());
    (ev.progress, ev.cancel) = (None, None);
    let (evaluated, report) = evaluated?;
    let notes = report.notes(&graph);
    if !notes.is_empty() {
        return Err(GraphError { node: None, message: notes.join("; ") });
    }
    let mut baked = lib.clone();
    let baking = |done, of| step(Step::Baking { done, of });
    let artwork = evaluated.unpack_and_bake_observed(&mut baked, &baking, cancel).ok_or_else(cancelled)?;
    let library = (baked.revision() != lib.revision()).then(|| std::sync::Arc::new(baked));
    let mut design = (*evaluated).clone();
    design.graph = Some(serde_json::to_value(&graph).map_err(|e| GraphError { node: None, message: e.to_string() })?);
    Ok(OpenedGraph { design, evaluated, report, graph, artwork, library })
}

macro_rules! bundled {
    ($($name:literal => $slug:literal),* $(,)?) => {
        pub static BUNDLED: &[TemplateGraph] = &[$(
            TemplateGraph { name: $name, slug: $slug },
        )*];
    };
}

bundled! {
    "Court band" => "court-band",
    "Braided band" => "braided-band",
    "Split shank" => "split-shank",
    "Split gallery" => "split-gallery",
    "Shouldered cushion signet" => "shouldered-cushion-signet",
    "Cathedral solitaire" => "cathedral-solitaire",
    "Bezel solitaire" => "bezel-solitaire",
    "Halo" => "halo",
    "Trilogy" => "trilogy",
    "Toi et moi" => "toi-et-moi",
    "Split-shank basket" => "split-shank-basket",
    "Half eternity" => "half-eternity",
    "Gypsy trio" => "gypsy-trio",
}

/// Authored showcase designs, with the artwork embedded and each layer
/// exposed as nodes. Kept separate from the small procedural starters.
pub static SHOWCASE: &[TemplateGraph] = &[
    TemplateGraph { name: "Nocturne — original night garden", slug: "nocturne" },
    TemplateGraph { name: "Solstice — original sun seal", slug: "solstice" },
    TemplateGraph { name: "Aster Atelier", slug: "aster-atelier" },
    TemplateGraph { name: "Thalassa", slug: "thalassa" },
    // No guard rails: lost wax, made settings throughout. Its sand counterpart is Saurian, on stock.
    TemplateGraph { name: "Oriel — jewelled lantern", slug: "oriel" },
];

/// Metadata only: menus must not parse megabytes of embedded artwork on
/// every frame. Load just the selected template.
pub static IMPORTED: &[TemplateGraph] = &[
    TemplateGraph { name: "Nocturne — night garden", slug: "nocturne-imported" },
    TemplateGraph { name: "Solstice — sun seal", slug: "solstice-imported" },
    TemplateGraph { name: "Aurelia — sovereign sun", slug: "aurelia-imported" },
    TemplateGraph { name: "Vesper — celestial reliquary", slug: "vesper-imported" },
    // One theme each, face to palm, on the factory's own signets and held to two-part sand.
    TemplateGraph { name: "Saurian — beaded skin", slug: "saurian-imported" },
    TemplateGraph { name: "Zenith — the hunter's belt", slug: "zenith-imported" },
    TemplateGraph { name: "Caiman — armoured hide", slug: "caiman-imported" },
];
/// The Reptilia collection: full-ring skins, with subtractive bench detail kept
/// separate from the supported casting relief. Artwork is embedded in each graph.
pub static REPTILIA: &[TemplateGraph] = &[
    TemplateGraph { name: "Ecdysis — ventral scales", slug: "ecdysis-reptilia" },
    TemplateGraph { name: "Tessera — shield mosaic", slug: "tessera-reptilia" },
    TemplateGraph { name: "Lorica — crocodile armour", slug: "lorica-reptilia" },
    TemplateGraph { name: "Ophidian — amethyst serpent", slug: "ophidian-reptilia" },
    TemplateGraph { name: "Varanus — sovereign scales", slug: "varanus-reptilia" },
];
/// The Bestiarium: creatures told by hide, silk and weapons, face to palm.
/// Artwork and stored parts are embedded in each graph.
pub static BESTIARIUM: &[TemplateGraph] = &[
    TemplateGraph { name: "Arachne — the weaver", slug: "arachne-bestiarium" },
    TemplateGraph { name: "Manticora — the tail that throws", slug: "manticora-bestiarium" },
];
pub fn catalog() -> impl Iterator<Item = &'static TemplateGraph> {
    BUNDLED.iter().chain(SHOWCASE).chain(IMPORTED).chain(REPTILIA).chain(BESTIARIUM)
}

/// Curated templates carry only artwork that reaches a layer or mask. Within
/// each name, keep the source that `RingDesign::bake_all` actually uses. This
/// is an authoring operation, never an automatic edit to a user's project.
pub fn refine_sources(design: &ringdesign_core::RingDesign) -> ringdesign_core::RingDesign {
    use std::collections::HashSet;
    let mut result = design.clone();
    let referenced: HashSet<String> = design.layers.referenced_alphas().into_iter().map(str::to_owned).collect();
    let mut kept = HashSet::new();
    // Reverse bake order: recipes override SVGs, texts, drawings and PNGs.
    macro_rules! retain_last {
        ($field:ident) => {{
            result.$field.reverse();
            result.$field.retain(|source| referenced.contains(&source.name) && kept.insert(source.name.clone()));
            result.$field.reverse();
        }};
    }
    retain_last!(recipes);
    retain_last!(svgs);
    retain_last!(texts);
    retain_last!(drawn);
    retain_last!(embedded);
    result
}

#[cfg(test)]
mod refinement_tests {
    #[test]
    fn referenced_masks_survive_and_last_baked_source_wins() {
        use ringdesign_core::{RingDesign, EmbeddedAlpha, field::{Layer, LayerEntry}, tiling::TilingLayer, alpha::ProcRecipe};
        let mut design = RingDesign::default();
        let mut layer = LayerEntry::new("scales", Layer::Tiling(TilingLayer::default_for("Scales", &design.field_context())));
        layer.mask = Some("Mask".into());
        design.layers.layers.push(layer);
        design.embedded = ["Scales", "Mask", "Unused"].map(|name| EmbeddedAlpha { name: name.into(), png: String::new() }).into();
        design.recipes = vec![
            ProcRecipe { name: "Scales".into(), gamma: 1., ..Default::default() },
            ProcRecipe { name: "Scales".into(), gamma: 2., ..Default::default() },
            ProcRecipe { name: "Unused".into(), ..Default::default() },
        ];
        let refined = super::refine_sources(&design);
        assert_eq!(refined.embedded.len(), 1);
        assert_eq!(refined.embedded[0].name, "Mask");
        assert_eq!(refined.recipes.len(), 1);
        assert_eq!(refined.recipes[0].gamma, 2.);
        assert_eq!(design.embedded.len(), 3, "authoring cleanup leaves its source untouched");
    }
}

/// Bundled clusters: graphs with exposed inputs and outputs, usable as
/// one node. A user-dir cluster of the same name wins. The vine needs the
/// kernel, so a build without it does not list a cluster it cannot run.
#[cfg(not(feature = "kernel-manifold"))]
pub static BUNDLED_CLUSTERS: &[(&str, &str)] = &[
    ("Signet", "signet"),
    ("Wheel window", "wheel-window"),
    ("Pointed-arch section", "pointed-arch-section"),
    ("Rose tracery", "rose-tracery"),
    ("Lancet arcade", "lancet-arcade"),
];
#[cfg(feature = "kernel-manifold")]
pub static BUNDLED_CLUSTERS: &[(&str, &str)] = &[
    ("Signet", "signet"),
    ("Vine semi-mount", "vine-semi-mount"),
    ("Wheel window", "wheel-window"),
    ("Pointed-arch section", "pointed-arch-section"),
    ("Rose tracery", "rose-tracery"),
    ("Lancet arcade", "lancet-arcade"),
];

/// The Gothic clusters Tenebrae's templates are packaged with, by name and slug.
pub static GOTHIC_CLUSTERS: &[(&str, &str)] = &[
    ("Wheel window", "wheel-window"),
    ("Pointed-arch section", "pointed-arch-section"),
    ("Rose tracery", "rose-tracery"),
    ("Lancet arcade", "lancet-arcade"),
];

/// Bundled presets for the bundled clusters.
pub static BUNDLED_PRESETS: &[(&str, &str)] = &[("Heart signet", "heart-signet"), ("Cushion signet", "cushion-signet")];

/// A bundled cluster's or preset's document by the slug the tables above
/// carry. The tables hold slugs rather than text so the menus that read them
/// stay the size of their names.
pub fn bundled_json(family: &'static [ringdesign_assets::Asset], slug: &str) -> std::borrow::Cow<'static, str> {
    ringdesign_assets::find(family, slug).expect("bundled document").text()
}

pub fn bundled_clusters() -> Vec<Graph> {
    BUNDLED_CLUSTERS.iter().filter_map(|(_, slug)| crate::file::load_graph_str(&bundled_json(ringdesign_assets::CLUSTERS, slug), None).ok()).collect()
}

pub fn bundled_presets() -> Vec<crate::file::Preset> {
    BUNDLED_PRESETS.iter().filter_map(|(_, slug)| crate::file::load_preset_str(&bundled_json(ringdesign_assets::GRAPH_PRESETS, slug)).ok()).collect()
}

/// The signet construction as a cluster: one Width reaches the section,
/// the face fit and the shank; the head's own knobs and the shank's taper
/// are exposed; the design, head, shank and profile come out.
pub fn build_signet_cluster() -> Graph {
    signet_cluster().expect("the signet cluster wires")
}

/// The vine semi-mount as a Free-mode cluster: mandrel's ring options in,
/// the solid, its mesh, the mesh verdict and the stones out.
pub fn build_vine_cluster() -> Graph {
    vine_cluster().expect("the vine cluster wires")
}

fn vine_cluster() -> Result<Graph, GraphError> {
    let mut g = Graph::new("Vine semi-mount", Mode::Free);
    let dia = g.add("number")?;
    set(&mut g, dia, &[("value", n(16.5))])?;
    g.node_mut(dia).expect("added").label = Some("Inner diameter".into());
    let vine = g.add("solid.vine_semimount")?;
    g.connect(dia, "out", vine, "inner_diameter_mm")?;
    let fit = g.add("band.size.fit")?;
    g.connect(dia, "out", fit, "inner_diameter_mm")?;
    let d = g.add("design.new")?;
    set(&mut g, d, &[("name", t("Vine semi-mount"))])?;
    g.connect(fit, "size", d, "size")?;
    let mesh = g.add("solid.mesh")?;
    g.connect(vine, "solid", mesh, "solid")?;
    let verdict = g.add("sink.mesh_verdict")?;
    g.connect(mesh, "mesh", verdict, "mesh")?;
    g.connect(d, "design", verdict, "design")?;
    g.expose(dia, "value", "Inner diameter")?;
    g.expose(vine, "vine_radius_mm", "Vine radius")?;
    g.expose_output(vine, "solid", "solid")?;
    g.expose_output(mesh, "mesh", "mesh")?;
    g.expose_output(mesh, "volume_mm3", "volume_mm3")?;
    g.expose_output(vine, "stone_count", "stone_count")?;
    g.expose_output(vine, "total_carats", "total_carats")?;
    g.expose_output(vine, "stones", "stones")?;
    g.expose_output(verdict, "verdict", "verdict")?;
    g.expose_output(verdict, "undercut_pct", "undercut_pct")?;
    arrange(&mut g);
    Ok(g)
}

/// Tenebrae's 18.6 mm bore as the US size the Gothic clusters default to.
pub fn tenebrae_size() -> f64 {
    ringdesign_core::resize::size_from_bore(18.6).expect("18.6 mm is a ring size").0
}

/// The Gothic cluster called `name`, as its builder makes it.
pub fn build_gothic_cluster(name: &str) -> Option<Graph> {
    let g = match name {
        "Wheel window" => wheel_window_cluster(),
        "Pointed-arch section" => pointed_arch_cluster(),
        "Rose tracery" => rose_tracery_cluster(),
        "Lancet arcade" => lancet_arcade_cluster(),
        _ => return None,
    };
    Some(g.expect("a Gothic cluster wires"))
}

/// `input` of `node` promoted to the cluster's panel as `name`, with what it does.
fn control(g: &mut Graph, node: NodeId, input: &str, name: &str, doc: &str) -> Result<(), GraphError> {
    g.expose(node, input, name)?;
    if let Some(e) = g.exposed.iter_mut().find(|e| e.name == name) {
        e.doc = doc.into();
    }
    Ok(())
}

/// `out` of `node` handed out of the cluster as `name`, with what it is.
fn result(g: &mut Graph, node: NodeId, out: &str, name: &str, doc: &str) -> Result<(), GraphError> {
    g.expose_output(node, out, name)?;
    if let Some(e) = g.outputs.iter_mut().find(|e| e.name == name) {
        e.doc = doc.into();
    }
    Ok(())
}

fn labelled(g: &mut Graph, kind: &str, label: &str) -> Result<NodeId, GraphError> {
    let id = g.add(kind)?;
    g.node_mut(id).expect("added").label = Some(label.into());
    Ok(id)
}

/// A wheel of lancet lights about the bore, its circles following the ring size.
fn wheel_window_cluster() -> Result<Graph, GraphError> {
    use crate::nodes::gothic::LIGHTS;
    let mut g = Graph::new("Wheel window", Mode::SandRing);
    let size = labelled(&mut g, "band.size", "Bore")?;
    set(&mut g, size, &[("size", n(tenebrae_size()))])?;
    let sill = labelled(&mut g, "math.add", "Sill circle")?;
    g.connect(size, "bore_radius_mm", sill, "a")?;
    set(&mut g, sill, &[("b", n(1.0))])?;
    let outer = labelled(&mut g, "math.add", "Outer circle")?;
    g.connect(size, "bore_radius_mm", outer, "a")?;
    set(&mut g, outer, &[("b", n(3.1))])?;
    let apex = labelled(&mut g, "math.sub", "Apex circle")?;
    g.connect(outer, "out", apex, "a")?;
    set(&mut g, apex, &[("b", n(0.35))])?;
    let lights = labelled(&mut g, LIGHTS, "Wheel")?;
    set(&mut g, lights, &[("count", i(24)), ("bar_mm", n(0.9)), ("head", t("Pointed")), ("name", t("Wheel window"))])?;
    g.connect(sill, "out", lights, "sill_r_mm")?;
    g.connect(apex, "out", lights, "apex_r_mm")?;
    control(&mut g, size, "size", "Size", "US ring size; the wheel is drawn about its bore.")?;
    control(&mut g, lights, "count", "Lights", "How many lancet lights round the wheel.")?;
    control(&mut g, lights, "bar_mm", "Bar", "Metal between neighbouring lights, mm.")?;
    control(&mut g, sill, "b", "Rail", "Metal between the bore and the lights' sills, mm.")?;
    control(&mut g, outer, "b", "Reach", "How far outside the bore the wheel's outer circle stands, mm.")?;
    control(&mut g, apex, "b", "Drop", "How far inside the outer circle each light's apex stands, mm.")?;
    control(&mut g, lights, "head", "Head", "How each light's head closes.")?;
    control(&mut g, lights, "phase_deg", "Phase", "The first light's axis, degrees from the sketch's x; 90 is the crown.")?;
    control(&mut g, lights, "plane", "Plane", "The work plane the wheel lies on, by feature id: a parting plane offset to a side face.")?;
    result(&mut g, lights, "operation", "sketch_op", "The wheel as a Sketch feature's operation, a closed loop per light.")?;
    result(&mut g, lights, "lights", "lights", "How many lights the wheel drew.")?;
    result(&mut g, lights, "land_mm", "land_mm", "The narrowest metal between neighbouring lights, mm.")?;
    arrange(&mut g);
    Ok(g)
}

/// The blunt-lancet section a ring revolves from, standing on the bore the ring size gives.
fn pointed_arch_cluster() -> Result<Graph, GraphError> {
    use crate::nodes::gothic::ARCH;
    let mut g = Graph::new("Pointed-arch section", Mode::SandRing);
    let size = labelled(&mut g, "band.size", "Bore")?;
    set(&mut g, size, &[("size", n(tenebrae_size()))])?;
    let arch = labelled(&mut g, ARCH, "Section")?;
    set(&mut g, arch, &[("width_mm", n(5.6)), ("thickness_mm", n(3.6)), ("keel", n(1.0)), ("fillet_mm", n(0.3)), ("comfort_mm", n(0.12)), ("name", t("Pointed-arch section"))])?;
    g.connect(size, "bore_radius_mm", arch, "bore_r_mm")?;
    control(&mut g, size, "size", "Size", "US ring size; the section stands on its bore.")?;
    control(&mut g, arch, "width_mm", "Width", "Across the band, mm.")?;
    control(&mut g, arch, "thickness_mm", "Thickness", "From the bore's tightest point to the keel, mm.")?;
    control(&mut g, arch, "keel", "Keel", "1 springs the flanks from the bore corners into a sharp keel; 0 rounds the head on straight feet.")?;
    control(&mut g, arch, "fillet_mm", "Fillet", "Radius rounding each bore corner, mm.")?;
    control(&mut g, arch, "comfort_mm", "Comfort", "How much wider the bore is at the band's edges than at its middle, mm.")?;
    result(&mut g, arch, "operation", "sketch_op", "The section as a Sketch feature's operation on the plane through the finger's axis; revolve it about the finger.")?;
    arrange(&mut g);
    Ok(g)
}

/// Lights radiating between two net circles, inset half a bar from each.
fn rose_tracery_cluster() -> Result<Graph, GraphError> {
    use crate::nodes::gothic::LIGHTS;
    let mut g = Graph::new("Rose tracery", Mode::SandRing);
    let bar = labelled(&mut g, "number", "Bar")?;
    set(&mut g, bar, &[("value", n(0.8))])?;
    let half = labelled(&mut g, "math.mul", "Half the bar")?;
    g.connect(bar, "out", half, "a")?;
    set(&mut g, half, &[("b", n(0.5))])?;
    let sill = labelled(&mut g, "math.add", "Sill circle")?;
    set(&mut g, sill, &[("a", n(3.6))])?;
    g.connect(half, "out", sill, "b")?;
    let apex = labelled(&mut g, "math.sub", "Apex circle")?;
    set(&mut g, apex, &[("a", n(7.1))])?;
    g.connect(half, "out", apex, "b")?;
    let lights = labelled(&mut g, LIGHTS, "Rose")?;
    set(&mut g, lights, &[("count", i(8)), ("head", t("Trefoil")), ("name", t("Rose tracery"))])?;
    g.connect(bar, "out", lights, "bar_mm")?;
    g.connect(sill, "out", lights, "sill_r_mm")?;
    g.connect(apex, "out", lights, "apex_r_mm")?;
    control(&mut g, lights, "count", "Lights", "How many lights round the rose.")?;
    control(&mut g, sill, "a", "Inner radius", "The net's inner circle; each light's sill stands half a bar outside it, mm.")?;
    control(&mut g, apex, "a", "Outer radius", "The net's outer circle; each light's apex stands half a bar inside it, mm.")?;
    control(&mut g, bar, "value", "Bar", "Metal between neighbouring lights and to both circles, mm.")?;
    control(&mut g, lights, "head", "Head", "Pointed, Round, Trefoil, or the whole light a Mouchette.")?;
    control(&mut g, lights, "width_mm", "Width", "0 fills each cell; above 0, every light is parallel-sided at this width, mm.")?;
    control(&mut g, lights, "phase_deg", "Phase", "The first light's axis, degrees from the sketch's x.")?;
    control(&mut g, lights, "clockwise", "Clockwise", "Mouchettes lean clockwise instead of counter-clockwise.")?;
    control(&mut g, lights, "centre_x_mm", "Centre x", "Where the rose is centred along the sketch's x, mm.")?;
    control(&mut g, lights, "centre_y_mm", "Centre y", "Where the rose is centred along the sketch's y, mm.")?;
    control(&mut g, lights, "plane", "Plane", "The work plane or part the rose lies on, by feature id.")?;
    control(&mut g, lights, "face", "Face", "Which planar face of that feature; a work plane has one, 0.")?;
    result(&mut g, lights, "operation", "sketch_op", "The rose as a Sketch feature's operation, a closed loop per light.")?;
    result(&mut g, lights, "lights", "count", "How many lights the rose drew, for an array of stones round it.")?;
    result(&mut g, lights, "land_mm", "land_mm", "The narrowest metal between neighbouring lights, mm.")?;
    arrange(&mut g);
    Ok(g)
}

/// A row of lancet bays on a sill, as niches to cut and as one stamp outline.
fn lancet_arcade_cluster() -> Result<Graph, GraphError> {
    use crate::nodes::gothic::ARCADE;
    let mut g = Graph::new("Lancet arcade", Mode::SandRing);
    let arcade = labelled(&mut g, ARCADE, "Arcade")?;
    set(&mut g, arcade, &[("bays", i(3)), ("width_mm", n(6.0)), ("height_mm", n(2.4)), ("bar_mm", n(0.6)), ("sill_mm", n(0.4)), ("head", t("Pointed")), ("name", t("Lancet arcade"))])?;
    control(&mut g, arcade, "bays", "Bays", "How many bays.")?;
    control(&mut g, arcade, "width_mm", "Width", "Across every bay and the posts between them, mm.")?;
    control(&mut g, arcade, "height_mm", "Height", "From the sill's foot to the apexes, mm.")?;
    control(&mut g, arcade, "bar_mm", "Bar", "Width of each post between two bays, mm.")?;
    control(&mut g, arcade, "sill_mm", "Sill", "Height of the sill the bays stand on, mm.")?;
    control(&mut g, arcade, "head", "Head", "Pointed, Round, or Trefoil cusped.")?;
    control(&mut g, arcade, "x_mm", "X", "Where the arcade's centre sits along the sketch's x, mm.")?;
    control(&mut g, arcade, "y_mm", "Y", "Where the arcade's centre sits along the sketch's y, mm.")?;
    control(&mut g, arcade, "turn_deg", "Turn", "Turn about the arcade's centre, degrees.")?;
    control(&mut g, arcade, "plane", "Plane", "The work plane or part the arcade lies on, by feature id.")?;
    control(&mut g, arcade, "face", "Face", "Which planar face of that feature; a work plane has one, 0.")?;
    result(&mut g, arcade, "operation", "sketch_op", "The bays as a Sketch feature's operation, a closed loop each.")?;
    result(&mut g, arcade, "outline", "outline", "The bays on their sill as one stamp outline about the origin, mm.")?;
    result(&mut g, arcade, "bay_mm", "bay_mm", "Width of one bay between its jambs, mm.")?;
    arrange(&mut g);
    Ok(g)
}

/// The presets the bundled files come from.
pub fn build_presets() -> Vec<crate::file::Preset> {
    use crate::file::Preset;
    let preset = |name: &str, width: f64, thickness: f64, outline: &str| Preset {
        name: name.into(),
        cluster: "Signet".into(),
        values: [
            ("Name".to_string(), Literal::Text(name.into())),
            ("Width".to_string(), Literal::Number(width)),
            ("Thickness".to_string(), Literal::Number(thickness)),
            ("Outline".to_string(), Literal::Text(outline.into())),
        ]
        .into_iter()
        .collect(),
        doc: format!("A {outline} head on a {width} × {thickness} mm squared band, lofted."),
    };
    vec![preset("Heart signet", 15.5, 1.6, "Heart"), preset("Cushion signet", 14.5, 2.2, "Cushion")]
}

fn signet_cluster() -> Result<Graph, GraphError> {
    use ringdesign_core::profile::SIGNET_TAPER;
    let mut g = Graph::new("Signet", Mode::SandRing);
    let width = g.add("number")?;
    set(&mut g, width, &[("value", n(12.0))])?;
    g.node_mut(width).expect("added").label = Some("Width".into());
    let thickness = g.add("number")?;
    set(&mut g, thickness, &[("value", n(1.8))])?;
    g.node_mut(thickness).expect("added").label = Some("Thickness".into());
    let outline = g.add("text")?;
    set(&mut g, outline, &[("value", t("Oval"))])?;
    g.node_mut(outline).expect("added").label = Some("Outline".into());
    let name = g.add("text")?;
    set(&mut g, name, &[("value", t("Signet"))])?;
    g.node_mut(name).expect("added").label = Some("Name".into());
    let p = g.add("band.profile")?;
    set(&mut g, p, &[("style", t("Flat")), ("flatten_sides", b(true))])?;
    g.connect(width, "out", p, "width_mm")?;
    g.connect(thickness, "out", p, "thickness_mm")?;
    let h = g.add("head")?;
    g.connect(outline, "out", h, "outline")?;
    g.connect(width, "out", h, "fit_to_width_mm")?;
    let s = g.add("shank")?;
    set(&mut g, s, &[("kind", t("Signet")), ("amount", n(SIGNET_TAPER))])?;
    g.connect(h, "head", s, "head")?;
    let d = g.add("design.new")?;
    g.connect(name, "out", d, "name")?;
    g.connect(p, "profile", d, "profile")?;
    g.connect(s, "shank", d, "shank")?;
    let out = g.add(OUTPUT_KIND)?;
    g.connect(d, "design", out, OUTPUT_DESIGN_PIN)?;
    g.expose(width, "value", "Width")?;
    g.expose(thickness, "value", "Thickness")?;
    g.expose(outline, "value", "Outline")?;
    g.expose(name, "value", "Name")?;
    g.expose(d, "size", "Size")?;
    g.expose(h, "rise_mm", "Rise")?;
    g.expose(h, "shoulder_deg", "Shoulder")?;
    g.expose(h, "rim_round_mm", "Rim")?;
    g.expose(h, "loft", "Loft")?;
    g.expose(h, "table_dome_mm", "Cap")?;
    g.expose(s, "amount", "Taper")?;
    g.expose_output(d, "design", "design")?;
    g.expose_output(h, "head", "head")?;
    g.expose_output(s, "shank", "shank")?;
    g.expose_output(p, "profile", "profile")?;
    arrange(&mut g);
    Ok(g)
}

/// Every bundled template graph, parsed.
pub fn all() -> Vec<(&'static str, Graph)> {
    catalog().map(|t| (t.name, t.load())).collect()
}

pub fn graph(name: &str) -> Option<Graph> {
    catalog().find(|t| t.name == name).map(TemplateGraph::load)
}

/// The bundled starter graph the editor opens on: size, section, shank,
/// an empty stack and the output, with the design panel's knobs exposed.
pub fn simple() -> Graph {
    crate::file::load_graph_str(&ringdesign_assets::simple_graph(), None).expect("bundled graph parses")
}

/// The builders the committed files come from.
pub fn build(name: &str) -> Option<Graph> {
    let g = match name {
        "Court band" => court_band(),
        "Shouldered cushion signet" => shouldered_cushion_signet(),
        "Braided band" => braided_band(),
        _ => {
            let t = ringdesign_core::templates::all().iter().find(|t| t.name == name)?;
            crate::lift::from_design(&t.design(), &crate::registry::Registry::builtin(), &ringdesign_core::AlphaLibrary::builtin())
        }
    };
    Some(g.expect("a template builder wires a valid graph"))
}

pub fn build_simple() -> Graph {
    simple_graph().expect("the simple graph wires")
}

fn set(g: &mut Graph, id: NodeId, pins: &[(&str, Literal)]) -> Result<(), GraphError> {
    for (k, v) in pins {
        g.set_input(id, *k, v.clone())?;
    }
    Ok(())
}

fn n(x: f64) -> Literal {
    Literal::Number(x)
}
fn i(x: i64) -> Literal {
    Literal::Int(x)
}
fn t(s: &str) -> Literal {
    Literal::Text(s.into())
}
fn b(x: bool) -> Literal {
    Literal::Bool(x)
}

/// A flat-sided band: `templates::squared`.
fn squared(g: &mut Graph, width: f64, thickness: f64) -> Result<NodeId, GraphError> {
    let p = g.add("band.profile")?;
    set(g, p, &[("style", t("Flat")), ("width_mm", n(width)), ("thickness_mm", n(thickness)), ("flatten_sides", b(true))])?;
    Ok(p)
}

/// A signet on a squared band: `templates::signet`.
fn signet(g: &mut Graph, outline: &str, width: f64, thickness: f64) -> Result<(NodeId, NodeId), GraphError> {
    let p = squared(g, width, thickness)?;
    let s = g.add("shank.signet")?;
    set(g, s, &[("band_width_mm", n(width)), ("outline", t(outline))])?;
    Ok((p, s))
}

fn design(g: &mut Graph, name: &str, profile: NodeId, shank: Option<NodeId>) -> Result<NodeId, GraphError> {
    let d = g.add("design.new")?;
    set(g, d, &[("name", t(name))])?;
    g.connect(profile, "profile", d, "profile")?;
    if let Some(s) = shank {
        g.connect(s, "shank", d, "shank")?;
    }
    Ok(d)
}

/// `templates::side_tiling`: fitted to the side faces with square cells,
/// at a height, then patched.
fn side_tiling(g: &mut Graph, design: NodeId, alpha: &str, height: f64, patch: &[(&str, Literal)]) -> Result<NodeId, GraphError> {
    let fit = g.add("layer.tiling.fit")?;
    g.connect(design, "design", fit, "design")?;
    set(g, fit, &[("alpha", t(alpha))])?;
    let tl = g.add("layer.tiling")?;
    g.connect(fit, "layer", tl, "layer")?;
    set(g, tl, &[("height_mm", n(height))])?;
    set(g, tl, patch)?;
    Ok(tl)
}

fn entry(g: &mut Graph, layer: NodeId, name: &str) -> Result<NodeId, GraphError> {
    let e = g.add("entry")?;
    g.connect(layer, "layer", e, "layer")?;
    set(g, e, &[("name", t(name))])?;
    Ok(e)
}

/// Entries into a stack, assembled onto the design, and the output.
fn finish(g: &mut Graph, design: NodeId, entries: &[NodeId]) -> Result<NodeId, GraphError> {
    let mut last: Option<NodeId> = None;
    for e in entries {
        let st = g.add("stack")?;
        if let Some(prev) = last {
            g.connect(prev, "stack", st, "stack")?;
        }
        g.connect(*e, "entry", st, "entries")?;
        last = Some(st);
    }
    let mut out_design = design;
    if let Some(st) = last {
        let asm = g.add("design.assemble")?;
        g.connect(design, "design", asm, "design")?;
        g.connect(st, "stack", asm, "stack")?;
        out_design = asm;
    }
    let out = g.add(OUTPUT_KIND)?;
    g.connect(out_design, "design", out, OUTPUT_DESIGN_PIN)?;
    Ok(out)
}

fn court_band() -> Result<Graph, GraphError> {
    let mut g = Graph::new("Court band", Mode::SandRing);
    let p = g.add("band.profile")?;
    set(&mut g, p, &[("style", t("LowDome")), ("width_mm", n(4.0)), ("thickness_mm", n(2.0))])?;
    let d = design(&mut g, "Court band", p, None)?;
    finish(&mut g, d, &[])?;
    g.expose(p, "width_mm", "Width")?;
    g.expose(p, "thickness_mm", "Thickness")?;
    g.expose(p, "style", "Section")?;
    Ok(g)
}

fn shouldered_cushion_signet() -> Result<Graph, GraphError> {
    let mut g = Graph::new("Shouldered cushion signet", Mode::SandRing);
    let (p, s) = signet(&mut g, "Cushion", 14.5, 2.2)?;
    let d = design(&mut g, "Shouldered cushion signet", p, Some(s))?;
    let tl = side_tiling(&mut g, d, "Chevron", 0.28, &[("repeats_around", i(9)), ("rows", i(1))])?;
    let e = entry(&mut g, tl, "Shoulder ornament")?;
    let w = g.add("window")?;
    set(&mut g, w, &[("theta_deg", n(TOP_DEG)), ("span_deg", n(120.0)), ("invert", b(true))])?;
    g.connect(w, "window", e, "window")?;
    finish(&mut g, d, &[e])?;
    g.expose(w, "span_deg", "Blank table arc")?;
    Ok(g)
}

fn braided_band() -> Result<Graph, GraphError> {
    let mut g = Graph::new("Braided band", Mode::SandRing);
    let p = squared(&mut g, 7.5, 2.4)?;
    let d = design(&mut g, "Braided band", p, None)?;
    let tl = side_tiling(&mut g, d, "Braid", 0.30, &[("repeats_around", i(8)), ("rows", i(1))])?;
    let braid = entry(&mut g, tl, "Braid")?;
    let info = g.add("design.info")?;
    g.connect(d, "design", info, "design")?;
    let half = g.add("math.mul")?;
    g.connect(info, "band_v_len_mm", half, "a")?;
    set(&mut g, half, &[("b", n(0.5))])?;
    let m = g.add("layer.milgrain")?;
    g.connect(half, "out", m, "v_mm")?;
    set(&mut g, m, &[("bead_diameter_mm", n(0.5)), ("beads_around", i(130)), ("height_mm", n(0.22)), ("mirror", b(false))])?;
    let mil = entry(&mut g, m, "Milgrain")?;
    finish(&mut g, d, &[braid, mil])?;
    g.expose(m, "beads_around", "Beads around")?;
    Ok(g)
}

fn simple_graph() -> Result<Graph, GraphError> {
    let mut g = Graph::new("Simple ring", Mode::SandRing);
    let p = g.add("band.profile")?;
    set(&mut g, p, &[("style", t("LowDome")), ("width_mm", n(6.0)), ("thickness_mm", n(1.8))])?;
    let s = g.add("shank")?;
    set(&mut g, s, &[("kind", t("Uniform")), ("amount", n(0.5))])?;
    let d = g.add("design.new")?;
    set(&mut g, d, &[("name", t("Simple ring")), ("size", n(7.0))])?;
    g.connect(p, "profile", d, "profile")?;
    g.connect(s, "shank", d, "shank")?;
    let st = g.add("stack")?;
    let asm = g.add("design.assemble")?;
    g.connect(d, "design", asm, "design")?;
    g.connect(st, "stack", asm, "stack")?;
    let out = g.add(OUTPUT_KIND)?;
    g.connect(asm, "design", out, OUTPUT_DESIGN_PIN)?;
    g.expose(d, "size", "Size")?;
    g.expose(p, "style", "Section")?;
    g.expose(p, "width_mm", "Width")?;
    g.expose(p, "thickness_mm", "Thickness")?;
    g.expose(s, "kind", "Shank")?;
    g.expose(s, "amount", "Shank amount")?;
    g.expose(d, "name", "Name")?;
    for (k, id) in [(p, 0.0f32), (s, 1.0), (d, 2.0), (st, 2.0), (asm, 3.0), (out, 4.0)].iter().map(|(id, col)| (*col, *id)) {
        g.node_mut(id).expect("just added").pos = [k * 220.0, if id == st { 160.0 } else { 0.0 }];
    }
    Ok(g)
}

/// Tidy positions for a freshly built template: one column per node,
/// topologically, so the committed file opens readable.
pub fn arrange(g: &mut Graph) {
    if let Ok(order) = g.topo() {
        let mut depth: std::collections::BTreeMap<NodeId, usize> = std::collections::BTreeMap::new();
        for id in &order {
            let d = g.wires_into(*id).filter_map(|w| depth.get(&w.from)).max().map(|m| m + 1).unwrap_or(0);
            depth.insert(*id, d);
        }
        let mut rows: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
        for id in &order {
            let col = depth[id];
            let row = rows.entry(col).or_insert(0);
            if let Some(node) = g.node_mut(*id) {
                node.pos = [col as f32 * 240.0, *row as f32 * 150.0];
            }
            *row += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_open_says_each_step_and_makes_what_an_evaluation_makes_with_the_library_it_baked() {
        use std::sync::{Arc, Mutex};
        let reg = crate::registry::Registry::builtin();
        let lib = ringdesign_core::AlphaLibrary::builtin();
        let template = catalog().find(|t| t.slug == "aster-atelier").unwrap();
        let steps = Arc::new(Mutex::new(Vec::new()));
        let seen = steps.clone();
        let never = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut ev = Evaluator::new();
        let opened = template.open(&reg, &lib, &mut ev, Arc::new(move |step| seen.lock().unwrap().push(step)), &never).unwrap();
        let steps = steps.lock().unwrap().clone();
        let of = match steps[1] { Step::Evaluating { of, .. } => of, other => panic!("{other:?}") };
        let units = opened.evaluated.bake_units();
        assert_eq!(units, 6, "aster-atelier's artwork and fields");
        let mut want = vec![Step::Reading];
        want.extend((0..of).map(|done| Step::Evaluating { done, of }));
        assert_eq!(steps[..want.len()], want[..]);
        let mut baking: Vec<usize> = steps[want.len()..].iter().map(|s| match s { Step::Baking { done, of } if *of == units => *done, other => panic!("{other:?}") }).collect();
        baking.sort();
        assert_eq!(baking, (0..=units).collect::<Vec<_>>(), "the bake's size, then one step per source and field");
        assert!(ev.progress.is_none() && ev.cancel.is_none() && ev.cached_nodes() == of, "the evaluator keeps its cache and nothing else");
        let graph = template.load();
        let out = crate::eval::evaluate_design(&mut crate::eval::Evaluator::new(), &graph, &reg, &lib, 0).unwrap();
        let mut evaluated = (*out.design).clone();
        evaluated.graph = Some(serde_json::to_value(&graph).unwrap());
        assert_eq!(serde_json::to_value(&opened.design).unwrap(), serde_json::to_value(&evaluated).unwrap());
        let (baked, theirs) = (opened.library.expect("its artwork"), out.baked_library.unwrap());
        let names = |l: &ringdesign_core::AlphaLibrary| l.changed_since(&lib).map(|a| (a.name.clone(), a.data.clone())).collect::<Vec<_>>();
        assert!(!names(&baked).is_empty());
        assert_eq!(names(&baked), names(&theirs));
        assert_eq!(opened.artwork.iter().map(|a| a.name.clone()).collect::<Vec<_>>(), baked.changed_since(&lib).filter(|a| !a.name.ends_with(ringdesign_core::alpha::SDF_SUFFIX)).map(|a| a.name.clone()).collect::<Vec<_>>());
    }

    #[test]
    fn a_rebuild_of_an_unchanged_graph_runs_bakes_and_judges_nothing_again() {
        use std::sync::Arc;
        let reg = Registry::builtin();
        let lib = Arc::new(AlphaLibrary::builtin());
        let graph = catalog().find(|t| t.slug == "nocturne").unwrap().load();
        let mut ev = Evaluator::new();
        let first = crate::eval::evaluate_design_onto(&mut ev, &graph, &reg, &lib, &lib).unwrap();
        let held = first.baked_library.clone().expect("its artwork, baked");
        // The host takes the baked library; the next build evaluates against the one before it and bakes onto the host's.
        let again = crate::eval::evaluate_design_onto(&mut ev, &graph, &reg, &lib, &held).unwrap();
        assert!(again.report.ran().is_empty(), "every node from the cache: {:?}", again.report.ran());
        assert!(again.baked_library.is_none(), "the host's library already holds the artwork");
        assert!(Arc::ptr_eq(&again.design, &first.design));
        assert_eq!(format!("{:?}", again.field), format!("{:?}", first.field));
        // A phone-style evaluation keyed by the library it is handed settles the same way after one pass.
        let mut phone = Evaluator::new();
        let epoch = |l: &Arc<AlphaLibrary>| Arc::as_ptr(l) as usize as u64;
        let landed = evaluate_design(&mut phone, &graph, &reg, &held, epoch(&held)).unwrap();
        assert!(landed.baked_library.is_none());
        let settled = evaluate_design(&mut phone, &graph, &reg, &held, epoch(&held)).unwrap();
        assert!(settled.report.ran().is_empty() && settled.baked_library.is_none() && Arc::ptr_eq(&settled.design, &landed.design));
    }

    #[test]
    fn a_cancel_stops_an_open_between_nodes_and_between_bakes() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, Mutex};
        let reg = Registry::builtin();
        let lib = AlphaLibrary::builtin();
        let template = catalog().find(|t| t.slug == "nocturne").unwrap();
        for (at, stops) in [(Step::Evaluating { done: 3, of: 0 }, "evaluating"), (Step::Baking { done: 1, of: 0 }, "baking")] {
            let cancel = Arc::new(AtomicBool::new(false));
            let (flag, seen) = (cancel.clone(), Arc::new(Mutex::new(Vec::new())));
            let log = seen.clone();
            let step = Arc::new(move |s: Step| {
                log.lock().unwrap().push(s);
                let reached = match (s, at) {
                    (Step::Evaluating { done, .. }, Step::Evaluating { done: d, .. }) => done >= d,
                    (Step::Baking { done, .. }, Step::Baking { done: d, .. }) => done >= d,
                    _ => false,
                };
                if reached {
                    flag.store(true, Ordering::Relaxed);
                }
            });
            let mut ev = Evaluator::new();
            let err = template.open(&reg, &lib, &mut ev, step, &cancel).err().expect(stops);
            assert_eq!(err.message, crate::eval::CANCELLED, "{stops}");
            let seen = seen.lock().unwrap();
            match at {
                Step::Evaluating { done, .. } => {
                    assert_eq!(seen.last(), Some(&Step::Evaluating { done, of: match seen[1] { Step::Evaluating { of, .. } => of, _ => 0 } }), "no node runs past the cancel");
                    assert!(ev.cached_nodes() <= done + 1);
                }
                _ => assert!(seen.iter().any(|s| matches!(s, Step::Baking { done, .. } if *done > 0)), "the cancel came from inside the bake"),
            }
        }
    }
    use crate::eval::{Evaluator, evaluate_design};
    use crate::registry::Registry;
    use ringdesign_core::AlphaLibrary;
    use ringdesign_core::castability::Verdict;

    fn repo_graphs() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../graphs")
    }

    /// `RD_WRITE_TEMPLATE_GRAPHS=1 cargo test -p ringdesign-graph write_template_graphs`
    /// rewrites the committed files from the builders.
    #[test]
    fn write_template_graphs() {
        if std::env::var_os("RD_WRITE_TEMPLATE_GRAPHS").is_none() {
            return;
        }
        let dir = repo_graphs();
        std::fs::create_dir_all(dir.join("templates")).unwrap();
        for t in BUNDLED {
            let mut g = build(t.name).unwrap();
            arrange(&mut g);
            std::fs::write(dir.join("templates").join(format!("{}.graph.json", t.slug)), crate::file::graph_to_string(&g).unwrap()).unwrap();
        }
        let mut s = build_simple();
        arrange(&mut s);
        std::fs::write(dir.join("simple.graph.json"), crate::file::graph_to_string(&s).unwrap()).unwrap();
        std::fs::create_dir_all(dir.join("clusters")).unwrap();
        std::fs::create_dir_all(dir.join("presets")).unwrap();
        std::fs::write(dir.join("clusters/signet.cluster.json"), crate::file::graph_to_string(&build_signet_cluster()).unwrap()).unwrap();
        std::fs::write(dir.join("clusters/vine-semi-mount.cluster.json"), crate::file::graph_to_string(&build_vine_cluster()).unwrap()).unwrap();
        for (name, slug) in GOTHIC_CLUSTERS {
            let g = build_gothic_cluster(name).unwrap();
            std::fs::write(dir.join("clusters").join(format!("{slug}.cluster.json")), crate::file::graph_to_string(&g).unwrap()).unwrap();
        }
        for p in build_presets() {
            std::fs::write(dir.join("presets").join(format!("{}.preset.json", crate::file::slug(&p.name))), crate::file::preset_to_string(&p).unwrap()).unwrap();
        }
    }

    /// The signet cluster with a preset is the code template, byte for byte.
    #[test]
    fn the_signet_cluster_under_a_preset_is_the_template_byte_for_byte() {
        let reg = Registry::builtin();
        let lib = AlphaLibrary::builtin();
        let clusters = bundled_clusters();
        assert!(!clusters.is_empty());
        assert_eq!(clusters[0], build_signet_cluster(), "the committed cluster has drifted from its builder");
        if let Some(v) = clusters.iter().find(|c| c.name == "Vine semi-mount") {
            assert_eq!(*v, build_vine_cluster(), "the committed vine cluster has drifted from its builder");
        }
        assert!(clusters[0].validate(Some(&reg)).is_empty(), "{:?}", clusters[0].validate(Some(&reg)));
        let presets = bundled_presets();
        assert_eq!(presets.len(), 2);
        assert_eq!(presets, build_presets());
        for preset in &presets {
            let mut g = Graph::new("outer", Mode::SandRing);
            let n = crate::nodes::cluster::add_cluster(&mut g, &clusters[0]).unwrap();
            let unknown = preset.apply(g.node_mut(n).unwrap(), &reg);
            assert!(unknown.is_empty(), "{unknown:?}");
            let out = g.add(OUTPUT_KIND).unwrap();
            g.connect(n, "design", out, OUTPUT_DESIGN_PIN).unwrap();
            let res = evaluate_design(&mut Evaluator::new(), &g, &reg, &lib, 0).unwrap_or_else(|e| panic!("{}: {e}", preset.name));
            assert!(res.notes.is_empty(), "{}: {:?}", preset.name, res.notes);
            assert_ne!(res.field.verdict, Verdict::NotCastable, "{}", preset.name);
            if let Some(t) = ringdesign_core::templates::all().iter().find(|t| t.name == preset.name) {
                let got = serde_json::to_string(&*res.design).unwrap();
                let want = serde_json::to_string(&t.design()).unwrap();
                assert_eq!(got, want, "{}: the cluster's design differs from the code template", preset.name);
            }
        }
        // The file layer sees bundled clusters and presets, user ones first.
        assert!(crate::file::load_cluster("Signet", Some(&reg)).is_some());
        assert!(crate::file::list_presets().iter().any(|p| p.name == "Heart signet"));
    }

    /// `cluster` as a node in a graph of its own with `values` on its pins: the evaluation and the node.
    fn run_cluster(cluster: &Graph, values: &[(&str, Literal)]) -> (crate::eval::EvalReport, NodeId) {
        let reg = Registry::builtin();
        let mut g = Graph::new("outer", Mode::SandRing);
        let node = crate::nodes::cluster::add_cluster(&mut g, cluster).unwrap();
        for (pin, v) in values {
            g.set_input(node, *pin, v.clone()).unwrap();
        }
        assert!(g.validate(Some(&reg)).is_empty(), "{:?}", g.validate(Some(&reg)));
        let r = Evaluator::new().evaluate(&g, &reg, &AlphaLibrary::builtin(), 0, crate::eval::Targets::AllPure);
        assert!(!r.any_failed() && r.errors.is_empty(), "{}: {:?}", cluster.name, r.notes(&g));
        (r, node)
    }

    /// The Sketch operation a construction's sketch makes, under the name a cluster gives it.
    fn sketch_op(mut sketch: ringdesign_core::sketch::Sketch, name: &str) -> serde_json::Value {
        sketch.name = name.into();
        serde_json::to_value(ringdesign_core::cad::Operation::Sketch { sketch }).unwrap()
    }

    /// Each Gothic cluster is its committed file, reads at the plain graph format, and draws the core construction it wraps, byte for byte.
    #[test]
    fn the_gothic_clusters_are_their_files_and_draw_their_constructions() {
        use ringdesign_core::sketch::gothic::{self, Head};
        use ringdesign_core::sizing::RingSize;
        let reg = Registry::builtin();
        let bundled = bundled_clusters();
        let mut clusters = std::collections::BTreeMap::new();
        for (name, slug) in GOTHIC_CLUSTERS {
            let built = build_gothic_cluster(name).unwrap();
            let file = bundled.iter().find(|c| c.name == *name).unwrap_or_else(|| panic!("{name} is not bundled"));
            assert_eq!(*file, built, "{name}: the committed cluster has drifted from its builder; rerun with RD_WRITE_TEMPLATE_GRAPHS=1");
            assert!(file.validate(Some(&reg)).is_empty(), "{name}: {:?}", file.validate(Some(&reg)));
            assert_eq!(crate::file::graph_version_for(file), crate::file::PLAIN_GRAPH_FORMAT_VERSION, "{name} reads in every released build");
            assert!(ringdesign_assets::find(ringdesign_assets::CLUSTERS, slug).is_some(), "{slug} is in the binary");
            assert_eq!(crate::file::load_cluster(name, Some(&reg)).as_ref(), Some(file));
            clusters.insert(*name, built);
        }
        let json = |r: &crate::eval::EvalReport, node: NodeId, pin: &str| r.value(node, pin).and_then(crate::value::Value::to_json_any).unwrap_or_else(|| panic!("{pin}"));
        let bore = |size: f64| RingSize(size).inner_diameter_mm() * 0.5;

        // The wheel's circles follow the ring size: at Tenebrae's bore, and two sizes up with twenty lights on plane 2.
        let wheel = &clusters["Wheel window"];
        for (size, count, plane) in [(tenebrae_size(), 24u32, None), (tenebrae_size() + 2.0, 20, Some(2i64))] {
            let mut values = vec![("Size", Literal::Number(size)), ("Lights", Literal::Int(i64::from(count)))];
            values.extend(plane.map(|p| ("Plane", Literal::Int(p))));
            let (r, node) = run_cluster(wheel, &values);
            let b = bore(size);
            let l = gothic::Lights { count, sill_r_mm: b + 1.0, apex_r_mm: b + 3.1 - 0.35, ..gothic::Lights::default() };
            let mut want = gothic::lights(&l).unwrap();
            if let Some(p) = plane {
                want.plane.on_face = Some(ringdesign_core::sketch::FaceAnchor { feature: p as u64, face: ringdesign_core::cad::FaceRef::bare(0) });
            }
            assert_eq!(serde_json::to_string(&json(&r, node, "sketch_op")).unwrap(), serde_json::to_string(&sketch_op(want, "Wheel window")).unwrap());
            assert_eq!(r.value(node, "lights"), Some(&crate::value::Value::Int(i64::from(count))));
            assert!((r.value(node, "land_mm").and_then(crate::value::Value::as_number).unwrap() - 0.9).abs() < 1e-6);
        }

        // The section stands on the bore its size gives.
        let (r, node) = run_cluster(&clusters["Pointed-arch section"], &[("Keel", Literal::Number(0.6))]);
        let want = gothic::arch_section(&gothic::ArchSection { bore_r_mm: bore(tenebrae_size()), keel: 0.6, ..gothic::ArchSection::default() }).unwrap();
        assert_eq!(serde_json::to_string(&json(&r, node, "sketch_op")).unwrap(), serde_json::to_string(&sketch_op(want, "Pointed-arch section")).unwrap());

        // The rose's lights stand half a bar inside each net circle.
        let (r, node) = run_cluster(&clusters["Rose tracery"], &[("Head", Literal::Text("Mouchette".into())), ("Bar", Literal::Number(0.6))]);
        let l = gothic::Lights { count: 8, sill_r_mm: 3.6 + 0.6 * 0.5, apex_r_mm: 7.1 - 0.6 * 0.5, bar_mm: 0.6, head: Head::Mouchette, ..gothic::Lights::default() };
        assert_eq!(serde_json::to_string(&json(&r, node, "sketch_op")).unwrap(), serde_json::to_string(&sketch_op(gothic::lights(&l).unwrap(), "Rose tracery")).unwrap());
        assert_eq!(r.value(node, "count"), Some(&crate::value::Value::Int(8)));
        let (r, node) = run_cluster(&clusters["Rose tracery"], &[]);
        let l = gothic::Lights { count: 8, sill_r_mm: 3.6 + 0.8 * 0.5, apex_r_mm: 7.1 - 0.8 * 0.5, bar_mm: 0.8, head: Head::Trefoil, ..gothic::Lights::default() };
        assert_eq!(serde_json::to_string(&json(&r, node, "sketch_op")).unwrap(), serde_json::to_string(&sketch_op(gothic::lights(&l).unwrap(), "Rose tracery")).unwrap());

        // The arcade draws its niches and its stamp outline from the same bays.
        let (r, node) = run_cluster(&clusters["Lancet arcade"], &[("Head", Literal::Text("Trefoil".into())), ("Y", Literal::Number(11.0))]);
        let a = gothic::Arcade { head: Head::Trefoil, ..gothic::Arcade::default() };
        let want = gothic::arcade(&a, [0.0, 11.0], 0.0).unwrap();
        assert_eq!(serde_json::to_string(&json(&r, node, "sketch_op")).unwrap(), serde_json::to_string(&sketch_op(want, "Lancet arcade")).unwrap());
        assert_eq!(r.value(node, "outline"), Some(&crate::value::Value::from(gothic::arcade_outline(&a, ringdesign_core::outline::STEP).unwrap())));
        assert_eq!(r.value(node, "bay_mm"), Some(&crate::value::Value::Number(a.bay_mm())));
    }

    #[test]
    fn every_bundled_graph_equals_its_builder_and_its_code_template_byte_for_byte() {
        let reg = Registry::builtin();
        let lib = AlphaLibrary::builtin();
        let code: Vec<_> = ringdesign_core::templates::all().iter().collect();
        assert_eq!(BUNDLED.len(), code.len(), "every code template has a graph");
        for t in BUNDLED {
            let bundled = crate::file::load_graph_str(&t.json(), Some(&reg)).unwrap_or_else(|e| panic!("{}: {e}", t.name));
            let mut built = build(t.name).unwrap();
            arrange(&mut built);
            assert_eq!(bundled, built, "{}: the committed file has drifted from its builder — rerun with RD_WRITE_TEMPLATE_GRAPHS=1", t.name);
            assert!(bundled.validate(Some(&reg)).is_empty(), "{}: {:?}", t.name, bundled.validate(Some(&reg)));
            assert!(bundled.nodes.iter().filter(|n| n.kind == "design.set").count() <= 4, "{} carries too many opaque patches", t.name);
            assert!(t.json().len() <= 300_000, "{} exceeds the procedural template budget", t.name);
            let out = evaluate_design(&mut Evaluator::new(), &bundled, &reg, &lib, 0).unwrap_or_else(|e| panic!("{}: {e}", t.name));
            assert!(out.notes.is_empty(), "{}: {:?}", t.name, out.notes);
            let want = code.iter().find(|c| c.name == t.name).unwrap_or_else(|| panic!("{} is not a code template", t.name)).design();
            let got = serde_json::to_string(&*out.design).unwrap();
            let expect = serde_json::to_string(&want).unwrap();
            if got != expect {
                let g: serde_json::Value = serde_json::from_str(&got).unwrap();
                let e: serde_json::Value = serde_json::from_str(&expect).unwrap();
                let mut diffs = Vec::new();
                crate::lift::diff(&g, &e, "", &mut diffs);
                panic!("{}: the graph's design differs from the code template at {:?}", t.name, diffs.iter().map(|(p, v)| format!("{p} -> {v}")).collect::<Vec<_>>());
            }
            assert_ne!(out.field.verdict, Verdict::NotCastable, "{}: {:?}", t.name, out.field.notes);
        }
        let simple = simple();
        assert!(simple.validate(Some(&reg)).is_empty());
        let out = evaluate_design(&mut Evaluator::new(), &simple, &reg, &lib, 0).unwrap();
        assert_eq!(out.design.name, "Simple ring");
        assert_eq!(simple.exposed.len(), 7);
    }
}
