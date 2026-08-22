// The showcase: a curated collection of simple, elegant designs, one or two
// per feature family, every one held to the field verdict before it renders.
//
//   cargo run --release --example showcase [out_dir]
//
// Writes a hero PNG per ring (plus a face-on view where the design lives on
// the side faces, and a turntable GIF for two), and saves each design under
// <designs>/showcase/ so every piece opens in the app. Only builtin alphas,
// recipes and text are used, so the files are portable to an empty machine.
use ringdesign_core::alpha::{AlphaLibrary, ProcRecipe, Procedural};
use ringdesign_core::castability::{self, Verdict};
use ringdesign_core::curve::CurveLayer;
use ringdesign_core::field::{
    Blend, Decal, DecalLayer, FluteProfile, FlutesLayer, Layer, LayerEntry, MilgrainLayer,
    OpenworkLayer,
    SeatRunLayer, SeatStyle, SideFacePick, SignetOutline, VGate, SIDE_FACE_MIN_DRAFT_DEG,
};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::pave::{self, PaveRegion, PaveSpec};
use ringdesign_core::profile::{ShankKey, ShankKind, TOP_DEG};
use ringdesign_core::svg::SvgAlpha;
use ringdesign_core::text::{TextAlpha, TextFont};
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{library, render, stones, ProfileStyle, RingDesign};

const YELLOW: [f32; 3] = [0.86, 0.70, 0.42];
const ROSE: [f32; 3] = [0.84, 0.60, 0.49];
const SILVER: [f32; 3] = [0.79, 0.80, 0.81];
const WHITE: [f32; 3] = [0.83, 0.83, 0.80];
const PLATINUM: [f32; 3] = [0.75, 0.76, 0.78];

struct Shots {
    face: bool,
    gif: bool,
    /// View the -Z side face — where `wider()` puts single-face ornament.
    flip: bool,
    /// Hero attitude; 1.12 is the catalog three-quarter view. Geometry that
    /// moves along the finger (a twist's edge slide) needs a side-on pitch.
    hero_pitch: f64,
}

const HERO: Shots = Shots { face: false, gif: false, flip: false, hero_pitch: 1.12 };
const FACE: Shots = Shots { face: true, gif: false, flip: false, hero_pitch: 1.12 };
const GIF: Shots = Shots { face: false, gif: true, flip: false, hero_pitch: 1.12 };

/// Face shots for ornament that landed on one side face: view that face.
fn face_of(low_face: bool) -> Shots {
    Shots { face: true, gif: false, flip: low_face, hero_pitch: 1.12 }
}

/// Whether the wider side face — where single-face ornament lands — is the
/// low-`v` (-Z) one. The tie on a symmetric band breaks on float noise, so
/// this is read per design, never assumed.
fn wider_is_low(d: &RingDesign) -> bool {
    let ctx = d.field_context();
    ctx.side_faces_std()
        .and_then(|f| f.wider())
        .map(|(lo, hi)| 0.5 * (lo + hi) < ctx.crest_v_mm)
        .unwrap_or(false)
}

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

fn side_tiling(d: &RingDesign, alpha: &str, height: f64) -> TilingLayer {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for(alpha, &ctx);
    t.height_mm = height;
    if !t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG) {
        panic!("no side faces on a band meant to carry side relief");
    }
    t
}

