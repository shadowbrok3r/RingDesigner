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
pub mod drawn;
pub mod engine;
pub mod field;
pub mod library;
pub mod mesh;
pub mod metal;
pub mod profile;
pub mod refine;
pub mod sizing;
pub mod stl;
pub mod tiling;

pub use alpha::{Alpha, AlphaLibrary};
pub use castability::{CastReport, DraftSettings, FaceClass, Section};
pub use drawn::{DrawnAlpha, Stroke};
pub use engine::{DesignEngine, SharedEngine};
pub use field::{Blend, FieldContext, Layer, LayerEntry, LayerStack, SideFaces, Uv, Window};
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

    /// Inner (finger-hole) radius in mm.
    pub fn inner_radius_mm(&self) -> f64 {
        self.size.inner_diameter_mm() * 0.5
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
        }
    }
}
