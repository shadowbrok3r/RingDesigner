//! How far side-face relief spills onto the crown fillet where a keyframed station is wider or thinner than the reference.
//! cargo run -p ringdesign-core --release --example side_gate_probe -- [OUT_DIR]
use anyhow::Result;
use ringdesign_core::{
    AlphaLibrary, BuildParams, FieldContext, ProfileStyle, RingDesign,
    castability,
    field::{Layer, LayerEntry, SIDE_FACE_MIN_DRAFT_DEG, SIDE_GATE_FADE_MM, SideFacePick, SurfaceProfile, VGate},
    profile::{REFERENCE_PROFILE_STEPS, ShankKey, ShankKind},
    render,
    tiling::TilingLayer,
};

/// A face's run as shares of its own section's surface arc.
type Run = Option<(f64, f64)>;

/// The reference's side-face runs, and the station's own sampled as the reference is, with its surface arc and profile.
struct Station {
    reference: [Run; 2],
    own: [Run; 2],
    fade: f64,
    len: f64,
    surface: SurfaceProfile,
}

fn station(d: &RingDesign, ctx: &FieldContext, theta: f64) -> Station {
    let inner = d.inner_radius_mm();
    let m = d.modulation_at(theta, inner, d.reference_loop().crest_radius_mm);
    let l = d.profile.sample_mod(inner, REFERENCE_PROFILE_STEPS, &m);
    let mut c = ctx.clone();
    c.surface = SurfaceProfile::from_loop(&l, 257);
    c.band_v_len_mm = l.surface_len_mm;
    c.side_faces_cache = Default::default();
    let share = |r: Run, len: f64| r.map(|(a, b)| (a / len, b / len));
    let rf = ctx.side_faces_std();
    let own = c.side_faces(SIDE_FACE_MIN_DRAFT_DEG);
    let fade = rf.and_then(|f| f.low.or(f.high)).map_or(0.0, |(a, b)| SIDE_GATE_FADE_MM.min((b - a) * 0.25) / ctx.band_v_len_mm);
    Station {
        reference: [share(rf.and_then(|f| f.low), ctx.band_v_len_mm), share(rf.and_then(|f| f.high), ctx.band_v_len_mm)],
        own: [share(own.and_then(|f| f.low), l.surface_len_mm), share(own.and_then(|f| f.high), l.surface_len_mm)],
        fade,
        len: l.surface_len_mm,
        surface: c.surface,
    }
}

/// Per side, metal mm: spill past the station's face to the gate's zero and to its full strength, the draft at its zero, and face left bare.
fn spill(s: &Station) -> ([f64; 2], [f64; 2], [f64; 2], [f64; 2]) {
    let (mut reach, mut full, mut draft, mut short) = ([0.0; 2], [0.0; 2], [90.0; 2], [0.0; 2]);
    if let (Some(r), Some(o)) = (s.reference[0], s.own[0]) {
        reach[0] = (r.1 - o.1).max(0.0) * s.len;
        full[0] = (r.1 - s.fade - o.1).max(0.0) * s.len;
        short[0] = (o.1 - r.1).max(0.0) * s.len;
        draft[0] = s.surface.draft_deg(r.1 * s.len, s.len).unwrap_or(90.0);
    }
    if let (Some(r), Some(o)) = (s.reference[1], s.own[1]) {
        reach[1] = (o.0 - r.0).max(0.0) * s.len;
        full[1] = (o.0 - (r.0 + s.fade)).max(0.0) * s.len;
        short[1] = (r.0 - o.0).max(0.0) * s.len;
        draft[1] = s.surface.draft_deg(r.0 * s.len, s.len).unwrap_or(90.0);
    }
    (reach, full, draft, short)
}

fn phoenix_base() -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.width_mm = 6.6;
    d.profile.thickness_mm = 3.4;
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.flatten_sides();
    d.profile.comfort_fit_mm = 0.2;
    d
}

fn rubus_base() -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.width_mm = 7.0;
    d.profile.thickness_mm = 3.4;
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.crown_mm = 0.6;
    d.profile.flatten_sides();
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    d
}

fn keyed(mut d: RingDesign, keys: &[(f64, f64, f64, f64)]) -> RingDesign {
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = keys.iter().map(|&(theta_deg, width_scale, thickness_scale, crown_scale)| ShankKey { theta_deg, width_scale, thickness_scale, crown_scale }).collect();
    d
}

