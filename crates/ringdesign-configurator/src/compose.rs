//! The customer's choices, and the one function that turns them into a ring.
//!
//! [`Config`] is small, serializable data — a web frontend or an order file
//! can carry it verbatim — and [`compose`] is pure, so the same choices
//! always build the same design. Every option offered here is castable by
//! construction: stones and patterns are gated to bases that carry a side
//! face, and the test holds every combination to the field verdict.

use ringdesign_core::field::{
    Decal, DecalLayer, Layer, LayerEntry, MilgrainLayer, SeatPadLayer, SeatRunLayer, SeatStyle,
    SignetOutline, VGate, Window, SIDE_FACE_MIN_DRAFT_DEG,
};
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::profile::TOP_DEG;
use ringdesign_core::text::{TextAlpha, TextFont};
use ringdesign_core::{ProfileStyle, RingDesign, RingSize, ShankKind};
use serde::{Deserialize, Serialize};

/// The band the customer starts from. Curated: every base is castable bare.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Base {
    Court,
    WideBand,
    Cathedral,
    Wave,
    Twist,
    Split,
    SignetOval,
    SignetHeart,
}

impl Base {
    pub const ALL: &'static [Base] = &[
        Base::Court,
        Base::WideBand,
        Base::Cathedral,
        Base::Wave,
        Base::Twist,
        Base::Split,
        Base::SignetOval,
        Base::SignetHeart,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Base::Court => "Classic court",
            Base::WideBand => "Wide band",
            Base::Cathedral => "Cathedral",
            Base::Wave => "Wishbone wave",
            Base::Twist => "Twist",
            Base::Split => "Split shank",
            Base::SignetOval => "Oval signet",
            Base::SignetHeart => "Heart signet",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Base::Court => "The timeless rounded band.",
            Base::WideBand => "A broad flat band — the canvas for patterns and engraving.",
            Base::Cathedral => "Shoulders that rise toward a center stone.",
            Base::Wave => "One gentle wave, made to sit against a solitaire.",
            Base::Twist => "The light-line spirals around the band.",
            Base::Split => "Reads as two rails from the side.",
            Base::SignetOval => "A classic oval plate, blank for engraving.",
            Base::SignetHeart => "A heart-shaped plate with softly faired sides.",
        }
    }

    /// Whether this base carries the squared side faces that patterns,
    /// engraving and side pavé need.
    pub fn has_sides(self) -> bool {
        matches!(
            self,
            Base::WideBand | Base::Split | Base::SignetOval | Base::SignetHeart
        )
    }

    /// Whether a stone belongs on this base at all — a signet's face is the
    /// feature, and stones would fight it.
    pub fn takes_stones(self) -> bool {
        !matches!(self, Base::SignetOval | Base::SignetHeart)
    }

    /// Whether the crest stays at one `v` all the way round. Wave and Twist
    /// slide their edges along the finger, so a fixed-v bead row lands on
    /// the dome flank and leans — measured 3% at 50° before this gate.
    pub fn crest_is_stationary(self) -> bool {
        !matches!(self, Base::Wave | Base::Twist)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Stone {
    None,
    /// One center stone on a gypsy mound with prong stock.
    Solitaire { mm: f64 },
    /// A row of identical stones around the whole band.
    Eternity { mm: f64 },
}

impl Stone {
    pub fn label(self) -> String {
        match self {
            Stone::None => "No stone".into(),
            Stone::Solitaire { mm } => format!("Solitaire {mm:.1} mm"),
            Stone::Eternity { mm } => format!("Eternity row {mm:.1} mm"),
        }
    }
}

/// Curated side-face patterns — bold builtins that hold up in sand at band
/// scale, with the repeat count that reads well.
pub const PATTERNS: &[(&str, u32)] = &[
    ("Waves", 6),
    ("Braid", 8),
    ("Chevron", 9),
    ("Rope", 10),
    ("Florentine", 7),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub base: Base,
    /// US finger size.
    pub size: f64,
    /// Index into [`ringdesign_core::metal::METALS`].
    pub metal: usize,
    pub stone: Stone,
    /// Index into [`PATTERNS`], on the side faces.
    pub pattern: Option<usize>,
    /// Milgrain beads along the crest.
    pub milgrain: bool,
    /// Raised inscription on the side face.
    pub engraving: String,
    pub script_font: bool,
    /// Customer name, carried into the design and the order file.
    pub customer: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base: Base::Court,
            size: 7.0,
            metal: 0,
            stone: Stone::None,
            pattern: None,
            milgrain: false,
            engraving: String::new(),
            script_font: false,
            customer: String::new(),
        }
    }
}

