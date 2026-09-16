// The showoff finale: two maximal designs for the bake-off's last round.
// Apex is the ZBrush claim made native — three skins at one height under
// the tie-exact smooth-max, crossfaded around the ring by painted gradient
// masks, on a keyframed serpent body under a graded spine. Chimera morphs
// terraced facets into scale mail and pierces the blend itself.
//
//   cargo run --release --example showoff [out_dir]
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, Verdict};
use ringdesign_core::field::{
    Blend, Layer, LayerEntry, MilgrainLayer, OpenworkLayer, Remap, SeatRunLayer, SeatStyle,
    SideFacePick, VGate, Window, SIDE_FACE_MIN_DRAFT_DEG,
};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::{ShankKey, ShankKind, TOP_DEG};
use ringdesign_core::render::{self, Part};
use ringdesign_core::svg::SvgAlpha;
use ringdesign_core::tiling::{TilingLayer, WarpField};
use ringdesign_core::{gems, library, stones, ProfileStyle, RingDesign};

const YELLOW: [f32; 3] = [0.86, 0.70, 0.42];
const PLATINUM: [f32; 3] = [0.75, 0.76, 0.78];

fn squared(width: f64, thickness: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d
}

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

/// A seamless cosine bump along `u`, peaked at `center` (0..1): the painted
/// crossfade mask that hands one skin over to the next. Approximated with
/// dense gradient stops; constant across the band.
fn fade_svg(center: f64, width: f64) -> String {
    let mut s2 = String::new();
    for i in 0..=48 {
        let x = i as f64 / 48.0;
        let mut d = (x - center).abs();
        if d > 0.5 {
            d = 1.0 - d;
        }
        let t = (d / width).min(1.0);
        let w = 0.5 + 0.5 * (std::f64::consts::PI * t).cos();
        let v = (w * 255.0).round() as u8;
        s2.push_str(&format!(
            r##"<stop offset="{x:.3}" stop-color="#{v:02x}{v:02x}{v:02x}"/>"##
        ));
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><linearGradient id="f" x1="0" y1="0" x2="1" y2="0">{s2}</linearGradient></defs><rect width="100" height="100" fill="url(#f)"/></svg>"##
    )
}

