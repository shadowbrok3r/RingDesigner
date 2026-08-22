// The gallery: rings built out of the stone work this round added, every one
// held to the field verdict before it renders.
//
//   cargo run --release --example gallery [out_dir]
//
// Writes a hero PNG and a detail crop per ring, an index.html contact sheet,
// and saves each design under <designs>/gallery/ so every piece opens in the
// app. Builtin assets only, so the files are portable to an empty machine.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, Verdict};
use ringdesign_core::field::{
    Layer, LayerEntry, MilgrainLayer, SeatPadLayer, SeatRunLayer, SeatStyle,
};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::pave::{self, HaloSpec, PaveRegion, PaveSpec, PinnedSeat};
use ringdesign_core::profile::{ShankKey, ShankKind, TOP_DEG};
use ringdesign_core::{library, render, stones, ProfileStyle, RingDesign};

const YELLOW: [f32; 3] = [0.86, 0.70, 0.42];
const ROSE: [f32; 3] = [0.84, 0.60, 0.49];
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

fn domed(style: ProfileStyle, width: f64, thickness: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(style);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d
}

/// Whether the wider side face is the low-`v` one. The tie on a symmetric
/// band breaks on float noise, so it is read per design, never assumed.
fn wider_is_low(d: &RingDesign) -> bool {
    let ctx = d.field_context();
    ctx.side_faces_std()
        .and_then(|f| f.wider())
        .map(|(lo, hi)| 0.5 * (lo + hi) < ctx.crest_v_mm)
        .unwrap_or(false)
}

/// A `v` for a row of seats whose footprint reaches `foot` either way.
///
/// The whole footprint has to land on the side face, not just its centre.
/// A seat spilling onto the crown flank is the measured hazard: the same
/// bezel row 0.5 mm further in fields 0.64% at −18.6° and 4.03% at −45°,
/// against 0.0000% once it sits clear. And the face on a squared band runs
/// right up to the bore edge, so the outer bound is the band's, not the
/// face's.
fn row_v(d: &RingDesign, foot: f64) -> f64 {
    let ctx = d.field_context();
    let (lo, hi) = ctx.side_faces_std().and_then(|f| f.wider()).expect("a squared band has faces");
    let inner = lo + foot;
    let outer = (hi - foot).min(ctx.band_v_len_mm - foot - 0.35);
    assert!(
        inner <= outer,
        "a {:.2} mm face cannot hold a seat reaching {foot:.2} mm either way",
        hi - lo
    );
    (0.5 * (lo + hi)).clamp(inner, outer)
}

struct Piece {
    slug: &'static str,
    title: &'static str,
    blurb: &'static str,
    tint: [f32; 3],
    /// Hero attitude; 1.12 is the catalogue three-quarter.
    pitch: f64,
    /// Look down the finger at the -Z face instead of the +Z one.
    flip: bool,
    notes: Vec<String>,
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "gallery".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut lib = AlphaLibrary::builtin();
    let mut pieces = Vec::new();

    pieces.push(marquise_eternity(&dir, &mut lib));
    pieces.push(emerald_graduated(&dir, &mut lib));
    pieces.push(cabochon_gypsy(&dir, &mut lib));
    pieces.push(oval_halo(&dir, &mut lib));
    pieces.push(pinned_pave(&dir, &mut lib));
    pieces.push(baguette_channel(&dir, &mut lib));
    pieces.push(trilogy(&dir, &mut lib));
    pieces.push(melee_band(&dir, &mut lib));

    write_index(&dir, &pieces);
    println!("\n{} pieces -> {dir}/index.html", pieces.len());
}

