//! ringdesign-core — procedural, sand-castable ring generation.
//!
//! # Model
//!
//! A ring is a closed cross-section profile swept 360° about the finger axis
//! (Z), with every decorative element expressed as a scalar height field
//! `h(u, v)` displacing that surface along its outward normal.
//!
//! - `u` — arc distance around the ring at the crest radius (mm), wraps at the
//!   circumference.
//! - `v` — arc distance across the cross-section (mm), measured along the
//!   non-bore boundary: up one side face, over the outer surface, down the
//!   other side face.
//!
//! Tiled alphas, borders, milgrain, and raised gem-seat pads are all layers in
//! that field, so tiling, the unrolled layout editor, draft analysis, and
//! cross-sections all reduce to evaluating the same function.
//!
//! # Castability
//!
//! The mold parts along a plane perpendicular to Z and pulls in ±Z. The base
//! profile drops monotonically from a single crest, so the base surface is
//! undercut-free by construction; only the height field can introduce
//! undercuts, and [`castability::analyze`] reports where.

pub mod adaptive;
pub mod imported_base;
pub mod alpha;
pub mod castability;
pub mod contour;
pub mod csg;
pub mod curve;
pub mod dfm;
pub mod drawn;
pub mod engine;
pub mod field;
pub mod gem;
pub mod gems;
pub mod history;
pub mod gltf;
pub mod library;
pub mod manufacturing;
pub mod sketch;
pub mod cad;
pub mod resize;
pub mod mesh;
pub mod metal;
pub mod paint;
pub mod pave;
pub mod profile;
pub mod refine;
pub mod render;
pub mod reptile;
pub mod setstone;
pub mod setting;
pub mod skin;
pub mod parts;
pub mod pins;
pub mod sizing;
pub mod spec;
pub mod stl;
pub mod stonemap;
pub mod stones;
pub mod svg;
pub mod templates;
pub mod construction;
pub mod text;
pub mod threemf;
pub mod tiling;
pub mod interaction;
pub mod outline;

pub use alpha::{Alpha, AlphaLibrary};
pub use castability::{CastReport, DraftSettings, FaceClass, Section};
pub use drawn::{DrawnAlpha, Stroke};
pub use engine::{DesignEngine, SharedEngine};
pub use field::{Blend, CustomOutline, FieldContext, Layer, LayerEntry, LayerStack, SideFaces, Uv, Window};
pub use mesh::{BuildParams, BuildResult, Mesh, Report, Vec3, build};
pub use profile::{BandProfile, ProfileLoop, ProfileSample, ProfileStyle, ShankKind, ShankStyle};
pub use sizing::RingSize;

use serde::{Deserialize, Serialize};

/// A complete ring design: base geometry plus the decorative layer stack.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RingDesign {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_base: Option<imported_base::ImportedBase>,
    pub size: RingSize,
    pub profile: BandProfile,
    pub shank: ShankStyle,
    pub layers: LayerStack,
    pub build: BuildParams,
    pub draft: DraftSettings,
    /// Optional workshop setup; absent in designs created before the casting workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manufacturing: Option<manufacturing::Setup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cad: Option<cad::Document>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub casting_trials: Vec<manufacturing::trials::Trial>,
    /// Alphas drawn by hand, carried as strokes so the design stays self-contained. Rasterized
    /// into the library on load; layers reference them by name like any other alpha.
    #[serde(default)]
    pub drawn: Vec<DrawnAlpha>,
    /// Imported alphas the layer stack references, carried as PNG data so the
    /// design survives moving to a machine without them.
    #[serde(default)]
    pub embedded: Vec<EmbeddedAlpha>,
    /// Inscriptions carried as text and rasterized into the library on load,
    /// the same way drawn alphas travel as strokes.
    #[serde(default)]
    pub texts: Vec<text::TextAlpha>,
    /// Imported vector art carried as SVG text, rasterized on load.
    #[serde(default)]
    pub svgs: Vec<svg::SvgAlpha>,
    /// Parameterized builtin generators, rasterized on load.
    #[serde(default)]
    pub recipes: Vec<alpha::ProcRecipe>,
    /// The graph this design was evaluated from, carried as provenance the
    /// way a generated group carries its recipe: live until baked, opaque
    /// here (`ringdesign-graph` reads it), absent on a hand-made design.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<serde_json::Value>,
    /// Outlines extruded off the built surface and joined to it, or cut from it, by boolean: relief
    /// with true walls and a crisp silhouette, which the height field cannot hold at any resolution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stamps: Vec<setting::Stamp>,
    /// Named points on the ring the snaps and Measure read from; the file carries them and the geometry never reads them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<pins::Pin>,
}

