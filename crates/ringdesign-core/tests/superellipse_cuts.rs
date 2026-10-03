//! Fingerprints, taken on master 8e5a59a, of everything the ten superellipse cuts build: made parts, stock, preview,
//! census, stone map and clearance envelopes.
//!
//! ```text
//! RD_PRINT_CUT_PINS=1 cargo test -p ringdesign-core --test superellipse_cuts -- --nocapture
//! ```

use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::csg::Solid;
use ringdesign_core::field::{Layer, SeatPadLayer, SeatStyle, gem_half_extents_mm};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::mesh::BuildParams;
use ringdesign_core::setting::{self, Fit, Plan, SolidKind};
use ringdesign_core::{LayerEntry, ProfileStyle, RingDesign};

const CUTS: [GemCut; 10] = [
    GemCut::Round,
    GemCut::Oval,
    GemCut::Cushion,
    GemCut::Princess,
    GemCut::Emerald,
    GemCut::Baguette,
    GemCut::Marquise,
    GemCut::Radiant,
    GemCut::Asscher,
    GemCut::Hexagon,
];

struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Self(0xcbf29ce484222325)
    }
    fn u(&mut self, v: u64) {
        self.0 = (self.0 ^ v).wrapping_mul(0x100000001b3);
    }
    fn f(&mut self, v: f64) {
        self.u(v.to_bits());
    }
    fn solid(&mut self, s: &Solid) {
        self.u(s.v.len() as u64);
        for v in s.v.iter().flatten() {
            self.f(*v);
        }
        for i in s.f.iter().flatten() {
            self.u(*i as u64);
        }
    }
    fn text(&mut self, s: &str) {
        for b in s.bytes() {
            self.u(b as u64);
        }
    }
}

fn gems(cut: GemCut) -> [Gem; 4] {
    [Gem::calibrated(cut, 1.3), Gem::calibrated(cut, 3.0), Gem::calibrated(cut, 6.5), Gem::cabochon(cut, 6.0)]
}

/// Every made part, plan figure and extent a cut's stones read, hashed.
fn parts_of(cut: GemCut) -> u64 {
    let mut h = Fnv::new();
    let open = Fit { surface_z: 0.4, through_mm: Some(2.4), prongs: 0 };
    let blind = Fit { surface_z: -0.3, through_mm: None, prongs: 0 };
    for gem in gems(cut) {
        let plan = Plan::of(gem);
        h.f(plan.perimeter());
        for k in 0..24 {
            let phi = std::f64::consts::TAU * (k as f64 + 0.37) / 24.0;
            let (p, n) = (plan.point(phi), plan.normal(phi));
            p.iter().chain(n.iter()).for_each(|v| h.f(*v));
        }
        for n in 3..=8 {
            plan.claw_angles(n).iter().for_each(|a| h.f(*a));
        }
        h.u(setting::claw_count(gem, 0) as u64);
        for rot in [0.0, 30.0, 90.0, 137.0] {
            let (u, v) = gem_half_extents_mm(gem, rot);
            h.f(u);
            h.f(v);
        }
        h.solid(&setting::envelope(gem, 0.0));
        h.solid(&setting::envelope(gem, 0.02));
        h.solid(&setting::bur(gem, &open));
        h.solid(&setting::bur(gem, &blind));
        h.solid(&setting::collet(gem));
        if let Some(r) = setting::relief(gem, -0.4, 0.3, &open) {
            h.solid(&r);
        }
        for (c, r) in setting::beads(gem, &open) {
            c.iter().for_each(|v| h.f(*v));
            h.f(r);
        }
        match setting::claw_head(gem, 0) {
            Ok(s) => h.solid(&s),
            Err(e) => h.text(&format!("{e:?}")),
        }
    }
    h.0
}

/// A band carrying one seat of `cut` in each stock style, with height-field prongs on the boss.
fn stock_ring(cut: GemCut) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 7.0;
    d.profile.thickness_mm = 2.4;
    let v = d.field_context().crest_v_mm;
    for (k, (style, prongs, rot)) in [(SeatStyle::Boss, 4, 0.0), (SeatStyle::Bezel, 0, 30.0), (SeatStyle::GypsyMound, 0, 90.0)].into_iter().enumerate() {
        let mut pad = SeatPadLayer { theta_deg: 60.0 + 60.0 * k as f64, v_mm: v, style, prongs, rot_deg: rot, ..Default::default() };
        pad.fit_stone(Gem::calibrated(cut, 3.5));
        d.layers.layers.push(LayerEntry::new(format!("{style:?}"), Layer::SeatPad(pad)));
    }
    d
}

