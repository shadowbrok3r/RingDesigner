// Six plain bands whose whole subject is the pattern, built for two-part sand.
//
// The pass began with raster tilings and every one of them failed the sand's
// detail floor: a builtin mask carries several motif periods per tile, so at
// side-face scale its own strokes land at 0.02-0.2 mm against a 0.35 mm floor.
// `examples/pattern_floor` prints what each mask would need. The answer these
// bands take is that in sand a pattern wants to be *analytic* — flutes, rails,
// beads, a swept wire — where the finest feature is a number you set rather
// than a consequence of a raster. The two raster bands here are the exception
// that proves it: they are thick enough for the mask, and the repeat count is
// solved by `dfm::fit_to_floor` instead of guessed.
//
//   cargo run --release --example patternbands [out_dir]
//   BANDS=R,S runs only the named bands.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, Verdict};
use ringdesign_core::curve::CurveLayer;
use ringdesign_core::dfm::{self, FloorFit};
use ringdesign_core::field::{
    Blend, BorderLayer, BorderProfile, FluteProfile, FlutesLayer, Layer, LayerEntry, MilgrainLayer,
    Remap, VGate, Window, SIDE_FACE_MIN_DRAFT_DEG,
};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::{ShankKey, ShankKind, TOP_DEG};
use ringdesign_core::render::{self, Part};
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{library, ProfileStyle, RingDesign};

const GOLD: [f32; 3] = [0.86, 0.70, 0.42];

fn squared(width: f64, thickness: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d
}

fn domed(width: f64, thickness: f64, style: ProfileStyle) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(style);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d
}

/// Flutes running across the band, repeated around it. Their walls face along
/// the ring — square to the parting plane, never back under it — which is why
/// reeding casts where a groove running *around* the band would lock.
fn reeding(count: u32, width: f64, height: f64, profile: FluteProfile) -> FlutesLayer {
    let mut f = FlutesLayer::default();
    f.count = count;
    f.profile = profile;
    f.width_mm = width;
    f.height_mm = height;
    f.along = false;
    f
}

/// Fit a builtin mask to the side faces and let the sand pick the repeats.
fn solved_tiling(d: &RingDesign, lib: &AlphaLibrary, alpha: &str, height: f64) -> Option<TilingLayer> {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for(alpha, &ctx);
    t.height_mm = height;
    t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG);
    match dfm::fit_to_floor(&mut t, lib, &ctx, d.draft.min_detail_mm) {
        FloorFit::Repeats(n) => {
            println!("{:<20}   solver: {alpha} takes {n} repeats on this band", "");
            Some(t)
        }
        FloorFit::NeedsTallerCell { min_cell_h_mm } => {
            println!("{:<20}   solver: {alpha} needs a {min_cell_h_mm:.2} mm face — band too thin", "");
            None
        }
        FloorFit::Unmeasurable => None,
    }
}

fn want(key: &str) -> bool {
    std::env::var("BANDS").map(|v| v.split(',').any(|k| k.trim() == key)).unwrap_or(true)
}