/// One imported alpha embedded in the design file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmbeddedAlpha {
    pub name: String,
    /// Base64 of a 16-bit grayscale PNG.
    pub png: String,
}

impl Default for RingDesign {
    fn default() -> Self {
        Self {
            name: "Untitled".into(),
            imported_base: None,
            size: RingSize(7.0),
            profile: BandProfile::default(),
            shank: ShankStyle::default(),
            layers: LayerStack::default(),
            build: BuildParams::default(),
            draft: DraftSettings::default(),
            manufacturing: None,
            cad: None,
            casting_trials: Vec::new(),
            drawn: Vec::new(),
            embedded: Vec::new(),
            texts: Vec::new(),
            svgs: Vec::new(),
            recipes: Vec::new(),
            graph: None,
            stamps: Vec::new(),
            pins: Vec::new(),
        }
    }
}

/// One source of artwork a design carries, rasterized into the library on load.
enum Artwork<'a> {
    Embedded(&'a EmbeddedAlpha),
    Drawn(&'a DrawnAlpha),
    Text(&'a text::TextAlpha),
    Svg(&'a svg::SvgAlpha),
    Recipe(&'a alpha::ProcRecipe),
}

impl Artwork<'_> {
    /// The library name its raster lands under.
    fn name(&self) -> &str {
        match self {
            Artwork::Embedded(e) => &e.name,
            Artwork::Drawn(d) => &d.name,
            Artwork::Text(t) => &t.name,
            Artwork::Svg(s) => &s.name,
            Artwork::Recipe(r) => &r.name,
        }
    }

    /// Its raster, shared with every earlier bake of the same content.
    fn bake(&self) -> Option<std::sync::Arc<Alpha>> {
        use base64::Engine as _;
        match self {
            Artwork::Embedded(e) => alpha::shared_bake(alpha::source_key("embedded", e), || {
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(&e.png)
                    .map_err(anyhow::Error::from)
                    .and_then(|bytes| Alpha::from_png16(&e.name, &bytes));
                decoded.map_err(|err| log::warn!("could not unpack embedded alpha {}: {err}", e.name)).ok()
            }),
            Artwork::Drawn(d) => alpha::shared_bake(alpha::source_key("drawn", d), || Some(d.rasterize())),
            Artwork::Text(t) => alpha::shared_bake(alpha::source_key("text", t), || Some(t.rasterize())),
            Artwork::Svg(s) => alpha::shared_bake(alpha::source_key("svg", s), || Some(s.rasterize())),
            Artwork::Recipe(r) => alpha::shared_bake(alpha::source_key("recipe-256", r), || Some(r.rasterize(256))),
        }
    }
}

/// `each` over `items` on every core when the `parallel` feature is on and there are two or more, in order either way.
fn map_items<T: Sync, U: Send>(items: &[T], each: impl Fn(&T) -> U + Sync + Send) -> Vec<U> {
    #[cfg(feature = "parallel")]
    if items.len() > 1 {
        use rayon::prelude::*;
        return items.par_iter().map(each).collect();
    }
    items.iter().map(each).collect()
}

impl RingDesign {
    /// The design's artwork in the order it lands: embedded images, then drawings, inscriptions, SVG art and recipes.
    fn artwork(&self, embedded: bool, derived: bool) -> Vec<Artwork<'_>> {
        let mut out = Vec::new();
        if embedded {
            out.extend(self.embedded.iter().map(Artwork::Embedded));
        }
        if derived {
            out.extend(self.drawn.iter().filter(|d| !d.is_empty()).map(Artwork::Drawn));
            out.extend(self.texts.iter().filter(|t| !t.is_empty()).map(Artwork::Text));
            out.extend(self.svgs.iter().filter(|s| !s.is_empty()).map(Artwork::Svg));
            out.extend(self.recipes.iter().map(Artwork::Recipe));
        }
        out
    }

    /// Every alpha a tiling or openwork reads with `edge_mm` set, once each, in stack order.
    fn sdf_sources(&self) -> Vec<&str> {
        fn walk<'a>(stack: &'a LayerStack, out: &mut Vec<&'a str>) {
            for e in &stack.layers {
                let read = match &e.layer {
                    field::Layer::Tiling(t) if t.edge_mm > 1e-9 => Some(t.alpha.as_str()),
                    field::Layer::Openwork(o) if o.tiling.edge_mm > 1e-9 => Some(o.tiling.alpha.as_str()),
                    field::Layer::Group(g) => {
                        walk(&g.stack, out);
                        None
                    }
                    _ => None,
                };
                if let Some(name) = read.filter(|n| !out.contains(n)) {
                    out.push(name);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.layers, &mut out);
        out
    }

    /// Rasterizes `sources` on every core, then with `fields` derives the stack's distance fields on this thread, inserting rasters then fields; the rasters, or `None` once `cancel` is set.
    fn bake_pipeline(
        &self,
        sources: &[Artwork<'_>],
        fields: bool,
        lib: &mut AlphaLibrary,
        progress: &(dyn Fn(usize, usize) + Sync),
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Option<Vec<std::sync::Arc<Alpha>>> {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let read = if fields { self.sdf_sources() } else { Vec::new() };
        // A field reads the last source of its name, else the library's own alpha.
        let last_of = |name: &str| sources.iter().rposition(|a| a.name() == name);
        let chained: Vec<bool> = (0..sources.len()).map(|i| read.iter().any(|n| last_of(n) == Some(i))).collect();
        let held: Vec<std::sync::Arc<Alpha>> = read.iter().filter(|n| last_of(n).is_none()).filter_map(|n| lib.get_shared(n).cloned()).collect();
        let total = sources.len() + chained.iter().filter(|&&c| c).count() + held.len();
        progress(0, total);
        let counted = AtomicUsize::new(0);
        let tick = || progress(counted.fetch_add(1, Ordering::Relaxed) + 1, total);
        let stopped = || cancel.load(Ordering::Relaxed);
        let rasters = map_items(sources, |a| {
            if stopped() {
                return None;
            }
            let raster = a.bake();
            tick();
            raster
        });
        // Each distance field on this thread, its transform on every core.
        for (raster, _) in rasters.iter().zip(&chained).filter(|(_, c)| **c) {
            if stopped() {
                return None;
            }
            if let Some(r) = raster {
                alpha::shared_sdf(r);
            }
            tick();
        }
        for source in &held {
            if stopped() {
                return None;
            }
            alpha::shared_sdf(source);
            tick();
        }
        if stopped() {
            return None;
        }
        let rasters: Vec<std::sync::Arc<Alpha>> = rasters.into_iter().flatten().collect();
        for r in &rasters {
            lib.insert_shared(r.clone());
        }
        // Each field is the shared one of the alpha the library now holds, derived above.
        for name in read {
            if let Some(source) = lib.get_shared(name).cloned() {
                lib.insert_shared(alpha::shared_sdf(&source));
            }
        }
        Some(rasters)
    }

    /// Units of work [`unpack_and_bake_observed`](Self::unpack_and_bake_observed) reports at most: artwork sources and distance fields.
    pub fn bake_units(&self) -> usize {
        self.artwork(true, true).len() + self.sdf_sources().len()
    }

    /// [`unpack_embedded`](Self::unpack_embedded) then [`bake_all`](Self::bake_all) on every core, telling `progress` each unit done; the rasters inserted, or `None` once `cancel` is set.
    pub fn unpack_and_bake_observed(
        &self,
        lib: &mut AlphaLibrary,
        progress: &(dyn Fn(usize, usize) + Sync),
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Option<Vec<std::sync::Arc<Alpha>>> {
        self.bake_pipeline(&self.artwork(true, true), true, lib, progress, cancel)
    }

    /// Rasterize every drawn alpha into `lib`, replacing any entry of the same name.
    ///
    /// Call after loading a design and whenever a drawing changes: the strokes are the source of
    /// truth and the raster is derived, so nothing else needs to keep them in step.
    pub fn bake_drawn(&self, lib: &mut AlphaLibrary) {
        let sources: Vec<Artwork<'_>> = self.drawn.iter().filter(|d| !d.is_empty()).map(Artwork::Drawn).collect();
        self.bake_pipeline(&sources, false, lib, &|_, _| {}, &Default::default());
    }

    /// Rasterize every inscription into `lib`, replacing same-named entries.
    /// Call wherever [`bake_drawn`](Self::bake_drawn) is called.
    pub fn bake_texts(&self, lib: &mut AlphaLibrary) {
        let sources: Vec<Artwork<'_>> = self.texts.iter().filter(|t| !t.is_empty()).map(Artwork::Text).collect();
        self.bake_pipeline(&sources, false, lib, &|_, _| {}, &Default::default());
    }

    /// Rasterize every imported SVG into `lib`, replacing same-named entries.
    /// Call wherever [`bake_drawn`](Self::bake_drawn) is called.
    pub fn bake_svgs(&self, lib: &mut AlphaLibrary) {
        let sources: Vec<Artwork<'_>> = self.svgs.iter().filter(|s| !s.is_empty()).map(Artwork::Svg).collect();
        self.bake_pipeline(&sources, false, lib, &|_, _| {}, &Default::default());
    }

    /// Derive the signed-distance field of every alpha a tiling reads with
    /// `edge_mm` set. Derived data: regenerable from the source, never saved.
    /// Each is derived once per shared source alpha and reused after.
    pub fn bake_sdfs(&self, lib: &mut AlphaLibrary) {
        self.bake_pipeline(&[], true, lib, &|_, _| {}, &Default::default());
    }

    /// Whether any layer reading a distance field is missing one.
    ///
    /// `TilingLayer::height` falls back to brightness-as-height when the
    /// `##sdf` entry is absent, silently — so turning "Crisp edge" on in the
    /// editor produced a layer that looked like the old one and nobody could
    /// say why. Cheap: a walk of the stack and a map lookup per edge-enabled
    /// layer, so an editor can ask on every edit and only pay for the bake
    /// when the answer is yes.
    pub fn sdfs_missing(&self, lib: &AlphaLibrary) -> bool {
        fn walk(stack: &LayerStack, lib: &AlphaLibrary) -> bool {
            stack.layers.iter().any(|e| match &e.layer {
                field::Layer::Tiling(t) if t.edge_mm > 1e-9 => {
                    lib.get(&crate::alpha::sdf_name(&t.alpha)).is_none()
                }
                field::Layer::Openwork(o) if o.tiling.edge_mm > 1e-9 => {
                    lib.get(&crate::alpha::sdf_name(&o.tiling.alpha)).is_none()
                }
                field::Layer::Group(g) => walk(&g.stack, lib),
                _ => false,
            })
        }
        walk(&self.layers, lib)
    }

    /// Rasterize every parameterized generator recipe into `lib`.
    pub fn bake_recipes(&self, lib: &mut AlphaLibrary) {
        let sources: Vec<Artwork<'_>> = self.recipes.iter().map(Artwork::Recipe).collect();
        self.bake_pipeline(&sources, false, lib, &|_, _| {}, &Default::default());
    }

    /// Every derived bake in order — strokes, inscriptions, SVG art,
    /// generator recipes, then the distance fields that read the results.
    /// The one call every load site makes; adding a bake means adding it
    /// here, not at six sites. A source baked before is shared, not redrawn.
    pub fn bake_all(&self, lib: &mut AlphaLibrary) {
        self.bake_pipeline(&self.artwork(false, true), true, lib, &|_, _| {}, &Default::default());
    }

    /// Capture every referenced alpha that cannot be regenerated — not a
    /// builtin, not drawn — as embedded PNG data. Call on a save-time clone.
    pub fn embed_alphas(&mut self, lib: &AlphaLibrary) {
        use base64::Engine as _;
        self.embedded.clear();
        for name in self.layers.referenced_alphas() {
            let regenerable = alpha::Procedural::ALL.iter().any(|p| p.label() == name)
                || self.drawn.iter().any(|d| d.name == name)
                || self.texts.iter().any(|t| t.name == name);
            if regenerable {
                continue;
            }
            let Some(a) = lib.get(name) else { continue };
            match a.to_png16() {
                Ok(png) => self.embedded.push(EmbeddedAlpha {
                    name: name.to_string(),
                    png: base64::engine::general_purpose::STANDARD.encode(png),
                }),
                Err(e) => log::warn!("could not embed alpha {name}: {e}"),
            }
        }
    }

    /// Insert embedded alphas into `lib`. The local library wins on a name
    /// collision, so a machine that has the original keeps using it.
    pub fn unpack_embedded(&self, lib: &mut AlphaLibrary) {
        // A design's own embedded copy is authoritative *for that
        // design*, and the library accumulates for the whole session.
        // Skipping a name already present meant that opening a second
        // design carrying its own "band" or "sketch" silently rendered
        // the first one's art. `embed_alphas` never embeds anything
        // regenerable — no procedural builtin, no stroke, no inscription —
        // so replacing here cannot clobber one of those.
        self.bake_pipeline(&self.artwork(true, false), false, lib, &|_, _| {}, &Default::default());
    }

    /// Whether the ring is the swept band: no CAD document, or one whose parts stand on a
    /// `Band` feature. A document without one is the whole ring and builds through the kernel.
    pub fn band_is_procedural(&self) -> bool {
        self.cad.as_ref().is_none_or(|doc| !doc.replaces_band())
    }

    /// Inner (finger-hole) radius in mm.
    pub fn inner_radius_mm(&self) -> f64 {
        self.size.inner_diameter_mm() * 0.5
    }

    /// Shank modulation plus the profile morph at a ring angle. Every
    /// consumer of a modulated section goes through this, so the mesh, the
    /// section view and refinement always agree.
    pub fn modulation_at(&self, theta_deg: f64, inner_r: f64, crest_r: f64) -> profile::ShankMod {
        let mut m = self.shank.modulation(theta_deg, inner_r, crest_r, &self.profile);
        m.drop_blend = self.profile.morph_weight(theta_deg);
        m
    }

    /// The common base-surface path for sections, stones, picking and analysis.
    pub fn section_at(&self, theta: f64, steps: usize, density: Option<&adaptive::Density>, reference: Option<&ProfileLoop>) -> ProfileLoop {
        if let Some(base) = &self.imported_base {
            return base.section(self, theta, steps).unwrap_or_default();
        }
        let crest = reference.map(|r|r.crest_radius_mm).unwrap_or_else(||self.reference_loop().crest_radius_mm);
        let m = self.modulation_at(theta, self.inner_radius_mm(), crest);
        self.profile.sample_spaced(self.inner_radius_mm(), steps, &m, density, reference)
    }

    /// The reference cross-section used to parameterize the height field: the
    /// unmodulated profile, so `v` stays put as the shank tapers.
    ///
    /// Sampled at a fixed count rather than the build's, so `band_v_len_mm` —
    /// and with it the scale of every layer — is the same at preview and at
    /// export resolution. Adaptive spacing also derives from this, and a `v`
    /// span that moved with the sampling would make that circular.
    pub fn reference_loop(&self) -> ProfileLoop {
        if let Some(chart) = self.imported_base.as_ref().and_then(|b|b.chart.as_ref()) {
            return chart.profile.sample(self.inner_radius_mm(), profile::REFERENCE_PROFILE_STEPS);
        }
        self.profile
            .sample(self.inner_radius_mm(), profile::REFERENCE_PROFILE_STEPS)
    }

    /// Unrolled-space context for evaluating the layer stack.
    pub fn field_context(&self) -> FieldContext {
        let loop_ = self.reference_loop();
        let imported_surface=self.imported_base.as_ref().and_then(|b|b.field_surface(self).ok());
        let mut imported_seats=std::collections::HashMap::new();
        if let Some(surface)=&imported_surface {
            fn collect(stack:&field::LayerStack,surface:&imported_base::FieldSurface,span:f64,out:&mut std::collections::HashMap<(u64,u64),imported_base::TangentFrame>,depth:usize) {
                if depth>field::MAX_GROUP_DEPTH {return;}
                for e in &stack.layers {
                    match &e.layer {
                        field::Layer::SeatPad(s) if s.metal_true=>{out.insert((s.theta_deg.to_bits(),s.v_mm.to_bits()),surface.frame(s.theta_deg,s.v_mm/span));},
                        field::Layer::Group(g)=>collect(&g.stack,surface,span,out,depth+1),
                        _=>{},
                    }
                }
            }
            collect(&self.layers,surface,loop_.surface_len_mm.max(1e-9),&mut imported_seats,0);
        }
        let tables = self.station_tables();
        FieldContext {
            circumference_mm: std::f64::consts::TAU * loop_.crest_radius_mm,
            band_v_len_mm: loop_.surface_len_mm,
            crest_v_mm: loop_.crest_v_mm,
            crest_radius_mm: loop_.crest_radius_mm,
            surface: field::SurfaceProfile::from_loop(&loop_, 257),
            bore_radius_mm: self.inner_radius_mm(),
            side_faces_cache: Default::default(),
            stretch: tables.as_ref().map(|t| t.0.clone()),
            crest_scale: tables.map(|t| t.1),
            imported_surface,
            imported_seats,
        }
    }

    /// [`FieldContext::station_stretch`]'s table: metal mm per chart mm
    /// across the band, one entry per degree — each station's surface arc
    /// over the reference's, both sampled at the same count so an identity
    /// station reads exactly 1. `None` when nothing modulates the band.
    ///
    /// One computation for a ratio the decal measurement, the seat report
    /// and the serpentarium probes each derived on their own — the
    /// divergence this file warns about elsewhere. Cached like the signet
    /// tents; the key is the serialized profile and shank, so a field added
    /// to either can never serve a stale table.
    fn station_tables(&self) -> Option<StationTables> {
        if self.imported_base.is_none() && self.shank.kind == ShankKind::Uniform && self.profile.morph.is_none() {
            return None;
        }
        const STATIONS: usize = 360;
        const SECTION_STEPS: usize = 192;
        let inner = self.inner_radius_mm();
        let crest = inner + self.profile.thickness_mm;
        // Two tables in one: the section's stretch, then its crest radius over the reference's.
        let build = || {
            let reference = if self.imported_base.is_some() { self.reference_loop() } else { self.profile.sample(inner, SECTION_STEPS) };
            let (ref_len, ref_crest) = (reference.surface_len_mm.max(1e-9), reference.crest_radius_mm.max(1e-9));
            let rows: Vec<(f32, f32)> = (0..STATIONS)
                .map(|i| {
                    let theta = i as f64 * 360.0 / STATIONS as f64;
                    let m = self.modulation_at(theta, inner, crest);
                    if let Some(b) = &self.imported_base {
                        return ((b.section(self, theta, SECTION_STEPS).map(|s| s.surface_len_mm).unwrap_or(ref_len) / ref_len) as f32, 1.0);
                    }
                    let section = self.profile.sample_mod(inner, SECTION_STEPS, &m);
                    ((section.surface_len_mm / ref_len) as f32, (section.crest_radius_mm / ref_crest) as f32)
                })
                .collect();
            (rows.iter().map(|r| r.0).collect(), rows.iter().map(|r| r.1).collect())
        };
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        match serde_json::to_vec(&(&self.profile, &self.shank)) {
            Ok(bytes) => bytes.hash(&mut h),
            // A profile that cannot serialize cannot key the cache; build
            // uncached rather than serve someone else's table.
            Err(_) => {
                let (stretch, crest) = build();
                return Some((std::sync::Arc::new(stretch), std::sync::Arc::new(crest)));
            }
        }
        if let Some(b)=&self.imported_base { b.source.fingerprint().hash(&mut h);
            serde_json::to_vec(&b.chart).unwrap_or_default().hash(&mut h); }
        inner.to_bits().hash(&mut h);
        Some(stretch_cached(h.finish(), build))
    }
}

/// Per ring angle: the section's stretch, and its crest radius over the reference's.
type StationTables = (std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>);

/// Stretch tables are rebuilt only when something about the band changes:
/// the last few are kept by a hash of everything that shapes them.
fn stretch_cached(key: u64, build: impl FnOnce() -> (Vec<f32>, Vec<f32>)) -> StationTables {
    static CACHE: std::sync::Mutex<Vec<(u64, StationTables)>> = std::sync::Mutex::new(Vec::new());
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, t)) = c.iter().find(|(k, _)| *k == key) {
        return t.clone();
    }
    let (stretch, crest) = build();
    let t = (std::sync::Arc::new(stretch), std::sync::Arc::new(crest));
    if c.len() >= 8 {
        c.remove(0);
    }
    c.push((key, t.clone()));
    t
}

#[cfg(test)]
mod shared_bake_tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    /// A drawing, an inscription, SVG art and a recipe, with the drawing read through a bevelled tiling.
    fn artful(tag: &str) -> RingDesign {
        let mut d = RingDesign::default();
        let mut drawn = DrawnAlpha::new(format!("Drawn {tag}"), 64, 32);
        let mut stroke = drawn::Stroke::new(0.1, 0.3, false);
        stroke.push(0.2, 0.5, 1.0);
        stroke.push(0.8, 0.5, 1.0);
        drawn.strokes.push(stroke);
        d.drawn.push(drawn);
        d.texts.push(text::TextAlpha { name: format!("Text {tag}"), text: "Ab".into(), ..Default::default() });
        d.svgs.push(svg::SvgAlpha { name: format!("Svg {tag}"), svg: r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><circle cx="16" cy="16" r="9"/></svg>"#.into(), invert: false });
        d.recipes.push(alpha::ProcRecipe { name: format!("Recipe {tag}"), ..Default::default() });
        let ctx = d.field_context();
        let mut tiling = tiling::TilingLayer::default_for(format!("Drawn {tag}"), &ctx);
        tiling.edge_mm = 0.2;
        d.layers.layers.push(LayerEntry::new("Bevelled", Layer::Tiling(tiling)));
        d
    }

    /// Threads this process runs.
    #[cfg(all(target_os = "linux", feature = "parallel"))]
    fn threads() -> usize {
        std::fs::read_dir("/proc/self/task").map(|d| d.count()).unwrap_or(0)
    }

    /// Run alone in a fresh process by the test after it, where no pool exists yet.
    #[cfg(all(target_os = "linux", feature = "parallel"))]
    #[test]
    #[ignore = "run in a process of its own by a_bake_with_nothing_to_bake_starts_no_threads"]
    fn a_fresh_process_bakes_an_artless_design_on_its_own_thread() {
        let before = threads();
        RingDesign::default().bake_all(&mut AlphaLibrary::default());
        assert_eq!(threads(), before, "the bake started the pool");
    }

    #[cfg(all(target_os = "linux", feature = "parallel"))]
    #[test]
    fn a_bake_with_nothing_to_bake_starts_no_threads() {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "shared_bake_tests::a_fresh_process_bakes_an_artless_design_on_its_own_thread", "--ignored", "--test-threads=1"])
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success() && said.contains("1 passed"), "{said}");
    }

    /// Four bevelled drawings whose rasters fall under the counted grid size and whose fields' two 3x-wide grids each reach it.
    #[cfg(feature = "parallel")]
    #[test]
    fn a_bakes_distance_fields_are_derived_on_the_calling_thread() {
        let mut design = RingDesign::default();
        let ctx = design.field_context();
        for k in 0..4 {
            let name = format!("Field grid {k}");
            let mut drawn = DrawnAlpha::new(name.clone(), 256, 128);
            let mut stroke = drawn::Stroke::new(0.05, 0.4, false);
            stroke.push(0.1 + 0.1 * k as f32, 0.3, 1.0);
            stroke.push(0.9, 0.7, 1.0);
            drawn.strokes.push(stroke);
            design.drawn.push(drawn);
            let mut tiling = tiling::TilingLayer::default_for(name, &ctx);
            tiling.edge_mm = 0.2;
            design.layers.layers.push(LayerEntry::new(format!("Bevelled {k}"), Layer::Tiling(tiling)));
        }
        let mut lib = AlphaLibrary::builtin();
        let ((), grids) = alpha::grid_allocation_tests::grids_on_this_thread(|| design.bake_all(&mut lib));
        assert!((0..4).all(|k| lib.sdf_of(&format!("Field grid {k}")).is_some()));
        assert_eq!(grids, 8, "both grids of each of the four fields");
    }

    #[test]
    fn a_source_baked_once_is_shared_after_and_a_library_holding_it_is_left_as_it_was() {
        let design = artful("shared");
        let (mut first, mut second) = (AlphaLibrary::builtin(), AlphaLibrary::builtin());
        let before = first.revision();
        design.unpack_embedded(&mut first);
        design.bake_all(&mut first);
        assert_ne!(first.revision(), before);
        design.bake_all(&mut second);
        for name in ["Drawn shared", "Text shared", "Svg shared", "Recipe shared", "Drawn shared##sdf"] {
            let (a, b) = (first.get_shared(name).expect(name), second.get_shared(name).expect(name));
            assert!(Arc::ptr_eq(a, b), "{name} is one raster in both libraries");
        }
        let held = first.revision();
        design.bake_all(&mut first);
        assert_eq!(first.revision(), held, "baking what a library already holds changes nothing");
        // A drawing redrawn is a new raster, and its field follows it.
        let mut redrawn = design.clone();
        redrawn.drawn[0].strokes[0].push(0.5, 0.9, 1.0);
        redrawn.bake_all(&mut first);
        assert_ne!(first.revision(), held);
        assert!(!Arc::ptr_eq(first.get_shared("Drawn shared").unwrap(), second.get_shared("Drawn shared").unwrap()));
        assert!(!Arc::ptr_eq(first.get_shared("Drawn shared##sdf").unwrap(), second.get_shared("Drawn shared##sdf").unwrap()));
        assert!(Arc::ptr_eq(first.get_shared("Svg shared").unwrap(), second.get_shared("Svg shared").unwrap()));
    }

    #[test]
    fn an_observed_bake_counts_every_source_and_field_and_stops_when_cancelled() {
        let design = artful("observed");
        assert_eq!(design.bake_units(), 5);
        let (seen, top) = (AtomicUsize::new(0), AtomicUsize::new(0));
        let mut lib = AlphaLibrary::builtin();
        let baked = design
            .unpack_and_bake_observed(&mut lib, &|done, of| {
                seen.fetch_add(1, Ordering::Relaxed);
                top.fetch_max(done, Ordering::Relaxed);
                assert_eq!(of, 5);
            }, &AtomicBool::new(false))
            .expect("not cancelled");
        assert_eq!((seen.into_inner(), top.into_inner()), (6, 5), "the size, then one per unit");
        assert_eq!(baked.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["Drawn observed", "Text observed", "Svg observed", "Recipe observed"]);
        assert!(lib.sdf_of("Drawn observed").is_some());
        let mut untouched = AlphaLibrary::builtin();
        let revision = untouched.revision();
        assert!(artful("stopped").unpack_and_bake_observed(&mut untouched, &|_, _| {}, &AtomicBool::new(true)).is_none());
        assert_eq!(untouched.revision(), revision, "a bake cancelled before it starts inserts nothing");
    }

    #[test]
    fn the_revision_moves_only_when_the_content_does() {
        let mut lib = AlphaLibrary::default();
        assert_eq!(lib.revision(), 0);
        let a = Arc::new(Alpha::new("a", 2, 2, vec![0.5; 4]));
        lib.insert_shared(a.clone());
        let one = lib.revision();
        assert_ne!(one, 0);
        lib.insert_shared(a.clone());
        assert_eq!(lib.revision(), one, "the alpha it holds, inserted again");
        let copy = lib.clone();
        assert_eq!(copy.revision(), one);
        lib.insert(Alpha::new("a", 2, 2, vec![0.5; 4]));
        assert_ne!(lib.revision(), one, "equal texels in a new alpha are a change");
        let two = lib.revision();
        assert!(lib.remove("a") && lib.revision() != two);
        assert_eq!(copy.revision(), one, "a clone keeps its own");
    }

    #[test]
    fn a_mask_measured_on_several_threads_at_once_reads_one_measure() {
        let mask = Alpha::new("stripes", 48, 48, (0..48 * 48).map(|i| if (i % 48) / 6 % 2 == 0 { 1.0 } else { 0.0 }).collect());
        let got: Vec<Option<(f64, f64)>> = std::thread::scope(|s| (0..6).map(|_| s.spawn(|| mask.min_feature_px())).collect::<Vec<_>>().into_iter().map(|t| t.join().unwrap()).collect());
        assert!(got[0].is_some() && got.iter().all(|g| *g == got[0]), "{got:?}");
        assert_eq!(mask.min_feature_px(), got[0]);
    }
}

#[cfg(test)]
mod losing_work_tests {
    use super::*;

    /// `fs::write` truncates before it writes, so an interrupted save of a
    /// design carrying embedded PNGs left nothing at all where a stale file
    /// would have been fine. The write is atomic now, and it keeps one
    /// generation back.
    #[test]
    fn a_save_is_atomic_and_leaves_a_backup() {
        let dir = std::env::temp_dir().join("ringdesign-atomic-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("ring.ring.json");

        let mut first = RingDesign::default();
        first.name = "first".into();
        library::save_design(&path, &first).unwrap();
        assert!(path.exists());
        assert!(!path.with_extension("json.bak").exists(), "nothing to back up yet");

        let mut second = RingDesign::default();
        second.name = "second".into();
        library::save_design(&path, &second).unwrap();

        let now = std::fs::read_to_string(&path).unwrap();
        assert!(now.contains("second"));
        let bak = std::fs::read_to_string(path.with_extension("json.bak")).unwrap();
        assert!(bak.contains("first"), "the previous save is recoverable");
        // No temp left behind.
        assert!(!path.with_extension("json.tmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The library accumulates for a whole session, and `unpack_embedded`
    /// skipped any name already present — so opening a second design that
    /// carries its own alpha under a name the first one used rendered the
    /// first one's art, silently.
    #[test]
    fn a_second_design_brings_its_own_art() {
        let mut lib = AlphaLibrary::builtin();
        let art = |name: &str, v: f32| {
            crate::alpha::Alpha::new(name.to_string(), 4, 4, vec![v; 16])
        };

        let mut a = RingDesign::default();
        lib.insert(art("shared", 0.25));
        a.layers.layers.push(LayerEntry::new(
            "t",
            Layer::Tiling(crate::tiling::TilingLayer::default_for("shared", &a.field_context())),
        ));
        a.embed_alphas(&lib);
        assert_eq!(a.embedded.len(), 1, "the design carries its own copy");

        // A different design, same name, different art.
        let mut b = a.clone();
        lib.insert(art("shared", 0.75));
        b.embed_alphas(&lib);

        // Open A again into the session's library, which still holds B's.
        a.unpack_embedded(&mut lib);
        let got = lib.get("shared").expect("present");
        assert!(
            (got.data[0] - 0.25).abs() < 1e-3,
            "A's own art, not the one already in the library: {}",
            got.data[0]
        );
    }
}

#[cfg(test)]
mod design_tests {
    use super::*;

    /// Turning `edge_mm` on made a layer read a distance field that nothing
    /// had baked, and `TilingLayer::height` falls back to brightness-as-height
    /// without saying so — the crisp edge simply did not appear. The editor
    /// needs a cheap way to ask, so the bake can run only when one is absent.
    #[test]
    fn a_layer_that_wants_a_distance_field_says_when_it_has_none() {
        let mut lib = crate::AlphaLibrary::builtin();
        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let mut t = crate::tiling::TilingLayer::default_for("Beads", &ctx);
        t.edge_mm = 0.0;
        d.layers
            .layers
            .push(crate::LayerEntry::new("beads", crate::Layer::Tiling(t)));
        assert!(!d.sdfs_missing(&lib), "a layer with no crisp edge wants nothing");

        // The edit a user makes in the panel.
        if let crate::Layer::Tiling(t) = &mut d.layers.layers[0].layer {
            t.edge_mm = 0.35;
        }
        assert!(d.sdfs_missing(&lib), "now it wants one and has none");
        d.bake_sdfs(&mut lib);
        assert!(!d.sdfs_missing(&lib), "and the bake satisfies it");
    }
}
pub mod blend;