/// Field-check, render, save. The gallery refuses to ship an uncastable ring.
fn finish(
    dir: &str,
    slug: &'static str,
    title: &'static str,
    blurb: &'static str,
    tint: [f32; 3],
    pitch: f64,
    flip: bool,
    mut d: RingDesign,
    lib: &mut AlphaLibrary,
) -> Piece {
    d.name = title.into();
    d.bake_all(lib);
    let mut notes = Vec::new();

    let field = castability::analyze_field(&d, lib, &d.draft, 288, 144);
    println!("{slug:<22} {blurb}");
    println!(
        "{:<22} {}: {:.4}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
        "",
        field.verdict.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm,
    );
    notes.push(format!(
        "{} — {:.4}% undercut, thinnest wall {:.2} mm",
        field.verdict.label(),
        field.undercut_fraction() * 100.0,
        field.thinnest_wall_mm
    ));
    for n in &field.notes {
        println!("{:<22}   note: {n}", "");
    }
    for f in ringdesign_core::dfm::findings(&d) {
        println!("{:<22}   dfm: {}: {}", "", f.label, f.message);
        notes.push(format!("DFM: {}", f.message));
    }
    if let Some(s) = stones::report(&d, field.parting_z_mm) {
        println!("{:<22}   stones: {} • {:.2} ct", "", s.stone_count, s.total_carats);
        notes.push(format!("{} stones, {:.2} ct total", s.stone_count, s.total_carats));
        if let Some(c) = &s.closest {
            println!(
                "{:<22}   closest pair: {:.2} mm at the girdle, {:.2} at the culet",
                "", c.gap_mm, c.gap_deep_mm
            );
            notes.push(format!(
                "closest pair {:.2} mm at the girdle, {:.2} mm at the culet",
                c.gap_mm, c.gap_deep_mm
            ));
        }
        if let Some(n) = s.crowding_note() {
            println!("{:<22}   crowding: {n}", "");
            notes.push(format!("crowding: {n}"));
        }
        let mut seen = std::collections::BTreeSet::new();
        for seat in &s.seats {
            for w in &seat.warnings {
                if seen.insert(w.clone()) {
                    println!("{:<22}   stone warning: {w}", "");
                    notes.push(format!("bench: {w}"));
                }
            }
        }
    }
    assert_ne!(field.verdict, Verdict::NotCastable, "{slug} must not ship uncastable");

    let out = mesh::build(
        &d,
        lib,
        BuildParams { theta_steps: 1800, profile_steps: 400, ..Default::default() },
    );
    assert!(out.report.validation.watertight, "{slug} not watertight");

    // Stones are never in the mesh and never exported. They are in the
    // picture, because a ring with the stones left out is a picture of the
    // stock rather than of the piece.
    let stones_mesh = ringdesign_core::gems::preview_mesh(&d, lib);
    let mut parts = vec![render::Part::metal(&out.mesh, tint)];
    if let Some(g) = &stones_mesh {
        parts.push(render::Part::stone(g));
    }

    let pi = std::f64::consts::PI;
    let hero_pitch = if flip { pi - pitch } else { pitch };
    render::write_png_parts(format!("{dir}/{slug}-hero.png"), &parts, 0.55, hero_pitch, 1000)
        .unwrap();

    // The detail: straight down the finger, then cropped to the top of the
    // ring where the stone work is.
    let (yaw, dp) = if flip { (pi, pi - 0.30) } else { (0.0, 0.30) };
    let edge = 1600;
    let img = render::render_parts_ss(&parts, yaw, dp, edge, edge, 3);
    crop(&format!("{dir}/{slug}-detail.png"), &img, edge, 0.30, 0.02, 0.40);

    let designs = library::default_design_dir().join("gallery");
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design(designs.join(format!("{slug}.ring.json")), &d).unwrap();
    println!();
    Piece { slug, title, blurb, tint, pitch, flip, notes }
}