/// Field-check, render, save. The showcase refuses to ship an uncastable ring.
fn finish(
    dir: &str,
    slug: &str,
    blurb: &str,
    tint: [f32; 3],
    shots: Shots,
    mut d: RingDesign,
    lib: &mut AlphaLibrary,
) {
    d.name = slug
        .split_once('-')
        .map(|(_, rest)| rest.replace('-', " "))
        .unwrap_or_else(|| slug.into());
    d.bake_all(lib);

    let field = castability::analyze_field(&d, lib, &d.draft, 256, 128);
    println!("{slug:<24} {blurb}");
    println!(
        "{:<24} {}: {:.3}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
        "",
        field.verdict.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm,
    );
    for n in &field.notes {
        println!("{:<24}   note: {n}", "");
    }
    for f in ringdesign_core::dfm::findings(&d) {
        println!("{:<24}   dfm: {}: {}", "", f.label, f.message);
    }
    if let Some(s) = stones::report(&d, field.parting_z_mm) {
        println!(
            "{:<24}   stones: {} stones, {:.2} ct total",
            "", s.stone_count, s.total_carats,
        );
        let mut seen = std::collections::BTreeSet::new();
        for seat in &s.seats {
            for w in &seat.warnings {
                if seen.insert(w.clone()) {
                    println!("{:<24}   stone warning: {w}", "");
                }
            }
        }
    }
    assert_ne!(field.verdict, Verdict::NotCastable, "{slug} must not ship uncastable");

    let out = mesh::build(
        &d,
        lib,
        BuildParams { theta_steps: 1600, profile_steps: 360, ..Default::default() },
    );
    assert!(out.report.validation.watertight, "{slug} not watertight");

    // Pitch 1.12 tilts the +Z annulus toward the viewer; ornament that lives
    // on the -Z face (the `wider()` pick) needs the mirrored attitude.
    let pi = std::f64::consts::PI;
    let hero_pitch = if shots.flip { pi - shots.hero_pitch } else { shots.hero_pitch };
    render::write_png(format!("{dir}/{slug}-hero.png"), &out.mesh, 0.55, hero_pitch, 900, tint)
        .unwrap();
    if shots.face {
        // Nearly straight down the finger axis — the side-face annulus flat
        // to the viewer, a small tilt for depth. The flipped view also turns
        // half round so the ring's top stays at the top of the frame.
        let (face_yaw, face_pitch) = if shots.flip { (pi, pi - 0.35) } else { (0.0, 0.35) };
        render::write_png(
            format!("{dir}/{slug}-face.png"),
            &out.mesh,
            face_yaw,
            face_pitch,
            900,
            tint,
        )
        .unwrap();
    }
    if shots.gif {
        render::write_turntable_gif(format!("{dir}/{slug}.gif"), &out.mesh, 36, 480, tint).unwrap();
    }

    let designs = library::default_design_dir().join("showcase");
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design(designs.join(format!("{slug}.ring.json")), &d).unwrap();
    println!();
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp/ring-showcase".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut lib = AlphaLibrary::builtin();

    // --- 01. Cabochon signet: the domed table. -------------------------------
    // A buff-top round signet — the cap gives the table real draft everywhere,
    // where a flat table is a zero-draft plane.
    let mut d = signet(SignetOutline::Round, 13.0, 2.0);
    d.shank.head.dome = 1.0;
    d.shank.head.table_dome_mm = 1.4;
    finish(&dir, "01-cabochon-signet", "round head under a cabochon dome", YELLOW, GIF, d, &mut lib);

    // --- 02. Shield signet: an upright outline, bare. ------------------------
    // A shield reads up the finger, so the outline machinery turns it a
    // quarter round and the section slides off its own mid-plane to carry it.
    // A signet's ~2 mm side face holds no ornament the sand can keep, so it
    // holds none.
    let d = signet(SignetOutline::Shield, 13.5, 2.2);
    finish(&dir, "02-shield-signet", "upright shield head, blank for the engraver", SILVER, HERO, d, &mut lib);

    // --- 03. Posy band: an inscription in relief. ----------------------------
    // The oldest ring design there is — a motto round the band, raised script
    // on the side face, where lettering pulls straight out of the sand.
    let mut d = squared(7.0, 2.6);
    d.texts.push(TextAlpha {
        name: "Posy".into(),
        text: "amor vincit omnia".into(),
        font: TextFont::Script,
        tracking: 0.06,
    });
    d.bake_all(&mut lib);
    let ctx = d.field_context();
    let (lo, hi) = ctx.side_faces_std().and_then(|f| f.wider()).unwrap();
    let low_face = wider_is_low(&d);
    let a = lib.get("Posy").unwrap();
    // The raster pads above and below the glyphs; measure the ink band so
    // the letters, not the padding, fill 0.85 of the face. The stamp width
    // then follows the raster's own aspect.
    let (mut r0, mut r1) = (a.height, 0usize);
    for y in 0..a.height {
        if a.data[y * a.width..(y + 1) * a.width].iter().any(|&s| s > 0.05) {
            r0 = r0.min(y);
            r1 = r1.max(y);
        }
    }
    let glyph_frac = if r1 >= r0 { (r1 - r0 + 1) as f64 / a.height as f64 } else { 1.0 };
    let stamp_h = (hi - lo) * 0.85 / glyph_frac.max(0.05);
    let size = stamp_h * a.width as f64 / a.height as f64;
    let mut decals = DecalLayer::default();
    decals.alpha = "Posy".into();
    decals.feather_mm = 0.2;
    decals.decals = vec![Decal {
        theta_deg: TOP_DEG,
        v_mm: 0.5 * (lo + hi),
        size_mm: size,
        rotation_deg: 0.0,
        height_mm: 0.32,
        // The (u, v) chart reads true from -Z; a stamp on the high face must
        // mirror or its cast letters read backwards.
        flip: !low_face,
    }];
    let mut e = LayerEntry::new("Posy", Layer::Decals(decals));
    e.window.v_gate = VGate::SideFaces(SideFacePick::Wider);
    d.layers.layers.push(e);
    finish(&dir, "03-posy-band", "raised script motto on the side face", ROSE, face_of(low_face), d, &mut lib);

    // --- 04. Milgrain band: one bead line riding the crest. ------------------
    // A bead straddling the crest splits between cope and drag, so the row
    // releases; the same row 1.9 mm off-crest locked at -37 deg over 4.8% —
    // the rails lesson, measured again. One line at the crest is the sand's
    // whole milgrain vocabulary, so one line is what the band wears. (The
    // engine-turn family stays off this showcase for the same reason: every
    // guilloche generator's period sits under the sand's detail floor at any
    // tile size a band face can hold.)
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.2;
    d.profile.thickness_mm = 2.1;
    let ctx = d.field_context();
    d.layers.layers.push(LayerEntry::new(
        "Milgrain",
        Layer::Milgrain(MilgrainLayer {
            v_mm: ctx.crest_v_mm,
            bead_diameter_mm: 0.48,
            beads_around: 140,
            height_mm: 0.2,
            mirror: false,
        }),
    ));
    finish(&dir, "04-milgrain-band", "a single milgrain line riding the crest", SILVER, HERO, d, &mut lib);

    // --- 05. Laurel band: a running vine in wire. ----------------------------
    let mut d = squared(5.5, 2.6);
    let ctx = d.field_context();
    let (lo, hi) = ctx.side_faces_std().and_then(|f| f.wider()).unwrap();
    let mut vine = CurveLayer::preset_vine(&ctx);
    vine.retarget_v(0.5 * (lo + hi), (hi - lo) * 0.26);
    vine.width_mm = 0.6;
    vine.height_mm = 0.32;
    vine.repeats_around = 10;
    vine.mirror_v = true;
    let mut e = LayerEntry::new("Vine", Layer::Curve(vine));
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.25;
    d.layers.layers.push(e);
    finish(&dir, "05-laurel-vine", "a vine wire on each side face", YELLOW, FACE, d, &mut lib);

    // --- 06. Silk twist: the castable spiral. --------------------------------
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.0;
    d.profile.thickness_mm = 2.0;
    d.shank.kind = ShankKind::Twist;
    d.shank.amount = 0.9;
    finish(&dir, "06-silk-twist", "the light line spirals; both flanks stay monotone", ROSE, Shots { gif: true, hero_pitch: 1.5, ..HERO }, d, &mut lib);

    // --- 07. Gypsy eternity: a seat run with bur dimples. --------------------
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.6;
    d.profile.thickness_mm = 2.5;
    let ctx = d.field_context();
    let gem = Gem::calibrated(GemCut::Round, 2.0);
    let mut run = SeatRunLayer { gem, bridge_mm: 0.5, ..Default::default() };
    run.seat.v_mm = ctx.crest_v_mm;
    // No dimples here: a dimple is a pocket, and on the crown's zero-draft
    // mound tops its walls turn to ceilings. They go on the side-face seats.
    run.solve_spacing(&ctx);
    d.layers.layers.push(LayerEntry::new("Eternity run", Layer::SeatRun(run)));
    finish(&dir, "07-gypsy-eternity", "mound row round the whole band", PLATINUM, HERO, d, &mut lib);

    // --- 08. Bezel crescent: the packer, on the side face. -------------------
    // Bezel collars and bur dimples are both side-face features — a pocket's
    // walls stand parallel to the pull there and turn to ceilings on the
    // crown. A gypsy skirt wants a 3.3 mm face; a bezel collar fits in 2.5.
    let mut d = squared(6.5, 3.6);
    let low_face = wider_is_low(&d);
    let spec = PaveSpec {
        gem: Gem::calibrated(GemCut::Round, 1.0),
        bridge_mm: 0.4,
        theta_deg: TOP_DEG,
        span_deg: 150.0,
        region: PaveRegion::SideFace(SideFacePick::Wider),
        stagger: true,
        style: SeatStyle::Bezel,
        rot_deg: 0.0,
        blend_mm: 0.4,
        recess_mm: 0.4,
        pinned: Vec::new(),
    };
    let (mut entry, outcome) = pave::fill(&d, &spec).expect("pave should fit this face");
    println!("{:<24}   pave: {} seats in {} rows", "", outcome.seats, outcome.rows);
    // A slimmer collar wall, refitted: the default 0.5 mm wall pushes the
    // foot past the face's edge clearance, and a skirt tightened to make
    // room falls under the sand's own detail floor. No bur dimple — the
    // bezel's pocket already starts the drill true.
    if let Layer::Group(g) = &mut entry.layer {
        for e in &mut g.stack.layers {
            if let Layer::SeatPad(s) = &mut e.layer {
                s.bezel_wall_mm = 0.4;
                s.fit_stone(spec.gem);
                s.blend_mm = 0.35;
            }
        }
    }
    d.layers.layers.push(entry);
    finish(&dir, "08-bezel-crescent", "packed bezel collars over the top arc of a side face", WHITE, face_of(low_face), d, &mut lib);

    // --- 09. Hammered bombe: the cocktail dome under a peened skin. ----------
    // The melon variant — lobes along the ring — is honestly not sand-castable:
    // every groove between lobes is a valley, and a valley's two walls cannot
    // both clear one parting plane near the crest (the two-flange proof,
    // measured here at 8.2% / -22 deg). A hammered skin from a recipe keeps
    // to dimple slopes the dome's own draft absorbs.
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 9.0;
    d.profile.thickness_mm = 2.6;
    d.shank.kind = ShankKind::Bombe;
    d.shank.amount = 0.85;
    d.recipes.push(ProcRecipe {
        name: "Peen".into(),
        kind: Procedural::Hammered,
        repeats: 1,
        quarter_turns: 1,
        gamma: 0.8,
        invert: false,
    });
    // Peen the flanks only: a dimple is a pocket, and near the crest a
    // pocket's walls lock by their own slope (measured 2.7% at -9 deg for a
    // full-crown skin). On the flanks the dome's draft absorbs it, and the
    // polished crest ribbon between is the design.
    let ctx = d.field_context();
    for (label, side) in [("Peen low", -1.0), ("Peen high", 1.0)] {
        let mut t = TilingLayer::default_for("Peen", &ctx);
        t.height_mm = 0.06;
        t.contrast = 0.8;
        t.rows = 1;
        t.v_center_mm = ctx.crest_v_mm + side * 2.8;
        t.v_span_mm = 2.8;
        t.feather_mm = 0.7;
        t.repeats_around = t.repeats_for_square_cells(&ctx);
        d.layers.layers.push(LayerEntry::new(label, Layer::Tiling(t)));
    }
    finish(&dir, "09-hammered-bombe", "peened flanks under a polished crest ribbon", YELLOW, GIF, d, &mut lib);

    // --- 10. Reeded band: the coin edge. -------------------------------------
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.2;
    d.profile.thickness_mm = 2.0;
    d.layers.layers.push(LayerEntry::new(
        "Reeding",
        Layer::Flutes(FlutesLayer {
            count: 110,
            profile: FluteProfile::Round,
            width_mm: 0.42,
            height_mm: 0.12,
            lean: 0.35,
            along: false,
        }),
    ));
    finish(&dir, "10-reeded-band", "110 leaning reeds across the band", ROSE, HERO, d, &mut lib);

    // --- 11. Lace band: openwork carved toward the bore. ---------------------
    // The mask is a one-circle SVG, so each cell carves one round porthole —
    // bold enough for the sand, where every builtin lattice tile carries
    // several sub-floor periods. The SVG travels in the design file.
    let mut d = squared(6.0, 2.6);
    d.svgs.push(SvgAlpha {
        name: "Porthole".into(),
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="19" fill="#000"/></svg>"##
            .into(),
        invert: false,
    });
    d.bake_all(&mut lib);
    let mut mask = side_tiling(&d, "Porthole", 1.0);
    mask.edge_mm = 0.35;
    let mut e = LayerEntry::new(
        "Lace",
        Layer::Openwork(OpenworkLayer { tiling: mask, depth_mm: 1.2, keep_mm: 0.8 }),
    );
    e.blend = Blend::Add;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    finish(&dir, "11-lace-openwork", "a ring of portholes pierced into both side faces", SILVER, FACE, d, &mut lib);

    // --- 12. Ribbon: keyframed width and thickness. --------------------------
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.4;
    d.profile.thickness_mm = 2.1;
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = vec![
        ShankKey { theta_deg: TOP_DEG, width_scale: 1.35, thickness_scale: 1.12, crown_scale: 1.0 },
        ShankKey { theta_deg: TOP_DEG + 180.0, width_scale: 0.8, thickness_scale: 0.88, crown_scale: 1.0 },
    ];
    finish(&dir, "12-ribbon-keyframes", "graduated band from two authored stations", WHITE, HERO, d, &mut lib);

    // --- 13. Channel band: rails and a recess on the wide face. --------------
    let mut d = squared(6.0, 3.6);
    let low_face = wider_is_low(&d);
    let entry = pave::channel_set(&d, Gem::calibrated(GemCut::Round, 1.5), 0.45)
        .expect("channel set needs a thick band, and this one is");
    d.layers.layers.push(entry);
    finish(&dir, "13-channel-band", "two rails flanking a recessed channel", PLATINUM, face_of(low_face), d, &mut lib);

    println!(
        "renders in {dir}, designs in {}",
        library::default_design_dir().join("showcase").display()
    );
}
