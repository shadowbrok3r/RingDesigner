//! Cataphracta — Heloderma, the beaded one: a Gila monster's beadwork over a tail swollen with stored fat, poured in lost wax.
//! cargo build --release -p ringdesign-core --example cataphracta_heloderma
//! target/release/examples/cataphracta_heloderma [OUT_DIR] [--draft] [--verify]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    castability::{self, CastProcess, Verdict},
    csg, dfm,
    field::{Blend, Decal, DecalLayer, Layer, LayerEntry, SeatPadLayer, SeatStyle, Window},
    gem::{Gem, GemCut},
    library, manufacturing as mf, mesh,
    profile::ShankKey,
    render::{self, Part},
    setting::{self, SolidKind},
    stl,
    svg::SvgAlpha,
    tiling::{GradeLaw, TileGrade},
};
use std::f64::consts::PI;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

const SLUG: &str = "heloderma";
const NAME: &str = "Heloderma \u{2014} the beaded one";

/// The investment's fill floor for this ring, mm (Logan, 2026-09-27).
const MIN_SECTION_MM: f64 = 0.8;

/// Bead pitch round the ring at the swell's crest, metal mm; the grade takes it to `1 - GRADE_TAPER` of that at the palm.
const PITCH_TOP_MM: f64 = 1.45;
const GRADE_TAPER: f64 = 0.28;
/// Bead rows across the band, chart mm: a hexagon's 0.866 of the pitch once the swell's 1.25 stretch is on it.
const ROW_MM: f64 = 1.0;
/// The dorsal row's own bead, as a share of the field's, and the extra room either side of it, chart mm.
const DORSAL_SIZE: f64 = 1.28;
const DORSAL_ROOM_MM: f64 = 0.24;
/// Metal between neighbouring beads at their feet, as a share of the pitch.
const LAND: f64 = 0.08;
/// The tallest bead, mm: a decal's full ink.
const BEAD_MM: f64 = 0.50;
/// Peak heights in mm: the black bands' high beads, the salmon bands' low beads, and the dorsal row.
const HIGH_MM: f64 = 0.44;
const LOW_MM: f64 = 0.10;
/// The salmon granulation's pitch as a share of the beads', and its land.
const GRANULE: f64 = 0.44;
const GRANULE_LAND: f64 = 0.14;
const DORSAL_MM: f64 = 0.46;
/// Across the band, chart mm off the crest: beads are flattened shingles inside the first, full domes past the second.
const SHINGLE_MM: [f64; 2] = [0.9, 3.4];
/// Black bands round the ring, and the share of each period they hold at the crest.
const BANDS: f64 = 9.0;
const BLACK_SHARE: f64 = 0.56;
/// Sectors round the ring, one decal each: a 1024 px raster over a sector holds a bead to 0.01 mm.
const SECTORS: usize = 8;
/// Beads stop this far short of the band's edges, chart mm, so the comfort roll stays plain.
const EDGE_MM: f64 = 0.70;

fn params(draft: bool) -> BuildParams {
    let (t, p) = if draft { (768, 320) } else { (1536, 448) };
    BuildParams {
        theta_steps: t,
        profile_steps: p,
        refine: None,
        ..Default::default()
    }
}

/// The keyed half-round: a fat-tail swell at the face, slim at the palm, poured in lost wax.
fn band() -> RingDesign {
    let mut d = RingDesign {
        name: NAME.into(),
        ..RingDesign::default()
    };
    d.profile.width_mm = 8.0;
    d.profile.thickness_mm = 3.2;
    d.profile.apply_style(ProfileStyle::HalfRound);
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.2;
    d.size = ringdesign_core::resize::size_from_bore(18.6).unwrap();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let k = |theta_deg: f64, width_scale: f64, thickness_scale: f64, crown_scale: f64| ShankKey {
        theta_deg,
        width_scale,
        thickness_scale,
        crown_scale,
    };
    d.shank.keys = vec![
        k(35.0, 1.08, 1.12, 1.0),
        k(90.0, 1.22, 1.34, 1.05),
        k(145.0, 1.08, 1.12, 1.0),
        k(210.0, 0.96, 0.96, 1.0),
        k(270.0, 0.90, 0.90, 1.0),
        k(330.0, 0.96, 0.96, 1.0),
    ];
    d.draft.sand = None;
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    let mut setup = mf::Setup::from_design(&d);
    setup.recipe.name = format!("{NAME} / investment / Silver 925");
    setup.recipe.alloy = "Silver 925".into();
    setup.recipe.sand = None;
    setup.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925")
        .map_or(1.9, |m| m.shrink_pct);
    setup.bench_notes = "Procedural half-round keyed to a fat-tail swell, poured in lost wax. The Gila's beadwork: full round beads in the \
        black bands, flattened pebbles in the salmon ground, the dorsal row a size up on the crest, and a 3 mm spessartite in a collet \
        on the swell's mound. At the bench: burnish the collet's lip over the stone; polish the bead tops, leave the lands satin."
        .into();
    d.manufacturing = Some(setup);
    d
}