/// A square crop of an RGB buffer, given as fractions of the edge.
fn crop(path: &str, img: &[u8], edge: usize, x: f64, y: f64, size: f64) {
    let s = ((size * edge as f64) as usize).clamp(16, edge);
    let x0 = ((x * edge as f64) as usize).min(edge - s);
    let y0 = ((y * edge as f64) as usize).min(edge - s);
    let mut out = vec![0u8; s * s * 3];
    for r in 0..s {
        let src = ((y0 + r) * edge + x0) * 3;
        out[r * s * 3..(r + 1) * s * 3].copy_from_slice(&img[src..src + s * 3]);
    }
    image::save_buffer(path, &out, s as u32, s as u32, image::ColorType::Rgb8).unwrap();
}

fn write_index(dir: &str, pieces: &[Piece]) {
    let mut h = String::from(
        "<!doctype html><meta charset=\"utf-8\"><title>RingDesigner gallery</title>\
         <style>body{background:#14110e;color:#e8e2d8;font:15px/1.55 -apple-system,\
         Segoe UI,Roboto,sans-serif;margin:0;padding:40px}h1{font-weight:600;\
         letter-spacing:-.01em;margin:0 0 6px}.sub{color:#9a9086;margin:0 0 36px}\
         .p{display:grid;grid-template-columns:340px 340px 1fr;gap:24px;\
         align-items:start;padding:26px 0;border-top:1px solid #2b2620}\
         img{width:100%;border-radius:10px;background:#000;display:block}\
         h2{margin:0 0 6px;font-size:19px;font-weight:600}\
         .b{color:#c3b8a8;margin:0 0 14px}\
         ul{margin:0;padding-left:18px;color:#9a9086;font-size:13px}\
         li{margin:3px 0}</style>\
         <h1>RingDesigner gallery</h1>\
         <p class=\"sub\">Every piece field-checked before it rendered.</p>",
    );
    for p in pieces {
        h.push_str(&format!(
            "<div class=\"p\"><img src=\"{0}-hero.png\"><img src=\"{0}-detail.png\">\
             <div><h2>{1}</h2><p class=\"b\">{2}</p><ul>",
            p.slug, p.title, p.blurb
        ));
        for n in &p.notes {
            h.push_str(&format!("<li>{}</li>", n.replace('<', "&lt;")));
        }
        h.push_str("</ul></div></div>");
        let _ = (p.tint, p.pitch, p.flip);
    }
    std::fs::write(format!("{dir}/index.html"), h).unwrap();
}

// --- The pieces ------------------------------------------------------------

/// Marquise eternity: the seat is the stone's plan, not a circle round it.
fn marquise_eternity(dir: &str, lib: &mut AlphaLibrary) -> Piece {
    let mut d = domed(ProfileStyle::LowDome, 4.4, 2.4);
    let ctx = d.field_context();
    let mut run = SeatRunLayer::default();
    run.gem = Gem::calibrated(GemCut::Marquise, 1.8);
    run.seat.style = SeatStyle::GypsyMound;
    run.seat.v_mm = ctx.crest_v_mm;
    run.seat.height_mm = 0.55;
    run.seat.blend_mm = 0.35;
    run.bridge_mm = 0.35;
    run.solve_spacing(&ctx);
    d.layers.layers.push(LayerEntry::new("Marquise row", Layer::SeatRun(run)));

    let mut m = MilgrainLayer { v_mm: 0.55, ..Default::default() };
    m.bead_diameter_mm = 0.42;
    m.height_mm = 0.2;
    m.beads_around = 168;
    m.mirror = true;
    d.layers.layers.push(LayerEntry::new("Milgrain", Layer::Milgrain(m)));

    finish(
        dir,
        "01-marquise-eternity",
        "Marquise eternity",
        "A full ring of marquise stones lying along the band. The seat carries the girdle's \
         own pointed plan, so the stock reaches the stone's tips instead of a circle drawn \
         round its length.",
        WHITE,
        1.12,
        false,
        d,
        lib,
    )
}

