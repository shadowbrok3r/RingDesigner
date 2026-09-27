//! Cataphracta — Heloderma, the beaded one: a Gila monster's beadwork over a tail swollen with stored fat, poured in Delft clay.
//! cargo build --release -p ringdesign-core --example cataphracta_heloderma
//! target/release/examples/cataphracta_heloderma [OUT_DIR] [--draft] [--verify] [--bites]
use anyhow::{Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    castability::{self, CastProcess, SandProcess, Verdict},
    csg, dfm,
    field::{
        Blend, GroupLayer, Layer, LayerEntry, LayerStack, Remap, SandClamp, SeatPadLayer,
        SeatRunLayer, SeatStyle, Uv, VGate, Window,
    },
    gem::{Gem, GemCut},
    library, manufacturing as mf, mesh,
    profile::ShankKey,
    render::{self, Part},
    reptile::svg::{self as skins, Params},
    setting::SolidKind,
    skin, stl,
    svg::SvgAlpha,
    tiling::{GradeLaw, TileGrade, TilingLayer},
};
use std::f64::consts::PI;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

const SLUG: &str = "heloderma";
const NAME: &str = "Heloderma \u{2014} the beaded one";

/// The bead pitch round the ring at the swell's top, mm; the grade takes it down to 0.70 of that, 1.05, at the palm.
const PITCH_TOP_MM: f64 = 1.50;
const GRADE_TAPER: f64 = 0.30;
/// Metal between beads at the half-height line, as a share of the pitch: 0.44 mm at the palm's 1.05.
const LAND: f64 = 0.42;
/// Bead rows across the band in chart mm at the reference section: the swell's 1.25 stretch makes them a hexagon's 0.866 of the pitch.
const BEAD_ROW_REF_MM: f64 = 1.04;
/// Low (salmon) beads, near-smooth ground, and the black bands' full beads, mm.
const LOW_BEAD_MM: f64 = 0.02;
const HIGH_BEAD_MM: f64 = 0.36;
/// Transverse black bands round the ring, and the share of each period they cover.
const BANDS: usize = 16;
const BAND_SHARE: f64 = 0.5;
/// Half the crest ribbon, chart mm, where the band edges run straight across.
const RIBBON_MM: f64 = 0.8;

/// The crown's superellipse exponents.
const OGIVE: [f64; 2] = [1.1, 1.2];

/// How far each bead's peak stands off its centre toward the band's edge, as a share of its radius.
const BEAD_FOCUS: f64 = 0.3;

/// Salmon islands inside the black bands' flanks, off in the last block-out.
const ISLANDS: bool = false;

/// The live group clamp (C-R1). A slack under 1 leaves the field sampler a margin: at 1 the clamp holds walls at exactly zero draft
/// and the field reads them 1-2 degrees under.
const CLAMP: SandClamp = SandClamp {
    resolution: [2048, 768],
    slack: 0.8,
};

fn params(draft: bool) -> BuildParams {
    let (t, p) = if draft { (768, 320) } else { (1536, 448) };
    BuildParams {
        theta_steps: t,
        profile_steps: p,
        refine: None,
        ..Default::default()
    }
}

