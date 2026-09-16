// The bake-off: six new designs for the two-engine comparison against
// CrossGems, every one held to the field verdict and exported as OBJ so
// both engines can be judged by the same mesh referee. New ground, not
// serpentarium reruns: terraced plate mail, a kaleidoscope flow band, a
// moon-phase ring, a rope-bordered posy, a fully graded meteor run, and
// wave-slid strapping.
//
//   cargo run --release --example bakeoff [out_dir]
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, Verdict};
use ringdesign_core::curve::{CurveLayer, WireProfile};
use ringdesign_core::field::{
    Blend, BorderLayer, BorderProfile, Layer, LayerEntry, OpenworkLayer, Remap, SeatPadLayer,
    SeatRunLayer, SeatStyle, SideFacePick, VGate, Window, SIDE_FACE_MIN_DRAFT_DEG,
};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::{ShankKind, TOP_DEG};
use ringdesign_core::render::{self, Part};
use ringdesign_core::svg::SvgAlpha;
use ringdesign_core::text::{TextAlpha, TextFont};
use ringdesign_core::tiling::{TilingLayer, WarpField};
use ringdesign_core::{gems, library, stones, ProfileStyle, RingDesign};

const YELLOW: [f32; 3] = [0.86, 0.70, 0.42];
const ROSE: [f32; 3] = [0.84, 0.60, 0.49];
const SILVER: [f32; 3] = [0.79, 0.80, 0.81];
const WHITE: [f32; 3] = [0.83, 0.83, 0.80];
const PLATINUM: [f32; 3] = [0.75, 0.76, 0.78];

fn squared(width: f64, thickness: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d
}

fn wider_is_low(d: &RingDesign) -> bool {
    let ctx = d.field_context();
    ctx.side_faces_std()
        .and_then(|f| f.wider())
        .map(|(lo, hi)| 0.5 * (lo + hi) < ctx.crest_v_mm)
        .unwrap_or(false)
}