/// A graduated row of step cuts: the stations close up to hold the bridge.
fn emerald_graduated(dir: &str, lib: &mut AlphaLibrary) -> Piece {
    let mut d = domed(ProfileStyle::LowDome, 5.0, 2.6);
    let ctx = d.field_context();
    let mut run = SeatRunLayer::default();
    run.gem = Gem::calibrated(GemCut::Emerald, 2.2);
    run.seat.style = SeatStyle::GypsyMound;
    run.seat.v_mm = ctx.crest_v_mm;
    run.seat.height_mm = 0.6;
    run.seat.blend_mm = 0.4;
    run.bridge_mm = 0.45;
    run.taper = 0.55;
    run.taper_theta_deg = TOP_DEG;
    run.solve_spacing(&ctx);
    d.layers.layers.push(LayerEntry::new("Graduated emeralds", Layer::SeatRun(run)));

    finish(
        dir,
        "02-emerald-graduated",
        "Graduated emerald band",
        "Step cuts falling away from the top. The stations close up as the stones shrink so \
         the metal between them stays the same all the way round — and the report measures \
         that metal again at the culet, where the ring's own curvature has closed the arc in.",
        YELLOW,
        1.12,
        false,
        d,
        lib,
    )
}

/// A cabochon wants a bed, not a hole.
fn cabochon_gypsy(dir: &str, lib: &mut AlphaLibrary) -> Piece {
    let mut d = domed(ProfileStyle::LowDome, 9.0, 3.0);
    let ctx = d.field_context();
    let cab = Gem::cabochon(GemCut::Oval, 5.0);
    // No bur dimple: a pit's walls are a dome's inverse, and one locks even
    // on the crest. The setter drills it.
    let mut seat = SeatPadLayer {
        theta_deg: TOP_DEG,
        v_mm: ctx.crest_v_mm,
        style: SeatStyle::GypsyMound,
        height_mm: 0.5,
        blend_mm: 1.2,
        // Gypsy means flush: the girdle drops to the surrounding metal, and
        // the setter burnishes the rim over it. Without this the preview
        // stands the stone on an undrilled mound.
        set_depth_mm: Some(0.45),
        ..Default::default()
    };
    seat.fit_stone(cab);
    d.layers.layers.push(LayerEntry::new("Cabochon seat", Layer::SeatPad(seat)));

    let mut m = MilgrainLayer { v_mm: 0.6, ..Default::default() };
    m.bead_diameter_mm = 0.5;
    m.height_mm = 0.24;
    m.beads_around = 190;
    m.mirror = true;
    d.layers.layers.push(LayerEntry::new("Milgrain", Layer::Milgrain(m)));

    finish(
        dir,
        "03-cabochon-gypsy",
        "Gypsy-set oval cabochon",
        "A 5 mm flat-backed oval, gypsy set flush into the crown. The model used to refuse \
         this stone outright — it read a faceted pavilion and asked for millimetres of metal \
         under a cabochon that needs a bed and nothing more.",
        ROSE,
        1.12,
        false,
        d,
        lib,
    )
}

/// The halo follows the centre's own outline.
fn oval_halo(dir: &str, lib: &mut AlphaLibrary) -> Piece {
    // A cocktail silhouette: authored stations widen the band under the head
    // and take it back to a slim shank behind, which is the shape a halo
    // wants and a plain hoop is not.
    let mut d = domed(ProfileStyle::LowDome, 11.0, 3.2);
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = vec![
        ShankKey { theta_deg: TOP_DEG, width_scale: 1.0, thickness_scale: 1.0, crown_scale: 1.0 },
        ShankKey {
            theta_deg: TOP_DEG + 70.0,
            width_scale: 0.72,
            thickness_scale: 0.92,
            crown_scale: 1.0,
        },
        ShankKey {
            theta_deg: TOP_DEG + 180.0,
            width_scale: 0.42,
            thickness_scale: 0.82,
            crown_scale: 1.0,
        },
        ShankKey {
            theta_deg: TOP_DEG + 290.0,
            width_scale: 0.72,
            thickness_scale: 0.92,
            crown_scale: 1.0,
        },
    ];
    let spec = HaloSpec {
        center: Gem::calibrated(GemCut::Oval, 3.5),
        accent: Gem::calibrated(GemCut::Round, 1.0),
        theta_deg: TOP_DEG,
        gap_mm: 0.35,
        bridge_mm: 0.25,
        count: 0,
        v_mm: None,
        rot_deg: 0.0,
    };
    let (entry, n) = pave::halo(&d, &spec).expect("the halo must fit this band");
    println!("{:<22} halo: {n} accents round an oval plate", "");
    d.layers.layers.push(entry);

    finish(
        dir,
        "04-oval-halo",
        "Oval halo",
        "An oval centre on a gentle plate, ringed by melee, on a shank that widens under \
         the head and falls away behind. The halo is the centre stone's own outline grown by \
         the gap, with its accents spaced at equal arc length round it — an oval halo, not a \
         circle drawn round an oval.",
        PLATINUM,
        1.12,
        false,
        d,
        lib,
    )
}