fn finish(dir: &str, slug: &str, _blurb: &str, mut d: RingDesign, lib: &mut AlphaLibrary) {
    d.name = slug.split_once('-').map(|(_, r)| r.replace('-', " ")).unwrap_or_else(|| slug.into());
    d.bake_all(lib);
    let field = castability::attributed_field_report(&d, lib, &d.draft, 256, 128);
    println!(
        "{:<20} {} — {:.4}% undercut, worst {:+.1} deg, wall {:.2} mm",
        "",
        field.verdict.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm
    );
    for n in field.notes.iter().filter(|n| !n.starts_with("Field-sampled: the surface")) {
        println!("{:<20}   note: {n}", "");
    }
    let dfms = ringdesign_core::dfm::findings_in(&d, lib);
    for f in &dfms {
        println!("{:<20}   DFM: {}: {}", "", f.label, f.message);
    }
    assert_ne!(field.verdict, Verdict::NotCastable, "{slug} must not ship uncastable");
    assert!(dfms.is_empty(), "{slug} must clear the sand's detail floor");

    let out = mesh::build(&d, lib, BuildParams { theta_steps: 1600, profile_steps: 360, ..Default::default() });
    assert!(out.report.validation.watertight, "{slug} not watertight");
    println!("{:<20}   {:.1} mm3, {:.2} g silver", "", out.report.volume_mm3,
             out.report.metals.first().map(|m| m.grams).unwrap_or(0.0));
    let parts = vec![Part::metal(&out.mesh, GOLD)];
    render::write_png_parts(format!("{dir}/{slug}-hero.png"), &parts, 0.55, 1.05, 1200).unwrap();
    render::write_png_parts(format!("{dir}/{slug}-side.png"), &parts, 1.5708, 0.0, 1000).unwrap();
    let designs = library::default_design_dir().join("patternbands");
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design(designs.join(format!("{slug}.ring.json")), &d).unwrap();
    println!();
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut lib = AlphaLibrary::builtin();

    // R — Reed. Coin-edge reeding: 56 ribs across a low dome. The finest
    // feature is the rib itself, 0.62 mm, set as a number.
    if want("R") {
        println!("R-reed              Coin-edge reeding across a low dome, beads holding each edge.");
        let mut d = domed(5.0, 2.2, ProfileStyle::LowDome);
        d.layers.layers.push(LayerEntry::new("Reeding", Layer::Flutes(reeding(56, 0.62, 0.26, FluteProfile::Round))));
        finish(&dir, "R-reed", "", d, &mut lib);
    }

    // S — Step. The same reeding read through the terrace remap: each rib
    // stops being a smooth swell and becomes three flat treads, which is a
    // different object entirely and still a monotone drop per rib.
    if want("S") {
        println!("S-step              Reeding terraced into three treads a rib — architecture, not texture.");
        let mut d = domed(6.0, 2.6, ProfileStyle::CushionDome);
        let mut e = LayerEntry::new("Stepped reeding", Layer::Flutes(reeding(34, 1.05, 0.44, FluteProfile::Square)));
        e.remap = Remap::Terrace { steps: 3, span_mm: 0.44, riser: 0.30 };
        d.layers.layers.push(e);
        finish(&dir, "S-step", "", d, &mut lib);
    }

    // C — Cord. Two rope rails riding the side-face boundaries with a plain
    // polished crest between them. The rail's twist count is an integer, so it
    // closes on itself, and its section is a number the sand can hold.
    if want("C") {
        println!("C-cord              Twin rope rails flanking a polished crest ribbon.");
        let mut d = squared(6.4, 2.4);
        let ctx = d.field_context();
        let faces = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG).expect("a squared band has side faces");
        for (i, f) in [faces.low, faces.high].iter().flatten().enumerate() {
            let mut b = BorderLayer::default();
            b.v_mm = (f.0 + f.1) * 0.5;
            b.width_mm = 0.95;
            b.height_mm = 0.30;
            b.profile = BorderProfile::Rope;
            b.rope_twists = 84;
            b.mirror = false;
            d.layers.layers.push(LayerEntry::new(format!("Rope rail {}", i + 1), Layer::Border(b)));
        }
        finish(&dir, "C-cord", "", d, &mut lib);
    }

    // G — Cigar. The thick band the raster masks actually want. Lattice needs
    // a 3.1 mm face; a 5.6 mm cigar gives it one, and the solver picks the
    // count instead of me guessing it.
    if want("G") {
        println!("G-cigar-lattice     A cigar band thick enough to carry a real mask, repeats solved.");
        let mut d = squared(7.0, 4.4);
        if let Some(t) = solved_tiling(&d, &lib, "Lattice", 0.34) {
            d.layers.layers.push(LayerEntry::new("Lattice", Layer::Tiling(t)));
        }
        finish(&dir, "G-cigar-lattice", "", d, &mut lib);
    }
    wave2(&dir, &mut lib);
    wave3(&dir, &mut lib);
}

// ---------------------------------------------------------------------------
// Wave two: the silhouette carries as much of the pattern as the surface does.
// Every one of these is a shape somebody wears — the variety is in the shank
// kind and the profile, not in novelty for its own sake.
// ---------------------------------------------------------------------------

fn flute_layer(f: FlutesLayer, name: &str) -> LayerEntry {
    LayerEntry::new(name, Layer::Flutes(f))
}

/// Beads at a chosen `v`. On the crest line they straddle the parting plane
/// and split cleanly; anywhere else they must sit on a side face.
fn beads(v_mm: f64, count: u32, dia: f64, height: f64, name: &str) -> LayerEntry {
    let mut m = MilgrainLayer::default();
    m.beads_around = count;
    m.bead_diameter_mm = dia;
    m.height_mm = height;
    m.v_mm = v_mm;
    m.mirror = false;
    LayerEntry::new(name, Layer::Milgrain(m))
}