/// The keyed half-round: a fat-tail swell at the face, slim at the palm.
fn band() -> RingDesign {
    let mut d = RingDesign {
        name: NAME.into(),
        ..RingDesign::default()
    };
    d.profile.width_mm = 8.0;
    d.profile.thickness_mm = 3.2;
    d.profile.apply_style(ProfileStyle::HalfRound);
    // The half-round's crown sharpened to an ogive, a lizard's back in section: the flanks hold 28-40 degrees of draft from 0.4 mm
    // off the ridge to the edge, where the half-round keeps under 24 degrees for 2 mm either side of its crest, so a full bead
    // stands anywhere off the ridge and the dorsal row straddles the ridge itself.
    d.profile.shape_a = OGIVE[0];
    d.profile.shape_b = OGIVE[1];
    d.profile.style = ProfileStyle::Custom;
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
        k(35.0, 1.08, 1.10, 1.0),
        k(90.0, 1.22, 1.28, 1.05),
        k(145.0, 1.08, 1.10, 1.0),
        k(210.0, 0.96, 0.96, 1.0),
        k(270.0, 0.90, 0.92, 1.0),
        k(330.0, 0.96, 0.96, 1.0),
    ];
    let mut setup = mf::Setup::default();
    setup.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    setup.recipe.name = format!("{NAME} / Delft clay");
    setup.recipe.alloy = "Silver 925".into();
    setup.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925")
        .unwrap()
        .shrink_pct;
    setup.sample_pitch_mm = 0.1;
    setup.auto_parting = false;
    setup.parting_mm = 0.0;
    setup.flask.width_mm = 80.0;
    setup.flask.length_mm = 80.0;
    setup.channels = vec![
        mf::Channel {
            kind: mf::ChannelKind::Gate,
            start: [0.0, -11.0, 0.0],
            end: [0.0, -21.0, 0.0],
            diameter_mm: 4.0,
        },
        mf::Channel {
            kind: mf::ChannelKind::Sprue,
            start: [0.0, -21.0, 0.0],
            end: [0.0, -33.0, 0.0],
            diameter_mm: 6.0,
        },
    ];
    setup.bench_notes = "Procedural half-round, two-part Delft clay, Z=0 parting on the crest line. The Gila's beadwork: tall domed beads \
        in the black bands, low beads in the salmon bands, square pavers under the palm, a graded row of bare gypsy mounds down the spine \
        and a 3 mm spessartite flush-set in the swell's top mound. At the bench: drill the raised dot and cut the flush seat, set the stone; \
        deepen the bead lands on the flanks with a 0.06 mm graver; polish the high beads, leave the lands satin."
        .into();
    d.draft.process = setup.recipe.process;
    d.draft.sand = setup.recipe.sand;
    d.draft.min_detail_mm = setup.recipe.min_detail_mm;
    d.draft.min_section_mm = setup.recipe.min_section_mm;
    d.draft.min_draft_deg = setup.recipe.min_draft_deg;
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

/// Tiles round the ring that put `cell_mm` of chart at the swell's top.
fn repeats_for(d: &RingDesign, cell_mm: f64) -> u32 {
    let ctx = d.field_context();
    let g = grade();
    let x = 0.25;
    let e = 1e-4;
    let dphi = (g.phi(x + e) - g.phi(x - e)) / (2.0 * e);
    (ctx.circumference_mm / (cell_mm * dphi)).round() as u32
}

/// The two-by-two bead cell over the crest-to-edge half, mirrored onto the other.
fn bead_tiling(d: &RingDesign, alpha: &str, height_mm: f64) -> TilingLayer {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for(alpha, &ctx);
    // The crest line is the dorsal row's; the lattice's first row stands a row pitch off it.
    let lo = ctx.crest_v_mm;
    let hi = ctx.band_v_len_mm - 0.15;
    let cell_v = BEAD_ROW_REF_MM * 2.0;
    t.rows = ((hi - lo) / cell_v).ceil() as u32 + 1;
    t.v_span_mm = t.rows as f64 * cell_v;
    t.v_center_mm = lo + 0.5 * t.v_span_mm;
    t.offset_v = 0.25;
    t.repeats_around = repeats_for(d, 2.0 * PITCH_TOP_MM);
    t.mirror_v = true;
    t.offset_u = 0.0;
    t.height_mm = height_mm;
    t.feather_mm = 0.0;
    t.continuous = true;
    t.grade = Some(grade());
    t
}