fn spessartite() -> Gem {
    Gem {
        preview_tint: Some([0.95, 0.38, 0.06]),
        ..Gem::calibrated(GemCut::Round, 3.0)
    }
}

fn grade() -> TileGrade {
    TileGrade {
        taper: GRADE_TAPER,
        theta_deg: 90.0,
        law: GradeLaw::Cosine,
        isotropic: false,
    }
}

/// Ring fraction per lattice fraction at ring fraction `x`: the grade's local pitch against the ring's mean.
fn stretch_of_grade(x: f64) -> f64 {
    let g = grade();
    let e = 1e-4;
    2.0 * e / (g.phi(x + e) - g.phi(x - e))
}

fn hash(k: u64, s: u64) -> f64 {
    let mut z = (k + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ s.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 29)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 32;
    ((z >> 11) as f64 / (1u64 << 53) as f64) - 0.5
}

/// A smooth value noise over (ring fraction, across mm), periodic round the ring: -0.5..0.5.
fn noise(x: f64, y: f64, cells: f64, seed: u64) -> f64 {
    let (gx, gy) = (x.rem_euclid(1.0) * cells, y);
    let (ix, iy) = (gx.floor(), gy.floor());
    let (fx, fy) = (gx - ix, gy - iy);
    let s = |t: f64| t * t * (3.0 - 2.0 * t);
    let at = |i: f64, j: f64| hash((i.rem_euclid(cells) as u64) * 131 + ((j + 64.0) as u64), seed);
    let a = at(ix, iy) + (at(ix + 1.0, iy) - at(ix, iy)) * s(fx);
    let b = at(ix, iy + 1.0) + (at(ix + 1.0, iy + 1.0) - at(ix, iy + 1.0)) * s(fx);
    a + (b - a) * s(fy)
}

/// The Gila's banding: whether the bead at ring angle `theta`, `across` chart mm off the crest, is a black band's high bead.
///
/// Nine bands in the grade's own lattice coordinate, so they narrow toward the palm with the beads. Each band leans and bows with a
/// slow noise across the band, so no edge runs true; every third band forks into a Y toward one edge, and every fourth salmon gap
/// is bridged by a black strap on one flank, the Gila's reticulation. The stone's station falls mid-salmon.
fn black(theta: f64, across: f64) -> bool {
    let g = grade();
    let x = theta / 360.0;
    let a = across;
    // The lattice coordinate, set so the stone at 90 sits mid-gap.
    let phi = g.phi(x) - 0.25;
    let wander = 0.22 * noise(x, a / 3.5, 7.0, 1) + 0.035 * a * (0.5 + noise(x, 0.0, 5.0, 3));
    let q = BANDS * phi + wander + 0.5;
    let band = q.floor().rem_euclid(BANDS) as u64;
    let p = q - q.floor();
    let forked = band % 3 == 0;
    let share = if forked { BLACK_SHARE + 0.12 } else { BLACK_SHARE } + 0.10 * noise(x, a / 4.0, 9.0, 4);
    let d = (p - 0.5).abs();
    let mut on = d < 0.5 * share;
    if forked {
        // A salmon wedge opens from one edge and splits the band into a Y.
        let s = if band % 2 == 0 { 1.0 } else { -1.0 };
        let reach = s * a - 1.6;
        if reach > 0.0 && d < (0.05 + 0.045 * reach).min(0.15) {
            on = false;
        }
    }
    if !on && band % 4 == 1 && p > 0.5 {
        // A strap across the salmon gap that follows this band.
        let s = if band % 8 == 1 { 1.0 } else { -1.0 };
        if (s * a - 2.6).abs() < 0.55 {
            on = true;
        }
    }
    on && ring_delta(theta, 90.0).abs() > 9.0
}

fn ring_delta(a: f64, b: f64) -> f64 {
    (a - b + 540.0).rem_euclid(360.0) - 180.0
}

