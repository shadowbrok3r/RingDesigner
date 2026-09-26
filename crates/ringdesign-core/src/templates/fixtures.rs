//! Retired starter geometry retained for regression tests.
use super::*;

pub const NAMES: &[&str] = &[
    "Heart signet",
    "Waved hexagon signet",
    "Cathedral solitaire stock",
    "Toi et moi (two heads)",
    "Split channels",
    "Wishbone wave",
];

pub fn fixture(name: &str) -> Option<RingDesign> {
    NAMES.iter().position(|n| *n == name).map(|i| FIXTURES[i].design())
}

static FIXTURES: [Template; 6] = [
    Template {
        name: "Heart signet",
        blurb: "Heart head with a blank table for the engraver; the body is faired, not extruded.",
        view: (0.55, 1.12),
        build: || signet(SignetOutline::Heart, 15.5, 1.6),
    },
    Template {
        name: "Waved hexagon signet",
        blurb: "Bold waves on the side faces — relief there pulls straight out of the sand.",
        view: (0.55, 1.12),
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
        name: "Cathedral solitaire stock",
        blurb: "Gypsy mound with prong stock for a 5 mm round — cast the seat, set at the bench.",
        view: (0.55, 1.12),
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
        name: "Toi et moi",
        blurb: "Two heads sharing one band; the swells union, the trough stays wide.",
        view: (0.55, 1.12),
        build: || {
            let mut d = signet(SignetOutline::Oval, 12.0, 1.8);
            d.shank.amount = 0.75;
            d.shank.head.theta_deg = TOP_DEG - 26.0;
            d.shank.head.length_mm = 8.0;
            d.shank.extra_heads.push(SignetHead {
                outline: SignetOutline::Heart,
                theta_deg: TOP_DEG + 26.0,
                length_mm: 6.5,
                ..SignetHead::lofted()
            });
            d
        },
    },    Template {
        name: "Split shank",
        blurb: "Side-face channels and a width flare — reads as two rails, pulls as one band.",
        view: (0.55, 1.12),
        build: || {
            let mut d = squared(5.5, 2.0);
            d.shank.kind = ShankKind::Split;
            d.shank.amount = 0.85;
            d
        },
    },
    Template {
        name: "Wishbone wave",
        blurb: "One wave per turn — the curved band that hugs a solitaire's ring.",
        view: (0.55, 1.12),
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
];