/// Heloderma's beads, the ring's own `bead_lattice`: the same two-by-two hexagon-staggered cell, but each bead falls as a raised
/// cosine from a peak set `BEAD_FOCUS` of its radius toward the band's edge. Its crest-side flank is a long ramp no steeper than
/// the ogive's draft allows, and its edge-side flank drops steeply away from the parting line, where a drop pulls. A quarter-circle
/// dome stands vertical at its rim, which no flank short of a side face will release.
fn bead_svg() -> String {
    let (w, h) = (2.0 * PITCH_TOP_MM, PITCH_TOP_MM * 3f64.sqrt());
    // The half-height line of a raised cosine sits halfway out: the bead is `d` across there.
    let d = PITCH_TOP_MM * (1.0 - LAND);
    let r = d;
    let f = 0.5 + 0.5 * BEAD_FOCUS.clamp(-0.9, 0.9);
    let mut defs =
        format!(r##"<radialGradient id="b" cx="0.5" cy="0.5" r="0.5" fx="0.5" fy="{f:.4}">"##);
    for k in 0..=16 {
        let t = k as f64 / 16.0;
        let _ = write!(
            defs,
            r##"<stop offset="{t:.4}" stop-color="#000" stop-opacity="{:.4}"/>"##,
            0.5 + 0.5 * (PI * t).cos()
        );
    }
    defs.push_str("</radialGradient>");
    let mut body = String::new();
    let pts: Vec<[f64; 2]> = (0..2)
        .flat_map(|j| {
            (0..2).map(move |i| {
                [
                    (i as f64 + 0.25 + 0.5 * (j % 2) as f64) * w / 2.0,
                    (j as f64 + 0.5) * h / 2.0,
                ]
            })
        })
        .collect();
    for q in pts {
        for dx in [-w, 0.0, w] {
            for dy in [-h, 0.0, h] {
                let (cx, cy) = (q[0] + dx, q[1] + dy);
                if cx + r > 0.0 && cx - r < w && cy + r > 0.0 && cy - r < h {
                    let _ = write!(
                        body,
                        r##"<circle cx="{cx:.4}" cy="{cy:.4}" r="{r:.4}" fill="url(#b)"/>"##
                    );
                }
            }
        }
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.4}" height="{h:.4}" viewBox="0 0 {w:.4} {h:.4}"><defs>{defs}</defs>{body}</svg>"##
    )
}

/// The Gila's banding over the whole unrolled band, white where the black beadwork stands: transverse bands that run straight across
/// the crest ribbon (G4) and wander, fork and break into islands on the flanks, drawn at the grade's own stations so each band keeps
/// its share of the period round the ring. The stone's station falls mid-gap, in salmon.
fn reticulation_svg(d: &RingDesign) -> String {
    let ctx = d.field_context();
    let (w, h) = (ctx.circumference_mm, ctx.band_v_len_mm);
    let g = grade();
    let crest = ctx.crest_v_mm;
    let jit = |k: usize, s: u64| {
        let mut z = (k as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ s.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 29)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z = z ^ (z >> 32);
        ((z >> 11) as f64 / (1u64 << 53) as f64) - 0.5
    };
    let n = BANDS as f64;
    // A lattice fraction to mm round the ring, unwrapped near `near`.
    let x_at = |f: f64, near: f64| {
        let x = g.x_of_phi(f).rem_euclid(1.0) * w;
        if x - near > 0.5 * w {
            x - w
        } else if near - x > 0.5 * w {
            x + w
        } else {
            x
        }
    };
    let mut body = String::new();
    let _ = write!(
        body,
        r##"<defs><filter id="soft" x="-0.1" y="-0.3" width="1.2" height="1.6"><feGaussianBlur stdDeviation="0.5"/></filter></defs>"##
    );
    let _ = write!(
        body,
        r##"<rect width="{w:.3}" height="{h:.3}" fill="#000"/><g filter="url(#soft)">"##
    );
    let steps = 60;
    for k in 0..BANDS {
        let c = k as f64 / n;
        let xc = x_at(c, 0.0);
        let half = (0.5 * BAND_SHARE + 0.06 * jit(k, 1)) / n;
        // Off the ribbon the edges wander by up to 0.45 of the half-width, differently on each flank and each side.
        let edge = |y: f64, side: f64| {
            let off = smooth(RIBBON_MM, RIBBON_MM + 1.2, (y - crest).abs());
            let fl = if y > crest { 1.0 } else { 0.0 };
            let s = if side > 0.0 { 5u64 } else { 3 };
            let wave = (y * (1.1 + 0.5 * jit(k, s + 20)) + 6.0 * jit(k, s + 30) + fl * 2.0).sin();
            c + side * half * (1.0 + off * (0.30 * wave + 0.30 * jit(k, s + 40 + fl as u64)))
        };
        let mut pts = Vec::new();
        for i in 0..=steps {
            let y = i as f64 / steps as f64 * h;
            pts.push((x_at(edge(y, -1.0), xc), y));
        }
        for i in (0..=steps).rev() {
            let y = i as f64 / steps as f64 * h;
            pts.push((x_at(edge(y, 1.0), xc), y));
        }
        for dx in [-w, 0.0, w] {
            body.push_str(r##"<polygon fill="#fff" points=""##);
            for (x, y) in &pts {
                let _ = write!(body, "{:.3},{:.3} ", x + dx, y);
            }
            body.push_str(r##""/>"##);
        }
        for (side, s) in [(-1.0f64, 11u64), (1.0, 13u64)] {
            // A salmon island in the band's flank.
            if ISLANDS && jit(k, s + 50) > -0.2 {
                let y = crest + side * (3.1 + 0.8 * jit(k, s));
                let x = x_at(c + 0.3 * half * jit(k, s + 2), xc);
                let rx = 0.32 * (x_at(c + half, xc) - x_at(c - half, xc));
                let _ = write!(
                    body,
                    r##"<ellipse cx="{x:.3}" cy="{y:.3}" rx="{rx:.3}" ry="{:.3}" fill="#000"/>"##,
                    0.75 + 0.2 * jit(k, s + 4)
                );
            }
            // A fork reaching across the salmon gap to the next band, on alternate flanks.
            if (k + s as usize) % 2 == 0 {
                let y = crest + side * (2.2 + 1.2 * (0.5 + jit(k, s + 6)));
                let (x0, x1) = (x_at(c + half, xc), x_at(c + 1.0 / n - half, xc));
                let x1 = if x1 < x0 { x1 + w } else { x1 };
                let bow = side * (0.4 + 0.5 * jit(k, s + 8));
                for dx in [-w, 0.0, w] {
                    let _ = write!(
                        body,
                        r##"<path d="M{:.3},{y:.3} Q{:.3},{:.3} {:.3},{:.3}" stroke="#fff" stroke-width="1.1" fill="none" stroke-linecap="round"/>"##,
                        x0 - 0.3 + dx,
                        0.5 * (x0 + x1) + dx,
                        y + bow,
                        x1 + 0.3 + dx,
                        y + 0.6 * bow
                    );
                }
            }
        }
    }
    body.push_str("</g>");
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.3}" height="{h:.3}" viewBox="0 0 {w:.3} {h:.3}">{body}</svg>"##
    )
}

