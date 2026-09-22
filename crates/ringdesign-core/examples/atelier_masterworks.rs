// Two master rings, each built to show what the model can do at the limit of
// one process — and each the source of a graph template.
//
//   Palisade — deco colonnade.  Two-part sand. Everything on it is where the
//     doctrine says ornament casts: gadroons whose flanks face round the ring,
//     chevrons on the side faces and the head's flanks, a gem column on the
//     parting line, a bead row on the crest, an inscription on a side face.
//     The table's sunburst is cut at the bench and says so.
//
//   Oriel — jewelled lantern.  Lost wax, no guard rails: a cabochon under a
//     proud halo, pave down both shoulders, a pierced gallery, vine wires and
//     rope rails off the crest, engine turning finer than any sand holds.
//
//   cargo run --release -p ringdesign-core --example atelier_masterworks -- OUT_DIR [--write]
//   MASTER=palisade|oriel builds one.
use ringdesign_core::alpha::{AlphaLibrary, ProcRecipe, Procedural};
use ringdesign_core::castability::{self, CastProcess, SandProcess, Verdict};
use ringdesign_core::curve::{CurveLayer, WireProfile};
use ringdesign_core::field::{
    Blend, BorderLayer, BorderProfile, Decal, DecalLayer, FluteProfile, FlutesLayer, Layer, LayerEntry, MilgrainLayer, OpenworkLayer, Remap, SeatPadLayer, SeatRunLayer, SeatStyle, SideFacePick,
    SignetOutline, VGate, Window, SIDE_FACE_MIN_DRAFT_DEG,
};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::pave::{self, PaveRegion, PaveSpec};
use ringdesign_core::profile::TOP_DEG;
use ringdesign_core::render::{self, Part};
use ringdesign_core::setting::SolidKind;
use ringdesign_core::svg::SvgAlpha;
use ringdesign_core::text::{TextAlpha, TextFont};
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{gems, library, stones, ProfileStyle, RingDesign, RingSize};

const YELLOW: [f32; 3] = [0.86, 0.70, 0.42];
const PLATINUM: [f32; 3] = [0.78, 0.79, 0.81];

fn signet(outline: SignetOutline, width: f64, thickness: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d.shank.apply_signet(width);
    d.shank.head.outline = outline;
    d.shank.head.fit_length_to(width);
    d
}

fn side_tiling(d: &RingDesign, alpha: &str, height: f64, repeats: u32) -> TilingLayer {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for(alpha, &ctx);
    t.height_mm = height;
    t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG);
    t.repeats_around = repeats;
    t.rows = 1;
    t
}