/// The body with a cellular relief fitted to its side faces and gated to them on both.
fn relieved(mut d: RingDesign, height: f64) -> RingDesign {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for("Voronoi", &ctx);
    t.height_mm = height;
    t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG);
    let mut e = LayerEntry::new("Side-face cells", Layer::Tiling(t));
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    d
}

fn field_line(d: &RingDesign, lib: &AlphaLibrary) -> String {
    let f = castability::analyze_field(d, lib, &d.draft, 384, 192);
    format!("{:?} {:.4}% worst {:+.1}°", f.verdict, f.undercut_fraction() * 100.0, f.worst_draft_deg)
}

fn main() -> Result<()> {
    let out = std::env::args().nth(1);
    let lib = AlphaLibrary::builtin();
    for (name, base) in [("Phoenix square, Flat 6.6 x 3.4", phoenix_base()), ("Rubus, LowDome 7.0 x 3.4 crown 0.6", rubus_base())] {
        let ctx = base.field_context();
        let rf = ctx.side_faces_std();
        println!("== {name}: reference faces {:?} of {:.2} mm", rf, ctx.band_v_len_mm);
        println!("   uniform keys w x t: spill to zero low/high mm | at full strength | base draft there | face left bare | field, 0.5 mm cells");
        for w in [0.88, 1.0, 1.16, 1.36, 1.45] {
            for t in [0.92, 1.0, 1.07, 1.22] {
                let d = keyed(base.clone(), &[(0.0, w, t, 1.0), (90.0, w, t, 1.0), (180.0, w, t, 1.0), (270.0, w, t, 1.0)]);
                let s = station(&d, &d.field_context(), 90.0);
                let (reach, full, draft, short) = spill(&s);
                println!(
                    "   {w:.2} x {t:.2}: {:.3} / {:.3} | {:.3} / {:.3} | {:.1}° / {:.1}° | {:.3} / {:.3} | {}",
                    reach[0], reach[1], full[0], full[1], draft[0], draft[1], short[0], short[1], field_line(&relieved(d, 0.5), &lib)
                );
            }
        }
    }
    let phoenix = keyed(
        phoenix_base(),
        &[
            (90.0, 1.45, 1.22, 0.85),
            (62.0, 1.36, 1.18, 0.90),
            (118.0, 1.36, 1.18, 0.90),
            (28.0, 1.16, 1.07, 1.0),
            (152.0, 1.16, 1.07, 1.0),
            (340.0, 1.0, 1.0, 1.0),
            (200.0, 1.0, 1.0, 1.0),
            (300.0, 0.92, 0.95, 1.0),
            (240.0, 0.92, 0.95, 1.0),
            (270.0, 0.88, 0.92, 1.0),
        ],
    );
    let ctx = phoenix.field_context();
    println!("== Phoenix's own ten keys, per station");
    for theta in [90.0, 62.0, 28.0, 340.0, 300.0, 270.0] {
        let (reach, full, draft, short) = spill(&station(&phoenix, &ctx, theta));
        println!("   {theta:>5.0}°: spill {:.3} / {:.3} mm, full strength {:.3} / {:.3}, draft {:.1}° / {:.1}°, bare {:.3} / {:.3}", reach[0], reach[1], full[0], full[1], draft[0], draft[1], short[0], short[1]);
    }
    for h in [0.3, 0.5, 1.0] {
        let d = relieved(phoenix.clone(), h);
        let f = castability::attributed_field_report(&d, &lib, &d.draft, 384, 192);
        println!("   cells {h} mm: {:?} {:.4}% worst {:+.1}°", f.verdict, f.undercut_fraction() * 100.0, f.worst_draft_deg);
        for n in f.notes.iter().filter(|n| n.contains("Undercut")).take(4) {
            println!("      {n}");
        }
    }
    if let Some(out) = out {
        std::fs::create_dir_all(&out)?;
        let d = relieved(phoenix, 0.5);
        let built = ringdesign_core::mesh::try_build(&d, &lib, BuildParams { theta_steps: 1024, profile_steps: 320, ..Default::default() })?;
        for (view, yaw, pitch) in [("hero", 0.48, 1.0), ("side", 0.0, 0.05), ("top", std::f64::consts::FRAC_PI_2, 0.3)] {
            render::write_png(std::path::Path::new(&out).join(format!("phoenix-gate-{view}.png")), &built.mesh, yaw, pitch, 1000, render::GOLD)?;
        }
    }
    Ok(())
}