fn smooth(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn paver_svg(pitch: f64) -> String {
    skins::paver(&Params::new(pitch, pitch, 0.45, 0.35))
}

fn entry(name: &str, layer: Layer, blend: Blend, soft: f64, window: Window) -> LayerEntry {
    let mut e = LayerEntry::new(name, layer);
    e.blend = blend;
    e.soft_mm = soft;
    e.window = window;
    e
}

fn window(theta: f64, span: f64, fade: f64) -> Window {
    Window {
        fade_deg: fade,
        ..Window::around(theta, span)
    }
}

/// The ring and its library, alphas and clamp baked.
fn design() -> Result<(RingDesign, AlphaLibrary, Vec<(String, skin::ClampReport)>)> {
    let mut d = band();
    let ctx = d.field_context();
    d.svgs.push(SvgAlpha {
        name: "Bead lattice".into(),
        svg: bead_svg(),
        invert: false,
    });
    d.svgs.push(SvgAlpha {
        name: "Reticulation".into(),
        svg: reticulation_svg(&d),
        invert: false,
    });
    d.svgs.push(SvgAlpha {
        name: "Belly paver".into(),
        svg: paver_svg(1.0),
        invert: false,
    });

    // Beads rise with the base draft: none on the crest ribbon, where the dorsal row stands, full by 25 degrees.
    let bead_gate = VGate::Draft {
        min_deg: 3.0,
        fade_deg: 22.0,
    };
    // Everything but the stone's mound and the palm's pavers.
    let beads_window = Window {
        fade_deg: 4.0,
        ..Window::except(90.0, 14.0)
    };

    let mut low = entry(
        "Low beads \u{2014} salmon bands",
        Layer::Tiling(bead_tiling(&d, "Bead lattice", LOW_BEAD_MM)),
        Blend::Add,
        0.18,
        beads_window,
    );
    low.window.v_gate = bead_gate;
    let lift = HIGH_BEAD_MM - LOW_BEAD_MM;
    let mut high = entry(
        "High beads \u{2014} black bands",
        Layer::Tiling(bead_tiling(&d, "Bead lattice", lift)),
        Blend::Add,
        0.18,
        beads_window,
    );
    high.window.v_gate = bead_gate;
    high.mask = Some("Reticulation".into());

    // Belly pavers: rounded squares in transverse rows under the palm, in place of the beads there.
    let mut pav = TilingLayer::default_for("Belly paver", &ctx);
    pav.v_center_mm = 0.5 * ctx.band_v_len_mm;
    pav.v_span_mm = ctx.band_v_len_mm - 0.3;
    pav.rows = (pav.v_span_mm / 1.08).round() as u32;
    pav.repeats_around = 78;
    pav.height_mm = 0.24;
    pav.feather_mm = 0.0;
    let mut pavers = entry(
        "Belly pavers",
        Layer::Tiling(pav),
        Blend::SmoothMax,
        0.12,
        window(270.0, 70.0, 16.0),
    );
    pavers.remap = Remap::Terrace {
        steps: 1,
        span_mm: 0.20,
        riser: 0.35,
    };
    pavers.window.v_gate = VGate::Draft {
        min_deg: 6.0,
        fade_deg: 26.0,
    };

    let mut body = LayerStack {
        layers: vec![low, high],
    };
    for e in &mut body.layers {
        // The pavers take the palm.
        e.window = Window {
            fade_deg: 16.0,
            ..Window::except(270.0, 70.0)
        };
        e.window.v_gate = bead_gate;
    }
    let beads = entry(
        "Beads",
        Layer::Group(GroupLayer {
            stack: body,
            recipe: None,
            clamp: None,
        }),
        Blend::Max,
        0.3,
        beads_window,
    );
    let group = entry(
        "Beadwork",
        Layer::Group(GroupLayer {
            stack: LayerStack {
                layers: vec![beads, pavers],
            },
            recipe: None,
            clamp: Some(CLAMP),
        }),
        Blend::Max,
        0.3,
        Window::default(),
    );

    // The dorsal bead row: bare gypsy mounds on the crest line, graded from the swell to the palm.
    let seat = SeatPadLayer {
        theta_deg: 90.0,
        style: SeatStyle::GypsyMound,
        diameter_mm: 1.8,
        height_mm: 0.6,
        blend_mm: 0.2,
        crown: 1.0,
        v_mm: ctx.crest_v_mm,
        ..Default::default()
    };
    let mut run = SeatRunLayer {
        seat,
        bare: true,
        taper: 0.35,
        taper_theta_deg: 90.0,
        bridge_mm: 0.35,
        centre_phase: Some(0.0),
        ..Default::default()
    };
    run.solve_spacing(&ctx);
    // Added over the beadwork, so each mound stands its own height over a saddle or a gap alike: a crest-symmetric mound on a field
    // that varies only round the ring there still pulls.
    let dorsal = entry(
        "Dorsal bead row",
        Layer::SeatRun(run),
        Blend::Add,
        0.3,
        Window {
            fade_deg: 2.0,
            ..Window::except(90.0, 22.0)
        },
    );

    // The spessartite: the swell's top bead, a gypsy mound flush-set.
    let gem = spessartite();
    let mut pad = SeatPadLayer {
        theta_deg: 90.0,
        v_mm: ctx.crest_v_mm,
        style: SeatStyle::GypsyMound,
        crown: 1.0,
        blend_mm: 0.45,
        solid: SolidKind::Bezel,
        through: true,
        ..Default::default()
    };
    pad.fit_stone(gem);
    pad.height_mm = 0.45;
    pad.set_depth_mm = Some(0.35);

    pad.mark_mm = 0.5;
    let stone = entry(
        "Spessartite",
        Layer::SeatPad(pad),
        Blend::Max,
        0.3,
        Window::default(),
    );

    d.layers.layers = vec![group, dorsal, stone];
    let mut lib = AlphaLibrary::builtin();
    d.bake_all(&mut lib);
    let bites = d.bake_clamps(&mut lib);
    Ok((d, lib, bites))
}

/// Where the clamp bites: the group's composite painted unclamped over its atlas, clamped, and the cut reported by column and row.
fn bite_map(d: &RingDesign, lib: &AlphaLibrary, out: &Path) -> Result<()> {
    let mut loose = d.clone();
    loose
        .layers
        .layers
        .retain(|e| matches!(e.layer, Layer::Group(_)));
    for e in &mut loose.layers.layers {
        if let Layer::Group(g) = &mut e.layer {
            g.clamp = None;
            if let Ok(only) = std::env::var("HELO_ONLY") {
                let keep: Vec<usize> = only.split(',').filter_map(|x| x.parse().ok()).collect();
                let mut i = 0;
                g.stack.layers.retain(|_| {
                    i += 1;
                    keep.contains(&(i - 1))
                });
            }
        }
    }
    let (w, h) = (1024usize, 384usize);
    let a = skin::Atlas::of(d, w, h)?;
    let ctx = d.field_context();
    let comp: Vec<f64> = a
        .samples
        .iter()
        .map(|s| {
            loose.layers.height(
                Uv {
                    u: s.theta / 360.0 * ctx.circumference_mm,
                    v: s.v,
                },
                &ctx,
                lib,
            )
        })
        .collect();
    let top = comp.iter().copied().fold(1e-6, f64::max);
    let mut alpha = Alpha::new(
        "bite",
        w,
        h,
        comp.iter()
            .map(|x| (x / top).clamp(0.0, 1.0) as f32)
            .collect(),
    );
    let before = alpha.data.clone();
    let r = skin::draft_clamp(&a, &mut alpha, top)?;
    println!(
        "  bite map: worst {:.3} mm over {} texels",
        r.worst_mm, r.texels_cut
    );
    let cut: Vec<f64> = before
        .iter()
        .zip(&alpha.data)
        .map(|(b, c)| (b - c) as f64 * top)
        .collect();
    let mut worst: Vec<(f64, f64, f64, f64)> = a
        .samples
        .iter()
        .map(|s| {
            (
                cut[s.i],
                s.theta,
                s.v - ctx.crest_v_mm,
                ctx.draft_at(s.theta, s.v).unwrap_or(-1.0),
            )
        })
        .filter(|c| c.0 > 0.02)
        .collect();
    worst.sort_by(|p, q| q.0.total_cmp(&p.0));
    for (c, t, v, dr) in worst.iter().take(12) {
        println!("    cut {c:.3} at theta {t:.1}, {v:+.2} mm off crest, draft {dr:.0}");
    }
    let img: Vec<u8> = (0..w * h)
        .flat_map(|i| {
            let c = (cut[i] / 0.1).clamp(0.0, 1.0);
            let g = (comp[i] / top * 160.0) as u8;
            [((c * 255.0) as u8).max(g), g, g]
        })
        .collect();
    image::save_buffer(
        out.join("bite-map.png"),
        &img,
        w as u32,
        h as u32,
        image::ColorType::Rgb8,
    )?;
    Ok(())
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

    image::save_buffer(
        out.join("hero-300.png"),
        &small[0],
        300,
        300,
        image::ColorType::Rgb8,
    )?;
    image::save_buffer(
        out.join("face-300.png"),
        &small[1],
        300,
        300,
        image::ColorType::Rgb8,
    )?;
    tile(&out.join("contact-300.png"), &small, 300, 3)?;
    // The stone close-up: the swell's top from above and a little aft.
    render::write_png_parts(out.join("stones.png"), &parts, 0.25, 1.3, edge)?;
    let bare = band();
    let b = mesh::try_build(&bare, lib, params(true))?;
    let left = render::render_parts_ss(
        &[Part::metal(&b.mesh, render::GOLD)],
        HERO.0,
        HERO.1,
        edge,
        edge,
        2,
    );
    let right = render::render_parts_ss(&parts, HERO.0, HERO.1, edge, edge, 2);
    tile(&out.join("bare-vs-finished.png"), &[left, right], edge, 2)?;
    Ok(())
}

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid {
        v: m.vertices
            .iter()
            .map(|p| [p.0 as f64, p.1 as f64, p.2 as f64])
            .collect(),
        f: m.faces.clone(),
    }
}