/// Palisade: the sand ring.
fn palisade() -> RingDesign {
    // A flat table is a zero-draft plane and a seventh of this ring's
    // surface: on any section it reads "castable with care". Buffed to a
    // 0.8 mm dome the whole head has draft and the bare ring reads Castable
    // at 0.0000%, with 1.8 mm of side face left for ornament.
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 15.0;
    d.profile.thickness_mm = 3.0;
    d.profile.flatten_sides();
    d.shank.apply_signet(15.0);
    // Domed, an octagon or a hexagon creases along the parting line where
    // its straight ends meet the cap; an oval's dome is one smooth surface.
    d.shank.head.outline = SignetOutline::Oval;
    d.shank.head.fit_length_to(15.0);
    d.name = "Palisade \u{2014} deco colonnade".into();
    d.size = RingSize::new(9.0);
    SandProcess::DelftClay.apply(&mut d.draft);
    d.shank.head.table_dome_mm = 0.8;
    d.shank.head.rim_round_mm = 0.7;
    d.shank.head.hollow_mm = 0.6;
    d.build = BuildParams { theta_steps: 1536, profile_steps: 448, ..d.build };
    let ctx = d.field_context();
    let crest = ctx.crest_v_mm;
    let faces = ctx.side_faces_std().expect("a flattened low dome keeps its side faces");
    let (low, high) = (faces.low.expect("low face"), faces.high.expect("high face"));
    // The outside runs between the two side faces; its flanks are either side of the crest.
    let outer_half = crest - low.1;

    // The colonnade: reeding down the side faces of both shoulders and the
    // head's flanks between them. Across the crown the same flutes lean: on
    // a shoulder that is still widening, a wall that faces round the ring
    // picks up an axial tilt, and beside the crest there is no draft to
    // measure it against (1-10 degrees run over the crest, 19 where a flank
    // gate left them an edge facing it). On a side face the wall a flute
    // raises is parallel to the pull at any height, widening or not.
    // The head's own flanks stay polished: a lofted head's wall curls under
    // its table, and reeding there tipped 9 degrees over a few tenths of a
    // square millimetre.
    for (name, side) in [("Colonnade reeding, right shoulder", 1.0), ("Colonnade reeding, left shoulder", -1.0)] {
        let flutes = FlutesLayer { count: 48, profile: FluteProfile::Round, width_mm: 0.9, height_mm: 0.34, lean: 0.0, along: false };
        let mut e = LayerEntry::new(name, Layer::Flutes(flutes));
        e.window = Window { fade_deg: 6.0, v_gate: VGate::SideFaces(SideFacePick::Both), ..Window::around(TOP_DEG + side * 72.0, 60.0) };
        d.layers.layers.push(e);
    }

    // Gadroons on the shoulder flanks, either side of the polished rib that
    // carries the stones. Their crest-side end is the hazard: an edge facing
    // the crest leans wherever it is steeper than the dome's own draft, and
    // the chart squeezes it as the shoulder narrows. So the fade toward the
    // crest is most of the flank wide, the relief is low, and the arc stops
    // before the shank gets narrow.
    let (g_height, g_center, g_span, g_fade, g_arc, g_off): (f64, f64, f64, f64, f64, f64) = (
        std::env::var("G_H").ok().and_then(|v| v.parse().ok()).unwrap_or(0.24),
        std::env::var("G_C").ok().and_then(|v| v.parse().ok()).unwrap_or(0.66),
        std::env::var("G_S").ok().and_then(|v| v.parse().ok()).unwrap_or(0.30),
        std::env::var("G_F").ok().and_then(|v| v.parse().ok()).unwrap_or(0.46),
        std::env::var("G_A").ok().and_then(|v| v.parse().ok()).unwrap_or(40.0),
        std::env::var("G_O").ok().and_then(|v| v.parse().ok()).unwrap_or(58.0),
    );
    if g_height > 0.0 {
        for (shoulder_name, side) in [("right shoulder", 1.0), ("left shoulder", -1.0)] {
            for (flank_name, sign) in [("upper flank", 1.0), ("lower flank", -1.0)] {
                let flutes = FlutesLayer { count: 36, profile: FluteProfile::Round, width_mm: 1.5, height_mm: g_height, lean: 0.0, along: false };
                let mut e = LayerEntry::new(format!("Gadroons, {shoulder_name}, {flank_name}"), Layer::Flutes(flutes));
                e.window = Window { fade_deg: 7.0, ..Window::around(TOP_DEG + side * g_off, g_arc) };
                e.window.v_gate = VGate::Band { center_mm: crest + sign * outer_half * g_center, span_mm: outer_half * g_span, fade_mm: outer_half * g_fade };
                d.layers.layers.push(e);
            }
        }
    }

    // Deco zigzags take over on the side faces under the hand, where the
    // reeding has faded out. A wire is analytic: its width is a number, not
    // a texture's guess at what the sand will hold.
    for (name, (lo, hi)) in [("Zigzag wire, lower face", low), ("Zigzag wire, upper face", high)] {
        let (mid, amp) = (0.5 * (lo + hi), 0.5 * (hi - lo) - 0.42);
        let wire = CurveLayer {
            points: vec![[0.0, mid - amp], [0.5, mid + amp], [1.0, mid - amp]],
            repeats_around: 30,
            closed: false,
            width_mm: 0.62,
            height_mm: 0.34,
            profile: WireProfile::Round,
            taper: 0.0,
            mirror_v: false,
        };
        let mut e = LayerEntry::new(name, Layer::Curve(wire));
        e.window = Window { fade_deg: 6.0, ..Window::around(270.0, 130.0) };
        d.layers.layers.push(e);
    }

    // A bead row on the crest line under the hand — only where the shank has
    // stopped tapering. On the arc that is still changing width a bead's
    // round-the-ring flanks pick up the same tilt the flutes did.
    let mut beads = LayerEntry::new(
        "Crest beads",
        Layer::Milgrain(MilgrainLayer { v_mm: crest, bead_diameter_mm: 0.9, beads_around: 80, height_mm: 0.28, mirror: false }),
    );
    beads.window = Window { fade_deg: 5.0, ..Window::around(270.0, 76.0) };
    d.layers.layers.push(beads);

    // The gem column: an emerald cut on the parting line with a round either
    // side of it. A mound astride the parting plane splits cleanly between
    // cope and drag; the same mound two millimetres off it locks.
    // Every stone is flush set: the mound casts, and the setting bur — bevel, girdle wall, bearing,
    // pilot through to the finger — is cut after the pour and is in the finished ring, not the pattern.
    let mut centre = SeatPadLayer { theta_deg: TOP_DEG, v_mm: crest, style: SeatStyle::GypsyMound, height_mm: 0.55, crown: 1.0, blend_mm: 0.6, solid: SolidKind::Flush, through: true, ..Default::default() };
    centre.fit_stone(Gem::calibrated(GemCut::Emerald, 4.0));
    d.layers.layers.push(LayerEntry::new("Emerald cut, on the parting line", Layer::SeatPad(centre)));
    for (name, side) in [("Round, toward the right shoulder", 1.0), ("Round, toward the left shoulder", -1.0)] {
        let mut seat = SeatPadLayer { theta_deg: TOP_DEG + side * 21.0, v_mm: crest, style: SeatStyle::GypsyMound, height_mm: 0.4, crown: 1.0, blend_mm: 0.5, solid: SolidKind::Flush, through: true, ..Default::default() };
        seat.fit_stone(Gem::calibrated(GemCut::Round, 2.0));
        d.layers.layers.push(LayerEntry::new(name, Layer::SeatPad(seat)));
    }
    // And the column runs on down each shoulder's polished rib, graduated:
    // the stones shrink away from the head and the stations close up with
    // them, so the metal between neighbours stays what this sand fills.
    for (name, side) in [("Graduated rounds, right rib", 1.0), ("Graduated rounds, left rib", -1.0)] {
        let mut seat = SeatPadLayer { v_mm: crest, style: SeatStyle::GypsyMound, height_mm: 0.36, crown: 1.0, blend_mm: 0.45, solid: SolidKind::Flush, through: true, ..Default::default() };
        let gem = Gem::calibrated(GemCut::Round, 1.75);
        seat.fit_stone(gem);
        let run = SeatRunLayer { seat, count: 17, gem, bridge_mm: 0.85, taper: 0.45, taper_theta_deg: TOP_DEG, shared_prong_mm: 0.0, tilt_deg: 0.0 };
        let mut e = LayerEntry::new(name, Layer::SeatRun(run));
        e.window = Window { fade_deg: 0.5, ..Window::around(TOP_DEG + side * 70.0, 64.0) };
        d.layers.layers.push(e);
    }

    // The maker's line and the table's sunburst are graver's work. A serif's
    // hairlines on a 1.8 mm face are a tenth of what Delft clay holds, and a
    // line cut square into the head off the parting line is a ledge the sand
    // cannot leave: both are in the ring and neither is in the pattern.
    d.texts.push(TextAlpha { name: "Palisade inscription".into(), text: "PALISADE \u{b7} MMXXVI".into(), font: TextFont::Serif, tracking: 0.12 });
    let stamp = Decal { theta_deg: 270.0, v_mm: 0.5 * (low.0 + low.1), size_mm: 15.0, rotation_deg: 180.0, height_mm: 0.18, flip: false };
    let mut e = LayerEntry::new("Maker's line, cut at the bench", Layer::Decals(DecalLayer { alpha: "Palisade inscription".into(), decals: vec![stamp], feather_mm: 0.05, invert: false }));
    e.blend = Blend::Subtract;
    e.bench_only = true;
    d.layers.layers.push(e);
    d.recipes.push(ProcRecipe { name: "Table sunburst".into(), kind: Procedural::Starburst, repeats: 1, quarter_turns: 0, gamma: 1.4, invert: false });
    // One tile is a sixth of the way round, centred on the top: the rays
    // meet under the emerald cut instead of being a slice of a burst that
    // spans the whole ring.
    let mut burst = TilingLayer::default_for("Table sunburst", &ctx);
    burst.repeats_around = 6;
    burst.rows = 1;
    burst.offset_u = 0.0;
    burst.height_mm = 0.09;
    burst.v_center_mm = crest;
    burst.v_span_mm = outer_half * 1.7;
    burst.continuous = false;
    let mut e = LayerEntry::new("Sunburst, cut at the bench", Layer::Tiling(burst));
    e.blend = Blend::Subtract;
    e.bench_only = true;
    e.window = Window { fade_deg: 2.0, ..Window::around(TOP_DEG, 58.0) };
    d.layers.layers.push(e);
    d
}