/// One bead in chart millimetres: centre, semi-axes, peak height in mm, and how round its dome stands (0 a flattened shingle,
/// 1 a full dome).
struct Bead {
    u: f64,
    v: f64,
    rx: f64,
    ry: f64,
    peak_mm: f64,
    round: f64,
    high: bool,
}

/// Every bead on the ring, laid in rows round the band: the dorsal row on the crest, then rows a fixed chart pitch apart, each row
/// counted so its beads keep the crest's metal pitch at its own radius, graded round the ring, jittered so no column runs true.
fn beads(d: &RingDesign) -> Vec<Bead> {
    let ctx = d.field_context();
    let g = grade();
    let circ = ctx.circumference_mm;
    let crest = ctx.crest_v_mm;
    let len = ctx.band_v_len_mm;
    let stone_r = stone_clear_mm();
    // Columns round the crest that put `PITCH_TOP_MM` of metal at the face.
    let face_pitch_chart = PITCH_TOP_MM / ctx.crest_scale(90.0);
    let cols = (circ * stretch_of_grade(0.25) / face_pitch_chart).round();
    let mut rows: Vec<(i64, f64)> = vec![(0, crest)];
    for k in 1..12i64 {
        let off = DORSAL_ROOM_MM + k as f64 * ROW_MM;
        for (s, v) in [(k, crest + off), (-k, crest - off)] {
            if v >= EDGE_MM && v <= len - EDGE_MM {
                rows.push((s, v));
            }
        }
    }
    let mut out = Vec::new();
    for (i, v0) in rows {
        let a = ctx.arc_scale(v0);
        let dorsal = i == 0;
        let n = if dorsal { (cols / DORSAL_SIZE).round() } else { (cols * a).round().max(8.0) };
        let stagger = if i.rem_euclid(2) == 1 { 0.5 } else { 0.0 };
        for j in 0..n as i64 {
            let seed = ((i + 64) * 4096 + j) as u64;
            let jit = if dorsal { 0.0 } else { 1.0 };
            let phi = (j as f64 + stagger + 0.12 * jit * hash(seed, 1)) / n;
            let x = g.x_of_phi(phi).rem_euclid(1.0);
            let theta = x * 360.0;
            let v = v0 + 0.07 * jit * ROW_MM * hash(seed, 2);
            let across = v - crest;
            // Metal pitch round and across at this bead, and the bead's metal diameter.
            let cs = ctx.crest_scale(theta);
            let st = ctx.station_stretch(theta);
            let around = circ / n * stretch_of_grade(x) * cs * a;
            let over = if dorsal { f64::MAX } else { ROW_MM * st * 2.0 / 3f64.sqrt() };
            let dia = (1.0 - LAND) * around.min(over) * (1.0 + 0.08 * jit * hash(seed, 3));
            // Clear of the stone's mound.
            let du = ring_delta(theta, 90.0) / 360.0 * circ * cs;
            let dv = across * st;
            if du.hypot(dv) < stone_r + 0.5 * dia {
                continue;
            }
            let high = !dorsal && black(theta, across);
            let peak_mm = if dorsal {
                DORSAL_MM
            } else if high {
                HIGH_MM
            } else {
                LOW_MM
            };
            // Salmon beads lie as flattened shingles everywhere, a bright pavement; the black bands' beads round up from half-domes on
            // the crest to full domes at the edges.
            let edge = ((across.abs() - SHINGLE_MM[0]) / (SHINGLE_MM[1] - SHINGLE_MM[0])).clamp(0.0, 1.0);
            let round = if dorsal {
                0.85
            } else if high {
                0.5 + 0.5 * edge
            } else {
                0.0
            };
            out.push(Bead {
                u: x * circ,
                v,
                rx: 0.5 * dia / (cs * a),
                ry: 0.5 * dia / st,
                peak_mm,
                round,
                high,
            });
        }
    }
    // The salmon bands: fine granulation on a lattice a `GRANULE` of the beads', low and round, filling every salmon bead's place
    // and kept off the black bands' beads and the dorsal row.
    let big: Vec<Bead> = out.into_iter().filter(|b| b.high || b.peak_mm >= DORSAL_MM).collect();
    let mut fine = Vec::new();
    let row = ROW_MM * GRANULE;
    let span = ((crest - EDGE_MM) / row).floor() as i64;
    for i in -span..=span {
        let v0 = crest + i as f64 * row;
        let a = ctx.arc_scale(v0);
        let n = (cols / GRANULE * a).round();
        let stagger = if i.rem_euclid(2) == 1 { 0.5 } else { 0.0 };
        for j in 0..n as i64 {
            let seed = ((i + 256) * 8192 + j) as u64 + 7_000_000;
            let phi = (j as f64 + stagger + 0.15 * hash(seed, 1)) / n;
            let x = g.x_of_phi(phi).rem_euclid(1.0);
            let theta = x * 360.0;
            let v = v0 + 0.08 * row * hash(seed, 2);
            let cs = ctx.crest_scale(theta);
            let st = ctx.station_stretch(theta);
            let around = circ / n * stretch_of_grade(x) * cs * a;
            let over = row * st * 2.0 / 3f64.sqrt();
            let dia = (1.0 - GRANULE_LAND) * around.min(over);
            let (rx, ry) = (0.5 * dia / (cs * a), 0.5 * dia / st);
            let du = ring_delta(theta, 90.0) / 360.0 * circ * cs;
            if du.hypot((v - crest) * st) < stone_r + 0.5 * dia {
                continue;
            }
            // Clear of every big bead's foot by a land.
            let u = x * circ;
            let clear = big.iter().all(|b| {
                let du = ((u - b.u + 1.5 * circ).rem_euclid(circ)) - 0.5 * circ;
                let (ex, ey) = (du / (b.rx + rx + 0.04), (v - b.v) / (b.ry + ry + 0.04));
                ex * ex + ey * ey >= 1.0
            });
            if !clear {
                continue;
            }
            fine.push(Bead { u, v, rx, ry, peak_mm: LOW_MM, round: 0.6, high: false });
        }
    }
    big.into_iter().chain(fine).collect()
}