fn release_json(r: &mf::release::ReleaseReport) -> serde_json::Value {
    serde_json::json!({ "status": format!("{:?}", r.status), "obstructions": r.obstructions.len(), "unresolved_rays": r.unresolved_rays })
}

/// Every gate on one build; the report block for it.
fn gates(
    d: &RingDesign,
    lib: &AlphaLibrary,
    bites: &[(String, skin::ClampReport)],
    p: BuildParams,
) -> Result<(mesh::BuildResult, serde_json::Value, bool)> {
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
    let field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    let findings = dfm::findings_in(d, lib);
    let stones = ringdesign_core::stones::report(d, field.parting_z_mm)
        .map_or(0, |r| r.stone_count as usize);
    let preview = ringdesign_core::gems::built_meshes(d, lib, &built).len();
    let setup = d.manufacturing.clone().unwrap();
    let inspection = mf::inspect(d, lib, &setup, p)?;
    let mut fine = setup.clone();
    fine.sample_pitch_mm = 0.075;
    let rf = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
    let r = &inspection.release;
    let pattern = mesh::try_build_pattern(d, lib, p)?;
    let pv = &pattern.report.validation;
    let pattern_crossings = csg::self_crossings(&solid_of(&pattern.mesh));
    let worst_bite = bites.iter().map(|b| b.1.worst_mm).fold(0.0, f64::max);
    let drag = 100.0 * (field.marginal_area_mm2 + field.vertical_area_mm2)
        / field.total_area_mm2.max(1e-9);
    let list = [
        (
            "watertight, 0 degenerate faces",
            v.watertight && q.degenerate_faces == 0,
        ),
        ("0 self-crossings on the ring", crossings == 0),
        (
            "solids notes empty, every stamp resolved",
            built.solids.notes.is_empty() && built.solids.stamped == d.stamps.len(),
        ),
        ("nothing inside the finger hole", inside == 0),
        (
            "field Castable (sand, two-part)",
            field.process == CastProcess::SandTwoPart && field.verdict == Verdict::Castable,
        ),
        (
            "ray release clean at 0.100 mm",
            r.obstructions.is_empty() && r.unresolved_rays == 0,
        ),
        (
            "ray release clean at 0.075 mm",
            rf.obstructions.is_empty() && rf.unresolved_rays == 0,
        ),
        ("every draft-clamp bite at most 0.05 mm", worst_bite <= 0.05),
        ("0 DFM findings", findings.is_empty()),
        ("stone count equals the preview", stones == preview),
        (
            "casting pattern watertight, 0 degenerate, 0 crossings",
            pv.watertight && pattern.report.quality.degenerate_faces == 0 && pattern_crossings == 0,
        ),
        (
            "within 2 million triangles",
            built.mesh.faces.len() <= 2_000_000,
        ),
    ];
    let pass = list.iter().all(|g| g.1);
    println!(
        "  {}x{}: {} tris in {:.0} ms; watertight {}, degenerate {}, crossings {crossings}; inside bore {inside}; field {} ({:.4}% undercut, drag {drag:.1}%); release {}/{} and {}/{}; bite {worst_bite:.3}; dfm {}; stones {stones}/{preview}; pattern {} {} {}",
        p.theta_steps,
        p.profile_steps,
        built.mesh.faces.len(),
        build_ms,
        v.watertight,
        q.degenerate_faces,
        field.verdict.label(),
        field.undercut_fraction() * 100.0,
        r.obstructions.len(),
        r.unresolved_rays,
        rf.obstructions.len(),
        rf.unresolved_rays,
        findings.len(),
        pv.watertight,
        pattern.report.quality.degenerate_faces,
        pattern_crossings
    );
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    for n in field.notes.iter().chain(&built.solids.notes) {
        println!("    note: {n}");
    }
    for o in r.obstructions.iter().chain(&rf.obstructions).take(10) {
        let th = o.world[1].atan2(o.world[0]).to_degrees().rem_euclid(360.0);
        println!(
            "    obstruction {:.3} mm deep at theta {th:.1}, z {:.2}, r {:.2}",
            o.depth_mm,
            o.world[2],
            o.world[0].hypot(o.world[1])
        );
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
        "field": { "process": format!("{:?}", field.process), "verdict": field.verdict.label(), "undercut_percent": field.undercut_fraction() * 100.0, "worst_draft_deg": field.worst_draft_deg, "thinnest_wall_mm": field.thinnest_wall_mm, "drag_percent": drag, "notes": field.notes },
        "release_0100": release_json(r),
        "release_0075": release_json(&rf),
        "clamp": bites.iter().map(|(n, c)| serde_json::json!({ "group": n, "texels_cut": c.texels_cut, "worst_mm": c.worst_mm })).collect::<Vec<_>>(),
        "clamp_worst_mm": worst_bite,
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
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../showcase/cataphracta")
                .join(SLUG)
        });
    std::fs::create_dir_all(&out)?;
    println!("{NAME}");
    let t = Instant::now();
    let (mut d, lib, bites) = design()?;
    println!(
        "  authored in {:.1} s; clamp {:?}",
        t.elapsed().as_secs_f64(),
        bites
            .iter()
            .map(|(n, c)| (n, c.texels_cut, (c.worst_mm * 1000.0).round() / 1000.0))
            .collect::<Vec<_>>()
    );
    if args.iter().any(|a| a == "--mask") {
        let ctx = d.field_context();
        let a = lib.get("Reticulation").expect("reticulation");
        println!("  reticulation {}x{}", a.width, a.height);
        for i in 0..36 {
            let th = i as f64 * 10.0;
            let uv = Uv {
                u: th / 360.0 * ctx.circumference_mm,
                v: ctx.crest_v_mm + 2.5,
            };
            let x = uv.u / ctx.circumference_mm;
            let y = uv.v / ctx.band_v_len_mm;
            println!("  {th:5.1}: mask {:.2}", a.sample_wrapped(x, y));
        }
        let img: Vec<u8> = a.data.iter().map(|v| (v * 255.0) as u8).collect();
        image::save_buffer(
            out.join("reticulation.png"),
            &img,
            a.width as u32,
            a.height as u32,
            image::ColorType::L8,
        )?;
        return Ok(());
    }
    if args.iter().any(|a| a == "--crest") {
        let ctx = d.field_context();

        for i in 0..60 {
            let th = 20.0 + i as f64 * 1.0;
            let uv = Uv {
                u: th / 360.0 * ctx.circumference_mm,
                v: ctx.crest_v_mm,
            };
            let hs: Vec<String> = d
                .layers
                .layers
                .iter()
                .map(|e| {
                    format!(
                        "{:.3}",
                        e.layer.height(uv, &ctx, &lib) * e.mask_at(uv, &ctx, &lib)
                    )
                })
                .collect();
            println!("  {th:5.1}: {}", hs.join(" "));
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--bites") {
        return bite_map(&d, &lib, &out);
    }
    let p = params(draft);
    d.build = p;
    // The draft block always; the export block unless drafting.
    let (draft_built, draft_block, draft_pass) = gates(&d, &lib, &bites, params(true))?;
    let (built, export_block, export_pass) = if draft {
        (draft_built, serde_json::Value::Null, true)
    } else {
        gates(&d, &lib, &bites, p)?
    };
    let coarse = if !d.stamps.is_empty() {
        Some(
            gates(
                &d,
                &lib,
                &bites,
                BuildParams {
                    theta_steps: 384,
                    profile_steps: 192,
                    ..p
                },
            )?
            .1,
        )
    } else {
        None
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
            if same {
                "identical vertices, faces and normals"
            } else {
                "CHANGED"
            }
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
        "sand": format!("{:?}", d.draft.sand),
        "size": d.size.display(),
        "bore_mm": 2.0 * d.inner_radius_mm(),
        "design_bytes": design_bytes,
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "draft": draft_block,
        "export": export_block,
        "coarse_384x192": coarse,
        "cold_reload_identical": cold,
        "chart": { "circumference_mm": ctx.circumference_mm, "band_v_len_mm": ctx.band_v_len_mm, "crest_v_mm": ctx.crest_v_mm },
        "gates_passed": draft_pass && export_pass && cold != Some(false),
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, NAME)?;
        let pattern = mesh::try_build_pattern(&d, &lib, p)?;
        stl::write_stl(
            out.join("casting-pattern.stl"),
            &pattern.mesh,
            &format!("{NAME} / casting pattern"),
        )?;
        let mut stones = Vec::new();
        for (m, tint) in &ringdesign_core::gems::built_meshes(&d, &lib, &built) {
            stl::write_stl(
                out.join("reference-spessartite.stl"),
                m,
                "Heloderma reference spessartite",
            )?;
            stones.push(serde_json::json!({ "mesh": "reference-spessartite.stl", "name": "Spessartite", "tint": tint, "ior": 1.80, "dispersion": 0.027, "roughness": 0.05, "transmission": 0.55 }));
        }
        std::fs::write(
            out.join("stones.json"),
            serde_json::to_vec_pretty(&serde_json::json!({ "stones": stones }))?,
        )?;
    }
    renders(&out, &d, &lib, &built, draft)?;
    let art = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples/cataphracta/art")
        .join(SLUG);
    std::fs::create_dir_all(&art)?;
    for s in &d.svgs {
        std::fs::write(
            art.join(format!("{}.svg", s.name.to_lowercase().replace(' ', "-"))),
            &s.svg,
        )?;
    }
    ensure!(
        draft_pass && export_pass,
        "{NAME} failed its gates; see {}",
        out.join("report.json").display()
    );
    Ok(())
}