const LANTERN_FLEUR: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect width="100" height="100" fill="#fff"/><g fill="#000"><path d="M50 6 C62 24 66 38 50 56 C34 38 38 24 50 6 Z"/><path d="M50 58 C40 44 22 40 10 50 C20 66 38 68 50 58 Z"/><path d="M50 58 C60 44 78 40 90 50 C80 66 62 68 50 58 Z"/><path d="M44 60 H56 L54 92 H46 Z"/><circle cx="50" cy="60" r="7"/></g></svg>"##;

/// Oriel: the lost-wax ring.
fn oriel() -> RingDesign {
    let mut d = signet(SignetOutline::Cushion, 14.0, 2.3);
    d.name = "Oriel \u{2014} jewelled lantern".into();
    d.size = RingSize::new(7.0);
    CastProcess::LostWax.apply(&mut d.draft);
    d.shank.head.table_dome_mm = 1.1;
    d.shank.head.rim_round_mm = 0.8;
    d.shank.head.rise_mm = 0.6;
    d.build = BuildParams { theta_steps: 1536, profile_steps: 448, ..d.build };
    let ctx = d.field_context();
    let crest = ctx.crest_v_mm;
    let band = ctx.band_v_len_mm;

    // Engine turning over the whole outside, finer than any sand holds.
    d.recipes.push(ProcRecipe { name: "Engine turning".into(), kind: Procedural::GuillocheWeave, repeats: 1, quarter_turns: 0, gamma: 1.0, invert: false });
    let mut ground = TilingLayer::default_for("Engine turning", &ctx);
    ground.repeats_around = 22;
    ground.rows = 3;
    ground.height_mm = 0.06;
    ground.v_center_mm = crest;
    ground.v_span_mm = band * 0.62;
    let mut e = LayerEntry::new("Engine-turned ground", Layer::Tiling(ground));
    e.window = Window { fade_deg: 6.0, ..Window::except(TOP_DEG, 78.0) };
    d.layers.layers.push(e);

    // The gallery: the shank pierced through under the hand.
    d.recipes.push(ProcRecipe { name: "Lantern lattice".into(), kind: Procedural::Trellis, repeats: 1, quarter_turns: 0, gamma: 1.0, invert: false });
    let mut lattice = TilingLayer::default_for("Lantern lattice", &ctx);
    lattice.repeats_around = 9;
    lattice.rows = 1;
    lattice.v_center_mm = crest;
    lattice.v_span_mm = band * 0.34;
    lattice.invert = true;
    let mut e = LayerEntry::new("Pierced gallery", Layer::Openwork(OpenworkLayer { tiling: lattice, depth_mm: 1.4, keep_mm: 0.55 }));
    e.window = Window { fade_deg: 5.0, ..Window::around(270.0, 124.0) };
    d.layers.layers.push(e);

    // Rope rails along both edges of the outside, and a vine between them.
    let rail_v = band * 0.5 - band * 0.29;
    let mut rails = LayerEntry::new(
        "Rope rails",
        Layer::Border(BorderLayer { v_mm: rail_v, width_mm: 0.75, height_mm: 0.32, profile: BorderProfile::Rope, mirror: true, rope_twists: 72 }),
    );
    // The dome is the lantern's glass: nothing crosses it but the halo.
    rails.window = Window { fade_deg: 7.0, ..Window::except(TOP_DEG, 92.0) };
    d.layers.layers.push(rails);
    let a = band * 0.16;
    let vine = CurveLayer {
        points: vec![[0.0, crest - a], [0.25, crest + a], [0.5, crest - a], [0.75, crest + a], [1.0, crest - a]],
        repeats_around: 7,
        closed: false,
        width_mm: 0.62,
        height_mm: 0.34,
        profile: WireProfile::Round,
        taper: 0.0,
        mirror_v: true,
    };
    let mut e = LayerEntry::new("Vine wires", Layer::Curve(vine));
    e.blend = Blend::SmoothMax;
    e.window = Window { fade_deg: 8.0, ..Window::except(TOP_DEG, 150.0) };
    d.layers.layers.push(e);

    // Pave down both shoulders, packed live against the band it is on.
    for (name, side) in [("Pave, right shoulder", 1.0), ("Pave, left shoulder", -1.0)] {
        let spec = PaveSpec {
            gem: Gem::calibrated(GemCut::Round, 1.25),
            bridge_mm: 0.28,
            theta_deg: TOP_DEG + side * 66.0,
            span_deg: 46.0,
            region: PaveRegion::VBand { center_mm: crest, width_mm: band * 0.42 },
            stagger: true,
            style: SeatStyle::Boss,
            rot_deg: 0.0,
            blend_mm: 0.25,
            recess_mm: 0.0,
            pinned: Vec::new(),
        };
        if let Some((mut e, outcome)) = pave::fill(&d, &spec) {
            e.name = name.into();
            // Baked: the seats are the design's own from here, so the graph
            // lifted from it carries each one as a node, not a recipe patched
            // onto a stack index.
            if let Layer::Group(g) = &mut e.layer {
                g.recipe = None;
                // Bead set: every seat is cut with the bur and holds its stone under raised beads.
                for seat in &mut g.stack.layers {
                    if let Layer::SeatPad(p) = &mut seat.layer {
                        p.solid = SolidKind::Bead;
                        p.prongs = 0;
                    }
                }
            }
            println!("  {name}: {} seats in {} rows", outcome.seats, outcome.rows);
            d.layers.layers.push(e);
        }
    }

    // A fleur at the root of each shoulder, from artwork carried in the file.
    d.svgs.push(SvgAlpha { name: "Lantern fleur".into(), svg: LANTERN_FLEUR.into(), invert: false });
    let fleurs = [1.0, -1.0].map(|side: f64| Decal { theta_deg: TOP_DEG + side * 92.0, v_mm: crest, size_mm: 4.2, rotation_deg: if side > 0.0 { 90.0 } else { -90.0 }, height_mm: 0.38, flip: false });
    let mut e = LayerEntry::new("Fleurs at the shoulder roots", Layer::Decals(DecalLayer { alpha: "Lantern fleur".into(), decals: fleurs.to_vec(), feather_mm: 0.1, invert: false }));
    e.blend = Blend::Max;
    e.remap = Remap::Terrace { steps: 3, span_mm: 0.38, riser: 0.4 };
    d.layers.layers.push(e);

    // Milgrain either side of the pave, off the crest: lost wax does not mind.
    let mut grain = LayerEntry::new(
        "Milgrain rails",
        Layer::Milgrain(MilgrainLayer { v_mm: band * 0.5 - band * 0.235, bead_diameter_mm: 0.42, beads_around: 190, height_mm: 0.2, mirror: true }),
    );
    grain.window = Window { fade_deg: 7.0, ..Window::except(TOP_DEG, 92.0) };
    d.layers.layers.push(grain);

    // The lantern itself: a cabochon in a burnished bezel under a halo of
    // proud melee. The generator lays its ring out in chart millimetres, and
    // across a lofted head one chart millimetre is 1.7 of metal — the ring
    // came out hugging the rim instead of the stone. Here every seat is
    // placed in metal millimetres by its own station's stretch and flagged
    // to cast as drawn, in a group that blends as one.
    let cab = Gem::cabochon(GemCut::Oval, 6.0);
    let melee = Gem::calibrated(GemCut::Round, 1.3);
    // The centre is a made collet — tapered wall, bearing ledge, a lip up the dome — stood on a low
    // platform and resolved into the head, so it fits its stone in metal millimetres wherever it lands.
    const PLATE_MM: f64 = 0.55;
    let mut bezel = SeatPadLayer { theta_deg: TOP_DEG, v_mm: crest, style: SeatStyle::Boss, crown: 0.0, blend_mm: 0.3, metal_true: true, solid: SolidKind::Bezel, ..Default::default() };
    bezel.fit_stone(cab);
    bezel.height_mm = PLATE_MM;
    let r_crest = ctx.crest_radius_mm * ctx.crest_scale(TOP_DEG);
    // The halo is the bezel's own outline grown by the gap, melee at equal arc length round it.
    let reach = 0.82 + 0.35 + melee.w_mm * 0.5;
    let (ha, hb) = (cab.l_mm * 0.5 + reach, cab.w_mm * 0.5 + reach);
    // One plate under the collet and its halo, flat-topped, a rim's width past the melee.
    let rim = melee.w_mm * 0.5 + 0.45;
    let mut plate = SeatPadLayer { theta_deg: TOP_DEG, v_mm: crest, style: SeatStyle::Boss, height_mm: PLATE_MM, crown: 0.0, blend_mm: 0.45, metal_true: true, plan_pow: 2.0, ..Default::default() };
    plate.diameter_mm = 2.0 * (hb + rim);
    plate.elong = (ha + rim) / (hb + rim);
    let mut seats = vec![LayerEntry::new("Halo plate", Layer::SeatPad(plate)), LayerEntry::new("Cabochon collet", Layer::SeatPad(bezel))];
    let count = 18usize;
    let samples: Vec<(f64, f64)> = (0..=720).map(|i| { let t = i as f64 / 720.0 * std::f64::consts::TAU; (ha * t.cos(), hb * t.sin()) }).collect();
    let lengths: Vec<f64> = samples.windows(2).scan(0.0, |acc, w| { *acc += ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt(); Some(*acc) }).collect();
    let total = *lengths.last().unwrap_or(&1.0);
    for k in 0..count {
        let want = total * k as f64 / count as f64;
        let at = lengths.iter().position(|l| *l >= want).unwrap_or(0);
        let (along, across) = samples[at];
        let theta = TOP_DEG + (along / r_crest).to_degrees();
        let stretch = ctx.station_stretch(theta).max(1.0);
        // Bead set into the plate: the seat's own stock is the plate's height, so its girdle reads off the same top.
        let mut seat = SeatPadLayer { theta_deg: theta, v_mm: crest + across / stretch, style: SeatStyle::Boss, crown: 0.0, blend_mm: 0.1, metal_true: true, solid: SolidKind::Bead, ..Default::default() };
        seat.fit_stone(melee);
        seat.height_mm = PLATE_MM;
        seats.push(LayerEntry::new(format!("Halo melee {:02}", k + 1), Layer::SeatPad(seat)));
    }
    println!("  halo: {count} melee round a {:.1} x {:.1} mm cabochon", cab.l_mm, cab.w_mm);
    let mut halo = LayerEntry::new("Cabochon and halo", Layer::Group(ringdesign_core::field::GroupLayer { stack: ringdesign_core::field::LayerStack { layers: seats }, recipe: None }));
    halo.blend = Blend::Max;
    d.layers.layers.push(halo);

    // A script line inside the pattern of the side face.
    d.texts.push(TextAlpha { name: "Oriel inscription".into(), text: "lux in tenebris".into(), font: TextFont::Script, tracking: 0.04 });
    if let Some((lo, hi)) = ctx.side_faces_std().and_then(|f| f.wider()) {
        let low_face = 0.5 * (lo + hi) < crest;
        // Turned to read with the head uppermost, the way a ring is looked at in the hand.
        let stamp = Decal { theta_deg: 270.0, v_mm: 0.5 * (lo + hi), size_mm: 12.0, rotation_deg: 180.0, height_mm: 0.16, flip: !low_face };
        let mut e = LayerEntry::new("Script line", Layer::Decals(DecalLayer { alpha: "Oriel inscription".into(), decals: vec![stamp], feather_mm: 0.08, invert: false }));
        e.blend = Blend::Subtract;
        d.layers.layers.push(e);
    }
    // Guilloche on the side faces, under the script.
    d.recipes.push(ProcRecipe { name: "Moire sides".into(), kind: Procedural::Moire, repeats: 1, quarter_turns: 0, gamma: 1.0, invert: false });
    let mut sides = side_tiling(&d, "Moire sides", 0.07, 14);
    sides.kfold = 0;
    let mut e = LayerEntry::new("Moire on the side faces", Layer::Tiling(sides));
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.insert(0, e);
    d
}

