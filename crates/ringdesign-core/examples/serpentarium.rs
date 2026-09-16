// Serpentarium: twelve snakes, each a different read of the same animal —
// a bypass with gem eyes on its passing heads, a cigar band in warped
// scale mail, a viper-head signet on a drawn plan, a keyframed ouroboros
// swallowing its own tail, a graded diamondback, a twin-head toi et moi,
// a keyframed cobra hood under an oval, a rib-banded krait, a graded
// rattle of keyframed segments, pierced crescent scales, three serpents
// interwoven in wire, and a sidewinder's oblique reeds under a graded
// row. Every ring is held to the field verdict, rendered with its
// stones, given its sheet and setter's map, and saved so it opens in
// the app. Only builtin generators and SVG carried in the design file,
// so every piece is portable to an empty machine.
//
//   cargo run --release --example serpentarium [out_dir]
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, Verdict};
use ringdesign_core::curve::{CurveLayer, WireProfile};
use ringdesign_core::field::{
    Blend, BorderLayer, BorderProfile, Decal, DecalLayer, FieldContext, FluteProfile, FlutesLayer,
    Layer, LayerEntry, MilgrainLayer, OpenworkLayer, Remap, SeatPadLayer, SeatRunLayer, SeatStyle,
    SideFacePick, VGate, Window, SIDE_FACE_MIN_DRAFT_DEG,
};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::{ShankKey, ShankKind, TOP_DEG};
use ringdesign_core::render::{self, Part};
use ringdesign_core::svg::SvgAlpha;
use ringdesign_core::tiling::{TilingLayer, WarpField};
use ringdesign_core::{gems, library, stonemap, stones, CustomOutline, ProfileStyle, RingDesign};

const YELLOW: [f32; 3] = [0.86, 0.70, 0.42];
const ROSE: [f32; 3] = [0.84, 0.60, 0.49];
const SILVER: [f32; 3] = [0.79, 0.80, 0.81];
const WHITE: [f32; 3] = [0.83, 0.83, 0.80];
const PLATINUM: [f32; 3] = [0.75, 0.76, 0.78];

struct Shots {
    hero_pitch: f64,
    /// Face-on view of the side-face annulus; true flips to the -Z face.
    face: Option<bool>,
    top: bool,
    gif: bool,
}

const HERO: Shots = Shots { hero_pitch: 1.12, face: None, top: false, gif: false };

fn squared(width: f64, thickness: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d
}

/// Whether the wider side face is the low (-Z) one. The tie on a symmetric
/// band breaks on float noise, so it is read per design, never assumed.
fn wider_is_low(d: &RingDesign) -> bool {
    let ctx = d.field_context();
    ctx.side_faces_std()
        .and_then(|f| f.wider())
        .map(|(lo, hi)| 0.5 * (lo + hi) < ctx.crest_v_mm)
        .unwrap_or(false)
}

/// Chart `v` of the taller hump in one half of a modulated section — where a
/// bypass arm's own crest runs at this angle.
fn hump_v(d: &RingDesign, ctx: &FieldContext, theta: f64, low_half: bool) -> f64 {
    let inner = d.inner_radius_mm();
    let crest_r = inner + d.profile.thickness_mm;
    let m = d.modulation_at(theta, inner, crest_r);
    let l = d.profile.sample_mod(inner, 256, &m);
    let mid = 0.5 * l.surface_len_mm;
    let mut best = (f64::MIN, mid);
    for p in l.pts.iter().filter(|p| p.surface && ((p.v_mm < mid) == low_half)) {
        if p.r > best.0 {
            best = (p.r, p.v_mm);
        }
    }
    best.1 / l.surface_len_mm.max(1e-9) * ctx.band_v_len_mm
}

/// Chart `v` of the most side-facing point on the belly of a modulated
/// section's wall — a cheek on a signet head's flank — and the section's
/// stretch `k` (local arc over the reference's). Restricted to the middle
/// radii so neither the bore corner nor the rim fillet wins.
fn cheek(d: &RingDesign, ctx: &FieldContext, theta: f64, low_half: bool) -> (f64, f64) {
    let inner = d.inner_radius_mm();
    let crest_r = inner + d.profile.thickness_mm;
    let m = d.modulation_at(theta, inner, crest_r);
    let l = d.profile.sample_mod(inner, 384, &m);
    let mid = 0.5 * l.surface_len_mm;
    let (r0, r1) = (
        inner + 0.35 * (l.crest_radius_mm - inner),
        inner + 0.80 * (l.crest_radius_mm - inner),
    );
    let mut best = (f64::MIN, 0.25 * mid);
    for p in l.pts.iter().filter(|p| p.surface && ((p.v_mm < mid) == low_half)) {
        if p.r > r0 && p.r < r1 && p.nz.abs() > best.0 {
            best = (p.nz.abs(), p.v_mm);
        }
    }
    let k = l.surface_len_mm / ctx.band_v_len_mm.max(1e-9);
    (best.1 / l.surface_len_mm.max(1e-9) * ctx.band_v_len_mm, k)
}

/// Section stretch `k` at a ring angle: local surface arc over the
/// reference's. The chart's `v` is arc normalized, so a stamp stands `k`
/// times taller than it is wide there; art drawn squashed by `k` comes
/// out round.
fn stretch_at(d: &RingDesign, ctx: &FieldContext, theta: f64) -> f64 {
    let inner = d.inner_radius_mm();
    let crest_r = inner + d.profile.thickness_mm;
    let m = d.modulation_at(theta, inner, crest_r);
    let l = d.profile.sample_mod(inner, 192, &m);
    l.surface_len_mm / ctx.band_v_len_mm.max(1e-9)
}