/// Radius of the stone's mound that the beadwork keeps clear of, metal mm.
fn stone_clear_mm() -> f64 {
    let pad = stone_pad(0.0);
    0.5 * pad.diameter_mm + pad.blend_mm + 0.05
}

/// Dome buckets from shingle to full round.
const ROUNDS: usize = 5;

/// A bead's profile at `t` of its radius: a flattened shingle (a cushion with a broad flat top) blended toward a full dome.
fn profile(t: f64, round: f64) -> f64 {
    let dome = (1.0 - t * t).max(0.0).powf(0.7);
    let q = ((1.0 - t) / 0.45).clamp(0.0, 1.0);
    let shingle = q * q * (3.0 - 2.0 * q);
    shingle + (dome - shingle) * round
}

/// Sector `k`'s beads as one SVG over its stretch of the unrolled band: black ink is height.
fn sector_svg(d: &RingDesign, all: &[Bead], k: usize) -> String {
    let ctx = d.field_context();
    let circ = ctx.circumference_mm;
    let (w, h) = (circ / SECTORS as f64, ctx.band_v_len_mm);
    let u0 = k as f64 * w;
    let mut defs = String::new();
    let peaks = [HIGH_MM, LOW_MM, DORSAL_MM];
    for (c, peak) in peaks.iter().enumerate() {
        for r in 0..ROUNDS {
            let round = r as f64 / (ROUNDS - 1) as f64;
            let _ = write!(defs, r##"<radialGradient id="g{c}{r}" cx="0.5" cy="0.5" r="0.5">"##);
            for i in 0..=24 {
                let t = i as f64 / 24.0;
                let _ = write!(
                    defs,
                    r##"<stop offset="{t:.3}" stop-color="#000" stop-opacity="{:.4}"/>"##,
                    peak / BEAD_MM * profile(t, round)
                );
            }
            defs.push_str("</radialGradient>");
        }
    }
    let mut body = String::new();
    for b in all {
        for wrap in [-circ, 0.0, circ] {
            let x = b.u + wrap - u0;
            if x + b.rx < 0.0 || x - b.rx > w {
                continue;
            }
            // SVG y runs down from the decal's top, which is the band's far edge.
            let y = h - b.v;
            let c = if b.peak_mm >= DORSAL_MM {
                2
            } else if b.high {
                0
            } else {
                1
            };
            let r = (b.round * (ROUNDS - 1) as f64).round() as usize;
            let _ = write!(
                body,
                r##"<ellipse cx="{x:.4}" cy="{y:.4}" rx="{:.4}" ry="{:.4}" fill="url(#g{c}{r})"/>"##,
                b.rx, b.ry
            );
        }
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.4}" height="{h:.4}" viewBox="0 0 {w:.4} {h:.4}"><defs>{defs}</defs>{body}</svg>"##
    )
}