fn wave2(dir: &str, lib: &mut AlphaLibrary) {
    // 1 — Twist. The band spirals; the crest stays on the parting plane and
    // both flanks stay monotone drops, which is what makes a twist castable.
    if want("1") {
        println!("1-twist             A spiralling band, reeded along the twist.");
        let mut d = domed(5.0, 2.4, ProfileStyle::HalfRound);
        d.shank.kind = ShankKind::Twist;
        d.shank.amount = 0.8;
        d.layers.layers.push(flute_layer(reeding(52, 0.62, 0.11, FluteProfile::Round), "Fine reeding"));
        finish(dir, "1-twist", "", d, lib);
    }

    // the wave wedding band, and the reason milgrain must not ride it.
    if want("2") {
        println!("2-wave              A wave band — the edges travel, the crest does not.");
        let mut d = domed(6.0, 2.3, ProfileStyle::LowDome);
        d.shank.kind = ShankKind::Wave;
        d.shank.amount = 0.75;
        finish(dir, "2-wave", "", d, lib);
    }

    // the band flares and a channel is cut into each side face, where the
    // walls stand radial and the floor faces the pull.
    if want("3") {
        println!("3-split             A split shank read as two rails, channel on the side faces.");
        let mut d = domed(5.4, 3.0, ProfileStyle::DShape);
        d.shank.kind = ShankKind::Split;
        d.shank.amount = 0.65;
        finish(dir, "3-split", "", d, lib);
    }

    // 4 — Knife. A crisp central ridge with the flanks finely reeded — the
    // superellipse drop guarantees the ridge itself can never undercut.
    if want("4") {
        println!("4-knife             Knife-edge ridge, flanks reeded fine.");
        let mut d = domed(5.6, 2.8, ProfileStyle::KnifeEdge);
        d.layers.layers.push(flute_layer(reeding(88, 0.36, 0.14, FluteProfile::Vee), "Flank reeding"));
        finish(dir, "4-knife", "", d, lib);
    }

    // 5 — Tuxedo. A beveled men's band: flat crown, chamfered shoulders, and
    // a bead line dropped onto each chamfer where the metal faces the pull.
    if want("5") {
        println!("5-tuxedo            Beveled men's band, bead lines on the chamfers.");
        let mut d = domed(6.5, 2.2, ProfileStyle::Beveled);
        d.profile.flatten_sides();
        let ctx = d.field_context();
        let faces = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG).expect("beveled sides");
        for (i, f) in [faces.low, faces.high].iter().flatten().enumerate() {
            d.layers.layers.push(beads(f.0 + (f.1 - f.0) * 0.55, 86, 0.46, 0.20, &format!("Bead line {}", i + 1)));
        }
        finish(dir, "5-tuxedo", "", d, lib);
    }

    // 6 — Facet. Twelve broad vee flutes: a diamond-cut band, each facet a
    // flat plane, the arrises catching light all the way round.
    if want("6") {
        println!("6-facet             Twelve broad facets — a diamond-cut band.");
        let mut d = domed(5.8, 2.5, ProfileStyle::CushionDome);
        d.layers.layers.push(flute_layer(reeding(14, 2.0, 0.34, FluteProfile::Vee), "Facets"));
        finish(dir, "6-facet", "", d, lib);
    }

    // 7 — Cobble. Big beads marching down both side faces: a cobbled edge,
    // which is milgrain taken up two sizes and moved off the crest.
    if want("7") {
        println!("7-cobble            Cobbled side faces — beads taken up two sizes.");
        let mut d = squared(6.8, 2.8);
        let ctx = d.field_context();
        let faces = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG).expect("squared sides");
        for (i, f) in [faces.low, faces.high].iter().flatten().enumerate() {
            d.layers.layers.push(beads((f.0 + f.1) * 0.5, 46, 1.05, 0.34, &format!("Cobbles {}", i + 1)));
        }
        finish(dir, "7-cobble", "", d, lib);
    }

    // all the work and there is no ornament at all.
    if want("8") {
        println!("8-cathedral         Sweeping cathedral shoulders, no ornament at all.");
        let mut d = domed(4.6, 2.2, ProfileStyle::HighDome);
        d.shank.kind = ShankKind::Cathedral;
        d.shank.amount = 0.8;
        finish(dir, "8-cathedral", "", d, lib);
    }

    // 9 — Flare. A reverse taper — narrow at the palm, widest at the top —
    // with a single groove pair framing a polished crest ribbon. The grooves
    // sit on the side faces, never beside the crest, which is where rails lean.
    if want("9") {
        println!("9-flare             Reverse taper with grooves framing a polished crest.");
        let mut d = squared(7.2, 2.4);
        d.shank.kind = ShankKind::ReverseTaper;
        d.shank.amount = 0.7;
        let ctx = d.field_context();
        let faces = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG).expect("squared sides");
        for (i, f) in [faces.low, faces.high].iter().flatten().enumerate() {
            let mut b = BorderLayer::default();
            b.v_mm = f.0 + (f.1 - f.0) * 0.35;
            b.width_mm = 0.7;
            b.height_mm = 0.26;
            b.profile = BorderProfile::Step;
            b.mirror = false;
            d.layers.layers.push(LayerEntry::new(format!("Step rail {}", i + 1), Layer::Border(b)));
        }
        finish(dir, "9-flare", "", d, lib);
    }

    // shape that wears thin where a hand wants it thin.
    if want("10") {
        println!("10-pinch            Pinched over the knuckle, full at the palm.");
        let mut d = domed(6.4, 2.6, ProfileStyle::DShape);
        d.shank.kind = ShankKind::Pinched;
        d.shank.amount = 0.75;
        d.layers.layers.push(beads(d.field_context().crest_v_mm, 96, 0.44, 0.19, "Crest beads"));
        finish(dir, "10-pinch", "", d, lib);
    }

    // 11 — Barrel. Broad reeding running only over the top third, the rest
    // left plain: a windowed pattern, which is how ornament earns its place.
    if want("11") {
        println!("11-barrel           Reeding windowed to the crown arc, plain at the palm.");
        let mut d = domed(6.6, 2.6, ProfileStyle::CushionDome);
        d.shank.kind = ShankKind::Bombe;
        d.shank.amount = 0.6;
        let mut e = flute_layer(reeding(46, 0.85, 0.30, FluteProfile::Round), "Crown reeding");
        e.window = Window::around(TOP_DEG, 150.0);
        e.window.fade_deg = 26.0;
        d.layers.layers.push(e);
        finish(dir, "11-barrel", "", d, lib);
    }

    // 12 — Cushion. Four gentle stations make the plan a soft square: a
    // cushion band, where the silhouette is the whole idea and every section
    // is still one dome.
    if want("12") {
        println!("12-cushion          A soft-square plan — the silhouette is the pattern.");
        let mut d = domed(5.0, 2.4, ProfileStyle::HalfRound);
        d.shank.kind = ShankKind::Keyframes;
        d.shank.amount = 1.0;
        d.shank.keys = (0..4)
            .map(|i| ShankKey {
                theta_deg: TOP_DEG + 90.0 * i as f64,
                width_scale: 1.0,
                thickness_scale: 1.06,
                crown_scale: 0.92,
            })
            .collect();
        finish(dir, "12-cushion", "", d, lib);
    }
}