/// Gradient scale mail (the serpentarium alpha): shaded circles painted in
/// row order so each scale is its own dome.
fn scale_mail_svg() -> String {
    let mut circles = String::new();
    let mut y: f64 = 150.0;
    while y >= -50.0 {
        let k = (y / 25.0).round() as i64;
        let off = if k.rem_euclid(2) == 1 { 25.0 } else { 0.0 };
        for cx0 in [-100.0, 0.0, 100.0] {
            for i in 0..2 {
                let cx = cx0 + off + i as f64 * 50.0;
                circles
                    .push_str(&format!(r#"<circle cx="{cx}" cy="{y}" r="29" fill="url(#s)"/>"#));
            }
        }
        y -= 25.0;
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><radialGradient id="s" cx="50%" cy="58%" r="62%"><stop offset="0%" stop-color="#0a0a0a"/><stop offset="55%" stop-color="#4e4e4e"/><stop offset="100%" stop-color="#cfcfcf"/></radialGradient></defs>{circles}</svg>"##
    )
}

/// The gradient pyramid tile (no hairline phases), from the serpentarium.
fn keel_svg() -> String {
    let cells = concat!(
        r##"<rect x="0" y="0" width="100" height="50" fill="url(#n)"/>"##,
        r##"<rect x="0" y="50" width="100" height="50" fill="url(#s)"/>"##,
        r##"<path d="M0 0 L0 100 L50 50 Z" fill="url(#w)"/>"##,
        r##"<path d="M100 0 L100 100 L50 50 Z" fill="url(#e)"/>"##,
    )
    .to_string();
    let grad = |id: &str, x2: f64, y2: f64, rev: bool| {
        let stops = if rev {
            r##"<stop offset="0%" stop-color="#0a0a0a"/><stop offset="55%" stop-color="#666666"/><stop offset="100%" stop-color="#d4d4d4"/>"##
        } else {
            r##"<stop offset="0%" stop-color="#d4d4d4"/><stop offset="45%" stop-color="#666666"/><stop offset="100%" stop-color="#0a0a0a"/>"##
        };
        format!(
            r#"<linearGradient id="{id}" x1="0" y1="0" x2="{x2}" y2="{y2}">{stops}</linearGradient>"#
        )
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs>{}{}{}{}</defs>{cells}</svg>"##,
        grad("n", 0.0, 1.0, false),
        grad("s", 0.0, 1.0, true),
        grad("w", 1.0, 0.0, false),
        grad("e", 1.0, 0.0, true),
    )
}

/// A fat blunt crescent for the moon-phase openwork.
fn crescent_svg() -> String {
    let mut d = String::from("M");
    for i in 0..=40 {
        let a = std::f64::consts::PI * (1.25 + 0.5 * i as f64 / 40.0);
        d.push_str(&format!("{:.1} {:.1} L", 50.0 + 40.0 * a.cos(), 38.0 - 40.0 * a.sin()));
    }
    for i in 0..=40 {
        let a = std::f64::consts::PI * (1.75 - 0.5 * i as f64 / 40.0);
        d.push_str(&format!("{:.1} {:.1} ", 50.0 + 40.0 * a.cos(), 38.0 - 17.0 * a.sin()));
        if i < 40 {
            d.push('L');
        }
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><path d="{d}Z" fill="#000"/></svg>"##
    )
}

/// Field-check, render, export the referee OBJ, save. Refuses uncastable.
fn finish(
    dir: &str,
    slug: &str,
    blurb: &str,
    tint: [f32; 3],
    hero_pitch: f64,
    face: Option<bool>,
    mut d: RingDesign,
    lib: &mut AlphaLibrary,
) {
    d.name = slug
        .split_once('-')
        .map(|(_, r)| r.replace('-', " "))
        .unwrap_or_else(|| slug.into());
    d.bake_all(lib);
    let field = castability::attributed_field_report(&d, lib, &d.draft, 256, 128);
    println!("{slug:<22} {blurb}");
    println!(
        "{:<22} {} under {}: {:.3}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
        "",
        field.verdict.label(),
        d.draft.process.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm
    );
    for n in &field.notes {
        println!("{:<22}   note: {n}", "");
    }
    for f in ringdesign_core::dfm::findings_in(&d, lib) {
        println!("{:<22}   dfm: {}: {}", "", f.label, f.message);
    }
    let report = stones::report(&d, field.parting_z_mm);
    if let Some(s) = &report {
        println!("{:<22}   stones: {} stones, {:.2} ct", "", s.stone_count, s.total_carats);
        let mut seen = std::collections::BTreeSet::new();
        for seat in &s.seats {
            for w in &seat.warnings {
                if seen.insert(w.clone()) {
                    println!("{:<22}   stone warning: {w}", "");
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
    println!("{:<22}   volume {:.1} mm3", "", out.report.volume_mm3);
    for m in out.report.metals.iter().take(1) {
        println!("{:<22}   {}: {:.2} g", "", m.metal, m.grams);
    }
    let stones_mesh = gems::preview_mesh(&d, lib);
    let mut parts = vec![Part::metal(&out.mesh, tint)];
    if let Some(g) = &stones_mesh {
        parts.push(Part::stone(g));
    }
    let pi = std::f64::consts::PI;
    render::write_png_parts(format!("{dir}/{slug}-hero.png"), &parts, 0.55, hero_pitch, 900)
        .unwrap();
    if let Some(flip) = face {
        let (yaw, pitch) = if flip { (pi, pi - 0.35) } else { (0.0, 0.35) };
        render::write_png_parts(format!("{dir}/{slug}-face.png"), &parts, yaw, pitch, 900).unwrap();
    }
    // The shared-referee mesh, at the same resolution the CG meshes carry.
    let probe = mesh::build(
        &d,
        lib,
        BuildParams { theta_steps: 640, profile_steps: 160, ..Default::default() },
    );
    ringdesign_core::stl::write_obj(format!("{dir}/{slug}.obj"), &probe.mesh, &d.name).unwrap();
    let designs = library::default_design_dir().join("bakeoff");
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design(designs.join(format!("{slug}.ring.json")), &d).unwrap();
    println!();
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp/bakeoff".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut lib = AlphaLibrary::builtin();

    // --- 01. Pangolin: terraced plate mail. --------------------------------
    // The gradient mail put through a terrace remap: each scale becomes a
    // stepped plate, armour instead of skin. A knife keel rides the crest.
    let mut d = squared(7.5, 4.2);
    d.svgs.push(SvgAlpha { name: "Scale mail".into(), svg: scale_mail_svg(), invert: false });
    d.bake_all(&mut lib);
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for("Scale mail", &ctx);
    t.height_mm = 0.5;
    t.bias = 0.22;
    assert!(t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    let mut e = LayerEntry::new("Plate mail", Layer::Tiling(t));
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.25;
    e.remap = Remap::Terrace { steps: 3, span_mm: 0.5, riser: 0.4 };
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    let mut keel = LayerEntry::new(
        "Keel",
        Layer::Border(BorderLayer {
            v_mm: ctx.crest_v_mm,
            width_mm: 0.9,
            height_mm: 0.3,
            profile: BorderProfile::Knife,
            mirror: false,
            rope_twists: 0,
        }),
    );
    keel.blend = Blend::SmoothMax;
    keel.soft_mm = 0.3;
    d.layers.layers.push(keel);
    let low = wider_is_low(&d);
    finish(&dir, "01-pangolin", "terraced plate mail on both faces, knife keel on the crest", SILVER, 1.12, Some(low), d, &mut lib);

    // --- 02. Gyre: the kaleidoscope flow band. -----------------------------
    // The pyramid tile under an eight-fold kaleidoscope and a two-period
    // warp: facets fold and flow like water around the ring.
    let mut d = squared(6.5, 3.4);
    d.svgs.push(SvgAlpha { name: "Keels".into(), svg: keel_svg(), invert: false });
    d.bake_all(&mut lib);
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for("Keels", &ctx);
    t.height_mm = 0.32;
    assert!(t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    t.repeats_around = ((t.repeats_around as f64 / 1.5).round() as u32).max(1);
    t.kfold = 8;
    let face_c = t.v_center_mm;
    t.warp = Some(WarpField {
        points: (0..12)
            .map(|i| {
                let f = i as f64 / 12.0;
                [f, face_c + 0.6 * (2.0 * std::f64::consts::TAU * f).sin()]
            })
            .collect(),
        strength: 0.8,
        falloff_mm: 2.6,
    });
    let mut e = LayerEntry::new("Gyre", Layer::Tiling(t));
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.2;
    e.remap = Remap::cushion(0.32);
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    let low = wider_is_low(&d);
    finish(&dir, "02-gyre", "pyramid facets folded eightfold and warped along a wave", ROSE, 1.12, Some(low), d, &mut lib);

    // --- 03. Lunar: the moon-phase ring. -----------------------------------
    // Full moon flanked by half moons across the crown, crescents pierced
    // through the side faces over the dark half of the ring.
    let mut d = squared(6.8, 3.2);
    d.svgs.push(SvgAlpha { name: "Crescent".into(), svg: crescent_svg(), invert: false });
    d.bake_all(&mut lib);
    let ctx = d.field_context();
    // Rounds, not half-moon cuts: the half-moon preview stands like a
    // blade at any depth, and the phase story reads in the grading and the
    // pierced crescents.
    let full = Gem::calibrated(GemCut::Round, 3.5);
    let half = Gem::calibrated(GemCut::Round, 2.5);
    for (label, theta, gem, rot) in [
        ("Waxing", TOP_DEG + 25.0, half, 0.0),
        ("Full moon", TOP_DEG, full, 0.0),
        ("Waning", TOP_DEG - 25.0, half, 0.0),
    ] {
        let mut seat = SeatPadLayer {
            theta_deg: theta,
            v_mm: ctx.crest_v_mm,
            style: SeatStyle::GypsyMound,
            height_mm: 0.55,
            crown: 1.0,
            blend_mm: 0.5,
            rot_deg: rot,
            ..Default::default()
        };
        seat.fit_stone(gem);
        seat.rot_deg = rot;
        d.layers.layers.push(LayerEntry::new(label, Layer::SeatPad(seat)));
    }
    let mut mask = TilingLayer::default_for("Crescent", &ctx);
    mask.height_mm = 1.0;
    mask.edge_mm = 0.3;
    assert!(mask.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    let mut e = LayerEntry::new(
        "Dark side",
        Layer::Openwork(OpenworkLayer { tiling: mask, depth_mm: 1.0, keep_mm: 0.8 }),
    );
    e.blend = Blend::Add;
    e.window = Window::around(TOP_DEG + 180.0, 150.0);
    e.window.fade_deg = 18.0;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    finish(&dir, "03-lunar", "full and half moons across the crown, crescents pierced over the dark half", PLATINUM, 1.12, None, d, &mut lib);

    // --- 04. Vow: the rope-bound posy. -------------------------------------
    // A script motto raised on both side faces under a rope-twist crest
    // line — the posy band with the border the sand allows.
    let mut d = squared(7.2, 2.8);
    // Serif, not script: Great Vibes hairlines measured 0.04 mm at any
    // stamp a band holds. Garamond capitals carry castable strokes.
    d.texts.push(TextAlpha {
        name: "Vow".into(),
        text: "SUB UMBRA VIRENS".into(),
        font: TextFont::Serif,
        tracking: 0.14,
    });
    d.bake_all(&mut lib);
    let ctx = d.field_context();
    let (lo, hi) = ctx.side_faces_std().and_then(|f| f.wider()).unwrap();
    let low_face = wider_is_low(&d);
    let a = lib.get("Vow").unwrap();
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
    for (name, v, flip) in [
        ("Vow low", 0.5 * (lo + hi), !low_face),
        ("Vow high", ctx.band_v_len_mm - 0.5 * (lo + hi), low_face),
    ] {
        let mut decals = ringdesign_core::field::DecalLayer::default();
        decals.alpha = "Vow".into();
        decals.feather_mm = 0.2;
        decals.decals = vec![ringdesign_core::field::Decal {
            theta_deg: TOP_DEG,
            v_mm: v,
            size_mm: size,
            rotation_deg: 0.0,
            height_mm: 0.3,
            flip,
        }];
        let mut e = LayerEntry::new(name, Layer::Decals(decals));
        e.window.v_gate = VGate::SideFaces(if v < ctx.crest_v_mm {
            SideFacePick::Low
        } else {
            SideFacePick::High
        });
        d.layers.layers.push(e);
    }
    // A rope profile's bead spirals off the crest line and its off-crest
    // arcs measured 0.9% at -16 deg — the milgrain line is the crest's own
    // vocabulary.
    let beads = (ctx.circumference_mm / 0.5).round() as u32;
    d.layers.layers.push(LayerEntry::new(
        "Milgrain",
        Layer::Milgrain(ringdesign_core::field::MilgrainLayer {
            v_mm: ctx.crest_v_mm,
            bead_diameter_mm: 0.5,
            beads_around: beads,
            height_mm: 0.2,
            mirror: false,
        }),
    ));
    finish(&dir, "04-vow", "script motto on both faces under a rope-twist crest", YELLOW, 1.12, Some(low_face), d, &mut lib);

    // --- 05. Meteor: the fully graded run. ---------------------------------
    // Taper 0.8 around the whole circle — the eccentric-anomaly station law
    // made visible, stones bunching toward the tail — with leaning reeds
    // burning up behind the smallest stations.
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.8;
    d.profile.thickness_mm = 2.6;
    let ctx = d.field_context();
    let mut run = SeatRunLayer {
        gem: Gem::calibrated(GemCut::Round, 2.75),
        bridge_mm: 0.75,
        taper: 0.8,
        taper_theta_deg: TOP_DEG,
        ..Default::default()
    };
    run.seat.style = SeatStyle::GypsyMound;
    run.seat.height_mm = 0.55;
    run.seat.crown = 1.0;
    run.seat.blend_mm = 0.45;
    run.seat.v_mm = ctx.crest_v_mm;
    run.solve_spacing(&ctx);
    let mut e = LayerEntry::new("Meteor run", Layer::SeatRun(run));
    e.window = Window::around(TOP_DEG, 250.0);
    d.layers.layers.push(e);
    let count = (ctx.circumference_mm / 0.9).round() as u32;
    let pitch = ctx.circumference_mm / count as f64;
    let mut e = LayerEntry::new(
        "Burn-up",
        Layer::Flutes(ringdesign_core::field::FlutesLayer {
            count,
            profile: ringdesign_core::field::FluteProfile::Round,
            width_mm: pitch - 0.37,
            height_mm: 0.15,
            lean: 0.4,
            along: false,
        }),
    );
    e.window = Window::around(TOP_DEG + 180.0, 80.0);
    e.window.fade_deg = 14.0;
    d.layers.layers.push(e);
    finish(&dir, "05-meteor", "a run graded to a fifth of itself round the circle, reeds burning up behind", WHITE, 1.12, None, d, &mut lib);

    // --- 06. Dune: wave-slid strapping. ------------------------------------
    // The Wave shank slides the band's edges along the finger while flat
    // collars cross it — the bolt-ring family riding a moving band.
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Beveled);
    d.profile.width_mm = 5.6;
    d.profile.thickness_mm = 2.6;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Wave;
    // 0.4: the slid flat sides' corner fillets lean where the slide is
    // steepest — 0.25% at -14 deg at amount 0.7, 0.22% at 0.55; the wave
    // still reads at 0.4.
    d.shank.amount = 0.4;
    d.shank.waves = 3;
    let ctx = d.field_context();
    let collars = CurveLayer {
        points: vec![[0.5, 0.0], [0.5, ctx.band_v_len_mm]],
        repeats_around: 14,
        closed: false,
        width_mm: 2.0,
        height_mm: 0.3,
        profile: WireProfile::Flat,
        taper: 0.0,
        mirror_v: false,
    };
    d.layers.layers.push(LayerEntry::new("Straps", Layer::Curve(collars)));
    finish(&dir, "06-dune", "three waves sliding the band under flat straps", ROSE, 1.05, None, d, &mut lib);

    println!(
        "renders in {dir}, designs in {}",
        library::default_design_dir().join("bakeoff").display()
    );
}