const ROMAN: [&str; 8] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII"];

fn stone_pad(crest_v: f64) -> SeatPadLayer {
    let mut pad = SeatPadLayer {
        theta_deg: 90.0,
        v_mm: crest_v,
        style: SeatStyle::GypsyMound,
        crown: 1.0,
        blend_mm: 0.45,
        solid: SolidKind::Bezel,
        through: true,
        ..Default::default()
    };
    pad.fit_stone(spessartite());
    pad.height_mm = 0.45;
    pad.set_depth_mm = Some(0.35);
    pad.mark_mm = 0.5;
    pad
}

/// The ring and its library, alphas baked.
fn design() -> Result<(RingDesign, AlphaLibrary)> {
    let mut d = band();
    let ctx = d.field_context();
    let all = beads(&d);
    let w = ctx.circumference_mm / SECTORS as f64;
    let mut layers = Vec::new();
    for k in 0..SECTORS {
        let name = format!("Gila beadwork {}", ROMAN[k]);
        d.svgs.push(SvgAlpha {
            name: name.clone(),
            svg: sector_svg(&d, &all, k),
            invert: false,
        });
        let dl = DecalLayer {
            alpha: name.clone(),
            decals: vec![Decal {
                theta_deg: (k as f64 + 0.5) * 360.0 / SECTORS as f64,
                v_mm: 0.5 * ctx.band_v_len_mm,
                size_mm: w,
                rotation_deg: 0.0,
                height_mm: BEAD_MM,
                flip: false,
            }],
            feather_mm: 0.0,
            invert: false,
        };
        let mut e = LayerEntry::new(&name, Layer::Decals(dl));
        e.blend = Blend::Max;
        e.window = Window::default();
        layers.push(e);
    }
    let mut stone = LayerEntry::new("Spessartite", Layer::SeatPad(stone_pad(ctx.crest_v_mm)));
    stone.blend = Blend::Max;
    stone.window = Window::default();
    layers.push(stone);
    d.layers.layers = layers;
    let mut lib = AlphaLibrary::builtin();
    d.bake_all(&mut lib);
    Ok((d, lib))
}

fn tile(path: &Path, images: &[Vec<u8>], edge: usize, cols: usize) -> Result<()> {
    let rows = images.len().div_ceil(cols);
    let (w, h) = (edge * cols, edge * rows);
    let mut out = vec![0u8; w * h * 3];
    for (k, img) in images.iter().enumerate() {
        let (cx, cy) = (k % cols, k / cols);
        for y in 0..edge {
            let dst = ((cy * edge + y) * w + cx * edge) * 3;
            out[dst..dst + edge * 3].copy_from_slice(&img[y * edge * 3..(y + 1) * edge * 3]);
        }
    }
    image::save_buffer(path, &out, w as u32, h as u32, image::ColorType::Rgb8)?;
    Ok(())
}

const HERO: (f64, f64) = (0.12, 1.2);

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", HERO.0, HERO.1),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, PI * 0.5),
    ("side", 0.0, 0.0),
    ("shoulder", -0.9, 0.62),
    ("reverse", 1.6, 0.8),
];

fn renders(
    out: &Path,
    d: &RingDesign,
    lib: &AlphaLibrary,
    built: &mesh::BuildResult,
    draft: bool,
) -> Result<()> {
    let gems = ringdesign_core::gems::built_meshes(d, lib, built);
    let parts: Vec<Part> = std::iter::once(Part::metal(&built.mesh, render::GOLD))
        .chain(gems.iter().map(|(m, t)| Part::tinted_stone(m, *t)))
        .collect();
    let edge = if draft { 1000 } else { 1600 };
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    if let Ok(list) = std::env::var("HELO_TRY") {
        let tries: Vec<Vec<u8>> = list
            .split(';')
            .map(|yp| {
                let v: Vec<f64> = yp.split(',').map(|x| x.parse().unwrap()).collect();
                render::render_parts_ss(&parts, v[0], v[1], 300, 300, 2)
            })
            .collect();
        tile(&out.join("tries.png"), &tries, 300, 4)?;
    }
    let small: Vec<Vec<u8>> = VIEWS
        .iter()
        .map(|(_, yaw, pitch)| render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 3))
        .collect();
    image::save_buffer(out.join("hero-300.png"), &small[0], 300, 300, image::ColorType::Rgb8)?;
    image::save_buffer(out.join("face-300.png"), &small[1], 300, 300, image::ColorType::Rgb8)?;
    tile(&out.join("contact-300.png"), &small, 300, 3)?;
    // The stone close-up: the swell's top from above and a little aft.
    render::write_png_parts(out.join("stones.png"), &parts, 0.25, 1.3, edge)?;
    let bare = band();
    let b = mesh::try_build(&bare, lib, params(true))?;
    let left = render::render_parts_ss(&[Part::metal(&b.mesh, render::GOLD)], HERO.0, HERO.1, edge, edge, 2);
    let right = render::render_parts_ss(&parts, HERO.0, HERO.1, edge, edge, 2);
    tile(&out.join("bare-vs-finished.png"), &[left, right], edge, 2)?;
    Ok(())
}

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid {
        v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(),
        f: m.faces.clone(),
    }
}