/// A pavé the packer owns, with one seat the user does.
fn pinned_pave(dir: &str, lib: &mut AlphaLibrary) -> Piece {
    // Narrow and deep: the side face's width follows the band's thickness,
    // not its width, so this carries a wider face than a slab twice across.
    let mut d = squared(6.0, 5.0);
    let low = wider_is_low(&d);

    // A bezel collar, not a gypsy mound: on a side face its walls stand
    // parallel to the pull, and it spends a third of the mound's stock, which
    // is what lets a row fit a face at all.
    let mut held = SeatPadLayer {
        style: SeatStyle::Bezel,
        bezel_wall_mm: 0.45,
        recess_mm: 0.18,
        height_mm: 0.6,
        blend_mm: 0.35,
        theta_deg: TOP_DEG,
        ..Default::default()
    };
    held.blend_mm = 0.55;
    held.fit_stone(Gem::calibrated(GemCut::Round, 1.6));
    let v_face = row_v(&d, held.half_extents_mm().1 + held.blend_mm);
    held.v_mm = v_face;

    let spec = PaveSpec {
        gem: Gem::calibrated(GemCut::Round, 1.1),
        bridge_mm: 0.35,
        theta_deg: TOP_DEG,
        span_deg: 360.0,
        region: PaveRegion::VBand { center_mm: v_face, width_mm: 2.4 },
        stagger: false,
        style: SeatStyle::Bezel,
        rot_deg: 0.0,
        blend_mm: 0.55,
        recess_mm: 0.18,
        pinned: vec![
            PinnedSeat { theta_deg: TOP_DEG, v_mm: v_face, seat: Some(held), clear_mm: 0.0 },
            PinnedSeat { theta_deg: TOP_DEG + 180.0, v_mm: v_face, seat: None, clear_mm: 4.0 },
        ],
    };
    let (entry, out) = pave::fill(&d, &spec).expect("the fill must fit this face");
    println!(
        "{:<22} pavé: {} seats, {} pinned{}",
        "",
        out.seats,
        out.pinned,
        out.note.as_deref().map(|n| format!(" ({n})")).unwrap_or_default()
    );
    d.layers.layers.push(entry);

    finish(
        dir,
        "05-pinned-pave",
        "Pinned pavé",
        "A side-face pavé with one seat the packer does not own: a larger stone pinned at the \
         top, and a clear span cut opposite for an engraved date. Change the band and the row \
         re-packs around both.",
        WHITE,
        1.12,
        low,
        d,
        lib,
    )
}