/// The stock as built, the preview stones, the census, the setter's map and the clearance envelopes.
fn ring_of(cut: GemCut) -> u64 {
    let lib = AlphaLibrary::builtin();
    let d = stock_ring(cut);
    let mut h = Fnv::new();
    let built = ringdesign_core::mesh::build(&d, &lib, BuildParams { theta_steps: 192, profile_steps: 72, ..Default::default() });
    for p in &built.mesh.vertices {
        h.u(p.0.to_bits() as u64);
        h.u(p.1.to_bits() as u64);
        h.u(p.2.to_bits() as u64);
    }
    for v in ringdesign_core::gems::preview_vertices(&d, &lib) {
        h.u(v.to_bits() as u64);
    }
    let report = ringdesign_core::stones::report(&d, 0.0);
    h.text(&format!("{report:?}"));
    h.text(&ringdesign_core::stonemap::stone_map_svg(&d, report.as_ref()).unwrap_or_default());
    for e in ringdesign_core::interaction::clearance::envelopes(&d, 0.3) {
        e.girdle.iter().chain(e.deep.iter()).flatten().for_each(|v| h.f(*v));
        h.f(e.worst_gap);
    }
    h.0
}

/// One ring with a made setting of each kind, resolved by boolean.
fn made_ring() -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 6.0;
    d.profile.thickness_mm = 2.4;
    let v = d.field_context().crest_v_mm;
    for (k, (cut, kind)) in [
        (GemCut::Princess, SolidKind::Prong),
        (GemCut::Emerald, SolidKind::Bezel),
        (GemCut::Marquise, SolidKind::Flush),
        (GemCut::Cushion, SolidKind::Bead),
        (GemCut::Hexagon, SolidKind::Prong),
    ]
    .into_iter()
    .enumerate()
    {
        let mut pad = SeatPadLayer { theta_deg: 30.0 + 70.0 * k as f64, v_mm: v, style: SeatStyle::GypsyMound, blend_mm: 0.5, solid: kind, ..Default::default() };
        pad.fit_stone(Gem::calibrated(cut, 2.5));
        pad.height_mm = 0.0;
        d.layers.layers.push(LayerEntry::new(format!("{cut:?}"), Layer::SeatPad(pad)));
    }
    d
}

fn made_hash() -> u64 {
    let d = made_ring();
    let built = ringdesign_core::mesh::try_build(&d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 256, profile_steps: 96, ..Default::default() }).unwrap();
    let mut h = Fnv::new();
    for p in &built.mesh.vertices {
        h.u(p.0.to_bits() as u64);
        h.u(p.1.to_bits() as u64);
        h.u(p.2.to_bits() as u64);
    }
    for f in &built.mesh.faces {
        f.iter().for_each(|i| h.u(*i as u64));
    }
    h.text(&format!("{:?}", built.solids.notes));
    h.0
}

const PINS: &[(&str, u64)] = &[
    ("Round parts", 0x99615006ad0f11d8),
    ("Round ring", 0xde87dbe354472d82),
    ("Oval parts", 0x18638b934a62f8c2),
    ("Oval ring", 0xd6a77ecf6f3b5fe1),
    ("Cushion parts", 0xb7f649d2fa7dda9d),
    ("Cushion ring", 0x0c8ef9b692743f1f),
    ("Princess parts", 0x27382183708d0bdf),
    ("Princess ring", 0x1b5640736990791c),
    ("Emerald parts", 0xfb20f0a82d325d17),
    ("Emerald ring", 0x28ef0d8e937b2e3b),
    ("Baguette parts", 0x2cb5cc0af85c3528),
    ("Baguette ring", 0x34558d3f1059213f),
    ("Marquise parts", 0x8cdcacfe14787495),
    ("Marquise ring", 0xcf0e4a18fae3eb2d),
    ("Radiant parts", 0x99c5119be84f227e),
    ("Radiant ring", 0xb0952ddd2154d54d),
    ("Asscher parts", 0x4e82392d1a632c07),
    ("Asscher ring", 0x60ccdbcc4181bd68),
    ("Hexagon parts", 0xfa9bf90e7e2567d8),
    ("Hexagon ring", 0x5796dc66cd1927e2),
    ("made ring", 0x3c804d4222a724c0),
];

#[test]
fn the_superellipse_cuts_build_as_master_built_them() {
    let mut got: Vec<(String, u64)> = Vec::new();
    for cut in CUTS {
        got.push((format!("{cut:?} parts"), parts_of(cut)));
        got.push((format!("{cut:?} ring"), ring_of(cut)));
    }
    got.push(("made ring".into(), made_hash()));
    if std::env::var_os("RD_PRINT_CUT_PINS").is_some() {
        for (name, h) in &got {
            println!("    (\"{name}\", {h:#018x}),");
        }
    }
    assert_eq!(got.len(), PINS.len(), "pin table");
    for ((name, h), (pin_name, pin)) in got.iter().zip(PINS) {
        assert_eq!(name, pin_name);
        assert_eq!(h, pin, "{name} moved");
    }
}