/// The lands the investment has to fill: every made part's thinnest section, and the relief's, against the 0.8 mm floor, each one
/// under it named with its bench treatment.
fn land_widths(d: &RingDesign) -> Result<(serde_json::Value, bool)> {
    let mut parts_json = Vec::new();
    let mut ok = true;
    for (stone, _) in ringdesign_core::stones::stone_frames(d) {
        let stand = stone.stand_off_mm();
        let fit = setting::Fit {
            surface_z: stone.seat.height_mm - stand,
            through_mm: None,
            prongs: 0,
        };
        let parts = setting::parts(stone.gem, stone.seat.solid, fit).map_err(|e| anyhow::anyhow!("{e:?}"))?;
        for (i, part) in parts.add.iter().enumerate() {
            let (min, under) = dfm::part_sections(part, Some([0.0, 0.0, 1.0]), MIN_SECTION_MM);
            let wall = setting::collet_wall_mm(stone.gem);
            parts_json.push(serde_json::json!({
                "part": format!("{} collet {}", stone.label, i + 1),
                "thinnest_section_mm": min,
                "area_under_floor_mm2": under,
                "collet_wall_mm": wall,
                "treatment": if min < MIN_SECTION_MM { "the collet's wall and lip: the lip is burnished over the spessartite's girdle at the bench; the wall below it stands on the mound, sunk 0.35 mm into it, so the investment fills it from the mound's own section" } else { "at or above the floor" },
            }));
        }
    }
    let ctx = d.field_context();
    let all = beads(d);
    // The narrowest metal a bead offers the investment, and the narrowest gap between bead feet.
    let finest_bead = all
        .iter()
        .filter(|b| b.high)
        .map(|b| 2.0 * (b.rx * ctx.arc_scale(b.v)).min(b.ry))
        .fold(f64::MAX, f64::min);
    ok &= finest_bead >= d.draft.min_detail_mm;
    let json = serde_json::json!({
        "floor_mm": MIN_SECTION_MM,
        "detail_floor_mm": d.draft.min_detail_mm,
        "made_parts": parts_json,
        "band_thinnest_wall": "field.thinnest_wall_mm, gated at the floor",
        "finest_full_bead_mm": finest_bead,
        "bead_note": "relief beads on the band, judged at the detail floor: each is a dome over at least the band's own 2.7 mm section",
        "bead_land_share_of_pitch": LAND,
    });
    Ok((json, ok))
}