fn report(d: &RingDesign, lib: &AlphaLibrary) -> castability::FieldReport {
    let field = castability::attributed_field_report(d, lib, &d.draft, 256, 144);
    println!(
        "  {} under {}: {:.4}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
        field.verdict.label(),
        d.draft.process.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm
    );
    for n in &field.notes {
        println!("    note: {n}");
    }
    for f in ringdesign_core::dfm::findings_in(d, lib) {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    if let Some(s) = stones::report(d, field.parting_z_mm) {
        println!("    stones: {} stones, {:.2} ct", s.stone_count, s.total_carats);
        let mut seen = std::collections::BTreeSet::new();
        for w in s.seats.iter().flat_map(|seat| seat.warnings.iter()) {
            if seen.insert(w.clone()) {
                println!("    stone warning: {w}");
            }
        }
    }
    field
}

/// Which sections leave a lofted signet both drafted and with a side face.
fn probe() {
    let lib = AlphaLibrary::builtin();
    for style in [ProfileStyle::LowDome, ProfileStyle::Beveled, ProfileStyle::DShape] {
        for (thickness, dome, outline) in [(3.0, 0.0, SignetOutline::Octagon), (3.0, 0.7, SignetOutline::Octagon), (3.0, 1.1, SignetOutline::Octagon), (3.0, 1.1, SignetOutline::Cushion), (3.4, 1.4, SignetOutline::Octagon)] {
            let mut d = RingDesign::default();
            d.profile.apply_style(style);
            d.profile.width_mm = 15.0;
            d.profile.thickness_mm = thickness;
            d.profile.flatten_sides();
            d.shank.apply_signet(15.0);
            d.shank.head.outline = outline;
            d.shank.head.fit_length_to(15.0);
            d.shank.head.table_dome_mm = dome;
            SandProcess::DelftClay.apply(&mut d.draft);
            let f = castability::attributed_field_report(&d, &lib, &d.draft, 192, 128);
            let ctx = d.field_context();
            let face = ctx.side_faces_std().and_then(|s| s.wider()).map(|(lo, hi)| hi - lo).unwrap_or(0.0);
            let drag = (f.marginal_area_mm2 + f.vertical_area_mm2) / f.total_area_mm2.max(1e-9);
            println!("{style:?} {outline:?} t{thickness} dome{dome}: {} undercut {:.4}% drag {:.1}% side face {face:.2} mm", f.verdict.label(), f.undercut_fraction() * 100.0, drag * 100.0);
        }
    }
}

/// Bare heads, to see what an outline and a dome do before ornament hides it.
fn bare(dir: &str) {
    let lib = AlphaLibrary::builtin();
    for outline in [SignetOutline::Octagon, SignetOutline::Hexagon, SignetOutline::Cushion, SignetOutline::Oval, SignetOutline::Rectangle] {
        for dome in [0.0, 0.8] {
            let mut d = RingDesign::default();
            d.profile.apply_style(ProfileStyle::LowDome);
            d.profile.width_mm = 15.0;
            d.profile.thickness_mm = 3.0;
            d.profile.flatten_sides();
            d.shank.apply_signet(15.0);
            d.shank.head.outline = outline;
            d.shank.head.fit_length_to(15.0);
            d.shank.head.table_dome_mm = dome;
            d.shank.head.rim_round_mm = 0.7;
            SandProcess::DelftClay.apply(&mut d.draft);
            let f = castability::attributed_field_report(&d, &lib, &d.draft, 192, 128);
            let drag = (f.marginal_area_mm2 + f.vertical_area_mm2) / f.total_area_mm2.max(1e-9);
            println!("{outline:?} dome {dome}: {} drag {:.1}%", f.verdict.label(), drag * 100.0);
            let out = mesh::build(&d, &lib, BuildParams { theta_steps: 1024, profile_steps: 320, ..Default::default() });
            let parts = [Part::metal(&out.mesh, YELLOW)];
            render::write_png_parts(format!("{dir}/bare-{outline:?}-{dome}-table.png"), &parts, 0.0, std::f64::consts::FRAC_PI_2, 420).unwrap();
            render::write_png_parts(format!("{dir}/bare-{outline:?}-{dome}-hero.png"), &parts, 0.55, 0.95, 420).unwrap();
        }
    }
}

fn main() {
    if std::env::var("PROBE").is_ok() {
        return probe();
    }
    if std::env::var("BARE").is_ok() {
        let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp/atelier".into());
        std::fs::create_dir_all(&dir).unwrap();
        return bare(&dir);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args.first().cloned().unwrap_or_else(|| "/tmp/atelier".into());
    let write = args.iter().any(|a| a == "--write");
    let only = std::env::var("MASTER").ok();
    let edge: usize = std::env::var("EDGE").ok().and_then(|v| v.parse().ok()).unwrap_or(1100);
    std::fs::create_dir_all(&dir).unwrap();
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (slug, tint, build) in [("palisade", YELLOW, palisade as fn() -> RingDesign), ("oriel", PLATINUM, oriel as fn() -> RingDesign)] {
        if only.as_deref().is_some_and(|o| o != slug) {
            continue;
        }
        println!("{slug}");
        let mut d = build();
        let mut lib = AlphaLibrary::builtin();
        d.bake_all(&mut lib);
        let field = report(&d, &lib);
        if d.draft.process == CastProcess::SandTwoPart {
            assert_eq!(field.verdict, Verdict::Castable, "{slug} is the sand ring: it has to cast");
        } else {
            assert_ne!(field.verdict, Verdict::NotCastable, "{slug} must at least fill");
        }
        if std::env::var("NO_RENDER").is_ok() {
            if write {
                d.embed_alphas(&lib);
                let at = repo.join(format!("showcase/{slug}"));
                std::fs::create_dir_all(&at).unwrap();
                library::save_design(at.join("design.ring.json"), &d).unwrap();
                println!("  wrote {}", at.join("design.ring.json").display());
            }
            continue;
        }
        let params = BuildParams { theta_steps: 1536, profile_steps: 448, ..Default::default() };
        let out = mesh::build(&d, &lib, params);
        assert!(out.report.validation.watertight, "{slug} is not watertight");
        println!("  made settings: {} resolved in {} ms{}", out.solids.resolved, out.solids.ms, if out.solids.notes.is_empty() { String::new() } else { format!(", {} refused: {:?}", out.solids.notes.len(), out.solids.notes) });
        println!("  {} triangles, {:.0} mm3, relief {:+.2}..{:+.2} mm", out.report.validation.triangle_count, out.report.volume_mm3, out.report.min_relief_mm, out.report.max_relief_mm);
        for m in out.report.metals.iter().take(3) {
            println!("    {}: {:.2} g", m.metal, m.grams);
        }
        let stones_mesh = gems::preview_mesh(&d, &lib);
        let mut parts = vec![Part::metal(&out.mesh, tint)];
        if let Some(g) = &stones_mesh {
            parts.push(Part::stone(g));
        }
        use std::f64::consts::FRAC_PI_2;
        // The renderer turns the model by yaw about the finger axis, then
        // tips it by pitch: pitch 0 looks through the ring with the head at
        // the top of the frame, a quarter turn looks straight down on the table.
        for (view, yaw, pitch) in [("hero", 0.6, 0.9), ("table", 0.0, FRAC_PI_2), ("profile", 0.0, 0.0), ("front", 0.35, 0.42), ("shoulder", 1.25, 1.05), ("palm", 3.14159, 1.15)] {
            render::write_png_parts(format!("{dir}/{slug}-{view}.png"), &parts, yaw, pitch, edge).unwrap();
        }
        if write {
            d.embed_alphas(&lib);
            let at = repo.join(format!("showcase/{slug}"));
            std::fs::create_dir_all(&at).unwrap();
            library::save_design(at.join("design.ring.json"), &d).unwrap();
            render::write_turntable_gif(format!("{dir}/{slug}.gif"), &out.mesh, 48, 560, tint).unwrap();
            println!("  wrote {}", at.join("design.ring.json").display());
        }
    }
}