impl Config {
    /// Drop choices the current base cannot carry, so switching bases never
    /// leaves an impossible combination behind.
    pub fn reconcile(&mut self) {
        if !self.base.takes_stones() {
            self.stone = Stone::None;
        }
        if !self.base.has_sides() {
            self.pattern = None;
            self.engraving.clear();
        }
        if !self.base.crest_is_stationary() {
            self.milgrain = false;
        }
        // Engraving over a pattern is illegible in metal: one per side.
        if !self.engraving.trim().is_empty() {
            self.pattern = None;
        }
    }
}

/// Build the design the choices describe. Pure: same `Config`, same ring.
pub fn compose(cfg: &Config) -> RingDesign {
    let mut d = RingDesign::default();
    d.size = RingSize::new(cfg.size.clamp(3.0, 13.0));
    d.name = if cfg.customer.trim().is_empty() {
        format!("{} ring", cfg.base.label())
    } else {
        format!("{} for {}", cfg.base.label(), cfg.customer.trim())
    };

    match cfg.base {
        Base::Court => {
            d.profile.apply_style(ProfileStyle::LowDome);
            d.profile.width_mm = 4.0;
            d.profile.thickness_mm = 2.0;
        }
        Base::WideBand => {
            d.profile.apply_style(ProfileStyle::Flat);
            d.profile.width_mm = 7.0;
            // Thick on purpose: the side faces carry lettering, and letter
            // strokes need to clear the sand's detail floor.
            d.profile.thickness_mm = 2.6;
            d.profile.flatten_sides();
        }
        Base::Cathedral => {
            d.profile.apply_style(ProfileStyle::DShape);
            d.profile.width_mm = 4.0;
            d.profile.thickness_mm = 2.2;
            d.shank.kind = ShankKind::Cathedral;
            d.shank.amount = 0.8;
        }
        Base::Wave => {
            d.profile.apply_style(ProfileStyle::DShape);
            d.profile.width_mm = 3.6;
            d.profile.thickness_mm = 1.9;
            d.shank.kind = ShankKind::Wave;
            d.shank.amount = 0.7;
            d.shank.waves = 1;
        }
        Base::Twist => {
            d.profile.apply_style(ProfileStyle::DShape);
            d.profile.width_mm = 3.8;
            d.profile.thickness_mm = 2.0;
            d.shank.kind = ShankKind::Twist;
            d.shank.amount = 0.8;
            d.shank.waves = 2;
        }
        Base::Split => {
            d.profile.apply_style(ProfileStyle::Flat);
            d.profile.width_mm = 5.5;
            d.profile.thickness_mm = 2.0;
            d.profile.flatten_sides();
            d.shank.kind = ShankKind::Split;
            d.shank.amount = 0.85;
        }
        Base::SignetOval | Base::SignetHeart => {
            d.profile.apply_style(ProfileStyle::Flat);
            d.profile.width_mm = 14.0;
            d.profile.thickness_mm = 2.0;
            d.profile.flatten_sides();
            d.shank.apply_signet(14.0);
            d.shank.head.outline = if cfg.base == Base::SignetOval {
                SignetOutline::Oval
            } else {
                SignetOutline::Heart
            };
            d.shank.head.fit_length_to(14.0);
        }
    }

    let ctx = d.field_context();

    match cfg.stone {
        Stone::None => {}
        Stone::Solitaire { mm } => {
            let mut seat = SeatPadLayer {
                theta_deg: TOP_DEG,
                v_mm: ctx.crest_v_mm,
                height_mm: 0.9,
                crown: 0.35,
                blend_mm: 2.2,
                style: SeatStyle::GypsyMound,
                prongs: 4,
                ..Default::default()
            };
            seat.fit_stone(Gem::calibrated(GemCut::Round, mm.clamp(2.0, 8.0)));
            d.layers.layers.push(LayerEntry::new("Solitaire seat", Layer::SeatPad(seat)));
        }
        Stone::Eternity { mm } => {
            let mut run = SeatRunLayer::default();
            run.seat.style = SeatStyle::GypsyMound;
            run.seat.v_mm = ctx.crest_v_mm;
            run.gem = Gem::calibrated(GemCut::Round, mm.clamp(1.0, 3.0));
            run.solve_spacing(&ctx);
            d.layers.layers.push(LayerEntry::new("Eternity row", Layer::SeatRun(run)));
        }
    }

    if let Some(p) = cfg.pattern
        && let Some(&(alpha, repeats)) = PATTERNS.get(p)
    {
        let mut t = TilingLayer::default_for(alpha, &ctx);
        t.height_mm = 0.30;
        t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG);
        t.repeats_around = repeats;
        t.rows = 1;
        let mut e = LayerEntry::new(alpha, Layer::Tiling(t));
        // On a signet, keep the pattern off the head's own arc.
        if matches!(cfg.base, Base::SignetOval | Base::SignetHeart) {
            e.window = Window::except(TOP_DEG, 120.0);
        }
        d.layers.layers.push(e);
    }

    if cfg.milgrain {
        let beads = (ctx.circumference_mm / 0.55).round().max(24.0) as u32;
        d.layers.layers.push(LayerEntry::new(
            "Milgrain",
            Layer::Milgrain(MilgrainLayer {
                v_mm: ctx.crest_v_mm,
                bead_diameter_mm: 0.5,
                beads_around: beads,
                height_mm: 0.22,
                mirror: false,
            }),
        ));
    }

    let engraving = cfg.engraving.trim();
    if !engraving.is_empty()
        && cfg.base.has_sides()
        && let Some((lo, hi)) = ctx
            .side_faces(SIDE_FACE_MIN_DRAFT_DEG)
            .and_then(|f| f.wider())
    {
        let text = TextAlpha {
            name: "Engraving".into(),
            text: engraving.chars().take(24).collect(),
            font: if cfg.script_font { TextFont::Script } else { TextFont::Serif },
            tracking: 0.06,
        };
        d.texts.push(text);
        // One free-placed stamp on the lower shoulder, sized to sit inside
        // the face: the raster is wide-short, so width follows the letter
        // count and the height is checked against the face's own span.
        let face_h = (hi - lo) * 0.8;
        let aspect = 0.22 + 0.03 * engraving.chars().count() as f64;
        let size = (face_h / 0.30 * aspect).clamp(4.0, ctx.circumference_mm * 0.42);
        let mut layer = DecalLayer::default();
        layer.alpha = "Engraving".into();
        layer.feather_mm = 0.25;
        layer.decals = vec![Decal {
            theta_deg: TOP_DEG + 180.0,
            v_mm: 0.5 * (lo + hi),
            size_mm: size,
            rotation_deg: 0.0,
            height_mm: 0.35,
            flip: false,
        }];
        let mut e = LayerEntry::new("Engraving", Layer::Decals(layer));
        e.window.v_gate = VGate::SideFaces(
            ringdesign_core::field::SideFacePick::Wider,
        );
        d.layers.layers.push(e);
    }

    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::alpha::AlphaLibrary;
    use ringdesign_core::castability::{self, Verdict};
    use ringdesign_core::mesh::{build, BuildParams};

    /// Every base × every add-on family the UI can reach, held to the field
    /// verdict — a customer cannot compose an uncastable ring.
    #[test]
    fn every_offered_combination_is_castable() {
        let mut lib = AlphaLibrary::builtin();
        for &base in Base::ALL {
            // Two passes: the pattern-heavy dress and the engraved one —
            // reconcile makes them exclusive, so both must be proven.
            for engraved in [false, true] {
                let mut cfg = Config {
                    base,
                    stone: Stone::Solitaire { mm: 5.0 },
                    pattern: Some(1),
                    milgrain: true,
                    engraving: if engraved { "Amor vincit".into() } else { String::new() },
                    script_font: true,
                    customer: "Test".into(),
                    ..Config::default()
                };
                cfg.reconcile();
                let d = compose(&cfg);
                d.bake_texts(&mut lib);
                let out = build(
                    &d,
                    &lib,
                    BuildParams { theta_steps: 160, profile_steps: 96, ..Default::default() },
                );
                assert!(out.report.validation.watertight, "{base:?} not watertight");
                let field = castability::attributed_field_report(&d, &lib, &d.draft, 144, 96);
                assert_ne!(
                    field.verdict,
                    Verdict::NotCastable,
                    "{base:?} engraved={engraved}: {:?}",
                    field.notes
                );
            }
        }
    }

    #[test]
    fn reconcile_strips_what_the_base_cannot_carry() {
        let mut cfg = Config {
            base: Base::SignetHeart,
            stone: Stone::Eternity { mm: 1.5 },
            pattern: Some(0),
            engraving: "hi".into(),
            ..Config::default()
        };
        cfg.reconcile();
        assert_eq!(cfg.stone, Stone::None);
        // A signet has side faces, so the engraving survives — and it
        // displaces the pattern rather than stacking on it.
        assert!(!cfg.engraving.is_empty());
        assert!(cfg.pattern.is_none());

        cfg.base = Base::Court;
        cfg.reconcile();
        assert!(cfg.pattern.is_none());
        assert!(cfg.engraving.is_empty());

        // Engraving displaces the pattern rather than stacking on it.
        let mut both = Config {
            base: Base::WideBand,
            pattern: Some(0),
            engraving: "hello".into(),
            ..Config::default()
        };
        both.reconcile();
        assert!(both.pattern.is_none());
    }

    #[test]
    fn the_config_round_trips_as_json() {
        let cfg = Config {
            base: Base::Split,
            stone: Stone::Eternity { mm: 1.5 },
            pattern: Some(2),
            customer: "Ada".into(),
            ..Config::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(back.base, Base::Split);
        assert_eq!(back.customer, "Ada");
    }
}