// ---------------------------------------------------------------------------
// Wave three. The four that failed were shapes with nothing on them; a band
// needs a silhouette move *and* a surface move. These carry both, and the
// tuxedo comes back with the knife edge it wanted.
// ---------------------------------------------------------------------------

fn wave3(dir: &str, lib: &mut AlphaLibrary) {
    // 13 — Channel. A deep groove running the crest line, framed by bead
    // lines on the flats. A groove centred on the parting plane splits between
    // cope and drag and each half draws its own way out — the same reason a
    // bead row survives there and a rail beside it does not.
    if want("13") {
        println!("13-channel          A deep crest channel framed by bead lines — an inlay band in metal.");
        let mut d = domed(7.6, 3.0, ProfileStyle::CushionDome);
        let ctx = d.field_context();
        let mut b = BorderLayer::default();
        b.v_mm = ctx.crest_v_mm;
        b.width_mm = 3.4;
        b.height_mm = 1.05;
        b.profile = BorderProfile::Flat;
        b.mirror = false;
        let mut e = LayerEntry::new("Crest channel", Layer::Border(b));
        e.blend = Blend::Subtract;
        d.layers.layers.push(e);
        if let Some(faces) = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG) {
            for (i, f) in [faces.low, faces.high].iter().flatten().enumerate() {
                d.layers.layers.push(beads(f.0 + (f.1 - f.0) * 0.5, 76, 0.5, 0.22, &format!("Frame beads {}", i + 1)));
            }
        }
        finish(dir, "13-channel", "", d, lib);
    }

    // 14 — Bark. Two straight reedings at counts sharing no factor, joined by
    // a smooth max: neither run is the subject, the beat between them is, and
    // it reads as split bark. Both leans are zero, so every wall still faces
    // squarely around the ring. A rope border on the crest was tried here
    // first and fielded 1.96% — a rope's own helix leans, so unlike a bead row
    // it is not safe on the parting line, only on a side face.
    if want("14") {
        println!("14-bark             Two reedings beating against each other — split bark.");
        let mut d = domed(7.0, 2.8, ProfileStyle::CushionDome);
        d.layers.layers.push(flute_layer(reeding(29, 1.15, 0.34, FluteProfile::Round), "Bark A"));
        let mut e = flute_layer(reeding(41, 0.8, 0.26, FluteProfile::Vee), "Bark B");
        e.blend = Blend::SmoothMax;
        e.soft_mm = 0.14;
        d.layers.layers.push(e);
        finish(dir, "14-bark", "", d, lib);
    }

    // 15 — Ziggurat. Three step rails stacked down each side face: an
    // art-deco terrace where the shape of the *edge* is the ornament and the
    // crown stays a plain polished dome.
    if want("15") {
        println!("15-ziggurat         Stacked step rails terracing each side face.");
        let mut d = squared(8.0, 3.2);
        let ctx = d.field_context();
        let faces = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG).expect("squared sides");
        for (i, f) in [faces.low, faces.high].iter().flatten().enumerate() {
            for (k, (at, w, h)) in [(0.18, 1.0, 0.42), (0.46, 0.72, 0.28), (0.72, 0.5, 0.16)].iter().enumerate() {
                let mut b = BorderLayer::default();
                b.v_mm = f.0 + (f.1 - f.0) * at;
                b.width_mm = *w;
                b.height_mm = *h;
                b.profile = BorderProfile::Step;
                b.mirror = false;
                d.layers.layers.push(LayerEntry::new(format!("Terrace {}-{}", i + 1, k + 1), Layer::Border(b)));
            }
        }
        finish(dir, "15-ziggurat", "", d, lib);
    }

    // 16 — Torque. The twist that passed, taken up two sizes: fourteen broad
    // vee facets spiralling with the band. Straight flutes on a twisting
    // crest, so the lean stays zero and the spiral comes from the shank.
    if want("16") {
        println!("16-torque           A twisting band cut into fourteen broad spiral facets.");
        let mut d = domed(6.2, 2.9, ProfileStyle::HalfRound);
        d.shank.kind = ShankKind::Twist;
        d.shank.amount = 0.9;
        d.layers.layers.push(flute_layer(reeding(16, 1.8, 0.40, FluteProfile::Vee), "Spiral facets"));
        finish(dir, "16-torque", "", d, lib);
    }

    // 17 — Tuxedo II. The knife edge the first one wanted: a ridged crest
    // with the shoulders chamfered flat, bead lines dropped onto the flats.
    if want("17") {
        println!("17-tuxedo-knife     Knife-edge crest, chamfered flats, bead lines on the chamfers.");
        // Keep the ridge: flatten_sides would square the band and take the
        // knife away with it. A knife's flanks are steep, so they carry beads
        // with plenty of draft even though they are not side faces.
        let mut d = domed(7.0, 3.0, ProfileStyle::KnifeEdge);
        let ctx = d.field_context();
        for (i, sign) in [-1.0f64, 1.0].iter().enumerate() {
            let v = ctx.crest_v_mm + sign * ctx.band_v_len_mm * 0.30;
            d.layers.layers.push(beads(v, 84, 0.5, 0.22, &format!("Flank beads {}", i + 1)));
        }
        d.layers.layers.push(flute_layer(reeding(80, 0.42, 0.12, FluteProfile::Vee), "Ridge reeding"));
        finish(dir, "17-tuxedo-knife", "", d, lib);
    }
}
