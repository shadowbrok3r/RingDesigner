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
pub mod alpha;
pub mod castability;
pub mod contour;
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
pub mod mesh;
pub mod metal;
pub mod paint;
pub mod pave;
pub mod profile;
pub mod refine;
pub mod render;
pub mod setstone;
pub mod sizing;
pub mod spec;
pub mod stl;
pub mod stonemap;
pub mod stones;
pub mod svg;
pub mod templates;
pub mod text;
pub mod threemf;
pub mod tiling;

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
    pub size: RingSize,
    pub profile: BandProfile,
    pub shank: ShankStyle,
    pub layers: LayerStack,
    pub build: BuildParams,
    pub draft: DraftSettings,
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
            size: RingSize(7.0),
            profile: BandProfile::default(),
            shank: ShankStyle::default(),
            layers: LayerStack::default(),
            build: BuildParams::default(),
            draft: DraftSettings::default(),
            drawn: Vec::new(),
            embedded: Vec::new(),
            texts: Vec::new(),
            svgs: Vec::new(),
            recipes: Vec::new(),
            graph: None,
        }
    }
}

impl RingDesign {
    /// Rasterize every drawn alpha into `lib`, replacing any entry of the same name.
    ///
    /// Call after loading a design and whenever a drawing changes: the strokes are the source of
    /// truth and the raster is derived, so nothing else needs to keep them in step.
    pub fn bake_drawn(&self, lib: &mut AlphaLibrary) {
        for d in &self.drawn {
            if !d.is_empty() {
                lib.insert(d.rasterize());
            }
        }
    }

    /// Rasterize every inscription into `lib`, replacing same-named entries.
    /// Call wherever [`bake_drawn`](Self::bake_drawn) is called.
    pub fn bake_texts(&self, lib: &mut AlphaLibrary) {
        for t in &self.texts {
            if !t.is_empty() {
                lib.insert(t.rasterize());
            }
        }
    }

    /// Rasterize every imported SVG into `lib`, replacing same-named entries.
    /// Call wherever [`bake_drawn`](Self::bake_drawn) is called.
    pub fn bake_svgs(&self, lib: &mut AlphaLibrary) {
        for s in &self.svgs {
            if !s.is_empty() {
                lib.insert(s.rasterize());
            }
        }
    }

    /// Derive the signed-distance field of every alpha a tiling reads with
    /// `edge_mm` set. Derived data: regenerable from the source, never saved.
    pub fn bake_sdfs(&self, lib: &mut AlphaLibrary) {
        fn walk(stack: &LayerStack, lib: &mut AlphaLibrary) {
            for e in &stack.layers {
                match &e.layer {
                    field::Layer::Tiling(t) if t.edge_mm > 1e-9 => {
                        // Recomputed every bake: the source may have been
                        // redrawn, and only edge-enabled layers pay.
                        if let Some(src) = lib.get(&t.alpha).cloned() {
                            lib.insert(src.signed_distance_px());
                        }
                    }
                    field::Layer::Openwork(o) if o.tiling.edge_mm > 1e-9 => {
                        if let Some(src) = lib.get(&o.tiling.alpha).cloned() {
                            lib.insert(src.signed_distance_px());
                        }
                    }
                    field::Layer::Group(g) => walk(&g.stack, lib),
                    _ => {}
                }
            }
        }
        walk(&self.layers, lib);
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
        for r in &self.recipes {
            lib.insert(r.rasterize(256));
        }
    }

    /// Every derived bake in order — strokes, inscriptions, SVG art,
    /// generator recipes, then the distance fields that read the results.
    /// The one call every load site makes; adding a bake means adding it
    /// here, not at six sites.
    pub fn bake_all(&self, lib: &mut AlphaLibrary) {
        self.bake_drawn(lib);
        self.bake_texts(lib);
        self.bake_svgs(lib);
        self.bake_recipes(lib);
        self.bake_sdfs(lib);
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
        use base64::Engine as _;
        for e in &self.embedded {
            // A design's own embedded copy is authoritative *for that
            // design*, and the library accumulates for the whole session.
            // Skipping a name already present meant that opening a second
            // design carrying its own "band" or "sketch" silently rendered
            // the first one's art. `embed_alphas` never embeds anything
            // regenerable — no procedural builtin, no stroke, no inscription —
            // so replacing here cannot clobber one of those.
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(&e.png)
                .map_err(anyhow::Error::from)
                .and_then(|bytes| Alpha::from_png16(&e.name, &bytes));
            match decoded {
                Ok(a) => lib.insert(a),
                Err(err) => log::warn!("could not unpack embedded alpha {}: {err}", e.name),
            }
        }
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

    /// The reference cross-section used to parameterize the height field: the
    /// unmodulated profile, so `v` stays put as the shank tapers.
    ///
    /// Sampled at a fixed count rather than the build's, so `band_v_len_mm` —
    /// and with it the scale of every layer — is the same at preview and at
    /// export resolution. Adaptive spacing also derives from this, and a `v`
    /// span that moved with the sampling would make that circular.
    pub fn reference_loop(&self) -> ProfileLoop {
        self.profile
            .sample(self.inner_radius_mm(), profile::REFERENCE_PROFILE_STEPS)
    }

    /// Unrolled-space context for evaluating the layer stack.
    pub fn field_context(&self) -> FieldContext {
        let loop_ = self.reference_loop();
        FieldContext {
            circumference_mm: std::f64::consts::TAU * loop_.crest_radius_mm,
            band_v_len_mm: loop_.surface_len_mm,
            crest_v_mm: loop_.crest_v_mm,
            crest_radius_mm: loop_.crest_radius_mm,
            surface: field::SurfaceProfile::from_loop(&loop_, 257),
            bore_radius_mm: self.inner_radius_mm(),
            side_faces_cache: Default::default(),
        }
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