/// Baguettes turned across the band.
fn baguette_channel(dir: &str, lib: &mut AlphaLibrary) -> Piece {
    let mut d = domed(ProfileStyle::LowDome, 6.0, 2.8);
    let ctx = d.field_context();
    let mut run = SeatRunLayer::default();
    run.gem = Gem::calibrated(GemCut::Baguette, 1.6);
    run.seat.style = SeatStyle::GypsyMound;
    run.seat.v_mm = ctx.crest_v_mm;
    run.seat.height_mm = 0.5;
    run.seat.blend_mm = 0.35;
    run.seat.rot_deg = 90.0;
    run.bridge_mm = 0.35;
    run.solve_spacing(&ctx);
    d.layers.layers.push(LayerEntry::new("Baguette row", Layer::SeatRun(run)));

    finish(
        dir,
        "06-baguette-channel",
        "Turned baguette band",
        "Baguettes stood across the band instead of along it. The seat turns with the stone, \
         so the row re-packs to the reach it actually has and the report measures the band \
         edge against the stone's length rather than its width.",
        YELLOW,
        1.12,
        false,
        d,
        lib,
    )
}

/// Three stones, three plans, and the census that measures between them.
fn trilogy(dir: &str, lib: &mut AlphaLibrary) -> Piece {
    let mut d = domed(ProfileStyle::LowDome, 5.4, 2.6);
    let ctx = d.field_context();
    let pad = |theta: f64, gem: Gem, h: f64| {
        let mut s = SeatPadLayer {
            theta_deg: theta,
            v_mm: ctx.crest_v_mm,
            style: SeatStyle::GypsyMound,
            height_mm: h,
            blend_mm: 0.9,
            ..Default::default()
        };
        s.fit_stone(gem);
        s
    };
    let centre = pad(TOP_DEG, Gem::calibrated(GemCut::Oval, 4.0), 0.9);
    let span = centre.half_extents_mm().0;
    // Flanks placed off the centre's own reach, in metal at the crest.
    let step = (span + 1.9) / ctx.crest_radius_mm * 180.0 / std::f64::consts::PI;
    d.layers.layers.push(LayerEntry::new("Centre oval", Layer::SeatPad(centre)));
    for (i, s) in [-1.0, 1.0].into_iter().enumerate() {
        let p = pad(TOP_DEG + s * step, Gem::calibrated(GemCut::Pear, 2.4), 0.7);
        d.layers.layers.push(LayerEntry::new(format!("Pear {}", i + 1), Layer::SeatPad(p)));
    }

    finish(
        dir,
        "07-trilogy",
        "Oval and pear trilogy",
        "Three stones from three layers that know nothing about each other. The pairwise \
         census measures the metal between them anyway — at the girdle and again at the \
         culet, where the arc has closed in.",
        ROSE,
        1.12,
        false,
        d,
        lib,
    )
}

/// The reference: a plain band whose melee is packed in metal.
fn melee_band(dir: &str, lib: &mut AlphaLibrary) -> Piece {
    let mut d = squared(6.5, 4.5);
    let low = wider_is_low(&d);
    let blend = 0.55;
    let v_face = row_v(&d, 1.2 * 0.5 + 0.45 + blend);
    let spec = PaveSpec {
        gem: Gem::calibrated(GemCut::Round, 1.2),
        bridge_mm: 0.3,
        theta_deg: TOP_DEG,
        span_deg: 360.0,
        region: PaveRegion::VBand { center_mm: v_face, width_mm: 2.5 },
        stagger: false,
        style: SeatStyle::Bezel,
        rot_deg: 0.0,
        blend_mm: blend,
        recess_mm: 0.18,
        pinned: Vec::new(),
    };
    let (entry, out) = pave::fill(&d, &spec).expect("the fill must fit this face");
    println!("{:<22} pavé: {} seats in {} rows", "", out.seats, out.rows);
    d.layers.layers.push(entry);

    finish(
        dir,
        "08-melee-band",
        "Side-face melee band",
        "The plain case, and the one the arc metric changed. A side face sits at 0.85 of the \
         crest radius, so the row is packed by the metal it really has rather than by the \
         chart's arc — the count and the bridge are both honest now.",
        PLATINUM,
        1.12,
        low,
        d,
        lib,
    )
}