/// Every gate on one build; the report block for it.
fn gates(d: &RingDesign, lib: &AlphaLibrary, p: BuildParams) -> Result<(mesh::BuildResult, serde_json::Value, bool)> {
    let t = Instant::now();
    let built = mesh::try_build(d, lib, p)?;
    let build_ms = t.elapsed().as_secs_f64() * 1e3;
    let v = &built.report.validation;
    let q = built.report.quality;
    let crossings = csg::self_crossings(&solid_of(&built.mesh));
    // Nothing inside the finger hole: every vertex at least the bore radius less 0.01 mm from the axis.
    let bore = d.inner_radius_mm();
    let inside = built
        .mesh
        .vertices
        .iter()
        .filter(|p| (p.0 as f64).hypot(p.1 as f64) < bore - 0.01)
        .count();
    let closest = built
        .mesh
        .vertices
        .iter()
        .map(|p| (p.0 as f64).hypot(p.1 as f64))
        .fold(f64::MAX, f64::min);
    let mut field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, d, &built);
    // The two-part pull, reported and not gated: what the same ring would lock in sand.
    let mut sand = d.draft.clone();
    sand.process = CastProcess::SandTwoPart;
    sand.min_draft_deg = 3.0;
    let two_part = castability::attributed_field_report(d, lib, &sand, 256, 128);
    let findings = dfm::findings_in(d, lib);
    let stones = ringdesign_core::stones::report_built(d, field.parting_z_mm, &built)
        .map_or(0, |r| r.stone_count as usize);
    let preview = ringdesign_core::gems::built_meshes(d, lib, &built).len();
    let pattern = mesh::try_build_pattern(d, lib, p)?;
    let pv = &pattern.report.validation;
    let pattern_crossings = csg::self_crossings(&solid_of(&pattern.mesh));
    let (lands, lands_ok) = land_widths(d)?;
    let list = [
        ("watertight, 0 degenerate faces", v.watertight && q.degenerate_faces == 0),
        ("0 self-crossings on the ring", crossings == 0),
        (
            "solids notes empty, every stamp resolved",
            built.solids.notes.is_empty() && built.parts.notes.is_empty() && built.solids.stamped == d.stamps.len(),
        ),
        ("nothing inside the finger hole", inside == 0),
        (
            "lost-wax field verdict Castable with the 0.8 mm fill",
            field.process == CastProcess::LostWax && field.verdict == Verdict::Castable && field.thinnest_wall_mm >= MIN_SECTION_MM,
        ),
        ("land widths at the floor or named", lands_ok),
        ("0 DFM findings", findings.is_empty()),
        ("stone count equals the preview", stones == preview && preview == 1),
        (
            "casting pattern watertight, 0 degenerate, 0 crossings",
            pv.watertight && pattern.report.quality.degenerate_faces == 0 && pattern_crossings == 0,
        ),
        ("within 2 million triangles", built.mesh.faces.len() <= 2_000_000),
    ];
    let pass = list.iter().all(|g| g.1);
    println!(
        "  {}x{}: {} tris in {:.0} ms; watertight {}, degenerate {}, crossings {crossings}; inside bore {inside}; field {} (thinnest {:.2} mm); two-part undercut {:.3}%; dfm {}; stones {stones}/{preview}; pattern {} {} {}",
        p.theta_steps,
        p.profile_steps,
        built.mesh.faces.len(),
        build_ms,
        v.watertight,
        q.degenerate_faces,
        field.verdict.label(),
        field.thinnest_wall_mm,
        two_part.undercut_fraction() * 100.0,
        findings.len(),
        pv.watertight,
        pattern.report.quality.degenerate_faces,
        pattern_crossings
    );
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    for n in field.notes.iter().chain(&built.solids.notes).chain(&built.parts.notes) {
        println!("    note: {n}");
    }
    for (g, ok) in &list {
        if !ok {
            println!("    gate FAILED: {g}");
        }
    }
    let block = serde_json::json!({
        "build": { "theta_steps": p.theta_steps, "profile_steps": p.profile_steps, "triangles": built.mesh.faces.len(), "ms": build_ms },
        "geometry": { "watertight": v.watertight, "boundary_edges": v.boundary_edges, "non_manifold_edges": v.non_manifold_edges, "degenerate_faces": q.degenerate_faces, "self_crossings": crossings, "min_angle_deg": q.min_angle_deg },
        "made": { "stamps": d.stamps.len(), "stamped": built.solids.stamped, "solids_notes": built.solids.notes, "parts_notes": built.parts.notes },
        "bore": { "bore_radius_mm": bore, "closest_vertex_mm": closest, "vertices_inside": inside },
        "field": { "process": format!("{:?}", field.process), "verdict": field.verdict.label(), "thinnest_wall_mm": field.thinnest_wall_mm, "thinnest_wall_theta_deg": field.thinnest_wall_theta_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm, "notes": field.notes },
        "two_part_undercut": { "gated": false, "undercut_percent": two_part.undercut_fraction() * 100.0, "worst_draft_deg": two_part.worst_draft_deg, "verdict_if_sand": two_part.verdict.label() },
        "land_widths": lands,
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": { "reported": stones, "previewed": preview },
        "pattern": { "watertight": pv.watertight, "degenerate_faces": pattern.report.quality.degenerate_faces, "self_crossings": pattern_crossings, "triangles": pattern.mesh.faces.len() },
        "gates": list.iter().map(|(g, ok)| serde_json::json!({ "gate": g, "pass": ok })).collect::<Vec<_>>(),
        "passed": pass,
    });
    Ok((built, block, pass))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/cataphracta").join(SLUG));
    std::fs::create_dir_all(&out)?;
    println!("{NAME}");
    let t = Instant::now();
    let (mut d, lib) = design()?;
    println!("  authored in {:.1} s", t.elapsed().as_secs_f64());
    if args.iter().any(|a| a == "--chart") {
        let ctx = d.field_context();
        println!(
            "  circ {:.2}, len {:.2}, crest {:.2}; stretch 90 {:.3} 270 {:.3}; crest_scale 90 {:.3} 270 {:.3}",
            ctx.circumference_mm,
            ctx.band_v_len_mm,
            ctx.crest_v_mm,
            ctx.station_stretch(90.0),
            ctx.station_stretch(270.0),
            ctx.crest_scale(90.0),
            ctx.crest_scale(270.0)
        );
        for i in 0..=20 {
            let v = i as f64 / 20.0 * ctx.band_v_len_mm;
            println!("  v {v:5.2}: arc {:.3} draft {:?}", ctx.arc_scale(v), ctx.draft_at(90.0, v));
        }
        let all = beads(&d);
        println!("  beads {}, high {}", all.len(), all.iter().filter(|b| b.high).count());
        // A flat map of the hide: high beads dark, low beads light, 10 px per chart mm.
        let (w, h) = ((ctx.circumference_mm * 10.0) as usize, (ctx.band_v_len_mm * 10.0) as usize);
        let mut img = vec![255u8; w * h];
        for b in &all {
            let (cx, cy) = (b.u * 10.0, b.v * 10.0);
            for y in 0..h {
                for x in 0..w {
                    let (dx, dy) = ((x as f64 - cx) / (b.rx * 10.0), (y as f64 - cy) / (b.ry * 10.0));
                    if dx * dx + dy * dy < 1.0 {
                        img[y * w + x] = if b.high { 30 } else { 180 };
                    }
                }
            }
        }
        image::save_buffer(out.join("hide-map.png"), &img, w as u32, h as u32, image::ColorType::L8)?;
        return Ok(());
    }
    let p = params(draft);
    d.build = p;
    // The draft block always; the export block unless drafting.
    let (draft_built, draft_block, draft_pass) = gates(&d, &lib, params(true))?;
    let (built, export_block, export_pass) = if draft {
        (draft_built, serde_json::Value::Null, true)
    } else {
        gates(&d, &lib, p)?
    };
    library::save_design(out.join("design.ring.json"), &d)?;
    let design_bytes = std::fs::metadata(out.join("design.ring.json"))?.len();
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, p)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices
            && rebuilt.mesh.faces == built.mesh.faces
            && rebuilt.mesh.normals == built.mesh.normals;
        println!(
            "  cold reload with an empty library: {}",
            if same { "identical vertices, faces and normals" } else { "CHANGED" }
        );
        Some(same)
    } else {
        None
    };
    let ctx = d.field_context();
    let report = serde_json::json!({
        "ring": NAME,
        "slug": SLUG,
        "process": d.draft.process.label(),
        "size": d.size.display(),
        "bore_mm": 2.0 * d.inner_radius_mm(),
        "design_bytes": design_bytes,
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "draft": draft_block,
        "export": export_block,
        "cold_reload_identical": cold,
        "chart": { "circumference_mm": ctx.circumference_mm, "band_v_len_mm": ctx.band_v_len_mm, "crest_v_mm": ctx.crest_v_mm },
        "gates_passed": draft_pass && export_pass && cold != Some(false),
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, NAME)?;
        let pattern = mesh::try_build_pattern(&d, &lib, p)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, &format!("{NAME} / casting pattern"))?;
        let mut stones = Vec::new();
        for (m, tint) in &ringdesign_core::gems::built_meshes(&d, &lib, &built) {
            stl::write_stl(out.join("reference-spessartite.stl"), m, "Heloderma reference spessartite")?;
            stones.push(serde_json::json!({ "mesh": "reference-spessartite.stl", "name": "Spessartite", "tint": tint, "ior": 1.80, "dispersion": 0.027, "roughness": 0.05, "transmission": 0.55 }));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&serde_json::json!({ "stones": stones }))?)?;
    }
    renders(&out, &d, &lib, &built, draft)?;
    let art = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/cataphracta/art").join(SLUG);
    let _ = std::fs::remove_dir_all(&art);
    std::fs::create_dir_all(&art)?;
    for s in &d.svgs {
        std::fs::write(art.join(format!("{}.svg", s.name.to_lowercase().replace(' ', "-"))), &s.svg)?;
    }
    ensure!(draft_pass && export_pass, "{NAME} failed its gates; see {}", out.join("report.json").display());
    Ok(())
}