fn finish(
    dir: &str,
    slug: &str,
    blurb: &str,
    tint: [f32; 3],
    face_flip: Option<bool>,
    mut d: RingDesign,
    lib: &mut AlphaLibrary,
) {
    d.name = slug
        .split_once('-')
        .map(|(_, r)| r.replace('-', " "))
        .unwrap_or_else(|| slug.into());
    d.bake_all(lib);
    let field = castability::attributed_field_report(&d, lib, &d.draft, 256, 128);
    println!("{slug:<20} {blurb}");
    println!(
        "{:<20} {}: {:.3}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
        "",
        field.verdict.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm
    );
    for n in &field.notes {
        println!("{:<20}   note: {n}", "");
    }
    for f in ringdesign_core::dfm::findings_in(&d, lib) {
        println!("{:<20}   dfm: {}: {}", "", f.label, f.message);
    }
    if let Some(s) = stones::report(&d, field.parting_z_mm) {
        println!("{:<20}   stones: {} stones, {:.2} ct", "", s.stone_count, s.total_carats);
        let mut seen = std::collections::BTreeSet::new();
        for seat in &s.seats {
            for w in &seat.warnings {
                if seen.insert(w.clone()) {
                    println!("{:<20}   stone warning: {w}", "");
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
    println!("{:<20}   volume {:.1} mm3", "", out.report.volume_mm3);
    for m in out.report.metals.iter().take(1) {
        println!("{:<20}   {}: {:.2} g", "", m.metal, m.grams);
    }
    let stones_mesh = gems::preview_mesh(&d, lib);
    let mut parts = vec![Part::metal(&out.mesh, tint)];
    if let Some(g) = &stones_mesh {
        parts.push(Part::stone(g));
    }
    let pi = std::f64::consts::PI;
    render::write_png_parts(format!("{dir}/{slug}-hero.png"), &parts, 0.55, 1.1, 1000).unwrap();
    render::write_png_parts(format!("{dir}/{slug}-back.png"), &parts, 0.55 + pi, 1.1, 900).unwrap();
    if let Some(flip) = face_flip {
        let (yaw, pitch) = if flip { (pi, pi - 0.35) } else { (0.0, 0.35) };
        render::write_png_parts(format!("{dir}/{slug}-face.png"), &parts, yaw, pitch, 900).unwrap();
    }
    render::write_turntable_gif(format!("{dir}/{slug}.gif"), &out.mesh, 36, 520, tint).unwrap();
    let probe = mesh::build(
        &d,
        lib,
        BuildParams { theta_steps: 640, profile_steps: 160, ..Default::default() },
    );
    ringdesign_core::stl::write_obj(format!("{dir}/{slug}.obj"), &probe.mesh, &d.name).unwrap();
    let designs = library::default_design_dir().join("showoff");
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design(designs.join(format!("{slug}.ring.json")), &d).unwrap();
    println!();
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp/showoff".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut lib = AlphaLibrary::builtin();

    // --- 01. Apex: three skins, one hide. ----------------------------------
    // Broad mail is the base coat; fine mail and elongated keels each ride a
    // painted cosine mask, all three at the same height under the tie-exact
    // smooth-max — where two skins tie they crossfade instead of stacking,
    // which is the layer-brush blend, stated in field arithmetic. The body
    // is keyframed head-to-tail, and a graded spine of rounds runs the crest.
    let mut d = squared(8.2, 4.4);
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let key = |off: f64, w: f64, t: f64, c: f64| ShankKey {
        theta_deg: TOP_DEG + off,
        width_scale: w,
        thickness_scale: t,
        crown_scale: c,
    };
    d.shank.keys = vec![
        key(0.0, 1.16, 1.10, 0.9),
        key(60.0, 1.08, 1.05, 1.0),
        key(-60.0, 1.08, 1.05, 1.0),
        key(150.0, 0.96, 0.97, 1.0),
        key(-150.0, 0.96, 0.97, 1.0),
        key(180.0, 0.92, 0.94, 1.0),
    ];
    d.svgs.push(SvgAlpha { name: "Mail".into(), svg: scale_mail_svg(), invert: false });
    d.svgs.push(SvgAlpha { name: "Keels".into(), svg: keel_svg(), invert: false });
    d.svgs.push(SvgAlpha { name: "Fade back".into(), svg: fade_svg(0.75, 0.30), invert: false });
    d.svgs.push(SvgAlpha { name: "Fade right".into(), svg: fade_svg(0.06, 0.26), invert: false });
    d.bake_all(&mut lib);
    let ctx = d.field_context();
    let h_skin = 0.48;

    let mut a = TilingLayer::default_for("Mail", &ctx);
    a.height_mm = h_skin;
    a.bias = 0.22;
    assert!(a.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    let face_c = a.v_center_mm;
    a.warp = Some(WarpField {
        points: (0..12)
            .map(|i| {
                let f = i as f64 / 12.0;
                [f, face_c + 0.55 * (2.0 * std::f64::consts::TAU * f).sin()]
            })
            .collect(),
        strength: 0.8,
        falloff_mm: 3.0,
    });
    let mut e = LayerEntry::new("Skin broad", Layer::Tiling(a.clone()));
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.3;
    e.remap = Remap::cushion(h_skin);
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);

    let mut b = TilingLayer::default_for("Mail", &ctx);
    b.height_mm = h_skin;
    b.bias = 0.22;
    assert!(b.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    b.repeats_around = (b.repeats_around as f64 * 2.3).round() as u32;
    b.rows = 2;
    b.shear = 0.35;
    let mut e = LayerEntry::new("Skin fine", Layer::Tiling(b));
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.3;
    e.remap = Remap::cushion(h_skin);
    e.mask = Some("Fade back".into());
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);

    let mut c = TilingLayer::default_for("Keels", &ctx);
    c.height_mm = h_skin;
    assert!(c.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    c.repeats_around = ((c.repeats_around as f64 / 1.6).round() as u32).max(1);
    let mut e = LayerEntry::new("Skin keels", Layer::Tiling(c));
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.3;
    e.remap = Remap::cushion(h_skin);
    e.mask = Some("Fade right".into());
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);

    let mut run = SeatRunLayer {
        gem: Gem::calibrated(GemCut::Round, 2.5),
        bridge_mm: 0.75,
        taper: 0.5,
        taper_theta_deg: TOP_DEG,
        ..Default::default()
    };
    run.seat.style = SeatStyle::GypsyMound;
    run.seat.height_mm = 0.55;
    run.seat.crown = 1.0;
    run.seat.blend_mm = 0.45;
    run.seat.v_mm = ctx.crest_v_mm;
    run.solve_spacing(&ctx);
    let mut e = LayerEntry::new("Spine", Layer::SeatRun(run));
    e.window = Window::around(TOP_DEG, 220.0);
    d.layers.layers.push(e);
    finish(
        &dir,
        "01-apex",
        "broad mail, fine mail and keels crossfading at one height on a keyframed body, graded spine",
        YELLOW,
        Some(true),
        d,
        &mut lib,
    );

    // --- 02. Chimera: facets morph to scales, then the blend is pierced. ---
    // Terraced pyramids own the top, mail owns the back, the two crossfade
    // at one height through painted masks — and over the tail arc the
    // crescent openwork carves through whatever the blend put there, so the
    // piercing inherits the morph instead of fighting it. A milgrain line
    // rides the crest wherever the tilted princess row does not.
    let mut d = squared(7.4, 3.6);
    // More crown than the squared default: the tilted mounds' skirts leaned
    // 5 deg where they reached the flat crown's edge fillet.
    d.profile.crown_mm = 1.3;
    d.svgs.push(SvgAlpha { name: "Mail".into(), svg: scale_mail_svg(), invert: false });
    d.svgs.push(SvgAlpha { name: "Keels".into(), svg: keel_svg(), invert: false });
    d.svgs.push(SvgAlpha { name: "Fade top".into(), svg: fade_svg(0.25, 0.34), invert: false });
    d.svgs.push(SvgAlpha { name: "Fade tail".into(), svg: fade_svg(0.75, 0.34), invert: false });
    d.svgs.push(SvgAlpha {
        name: "Crescent".into(),
        svg: {
            let mut path = String::from("M");
            for i in 0..=40 {
                let ang = std::f64::consts::PI * (1.25 + 0.5 * i as f64 / 40.0);
                path.push_str(&format!(
                    "{:.1} {:.1} L",
                    50.0 + 40.0 * ang.cos(),
                    38.0 - 40.0 * ang.sin()
                ));
            }
            for i in 0..=40 {
                let ang = std::f64::consts::PI * (1.75 - 0.5 * i as f64 / 40.0);
                path.push_str(&format!(
                    "{:.1} {:.1} ",
                    50.0 + 40.0 * ang.cos(),
                    38.0 - 17.0 * ang.sin()
                ));
                if i < 40 {
                    path.push('L');
                }
            }
            format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><path d="{path}Z" fill="#000"/></svg>"##
            )
        },
        invert: false,
    });
    d.bake_all(&mut lib);
    let ctx = d.field_context();
    let h_skin = 0.42;

    let mut fac = TilingLayer::default_for("Keels", &ctx);
    fac.height_mm = h_skin;
    assert!(fac.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    fac.repeats_around = ((fac.repeats_around as f64 / 1.5).round() as u32).max(1);
    let mut e = LayerEntry::new("Facets", Layer::Tiling(fac));
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.28;
    e.remap = Remap::Terrace { steps: 3, span_mm: h_skin, riser: 0.4 };
    e.mask = Some("Fade top".into());
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);

    let mut sc = TilingLayer::default_for("Mail", &ctx);
    sc.height_mm = h_skin;
    sc.bias = 0.22;
    assert!(sc.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    let mut e = LayerEntry::new("Scales", Layer::Tiling(sc));
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.28;
    e.remap = Remap::cushion(h_skin);
    e.mask = Some("Fade tail".into());
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);

    let mut mask = TilingLayer::default_for("Crescent", &ctx);
    mask.height_mm = 1.0;
    mask.edge_mm = 0.3;
    assert!(mask.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    let mut e = LayerEntry::new(
        "Pierced tail",
        Layer::Openwork(OpenworkLayer { tiling: mask, depth_mm: 1.1, keep_mm: 0.8 }),
    );
    e.blend = Blend::Add;
    e.window = Window::around(TOP_DEG + 180.0, 120.0);
    e.window.fade_deg = 16.0;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);

    let mut run = SeatRunLayer {
        gem: Gem::calibrated(GemCut::Princess, 1.9),
        bridge_mm: 0.9,
        // 45, the measured-safe bearing: at 30 one corner of each turned
        // square reaches obliquely down the crown and every seat leaned 5-6
        // deg; the symmetric diagonal reads 0.03% on the diamondback.
        tilt_deg: 45.0,
        ..Default::default()
    };
    run.seat.style = SeatStyle::GypsyMound;
    run.seat.height_mm = 0.5;
    run.seat.crown = 1.0;
    run.seat.blend_mm = 0.45;
    run.seat.v_mm = ctx.crest_v_mm;
    run.solve_spacing(&ctx);
    let mut e = LayerEntry::new("Diadem", Layer::SeatRun(run));
    e.window = Window::around(TOP_DEG, 140.0);
    d.layers.layers.push(e);

    let beads = (ctx.circumference_mm / 0.5).round() as u32;
    let mut e = LayerEntry::new(
        "Milgrain",
        Layer::Milgrain(MilgrainLayer {
            v_mm: ctx.crest_v_mm,
            bead_diameter_mm: 0.5,
            beads_around: beads,
            height_mm: 0.2,
            mirror: false,
        }),
    );
    e.window = Window::except(TOP_DEG, 155.0);
    e.window.fade_deg = 10.0;
    d.layers.layers.push(e);
    finish(
        &dir,
        "02-chimera",
        "terraced facets morph into mail, the tail arc pierced through the blend, tilted diadem and milgrain sharing the crest",
        PLATINUM,
        Some(true),
        d,
        &mut lib,
    );

    println!(
        "renders in {dir}, designs in {}",
        library::default_design_dir().join("showoff").display()
    );
}
