//! Curated starter designs — the File menu's "New from template".
//!
//! Each template is built in code from the same API the panels drive, so it
//! can never go stale against the file format and every part of it stays
//! editable. Only builtin alphas are referenced — a template must open
//! identically on a machine with an empty library. The test holds each one
//! to the field verdict: none may ship NotCastable.

use crate::field::{
    Layer, LayerEntry, MilgrainLayer, SeatPadLayer, SeatStyle, SignetOutline, Window,
    SIDE_FACE_MIN_DRAFT_DEG,
};
use crate::gem::{Gem, GemCut};
use crate::profile::{ShankKind, SignetHead, TOP_DEG};
use crate::tiling::TilingLayer;
use crate::{ProfileStyle, RingDesign};

pub struct Template {
    pub name: &'static str,
    /// One sentence of what it teaches, shown as the menu item's hover.
    pub blurb: &'static str,
    build: fn() -> RingDesign,
}

impl Template {
    /// A fresh design named after the template.
    pub fn design(&self) -> RingDesign {
        let mut d = (self.build)();
        d.name = self.name.into();
        d
    }
}

pub fn all() -> &'static [Template] {
    &TEMPLATES
}

/// Flat profile with squared side faces — castable ground for relief.
fn squared(width: f64, thickness: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d
}

fn signet(outline: SignetOutline, width: f64, thickness: f64) -> RingDesign {
    let mut d = squared(width, thickness);
    d.shank.apply_signet(width);
    d.shank.head.outline = outline;
    d.shank.head.fit_length_to(width);
    d
}

/// Builtin tiling fitted onto the side faces, mirrored when both exist.
fn side_tiling(d: &RingDesign, alpha: &str, height: f64) -> TilingLayer {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for(alpha, &ctx);
    t.height_mm = height;
    t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG);
    t.repeats_around = t.repeats_for_square_cells(&ctx);
    t
}