/// A gem-dome eye as a stamp: a radial gradient fading to nothing at the
/// rim, pre-squashed by the section stretch `k` so it casts round.
fn eye_svg(k: f64) -> String {
    let ry = 46.0 / k.max(0.2);
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><radialGradient id="e"><stop offset="0%" stop-color="#000"/><stop offset="65%" stop-color="#3a3a3a"/><stop offset="100%" stop-color="#fff" stop-opacity="0"/></radialGradient></defs><ellipse cx="50" cy="50" rx="46" ry="{ry:.1}" fill="url(#e)"/></svg>"##
    )
}

/// Seamless faceted keels: each 50-unit cell is a pyramid built from two
/// gradient half-cell rects and two gradient triangles painted over them.
/// All four gradients run 0..1 over 25 units, so their values coincide
/// exactly along the diagonals — the antialiased seams of a four-triangle
/// build read as 0.08 mm phantom strokes in the granulometry.
fn keel_svg() -> String {
    // One pyramid per tile: a 2x2 grid halves every feature, and on a
    // 1.5 mm face the phases pinch under the detail floor.
    let cells = concat!(
        r##"<rect x="0" y="0" width="100" height="50" fill="url(#n)"/>"##,
        r##"<rect x="0" y="50" width="100" height="50" fill="url(#s)"/>"##,
        r##"<path d="M0 0 L0 100 L50 50 Z" fill="url(#w)"/>"##,
        r##"<path d="M100 0 L100 100 L50 50 Z" fill="url(#e)"/>"##,
    )
    .to_string();
    // The mid stop pulls the half-height contour out to ~0.4 of the face,
    // so the dark diamond clears the detail floor on a 1.5 mm face.
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

/// One fat crescent — a scale's shadow — for the pierced-scale openwork.
/// Sampled as a polyline so the arcs cannot fight SVG sweep flags. The
/// horns are clipped at a quarter period: run to their points they thin to
/// 0.30 mm at band cells, under the sand's floor.
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

/// Seamless imbricated scale mail, shaded so each scale is its own dome:
/// rows of circles at half-period offsets, painted top row last, each filled
/// with a radial gradient darkest low in the scale. The SVG rasterizer reads
/// darkness as height, so the crescents come out as overlapping shingles.
/// Two scales per tile edge; rows are drawn from a period beyond each tile
/// edge so the painting order wraps in both axes.
fn scale_mail_svg() -> String {
    let mut circles = String::new();
    let mut y: f64 = 150.0;
    while y >= -50.0 {
        let k = (y / 25.0).round() as i64;
        let off = if k.rem_euclid(2) == 1 { 25.0 } else { 0.0 };
        for cx0 in [-100.0, 0.0, 100.0] {
            for i in 0..2 {
                let cx = cx0 + off + i as f64 * 50.0;
                circles.push_str(&format!(
                    r#"<circle cx="{cx}" cy="{y}" r="29" fill="url(#s)"/>"#
                ));
            }
        }
        y -= 25.0;
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><radialGradient id="s" cx="50%" cy="58%" r="62%"><stop offset="0%" stop-color="#0a0a0a"/><stop offset="55%" stop-color="#4e4e4e"/><stop offset="100%" stop-color="#cfcfcf"/></radialGradient></defs>{circles}</svg>"##
    )
}

/// A viper-head plan: rounded skull base, jaw lobes, tapering snout. `dir`
/// flips the snout along the ring; `aspect` is length over width.
fn viper_outline(name: &str, dir: f64, aspect: f64) -> CustomOutline {
    let smooth = |t: f64| t * t * (3.0 - 2.0 * t);
    let n = 96;
    let half = |x: f64| -> f64 {
        if x < -0.55 {
            let t = ((x + 1.0) / 0.45).clamp(0.0, 1.0);
            0.62 * (1.0 - (1.0 - t) * (1.0 - t)).max(0.0).sqrt()
        } else if x < -0.05 {
            let t = ((x + 0.55) / 0.5).clamp(0.0, 1.0);
            0.62 + 0.38 * smooth(t)
        } else {
            let t = ((x + 0.05) / 1.05).clamp(0.0, 1.0);
            (1.0 - t.powf(1.7)).max(0.0).powf(1.25)
        }
    };
    let mut pts: Vec<[f64; 2]> = Vec::new();
    for i in 0..=n {
        let x = -1.0 + 2.0 * i as f64 / n as f64;
        pts.push([dir * x * aspect, half(x)]);
    }
    for i in (1..n).rev() {
        let x = -1.0 + 2.0 * i as f64 / n as f64;
        pts.push([dir * x * aspect, -half(x)]);
    }
    CustomOutline::from_points(name, &pts).expect("a viper head closes into an outline")
}

/// Field-check, render with stones, sheet, map, save. Refuses to ship an
/// uncastable ring.
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
        .map(|(_, r)| r.replace('-', " "))
        .unwrap_or_else(|| slug.into());
    d.bake_all(lib);
    let field = castability::attributed_field_report(&d, lib, &d.draft, 256, 128);
    println!("{slug:<24} {blurb}");
    println!(
        "{:<24} {} under {}: {:.3}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
        "",
        field.verdict.label(),
        d.draft.process.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm
    );
    for n in &field.notes {
        println!("{:<24}   note: {n}", "");
    }
    let dfm = ringdesign_core::dfm::findings_in(&d, lib);
    for f in &dfm {
        println!("{:<24}   dfm: {}: {}", "", f.label, f.message);
    }
    let report = stones::report(&d, field.parting_z_mm);
    if let Some(s) = &report {
        println!("{:<24}   stones: {} stones, {:.2} ct", "", s.stone_count, s.total_carats);
        if let Some(p) = &s.closest {
            println!(
                "{:<24}   closest pair: {} / {} — {:.2} mm at the girdle, {:.2} mm at the culet",
                "", p.a, p.b, p.gap_mm, p.gap_deep_mm
            );
        }
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
    for m in out.report.metals.iter().take(1) {
        println!("{:<24}   {}: {:.2} g", "", m.metal, m.grams);
    }
    let stones_mesh = gems::preview_mesh(&d, lib);
    let mut parts = vec![Part::metal(&out.mesh, tint)];
    if let Some(g) = &stones_mesh {
        parts.push(Part::stone(g));
    }
    let pi = std::f64::consts::PI;
    render::write_png_parts(format!("{dir}/{slug}-hero.png"), &parts, 0.55, shots.hero_pitch, 900)
        .unwrap();
    if let Some(flip) = shots.face {
        let (yaw, pitch) = if flip { (pi, pi - 0.35) } else { (0.0, 0.35) };
        render::write_png_parts(format!("{dir}/{slug}-face.png"), &parts, yaw, pitch, 900).unwrap();
    }
    if shots.top {
        render::write_png_parts(format!("{dir}/{slug}-top.png"), &parts, 0.0, 1.55, 800).unwrap();
    }
    if shots.gif {
        render::write_turntable_gif(format!("{dir}/{slug}.gif"), &out.mesh, 36, 480, tint).unwrap();
    }
    let sheet = ringdesign_core::spec::html(&d, &out.report, &field, report.as_ref(), &dfm, "serpentarium");
    std::fs::write(format!("{dir}/{slug}-sheet.html"), sheet).unwrap();
    if report.as_ref().is_some_and(|s| s.stone_count > 0) {
        stonemap::write_stone_map_svg(format!("{dir}/{slug}-stones.svg"), &d, report.as_ref())
            .unwrap();
    }
    let designs = library::default_design_dir().join("serpentarium");
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design(designs.join(format!("{slug}.ring.json")), &d).unwrap();
    println!();
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp/serpentarium".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut lib = AlphaLibrary::builtin();

    // --- 01. Uraeus: the two-headed bypass. --------------------------------
    // The Victorian snake ring is a bypass: two arms passing over the top,
    // each ending in a head. A marquise cabochon bridges the crossing (a cab
    // has no pavilion, so the crossing wedge costs it nothing), and each
    // arm carries a small gem eye on its own crest — probed from the
    // modulated section, because the arms slide off the mid-plane and a
    // fixed-v seat would land on a flank.
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 5.0;
    d.profile.thickness_mm = 2.3;
    d.shank.kind = ShankKind::Bypass;
    d.shank.amount = 1.0;
    let ctx = d.field_context();
    // Past the tip at +35 deg only the through arm remains; its hump's half
    // says which side each arm runs, and the eyes go on the tip arms.
    let through_is_low = {
        let inner = d.inner_radius_mm();
        let crest_r = inner + d.profile.thickness_mm;
        let m = d.modulation_at(TOP_DEG + 45.0, inner, crest_r);
        let l = d.profile.sample_mod(inner, 256, &m);
        let mid = 0.5 * l.surface_len_mm;
        let mut best = (f64::MIN, mid);
        for p in l.pts.iter().filter(|p| p.surface) {
            if p.r > best.0 {
                best = (p.r, p.v_mm);
            }
        }
        best.1 < mid
    };
    for (label, theta, low) in [
        ("Eye east", TOP_DEG + 22.0, !through_is_low),
        ("Eye west", TOP_DEG - 22.0, through_is_low),
    ] {
        let v = hump_v(&d, &ctx, theta, low);
        // Domed bosses, not gypsy mounds: gypsy stock is 1.8 mm around the
        // stone and its skirt reached onto the arm's rounded nose at 5 deg;
        // a boss carries 1.2 mm and stays on the hump.
        let mut eye = SeatPadLayer {
            theta_deg: theta,
            v_mm: v,
            style: SeatStyle::Boss,
            height_mm: 0.24,
            crown: 1.0,
            blend_mm: 0.45,
            ..Default::default()
        };
        eye.fit_stone(Gem::calibrated(GemCut::Round, 1.0));
        d.layers.layers.push(LayerEntry::new(label, Layer::SeatPad(eye)));
    }
    let mut head = SeatPadLayer {
        theta_deg: TOP_DEG,
        v_mm: ctx.crest_v_mm,
        style: SeatStyle::GypsyMound,
        height_mm: 0.55,
        crown: 1.0,
        blend_mm: 0.45,
        ..Default::default()
    };
    head.fit_stone(Gem::cabochon(GemCut::Marquise, 3.2));
    d.layers.layers.push(LayerEntry::new("Head cab", Layer::SeatPad(head)));
    // Snakeskin shimmer on the flanks only: a polished crest ribbon between,
    // the hammered-bombe lesson in scale form. The gradient mail, not the
    // builtin Scales — its seven scallops per tile leave 0.01 mm gaps at any
    // cell a 1.5 mm strip holds.
    d.svgs.push(SvgAlpha { name: "Scale mail".into(), svg: scale_mail_svg(), invert: false });
    d.bake_all(&mut lib);
    for (label, side) in [("Skin low", -1.0), ("Skin high", 1.0)] {
        let mut t = TilingLayer::default_for("Scale mail", &ctx);
        t.height_mm = 0.05;
        t.rows = 1;
        t.v_center_mm = ctx.crest_v_mm + side * 1.6;
        t.v_span_mm = 1.5;
        t.feather_mm = 0.5;
        // Floor under the gradient so the granulometry finds no hairline gaps.
        t.bias = 0.3;
        t.repeats_around = t.repeats_for_square_cells(&ctx);
        let mut e = LayerEntry::new(label, Layer::Tiling(t));
        // The arms slide over the 100 deg before the top, and a fixed-v strip
        // rides their moving flanks there — measured 2-4 deg of lean. Behind
        // the ring nothing slides, so the skin lives there and the heads
        // stay polished.
        e.window = Window::except(TOP_DEG, 120.0);
        e.window.fade_deg = 25.0;
        d.layers.layers.push(e);
    }
    finish(
        &dir,
        "01-uraeus-bypass",
        "two arms passing under a marquise cab, a gem eye on each head",
        YELLOW,
        Shots { gif: true, ..HERO },
        d,
        &mut lib,
    );

    // --- 02. Scute: warped scale mail on a cigar band. ---------------------
    // The scales are an SVG carried in the design: gradient-shaded circles
    // painted in row order, so each scale is its own dome and the shingle
    // step between rows is a wall across the band — parallel to the pull on
    // a side face, which is the one place that step is free. The warp bends
    // the rows along a three-period wave and the shear leans the columns,
    // so the mail flows like a moving body instead of running level.
    let mut d = squared(8.0, 4.5);
    d.svgs.push(SvgAlpha { name: "Scale mail".into(), svg: scale_mail_svg(), invert: false });
    d.bake_all(&mut lib);
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for("Scale mail", &ctx);
    t.height_mm = 0.5;
    t.bias = 0.22;
    assert!(t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    let face_c = t.v_center_mm;
    t.warp = Some(WarpField {
        points: (0..12)
            .map(|i| {
                let f = i as f64 / 12.0;
                [f, face_c + 0.8 * (3.0 * std::f64::consts::TAU * f).sin()]
            })
            .collect(),
        strength: 0.85,
        falloff_mm: 3.0,
    });
    t.shear = 0.35;
    let mut e = LayerEntry::new("Scale mail", Layer::Tiling(t));
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.25;
    // The warp moves the sampling `v`, so tile relief can reach past the
    // face's edge onto the shoulder — measured 0.89% at -45 deg without the
    // gate. The gate reads the true `v` and kills it there.
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    let beads = (ctx.circumference_mm / 0.5).round() as u32;
    d.layers.layers.push(LayerEntry::new(
        "Milgrain",
        Layer::Milgrain(MilgrainLayer {
            v_mm: ctx.crest_v_mm,
            bead_diameter_mm: 0.5,
            beads_around: beads,
            height_mm: 0.2,
            mirror: false,
        }),
    ));
    let low = wider_is_low(&d);
    finish(
        &dir,
        "02-scute-mail",
        "gradient-domed scale mail warped along a wave, bead line on the crest",
        SILVER,
        Shots { face: Some(low), gif: true, ..HERO },
        d,
        &mut lib,
    );

    // --- 03. Basilisk: a viper-head signet on a drawn plan. ----------------
    // The head is the band: a custom outline — skull, jaw lobes, snout —
    // through the same lofted construction as every factory preset, with a
    // low dome on the skull, the belly hollowed from the finger hole, and
    // an eye dot on each cheek. The cheeks are the head's side-facing wall
    // runs, probed from the modulated section; relief there pulls straight
    // out. Ventral scutes — flutes across the band — ride the back arc.
    // 2.0 thick and a 0.55 hollow: at 1.9 / 0.7 the scoop left a 0.63 mm
    // wall over the shoulder, under the 0.7 mm fill floor.
    let mut d = squared(11.0, 2.0);
    d.shank.apply_signet(11.0);
    let outline = viper_outline("Viper", 1.0, 1.25);
    let o = d.shank.adopt_outline(outline);
    d.shank.head.outline = o;
    d.shank.head.dome = d.shank.suggest_dome(o);
    d.shank.head.length_mm = 13.0;
    d.shank.head.rise_mm = 0.6;
    d.shank.head.rim_round_mm = 0.5;
    d.shank.head.table_dome_mm = 0.85;
    d.shank.head.hollow_mm = 0.55;
    // The loft's climb/dive corner at the table's theta-end reads as a hard
    // ridge down the flank; this is the fillet for it.
    d.shank.head.crest_round_mm = 0.8;
    let ctx = d.field_context();
    // Eyes as stamps, not pads: a pad is drawn in chart mm and the head's
    // wall runs at ~1.7x the reference arc, so a 1.5 mm pad cast 2.6 mm
    // tall with its skirt over the bore corner. The stamp is the cloud-curl
    // move — art squashed by the section's own stretch, sized in metal.
    let (v_lo, k_cheek) = cheek(&d, &ctx, TOP_DEG + 8.0, true);
    let (v_hi, _) = cheek(&d, &ctx, TOP_DEG + 8.0, false);
    d.svgs.push(SvgAlpha { name: "Viper eye".into(), svg: eye_svg(k_cheek), invert: false });
    let eyes = DecalLayer {
        alpha: "Viper eye".into(),
        decals: vec![
            Decal { theta_deg: TOP_DEG + 8.0, v_mm: v_lo, size_mm: 2.6, rotation_deg: 0.0, height_mm: 0.3, flip: false },
            Decal { theta_deg: TOP_DEG + 8.0, v_mm: v_hi, size_mm: 2.6, rotation_deg: 0.0, height_mm: 0.3, flip: false },
        ],
        feather_mm: 0.1,
        invert: false,
    };
    d.layers.layers.push(LayerEntry::new("Cheek eyes", Layer::Decals(eyes)));
    let count = (ctx.circumference_mm / 0.86).round() as u32;
    let pitch = ctx.circumference_mm / count as f64;
    let mut e = LayerEntry::new(
        "Ventral scutes",
        Layer::Flutes(FlutesLayer {
            count,
            profile: FluteProfile::Round,
            width_mm: pitch - 0.37,
            height_mm: 0.15,
            lean: 0.0,
            along: false,
        }),
    );
    e.window = Window::except(TOP_DEG, 170.0);
    e.window.fade_deg = 20.0;
    d.layers.layers.push(e);
    finish(
        &dir,
        "03-basilisk-signet",
        "lofted viper-head signet, domed skull, cheek eyes, scutes down the back",
        YELLOW,
        Shots { top: true, gif: true, ..HERO },
        d,
        &mut lib,
    );

    // --- 04. Ouroboros: the band is the snake. -----------------------------
    // Keyframed stations taper the body around the whole circle — broad head
    // at the top, thinning all the way round to a tail that runs back under
    // the head's jaw, the swallow being the one steep ramp between the last
    // tail station and the head. The scale mail follows the band as it
    // tapers because layers evaluate against the reference section, so the
    // scales shrink toward the tail without any per-station bookkeeping.
    let mut d = squared(6.0, 3.4);
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let key = |off: f64, w: f64, t: f64, c: f64| ShankKey {
        theta_deg: TOP_DEG + off,
        width_scale: w,
        thickness_scale: t,
        crown_scale: c,
    };
    d.shank.keys = vec![
        key(14.0, 1.30, 1.16, 0.90),
        key(42.0, 1.18, 1.10, 1.0),
        key(95.0, 1.06, 1.04, 1.0),
        key(160.0, 0.97, 1.00, 1.0),
        key(225.0, 0.88, 0.95, 1.0),
        key(285.0, 0.80, 0.90, 1.0),
        key(330.0, 0.73, 0.86, 1.0),
        key(352.0, 0.70, 0.84, 1.0),
    ];
    d.svgs.push(SvgAlpha { name: "Scale mail".into(), svg: scale_mail_svg(), invert: false });
    d.bake_all(&mut lib);
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for("Scale mail", &ctx);
    t.height_mm = 0.4;
    t.bias = 0.22;
    assert!(t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    t.shear = 0.25;
    let mut e = LayerEntry::new("Scale mail", Layer::Tiling(t));
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.25;
    e.window = Window::except(TOP_DEG + 8.0, 60.0);
    e.window.fade_deg = 14.0;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    // Eye stamps on both side faces at the head, squashed by the widened
    // station's own stretch so they cast round.
    let (lo, hi) = ctx.side_faces_std().and_then(|f| f.wider()).unwrap();
    let vc = 0.5 * (lo + hi);
    let k_head = stretch_at(&d, &ctx, TOP_DEG + 20.0);
    d.svgs.push(SvgAlpha { name: "Ouro eye".into(), svg: eye_svg(k_head), invert: false });
    let eyes = DecalLayer {
        alpha: "Ouro eye".into(),
        decals: vec![
            Decal { theta_deg: TOP_DEG + 20.0, v_mm: vc, size_mm: 1.9, rotation_deg: 0.0, height_mm: 0.26, flip: false },
            Decal { theta_deg: TOP_DEG + 20.0, v_mm: ctx.band_v_len_mm - vc, size_mm: 1.9, rotation_deg: 0.0, height_mm: 0.26, flip: false },
        ],
        feather_mm: 0.1,
        invert: false,
    };
    d.layers.layers.push(LayerEntry::new("Eyes", Layer::Decals(eyes)));
    let beads = (ctx.circumference_mm / 0.45).round() as u32;
    d.layers.layers.push(LayerEntry::new(
        "Spine beads",
        Layer::Milgrain(MilgrainLayer {
            v_mm: ctx.crest_v_mm,
            bead_diameter_mm: 0.45,
            beads_around: beads,
            height_mm: 0.18,
            mirror: false,
        }),
    ));
    finish(
        &dir,
        "04-ouroboros",
        "keyframed body tapering the whole way round to the tail under its own jaw",
        ROSE,
        Shots { gif: true, top: true, ..HERO },
        d,
        &mut lib,
    );

    // --- 05. Diamondback: the graded, turned eternity. ---------------------
    // Princess cuts turned 45 degrees make the diamondback's pattern down
    // the spine, graded toward the tail; the rattle is a short leaning
    // reeding at the bottom of the finger, and the flanks carry the Diamonds
    // generator cushioned so the pyramids read as keeled scales.
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Beveled);
    d.profile.width_mm = 5.2;
    d.profile.thickness_mm = 2.9;
    d.profile.flatten_sides();
    let ctx = d.field_context();
    // 0.75 asked so the graded row reports 0.7+ everywhere: at 0.5 the
    // smallest stations bridged 0.57 mm against the sand's 0.7 mm fill.
    let mut run = SeatRunLayer {
        gem: Gem::calibrated(GemCut::Princess, 2.0),
        bridge_mm: 0.75,
        taper: 0.55,
        taper_theta_deg: TOP_DEG,
        tilt_deg: 45.0,
        ..Default::default()
    };
    run.seat.style = SeatStyle::GypsyMound;
    run.seat.height_mm = 0.55;
    run.seat.crown = 1.0;
    run.seat.blend_mm = 0.45;
    run.seat.v_mm = ctx.crest_v_mm;
    run.solve_spacing(&ctx);
    let mut e = LayerEntry::new("Spine row", Layer::SeatRun(run));
    e.window = Window::around(TOP_DEG, 210.0);
    d.layers.layers.push(e);
    let count = (ctx.circumference_mm / 0.82).round() as u32;
    let pitch = ctx.circumference_mm / count as f64;
    let mut e = LayerEntry::new(
        "Rattle",
        Layer::Flutes(FlutesLayer {
            count,
            profile: FluteProfile::Round,
            width_mm: pitch - 0.37,
            height_mm: 0.16,
            lean: 0.25,
            along: false,
        }),
    );
    e.window = Window::around(TOP_DEG + 180.0, 46.0);
    e.window.fade_deg = 8.0;
    d.layers.layers.push(e);
    // The builtin Diamonds keeps 0.02 mm valleys at any cell this face
    // holds, and a bias floor cannot fill them; the gradient keels have no
    // gap phase at all.
    d.svgs.push(SvgAlpha { name: "Keels".into(), svg: keel_svg(), invert: false });
    d.bake_all(&mut lib);
    let mut t = TilingLayer::default_for("Keels", &ctx);
    t.height_mm = 0.28;
    assert!(t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    // Cells 1.7x wider than tall: the diamonds elongate along the ring.
    t.repeats_around = ((t.repeats_around as f64 / 1.7).round() as u32).max(1);
    let mut e = LayerEntry::new("Keels", Layer::Tiling(t));
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.2;
    e.remap = Remap::cushion(0.28);
    e.window = Window::except(TOP_DEG + 180.0, 60.0);
    e.window.fade_deg = 10.0;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    finish(
        &dir,
        "05-diamondback",
        "graded princesses on the diagonal, keeled flanks, a rattle at the palm",
        PLATINUM,
        Shots { top: true, ..HERO },
        d,
        &mut lib,
    );

    // --- 06. Hydra: twin heads, one band. ----------------------------------
    // A toi et moi of two lofted viper heads facing each other across the
    // top — the second head is `extra_heads`, the band the union of both.
    // Mirrored outlines point the snouts together; both bellies hollow with
    // the primary's setting.
    // 2.1 thick with a 0.25 hollow: at 1.9 / 0.5 the twin scoops pinched a
    // 0.19 mm wall on the west head's outer shoulder.
    let mut d = squared(9.5, 2.1);
    d.shank.apply_signet(9.5);
    // The outline's +x runs toward -theta on the table, so the head below
    // the top takes the west-pointing plan and the head past it the east
    // one — snouts meeting at the top (verified in the top render; the
    // other assignment points them apart).
    let east = d.shank.adopt_outline(viper_outline("Viper east", 1.0, 1.15));
    let west = d.shank.adopt_outline(viper_outline("Viper west", -1.0, 1.15));
    // Prism heads, not lofted: two small lofted heads on a 2.1 mm band run
    // their body row under the bore and the curled flank pinched a 0.42 mm
    // wall at 17 deg of lean on both outer shoulders. The prism is the
    // measured-0.000% family and keeps the viper plan.
    d.shank.head.outline = west;
    d.shank.head.theta_deg = TOP_DEG - 27.0;
    d.shank.head.length_mm = 8.0;
    d.shank.head.rise_mm = 0.6;
    d.shank.head.rim_round_mm = 0.45;
    d.shank.head.table_dome_mm = 0.7;
    d.shank.head.hollow_mm = 0.25;
    d.shank.head.loft = 0.0;
    let mut second = ringdesign_core::profile::SignetHead::lofted();
    second.outline = east;
    second.theta_deg = TOP_DEG + 27.0;
    second.length_mm = 8.0;
    second.rise_mm = 0.6;
    second.rim_round_mm = 0.45;
    second.table_dome_mm = 0.7;
    second.loft = 0.0;
    d.shank.extra_heads.push(second);
    let ctx = d.field_context();
    let count = (ctx.circumference_mm / 0.9).round() as u32;
    let pitch = ctx.circumference_mm / count as f64;
    let mut e = LayerEntry::new(
        "Ventral scutes",
        Layer::Flutes(FlutesLayer {
            count,
            profile: FluteProfile::Round,
            width_mm: pitch - 0.37,
            height_mm: 0.14,
            lean: 0.0,
            along: false,
        }),
    );
    e.window = Window::around(TOP_DEG + 180.0, 150.0);
    e.window.fade_deg = 18.0;
    d.layers.layers.push(e);
    finish(
        &dir,
        "06-hydra-twins",
        "toi et moi: two lofted viper heads nose to nose, hollowed bellies",
        WHITE,
        Shots { top: true, gif: true, ..HERO },
        d,
        &mut lib,
    );

    // --- 07. Cobra: the hood is the cathedral. -----------------------------
    // Keyframed stations flare a broad, flattened hood around the top — the
    // widened section carries the stone the plain band could not, the way
    // the princess solitaire's keyframes do — with an oval in a gypsy mound
    // on the hood's crest, scale mail on the hood's own side faces (they
    // grow with the hood, so the scales grow too), and a knife keel down
    // the spine behind it.
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.4;
    d.profile.thickness_mm = 2.3;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let key = |off: f64, w: f64, t: f64, c: f64| ShankKey {
        theta_deg: TOP_DEG + off,
        width_scale: w,
        thickness_scale: t,
        crown_scale: c,
    };
    d.shank.keys = vec![
        key(0.0, 1.75, 1.22, 0.60),
        key(26.0, 1.55, 1.15, 0.70),
        key(-26.0, 1.55, 1.15, 0.70),
        key(48.0, 1.18, 1.05, 0.90),
        key(-48.0, 1.18, 1.05, 0.90),
        key(75.0, 0.98, 1.00, 1.0),
        key(-75.0, 0.98, 1.00, 1.0),
        key(180.0, 0.92, 0.96, 1.0),
    ];
    d.svgs.push(SvgAlpha { name: "Scale mail".into(), svg: scale_mail_svg(), invert: false });
    d.bake_all(&mut lib);
    let ctx = d.field_context();
    let mut seat = SeatPadLayer {
        theta_deg: TOP_DEG,
        v_mm: ctx.crest_v_mm,
        style: SeatStyle::GypsyMound,
        height_mm: 0.65,
        crown: 1.0,
        blend_mm: 0.5,
        ..Default::default()
    };
    seat.fit_stone(Gem::calibrated(GemCut::Oval, 3.0));
    d.layers.layers.push(LayerEntry::new("Spectacle", Layer::SeatPad(seat)));
    let mut t = TilingLayer::default_for("Scale mail", &ctx);
    t.height_mm = 0.3;
    t.bias = 0.3;
    assert!(t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    let mut e = LayerEntry::new("Hood mail", Layer::Tiling(t));
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.25;
    e.window = Window::around(TOP_DEG, 120.0);
    e.window.fade_deg = 22.0;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    let mut keel = LayerEntry::new(
        "Keel",
        Layer::Border(BorderLayer {
            v_mm: ctx.crest_v_mm,
            width_mm: 0.8,
            height_mm: 0.28,
            profile: BorderProfile::Knife,
            mirror: false,
            rope_twists: 0,
        }),
    );
    keel.blend = Blend::SmoothMax;
    keel.soft_mm = 0.3;
    keel.window = Window::except(TOP_DEG, 130.0);
    keel.window.fade_deg = 25.0;
    d.layers.layers.push(keel);
    finish(
        &dir,
        "07-cobra-hood",
        "a keyframed hood flared over the top, oval on its crest, keel down the back",
        YELLOW,
        Shots { gif: true, top: true, ..HERO },
        d,
        &mut lib,
    );

    // --- 08. Krait: the banded body. ---------------------------------------
    // Broad flat collars across the whole band — the bolt-ring construction,
    // measured 0.000% there — under a round keel riding the crest through
    // them, on a gently tapered body. Round flutes were 0.7% at -5.6 deg:
    // their cosine flanks wrap the edge fillet's shallow draft at every rib
    // end, where a flat strap's ends run out square.
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Beveled);
    d.profile.width_mm = 6.5;
    d.profile.thickness_mm = 3.4;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Tapered;
    d.shank.amount = 0.35;
    let ctx = d.field_context();
    let ribs = CurveLayer {
        points: vec![[0.5, 0.0], [0.5, ctx.band_v_len_mm]],
        repeats_around: 26,
        closed: false,
        width_mm: 2.2,
        height_mm: 0.32,
        profile: WireProfile::Flat,
        taper: 0.0,
        mirror_v: false,
    };
    d.layers.layers.push(LayerEntry::new("Ribs", Layer::Curve(ribs)));
    // Taller than the collars, so the spine reads through the gaps instead
    // of drowning at their height.
    let mut keel = LayerEntry::new(
        "Keel",
        Layer::Border(BorderLayer {
            v_mm: ctx.crest_v_mm,
            width_mm: 1.2,
            height_mm: 0.5,
            profile: BorderProfile::Round,
            mirror: false,
            rope_twists: 0,
        }),
    );
    keel.blend = Blend::SmoothMax;
    keel.soft_mm = 0.35;
    d.layers.layers.push(keel);
    finish(
        &dir,
        "08-krait-bands",
        "broad ribs across the band under a continuous spine keel",
        ROSE,
        Shots { gif: true, ..HERO },
        d,
        &mut lib,
    );

    // --- 09. Cascabel: the rattle is the ring. -----------------------------
    // Seven keyframed segments graded around the circle, biggest at the top,
    // dips between — every section is still one dome, so the lobes are the
    // castable read of segmentation (the melon lesson's safe axis) — with a
    // round in a gypsy mound on the top segment.
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.6;
    d.profile.thickness_mm = 2.4;
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let lobe = |off: f64, w: f64, t: f64| ShankKey {
        theta_deg: TOP_DEG + off,
        width_scale: w,
        thickness_scale: t,
        crown_scale: 0.8,
    };
    let dip = |off: f64| ShankKey {
        theta_deg: TOP_DEG + off,
        width_scale: 0.93,
        thickness_scale: 0.92,
        crown_scale: 1.0,
    };
    d.shank.keys = vec![
        lobe(0.0, 1.48, 1.34),
        lobe(54.0, 1.32, 1.22),
        lobe(-54.0, 1.32, 1.22),
        lobe(108.0, 1.18, 1.12),
        lobe(-108.0, 1.18, 1.12),
        lobe(162.0, 1.06, 1.04),
        lobe(-162.0, 1.06, 1.04),
        dip(27.0),
        dip(-27.0),
        dip(81.0),
        dip(-81.0),
        dip(135.0),
        dip(-135.0),
        dip(180.0),
    ];
    let ctx = d.field_context();
    let mut seat = SeatPadLayer {
        theta_deg: TOP_DEG,
        v_mm: ctx.crest_v_mm,
        style: SeatStyle::GypsyMound,
        height_mm: 0.6,
        crown: 1.0,
        blend_mm: 0.5,
        ..Default::default()
    };
    seat.fit_stone(Gem::calibrated(GemCut::Round, 2.25));
    d.layers.layers.push(LayerEntry::new("Rattle stone", Layer::SeatPad(seat)));
    finish(
        &dir,
        "09-cascabel-rattle",
        "seven graded segments around the circle, a round set in the largest",
        PLATINUM,
        Shots { gif: true, ..HERO },
        d,
        &mut lib,
    );

    // --- 10. Molt: the shed skin, pierced. ---------------------------------
    // A ring of fat crescents — scale shadows — carved toward the bore on
    // both side faces, the porthole-lace construction with a scale
    // vocabulary, under a single bead line on the crest.
    let mut d = squared(7.0, 3.0);
    d.svgs.push(SvgAlpha { name: "Crescent".into(), svg: crescent_svg(), invert: false });
    d.bake_all(&mut lib);
    let ctx = d.field_context();
    let mut mask = TilingLayer::default_for("Crescent", &ctx);
    mask.height_mm = 1.0;
    mask.edge_mm = 0.3;
    assert!(mask.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    let mut e = LayerEntry::new(
        "Molt",
        Layer::Openwork(OpenworkLayer { tiling: mask, depth_mm: 1.1, keep_mm: 0.8 }),
    );
    e.blend = Blend::Add;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    let beads = (ctx.circumference_mm / 0.5).round() as u32;
    d.layers.layers.push(LayerEntry::new(
        "Milgrain",
        Layer::Milgrain(MilgrainLayer {
            v_mm: ctx.crest_v_mm,
            bead_diameter_mm: 0.5,
            beads_around: beads,
            height_mm: 0.2,
            mirror: false,
        }),
    ));
    let low = wider_is_low(&d);
    finish(
        &dir,
        "10-molt-pierced",
        "crescent scale shadows pierced into both faces, bead line on the crest",
        WHITE,
        Shots { face: Some(low), ..HERO },
        d,
        &mut lib,
    );

    // --- 11. Medusa: three serpents interwoven. ----------------------------
    // Three closed wave rails per side face at staggered phases, smooth-max
    // filleted where they cross, on a cigar band tall enough to hold the
    // weave — wire relief on a side face is the doctrine's home ground.
    let mut d = squared(6.0, 6.5);
    let ctx = d.field_context();
    let (lo, hi) = ctx.side_faces_std().and_then(|f| f.wider()).unwrap();
    let vc = 0.5 * (lo + hi);
    let span = hi - lo;
    // Centre offset plus amplitude plus the wire's half-width stays inside
    // 0.45 of the face span: at 0.28 + 0.36 the outer rails swung past the
    // corner onto the crown and the ring would not release (1.5% at -37).
    for (name, dv, amp, phase, h) in [
        ("Serpent low", -0.20, 0.15, 0.0, 0.4),
        ("Serpent mid", 0.0, 0.20, 1.0 / 3.0, 0.5),
        ("Serpent high", 0.20, 0.15, 2.0 / 3.0, 0.4),
    ] {
        let c = vc + dv * span;
        let a = amp * span;
        let pts: Vec<[f64; 2]> = (0..8)
            .map(|i| {
                let x = i as f64 / 8.0;
                [x, c + a * (std::f64::consts::TAU * (x + phase)).sin()]
            })
            .collect();
        let wire = CurveLayer {
            points: pts,
            repeats_around: 6,
            closed: true,
            width_mm: 0.9,
            height_mm: h,
            profile: WireProfile::Round,
            taper: 0.0,
            mirror_v: true,
        };
        let mut e = LayerEntry::new(name, Layer::Curve(wire));
        e.blend = Blend::SmoothMax;
        e.soft_mm = 0.3;
        d.layers.layers.push(e);
    }
    let mut seat = SeatPadLayer {
        theta_deg: TOP_DEG,
        v_mm: ctx.crest_v_mm,
        style: SeatStyle::GypsyMound,
        height_mm: 0.6,
        crown: 1.0,
        blend_mm: 0.5,
        ..Default::default()
    };
    seat.fit_stone(Gem::calibrated(GemCut::Round, 3.0));
    d.layers.layers.push(LayerEntry::new("Gaze", Layer::SeatPad(seat)));
    let low = wider_is_low(&d);
    finish(
        &dir,
        "11-medusa-weave",
        "three wave rails interwoven on each face of a cigar band, one round on the crest",
        YELLOW,
        Shots { face: Some(low), gif: true, ..HERO },
        d,
        &mut lib,
    );

    // --- 12. Sidewinder: oblique scutes under a graded spine. --------------
    // Strongly leaning reeds across the band — the sidewinder's diagonal
    // track — windowed to the body, under a graded round row over the top.
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Beveled);
    d.profile.width_mm = 5.4;
    d.profile.thickness_mm = 2.6;
    d.profile.flatten_sides();
    let ctx = d.field_context();
    let mut run = SeatRunLayer {
        gem: Gem::calibrated(GemCut::Round, 1.75),
        bridge_mm: 0.75,
        taper: 0.4,
        taper_theta_deg: TOP_DEG,
        ..Default::default()
    };
    run.seat.style = SeatStyle::GypsyMound;
    run.seat.height_mm = 0.5;
    run.seat.crown = 1.0;
    run.seat.blend_mm = 0.45;
    run.seat.v_mm = ctx.crest_v_mm;
    run.solve_spacing(&ctx);
    let mut e = LayerEntry::new("Spine row", Layer::SeatRun(run));
    e.window = Window::around(TOP_DEG, 150.0);
    d.layers.layers.push(e);
    let count = (ctx.circumference_mm / 0.95).round() as u32;
    let pitch = ctx.circumference_mm / count as f64;
    let mut e = LayerEntry::new(
        "Oblique scutes",
        Layer::Flutes(FlutesLayer {
            count,
            profile: FluteProfile::Round,
            width_mm: pitch - 0.4,
            height_mm: 0.18,
            // 0.45: at 0.7 the leaning flanks read 4 deg of undercut where
            // they cross the crest; the diamondback's 0.25 reads 1 deg.
            lean: 0.45,
            along: false,
        }),
    );
    e.window = Window::except(TOP_DEG, 165.0);
    e.window.fade_deg = 15.0;
    d.layers.layers.push(e);
    finish(
        &dir,
        "12-sidewinder",
        "leaning reeds round the body, a graded round row over the top",
        SILVER,
        Shots { top: true, ..HERO },
        d,
        &mut lib,
    );

    println!(
        "renders in {dir}, designs in {}",
        library::default_design_dir().join("serpentarium").display()
    );
}