static TEMPLATES: [Template; 9] = [
    Template {
        name: "Court band",
        blurb: "A plain comfort-fit court — the blank everything else starts from.",
        build: || {
            let mut d = RingDesign::default();
            d.profile.apply_style(ProfileStyle::LowDome);
            d.profile.width_mm = 4.0;
            d.profile.thickness_mm = 2.0;
            d
        },
    },
    Template {
        name: "Heart signet",
        blurb: "Heart head with a blank table for the engraver; the body is faired, not extruded.",
        build: || signet(SignetOutline::Heart, 15.5, 1.6),
    },
    Template {
        name: "Waved hexagon signet",
        blurb: "Bold waves on the side faces — relief there pulls straight out of the sand.",
        build: || {
            let mut d = signet(SignetOutline::Hexagon, 14.0, 2.6);
            let mut t = side_tiling(&d, "Waves", 0.30);
            // Three wave rows per tile; the face holds one tile, so each row
            // stays over the sand's detail floor.
            t.repeats_around = 6;
            t.rows = 1;
            t.contrast = 1.15;
            d.layers.layers.push(LayerEntry::new("Waves", Layer::Tiling(t)));
            d
        },
    },
    Template {
        name: "Shouldered cushion signet",
        blurb: "Ornament windowed onto the shoulders, off the table a graver wants blank.",
        build: || {
            let mut d = signet(SignetOutline::Cushion, 14.5, 2.2);
            let mut t = side_tiling(&d, "Chevron", 0.28);
            // Four zigzag bands per tile read at ~0.5 mm each on the face.
            t.repeats_around = 9;
            t.rows = 1;
            let mut e = LayerEntry::new("Shoulder ornament", Layer::Tiling(t));
            e.window = Window::except(TOP_DEG, 120.0);
            d.layers.layers.push(e);
            d
        },
    },
    Template {
        name: "Braided band",
        blurb: "Braid cords on the side faces, milgrain riding the crest.",
        build: || {
            let mut d = squared(7.5, 2.4);
            let mut t = side_tiling(&d, "Braid", 0.30);
            // Three cords per tile; 8 around keeps a cord ~2 mm at the crest.
            t.repeats_around = 8;
            t.rows = 1;
            d.layers.layers.push(LayerEntry::new("Braid", Layer::Tiling(t)));
            let ctx = d.field_context();
            d.layers.layers.push(LayerEntry::new(
                "Milgrain",
                Layer::Milgrain(MilgrainLayer {
                    v_mm: ctx.band_v_len_mm * 0.5,
                    bead_diameter_mm: 0.5,
                    beads_around: 130,
                    height_mm: 0.22,
                    mirror: false,
                }),
            ));
            d
        },
    },
    Template {
        name: "Cathedral solitaire stock",
        blurb: "Gypsy mound with prong stock for a 5 mm round — cast the seat, set at the bench.",
        build: || {
            let mut d = RingDesign::default();
            d.profile.apply_style(ProfileStyle::DShape);
            d.profile.width_mm = 4.0;
            d.profile.thickness_mm = 2.2;
            d.shank.kind = ShankKind::Cathedral;
            d.shank.amount = 0.8;
            let ctx = d.field_context();
            let stone = Gem::calibrated(GemCut::Round, 5.0);
            let mut seat = SeatPadLayer {
                theta_deg: TOP_DEG,
                v_mm: ctx.band_v_len_mm * 0.5,
                height_mm: 0.9,
                crown: 0.35,
                blend_mm: 2.2,
                style: SeatStyle::GypsyMound,
                prongs: 4,
                ..Default::default()
            };
            seat.fit_stone(stone);
            d.layers.layers.push(LayerEntry::new("Solitaire seat", Layer::SeatPad(seat)));
            d
        },
    },
    Template {
        name: "Wishbone wave",
        blurb: "One wave per turn — the curved band that hugs a solitaire's ring.",
        build: || {
            let mut d = RingDesign::default();
            d.profile.apply_style(ProfileStyle::DShape);
            d.profile.width_mm = 3.6;
            d.profile.thickness_mm = 1.9;
            d.shank.kind = ShankKind::Wave;
            d.shank.amount = 0.7;
            d.shank.waves = 1;
            d
        },
    },
    Template {
        name: "Split shank",
        blurb: "Side-face channels and a width flare — reads as two rails, pulls as one band.",
        build: || {
            let mut d = squared(5.5, 2.0);
            d.shank.kind = ShankKind::Split;
            d.shank.amount = 0.85;
            d
        },
    },
    Template {
        name: "Toi et moi",
        blurb: "Two heads sharing one band; the swells union, the trough stays wide.",
        build: || {
            let mut d = signet(SignetOutline::Oval, 12.0, 1.8);
            d.shank.amount = 0.75;
            d.shank.head.theta_deg = TOP_DEG - 26.0;
            d.shank.head.length_mm = 8.0;
            d.shank.extra_heads.push(SignetHead {
                outline: SignetOutline::Heart,
                theta_deg: TOP_DEG + 26.0,
                length_mm: 6.5,
                ..SignetHead::default()
            });
            d
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpha::AlphaLibrary;
    use crate::castability::{self, Verdict};
    use crate::mesh::{build, BuildParams};

    #[test]
    fn every_template_builds_watertight_and_none_is_uncastable() {
        let lib = AlphaLibrary::builtin();
        let mut seen = std::collections::HashSet::new();
        for t in all() {
            assert!(seen.insert(t.name), "duplicate template name {}", t.name);
            let d = t.design();
            assert_eq!(d.name, t.name);
            let out = build(
                &d,
                &lib,
                BuildParams { theta_steps: 192, profile_steps: 96, ..Default::default() },
            );
            assert!(out.report.validation.watertight, "{} not watertight", t.name);
            let field = castability::analyze_field(&d, &lib, &d.draft, 160, 96);
            assert_ne!(
                field.verdict,
                Verdict::NotCastable,
                "{}: {:?} — a starter design must not refuse to cast",
                t.name,
                field.notes
            );
        }
    }
}
