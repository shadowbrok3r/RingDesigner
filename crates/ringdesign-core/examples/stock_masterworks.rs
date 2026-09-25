//! Fresh compositions drawn on imported stock, never transplanted procedural layers.
//! cargo run -p ringdesign-core --release --example stock_masterworks -- NEW_DIR [--draft] [SLUG]
use anyhow::{Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    alpha::{ProcRecipe, Procedural},
    castability::{CastProcess, SandProcess},
    curve::{CurveLayer, WireProfile},
    drawn::{DrawnAlpha, Stroke},
    field::{
        Blend, BorderLayer, BorderProfile, Decal, DecalLayer, FluteProfile, FlutesLayer,
        GroupLayer, Layer, LayerEntry, LayerStack, MilgrainLayer, SeatPadLayer, SeatStyle, VGate,
        Window, smoothstep,
    },
    gem::{Gem, GemCut},
    imported_base::{ImportedBase, PRESETS, SurfaceChart},
    manufacturing as mf,
    render::{self, Part},
    svg::SvgAlpha,
    text::{TextAlpha, TextFont},
    tiling::{TilingLayer, WarpField},
};
use std::{
    f64::consts::{PI, TAU},
    path::Path,
};

const AW: usize = 2048;
const AH: usize = 768;

#[derive(Clone, Copy, Default)]
struct Sample {
    p: [f64; 3],
    n: [f64; 3],
    theta: f64,
    v: f64,
    /// Where it sits in the atlas.
    i: usize,
}
struct Atlas {
    samples: Vec<Sample>,
    top: f64,
    bore: f64,
    length: f64,
    width: f64,
}
impl Atlas {
    fn new(d: &RingDesign) -> Result<Self> {
        let b = d.imported_base.as_ref().unwrap();
        let span = d.reference_loop().surface_len_mm;
        let surface = b.field_surface(d)?;
        let mut samples = vec![Sample::default(); AW * AH];
        for x in 0..AW {
            let theta = x as f64 / AW as f64 * 360.;
            for y in 0..AH {
                let fraction = y as f64 / AH as f64;
                samples[y * AW + x] = Sample {
                    p: surface.point(theta, fraction),
                    theta,
                    v: fraction * span,
                    i: y * AW + x,
                    ..Default::default()
                };
            }
        }
        let top = samples.iter().map(|s| s.p[1]).fold(0_f64, f64::max);
        for y in 1..AH - 1 {
            for x in 0..AW {
                let a = samples[y * AW + (x + 1) % AW].p;
                let b = samples[y * AW + (x + AW - 1) % AW].p;
                let c = samples[(y + 1) * AW + x].p;
                let e = samples[(y - 1) * AW + x].p;
                let u: [f64; 3] = std::array::from_fn(|i| a[i] - b[i]);
                let v: [f64; 3] = std::array::from_fn(|i| c[i] - e[i]);
                let n = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                let l = n.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-9);
                samples[y * AW + x].n = n.map(|a| a / l);
            }
        }
        Ok(Self {
            samples,
            top,
            bore: d.inner_radius_mm(),
            length: d.shank.head.length_mm,
            width: d.profile.width_mm,
        })
    }
    fn face(&self, s: Sample) -> f64 {
        let a = s.p[0].abs() / (self.length * 0.5);
        let b = s.p[2].abs() / (self.width * 0.5);
        let q = a.max(b).max((a + b) / 1.68);
        smoothstep(0.91, 0.985, s.n[1])
            * (1. - smoothstep(0.82, 0.90, q))
            * smoothstep(self.top - 3., self.top - 2., s.p[1])
    }
    fn cheek(&self, s: Sample) -> f64 {
        let r = s.p[0].hypot(s.p[1]);
        smoothstep(0.65, 0.9, s.n[2].abs())
            * smoothstep(self.bore + 0.8, self.bore + 1.45, r)
            * (1. - smoothstep(self.top - 1.1, self.top - 0.55, s.p[1]))
            * smoothstep(-3., 2., s.p[1])
    }
    fn shoulder(&self, s: Sample) -> f64 {
        smoothstep(0.38, 0.68, s.n[0].abs())
            * (1. - smoothstep(0.28, 0.48, s.n[2].abs()))
            * smoothstep(-1., 1.2, s.p[1])
            * (1. - smoothstep(self.top - 3.5, self.top - 1.9, s.p[1]))
            * smoothstep(self.bore + 0.8, self.bore + 1.3, s.p[0].hypot(s.p[1]))
    }
    fn alpha(&self, name: &str, f: impl Fn(Sample) -> f64) -> Alpha {
        Alpha::new(
            name,
            AW,
            AH,
            self.samples
                .iter()
                .map(|&s| f(s).clamp(0., 1.) as f32)
                .collect(),
        )
    }
}
fn portable(lib: &mut AlphaLibrary, a: Alpha) {
    lib.insert(Alpha::from_png16(a.name.clone(), &a.to_png16().unwrap()).unwrap());
}
fn svg(body: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><filter id="soft"><feGaussianBlur stdDeviation="0.38"/></filter><radialGradient id="pearl"><stop stop-color="#000"/><stop offset=".4" stop-color="#222"/><stop offset=".8" stop-color="#888"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient></defs><g filter="url(#soft)">{body}</g></svg>"##
    )
}
fn line(path: &str, width: f64) -> String {
    [(1.6,"#c8c8c8"),(1.0,"#666666"),(0.5,"#080808")].iter().map(|(k,c)|format!(r#"<path d="{path}" stroke="{c}" stroke-width="{}" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#,width*k)).collect()
}
fn pearl(x: f64, y: f64, r: f64) -> String {
    format!(r##"<circle cx="{x}" cy="{y}" r="{r}" fill="url(#pearl)"/>"##)
}
fn frame(hex: bool) -> String {
    let p = if hex {
        "M25 8 L75 8 L94 50 L75 92 L25 92 L6 50 Z"
    } else {
        "M24 8 L76 8 L92 24 L92 76 L76 92 L24 92 L8 76 L8 24 Z"
    };
    let mut s = line(p, 1.5);
    s += &format!(
        r#"<g transform="translate(5 5) scale(.90)">{}</g>"#,
        line(p, 0.75)
    );
    s
}
fn petals(count: usize, rich: bool) -> String {
    let mut s = String::new();
    for i in 0..count {
        let mut petal = line(
            "M50 37 C34 27 38 12 50 8 C62 12 66 27 50 37 Z",
            if rich { 1.5 } else { 2.1 },
        );
        petal += &line("M50 34 C47 25 49 19 50 14", 0.8);
        if rich {
            petal += &line(
                "M48 28 C38 25 33 30 38 35 C44 39 46 31 42 31 M52 28 C62 25 67 30 62 35 C56 39 54 31 58 31",
                0.7,
            );
            petal += &pearl(50., 5., 1.1);
        }
        s += &format!(
            r#"<g transform="rotate({} 50 50)">{petal}</g>"#,
            i as f64 * 360. / count as f64
        );
    }
    s
}
fn palmette(rich: bool) -> String {
    let mut s = line("M50 91 C47 74 51 49 50 9", 3.2);
    for (y, spread) in [(79., 35.), (63., 33.), (47., 27.), (32., 19.)] {
        for side in [-1., 1.] {
            s += &line(
                &format!(
                    "M50 {y} C{} {} {} {} {} {} C{} {} {} {} 50 {y}",
                    50. + side * 14.,
                    y + 5.,
                    50. + side * spread,
                    y - 3.,
                    50. + side * spread,
                    y - 22.,
                    50. + side * 20.,
                    y - 25.,
                    50. + side * 9.,
                    y - 6.
                ),
                if rich { 2.1 } else { 3.3 },
            );
            if rich {
                s += &line(
                    &format!(
                        "M50 {y} Q{} {} {} {}",
                        50. + side * 18.,
                        y - 4.,
                        50. + side * (spread - 4.),
                        y - 17.
                    ),
                    1.0,
                );
            }
        }
    }
    s += &line("M18 87 Q50 105 82 87 M22 92 Q50 106 78 92", 2.0);
    s
}
fn sun(rich: bool) -> String {
    let mut s = String::new();
    for i in 0..if rich { 32 } else { 16 } {
        let a = i as f64 * TAU / if rich { 32. } else { 16. };
        let r = if i % 2 == 0 { 43. } else { 35. };
        s += &line(
            &format!(
                "M {} {} L {} {}",
                50. + a.cos() * 14.,
                50. + a.sin() * 14.,
                50. + a.cos() * r,
                50. + a.sin() * r
            ),
            if rich { 2.1 } else { 5.2 },
        );
    }
    s += &pearl(50., 50., 13.);
    s
}
fn add_svg(d: &mut RingDesign, lib: &mut AlphaLibrary, name: &str, body: String) {
    let a = SvgAlpha {
        name: name.into(),
        svg: svg(&body),
        invert: false,
    };
    lib.insert(a.rasterize());
    d.svgs.push(a);
}
fn atlas_layer(d: &RingDesign, name: &str, alpha: &str, height: f64, mask: &str) -> LayerEntry {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for(alpha, &ctx);
    t.repeats_around = 1;
    t.rows = 1;
    t.v_center_mm = ctx.band_v_len_mm * 0.5;
    t.v_span_mm = ctx.band_v_len_mm;
    t.height_mm = height;
    t.feather_mm = 0.;
    let mut e = LayerEntry::new(name, Layer::Tiling(t));
    e.mask = Some(mask.into());
    e.window = Window::around(
        90.,
        if mask == "Shoulder reserve" {
            170.
        } else {
            100.
        },
    );
    e.window.fade_deg = 8.;
    e
}
fn projected_layer(
    d: &mut RingDesign,
    lib: &mut AlphaLibrary,
    alpha: Alpha,
    name: &str,
    height: f64,
    mask: &str,
) {
    let mut entry = atlas_layer(d, name, name, height, mask);
    if d.draft.process == CastProcess::SandTwoPart && mask != "Shoulder reserve" {
        let occupied: Vec<_> = (0..alpha.width)
            .filter(|&x| (0..alpha.height).any(|y| alpha.data[y * alpha.width + x] > 0.0001))
            .collect();
        if let (Some(&lo), Some(&hi)) = (occupied.first(), occupied.last()) {
            let start = lo as f64 / alpha.width as f64 * 360.;
            let end = hi as f64 / alpha.width as f64 * 360.;
            entry.window = Window::around((start + end) * 0.5, end - start + 4.);
            entry.window.fade_deg = 2.;
        }
    }
    portable(lib, alpha);
    d.layers.layers.push(entry);
}
fn project_face(
    d: &mut RingDesign,
    lib: &mut AlphaLibrary,
    a: &Atlas,
    source: &str,
    name: &str,
    w: f64,
    h: f64,
    height: f64,
    draft: bool,
) {
    let mut art = lib.get(source).unwrap().clone();
    if draft {
        for x in 0..art.width {
            let mut high = 0_f32;
            for y in 0..art.height / 2 {
                let i = y * art.width + x;
                high = high.max(art.data[i]);
                art.data[i] = high;
            }
            high = 0.;
            for y in (art.height / 2..art.height).rev() {
                let i = y * art.width + x;
                high = high.max(art.data[i]);
                art.data[i] = high;
            }
        }
    }
    let sample = |x: f64, z: f64| {
        let (u, v) = (0.5 + x / w, 0.5 - z / h);
        if !(0.0..=1.).contains(&u) || !(0.0..=1.).contains(&v) {
            0.
        } else {
            art.sample(u, v) as f64
        }
    };
    let alpha = a.alpha(name, |s| {
        if a.face(s) <= 0. {
            return 0.;
        }
        sample(s.p[0], s.p[2])
    });
    projected_layer(d, lib, alpha, name, height, "Face reserve");
}
fn project_cheek(
    d: &mut RingDesign,
    lib: &mut AlphaLibrary,
    a: &Atlas,
    source: &str,
    name: &str,
    height: f64,
) {
    let art = lib.get(source).unwrap().clone();
    let alpha = a.alpha(name, |s| {
        if a.cheek(s) <= 0. {
            return 0.;
        }
        let u = 0.5 + s.p[0] / (a.length * 0.64);
        let v = (a.top - 0.8 - s.p[1]) / (a.top - a.bore - 1.6).max(1.2);
        if !(0.0..=1.).contains(&u) || !(0.0..=1.).contains(&v) {
            0.
        } else {
            art.sample(u, v) as f64
        }
    });
    projected_layer(d, lib, alpha, name, height, "Cheek reserve");
}
fn project_shoulder(
    d: &mut RingDesign,
    lib: &mut AlphaLibrary,
    a: &Atlas,
    source: &str,
    name: &str,
    height: f64,
    sand: bool,
) {
    let mut art = lib.get(source).unwrap().clone();
    if sand {
        // Shoulders withdraw across the motif: carry every lobe into its
        // central stem, creating broad chased leaves without trapped islands.
        for y in 0..art.height {
            let mut high = 0_f32;
            for x in 0..art.width / 2 {
                let i = y * art.width + x;
                high = high.max(art.data[i]);
                art.data[i] = high;
            }
            high = 0.;
            for x in (art.width / 2..art.width).rev() {
                let i = y * art.width + x;
                high = high.max(art.data[i]);
                art.data[i] = high;
            }
        }
    }
    let alpha = a.alpha(name, |s| {
        if a.shoulder(s) <= 0. {
            return 0.;
        }
        let u = 0.5 + s.p[2] / (a.width * 0.53);
        let v = (a.top - 2.6 - s.p[1]) / (a.top - 3.0);
        if !(0.0..=1.).contains(&u) || !(0.0..=1.).contains(&v) {
            0.
        } else {
            art.sample(u, v) as f64
        }
    });
    projected_layer(d, lib, alpha, name, height, "Shoulder reserve");
}
fn stamp(name: &str, alpha: &str, theta: f64, v: f64, size: f64, height: f64) -> LayerEntry {
    LayerEntry::new(
        name,
        Layer::Decals(DecalLayer {
            alpha: alpha.into(),
            decals: vec![Decal {
                theta_deg: theta,
                v_mm: v,
                size_mm: size,
                height_mm: height,
                ..Default::default()
            }],
            feather_mm: 0.18,
            invert: false,
        }),
    )
}
#[path = "common/sand_stock.rs"]
mod sand_stock;
use sand_stock::sand_stock;

fn base(slug: &str) -> Result<RingDesign> {
    let (name, id, length, width, bore, sand) = match slug {
        "nocturne" => ("Nocturne — night garden", "015", 17.0, 16.5, 18.2, false),
        "solstice" => ("Solstice — sun seal", "001", 18.5, 18.5, 18.2, true),
        "aurelia" => ("Aurelia — sovereign sun", "006", 20.0, 26.0, 18.8, true),
        "vesper" => (
            "Vesper — celestial reliquary",
            "019",
            22.0,
            22.0,
            19.2,
            false,
        ),
        "saurian" => ("Saurian — beaded skin", "013", 12.0, 11.5, 18.2, true),
        "zenith" => ("Zenith — the hunter's belt", "017", 17.0, 13.0, 18.6, true),
        "caiman" => ("Caiman — armoured hide", "015", 15.0, 14.0, 19.0, true),
        _ => anyhow::bail!("Unknown design {slug}"),
    };
    let mut d = RingDesign::default();
    let source = PRESETS
        .iter()
        .find(|p| p.id == id)
        .unwrap()
        .load()?;
    ImportedBase::attach(&mut d, if sand { sand_stock(source)? } else { source })?;
    d.imported_base.as_mut().unwrap().sand_envelope = sand;
    d.name = name.into();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.shank.head.length_mm = length;
    d.size = ringdesign_core::resize::size_from_bore(bore).unwrap();
    if slug == "aurelia" {
        d.shank.head.rise_mm += 1.0;
    }
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    // Establish the chart from THIS stock before drawing a single layer.
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart {
        profile: d.profile.clone(),
        bore_radius_mm: d.inner_radius_mm(),
    });
    d.build = BuildParams {
        theta_steps: 1536,
        profile_steps: 448,
        refine: None,
        ..Default::default()
    };
    let mut setup = mf::Setup::default();
    setup.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    setup.recipe.name = format!(
        "{name} / {}",
        if sand { "Delft clay" } else { "investment" }
    );
    setup.recipe.alloy = if slug == "solstice" {
        "Silver 925"
    } else {
        "Gold 18k"
    }
    .into();
    setup.recipe.shrink_pct = ringdesign_core::metal::find(&setup.recipe.alloy)
        .unwrap()
        .shrink_pct;
    if !sand {
        setup.recipe.process = CastProcess::LostWax;
        setup.recipe.sand = None;
        setup.recipe.min_draft_deg = 0.;
        setup.recipe.min_detail_mm = 0.15;
        setup.recipe.min_section_mm = 0.8;
    }
    setup.sample_pitch_mm = 0.1;
    setup.auto_parting = false;
    setup.parting_mm = 0.;
    setup.flask.width_mm = 80.;
    setup.flask.length_mm = 80.;
    setup.channels = vec![
        mf::Channel {
            kind: mf::ChannelKind::Gate,
            start: [0., -10.5, 0.],
            end: [0., -21., 0.],
            diameter_mm: 4.0,
        },
        mf::Channel {
            kind: mf::ChannelKind::Sprue,
            start: [0., -21., 0.],
            end: [0., -33., 0.],
            diameter_mm: 6.0,
        },
    ];
    setup.bench_notes=if sand{"Imported-stock master with axial solar relief and isolated cheek ornament. Z=0 parting, opposed Z withdrawal, molded bore. Cast all ornament except the explicitly marked fine finish and inscription. Dress seam/gate and polish reserved borders. Confirm shrink, sand support, filling, and actual release with a physical trial; channel layout is preliminary."}else{"Sacrificial investment pattern. Cast the relief and retained-floor gallery recesses in the metal body; stones are separate references. Cut final bearings and pavilion clearance to measured stones at the bench. Fine scrollwork and beading intentionally have no sand-draft restrictions. Polish reserved borders and high ornament; retain satin/oxidized recesses."}.into();
    d.draft.process = setup.recipe.process;
    d.draft.sand = setup.recipe.sand;
    d.draft.min_detail_mm = setup.recipe.min_detail_mm;
    d.draft.min_section_mm = setup.recipe.min_section_mm;
    d.draft.min_draft_deg = setup.recipe.min_draft_deg;
    d.manufacturing = Some(setup);
    Ok(d)
}
fn decorate(slug: &str) -> Result<(RingDesign, AlphaLibrary)> {
    if matches!(slug, "saurian" | "zenith" | "caiman") {
        return themed(slug);
    }
    let mut d = base(slug)?;
    let sand = matches!(slug, "solstice" | "aurelia");
    let grand = matches!(slug, "aurelia" | "vesper");
    let mut lib = AlphaLibrary::builtin();
    let a = Atlas::new(&d)?;
    portable(&mut lib, a.alpha("Face reserve", |s| a.face(s)));
    portable(&mut lib, a.alpha("Cheek reserve", |s| a.cheek(s)));
    portable(&mut lib, a.alpha("Shoulder reserve", |s| a.shoulder(s)));
    let ctx = d.field_context();
    let centre = a
        .samples
        .iter()
        .filter(|s| (s.theta - 90.).abs() < 0.01 && s.p[1] > a.top - 0.5)
        .min_by(|a, b| a.p[2].abs().total_cmp(&b.p[2].abs()))
        .unwrap();
    let mid = centre.v;
    println!(
        "{slug}: native face centre {:?}, chart {:.3} / {:.3}",
        centre.p, mid, ctx.band_v_len_mm
    );
    add_svg(
        &mut d,
        &mut lib,
        "Chased palmette",
        if sand {
            r##"<defs><linearGradient id="lotus"><stop stop-color="#ccc"/><stop offset=".35" stop-color="#333"/><stop offset=".6" stop-color="#111"/><stop offset="1" stop-color="#ccc"/></linearGradient></defs><g fill="url(#lotus)"><path d="M50 84 C32 63 39 37 50 10 C61 37 68 63 50 84 Z"/><path d="M38 84 C18 74 11 54 9 24 C28 39 33 61 38 84 Z"/><path d="M62 84 C82 74 89 54 91 24 C72 39 67 61 62 84 Z"/></g><path d="M19 91 Q50 98 81 91" stroke="#555" stroke-width="8" fill="none" stroke-linecap="round"/>"##.into()
        } else {
            palmette(grand)
        },
    );
    add_svg(&mut d, &mut lib, "Engraved frame", frame(slug == "vesper"));
    add_svg(&mut d, &mut lib, "Solar emblem", sun(grand));
    add_svg(
        &mut d,
        &mut lib,
        "Night flower",
        petals(if grand { 12 } else { 8 }, true),
    );
    let mut beads = String::new();
    let outline: Vec<[f64; 2]> = if slug == "vesper" {
        vec![
            [25., 8.],
            [75., 8.],
            [94., 50.],
            [75., 92.],
            [25., 92.],
            [6., 50.],
        ]
    } else {
        vec![
            [24., 8.],
            [76., 8.],
            [92., 24.],
            [92., 76.],
            [76., 92.],
            [24., 92.],
            [8., 76.],
            [8., 24.],
        ]
    };
    let lengths: Vec<_> = (0..outline.len())
        .map(|i| {
            let a = outline[i];
            let b = outline[(i + 1) % outline.len()];
            (b[0] - a[0]).hypot(b[1] - a[1])
        })
        .collect();
    let total: f64 = lengths.iter().sum();
    let count = if grand { 80 } else { 64 };
    for i in 0..count {
        let mut at = total * i as f64 / count as f64;
        let mut k = 0;
        while at > lengths[k] && k + 1 < lengths.len() {
            at -= lengths[k];
            k += 1;
        }
        let a = outline[k];
        let b = outline[(k + 1) % outline.len()];
        let f = at / lengths[k];
        beads += &pearl(
            a[0] + (b[0] - a[0]) * f,
            a[1] + (b[1] - a[1]) * f,
            if grand { 1.0 } else { 1.1 },
        );
    }
    add_svg(&mut d, &mut lib, "Hand pearled perimeter", beads);
    if sand {
        add_svg(
            &mut d,
            &mut lib,
            "Solar aureole",
            format!(
                "{}{}",
                line(
                    "M50 5 C75 5 93 25 93 50 C93 75 75 95 50 95 C25 95 7 75 7 50 C7 25 25 5 50 5 Z",
                    4.2
                ),
                line(
                    "M50 12 C73 12 86 30 86 50 C86 70 73 88 50 88 C27 88 14 70 14 50 C14 30 27 12 50 12 Z",
                    3.8
                )
            ),
        );
        project_face(
            &mut d,
            &mut lib,
            &a,
            "Solar aureole",
            "Double aureole / drafted medallion",
            a.length * 0.85,
            a.width * 0.87,
            if grand { 0.10 } else { 0.08 },
            false,
        );
        if grand {
            let mut bloom = String::from(
                r##"<defs><linearGradient id="petal"><stop stop-color="#eee"/><stop offset=".35" stop-color="#777"/><stop offset=".52" stop-color="#222"/><stop offset=".75" stop-color="#777"/><stop offset="1" stop-color="#eee"/></linearGradient></defs>"##,
            );
            for i in 0..12 {
                bloom += &format!(
                    r##"<g transform="rotate({} 50 50)"><path d="M50 40 C37 31 40 17 50 7 C60 17 63 31 50 40 Z" fill="url(#petal)"/></g>"##,
                    i * 30
                );
            }
            add_svg(&mut d, &mut lib, "Sovereign sun petals", bloom);
        }
        project_face(
            &mut d,
            &mut lib,
            &a,
            if grand {
                "Sovereign sun petals"
            } else {
                "Solar emblem"
            },
            "Sculpted solar corolla",
            a.length * 0.78,
            a.width * 0.80,
            if grand { 0.18 } else { 0.14 },
            false,
        );
        d.layers.layers.last_mut().unwrap().blend = Blend::Add;
        let mut disc = SeatPadLayer {
            theta_deg: 90.,
            v_mm: mid,
            diameter_mm: if grand { 4.8 } else { 3.6 },
            height_mm: if grand { 0.58 } else { 0.45 },
            style: SeatStyle::GypsyMound,
            blend_mm: 0.7,
            metal_true: true,
            ..Default::default()
        };
        disc.crown = 1.;
        let mut e = LayerEntry::new("Burnished solar heart", Layer::SeatPad(disc));
        e.mask = Some("Face reserve".into());
        d.layers.layers.push(e);
        project_cheek(
            &mut d,
            &mut lib,
            &a,
            "Chased palmette",
            "Paired cast palmette cartouches",
            if grand { 0.32 } else { 0.24 },
        );
        if grand {
            let mut ornament = String::new();
            for (x, y, angle) in [
                (18., 18., 0.),
                (82., 18., 90.),
                (82., 82., 180.),
                (18., 82., 270.),
            ] {
                ornament += &format!(
                    r#"<g transform="translate({x} {y}) rotate({angle}) scale(.28) translate(-50 -50)">{}</g>"#,
                    line(
                        "M18 82 C4 51 9 17 34 16 C58 15 57 43 41 45 C27 47 24 33 35 30 M18 82 C43 80 82 81 85 53 C86 33 67 25 59 39 C51 52 62 65 69 53",
                        3.6
                    )
                );
            }
            add_svg(&mut d, &mut lib, "Sovereign corner scrolls", ornament);
            project_face(
                &mut d,
                &mut lib,
                &a,
                "Sovereign corner scrolls",
                "Four sovereign corner cartouches",
                a.length * 0.89,
                a.width * 0.90,
                0.09,
                false,
            );
            // Fine veins are a separate editable bench pass over broad cast petals.
            let mut veins = String::new();
            for i in 0..12 {
                veins += &format!(
                    r#"<g transform="rotate({} 50 50)">{}</g>"#,
                    i * 30,
                    line("M50 35 Q46 26 50 14 M48 27 L42 22 M49 32 L42 28", 0.8)
                );
            }
            add_svg(&mut d, &mut lib, "Solar chasing", veins);
            project_face(
                &mut d,
                &mut lib,
                &a,
                "Solar chasing",
                "Petal veins / bench chasing",
                a.length * 0.78,
                a.width * 0.80,
                0.045,
                false,
            );
            let e = d.layers.layers.last_mut().unwrap();
            e.blend = Blend::Subtract;
            e.bench_only = true;
        }
    } else {
        project_face(
            &mut d,
            &mut lib,
            &a,
            "Engraved frame",
            "Inset double seal frame",
            a.length * 0.92,
            a.width * 0.92,
            0.22,
            false,
        );
        project_face(
            &mut d,
            &mut lib,
            &a,
            "Hand pearled perimeter",
            "Inset pearl gallery",
            a.length * 0.79,
            a.width * 0.79,
            0.21,
            false,
        );
        project_face(
            &mut d,
            &mut lib,
            &a,
            "Night flower",
            if grand {
                "Twelve celestial fleurons"
            } else {
                "Eight night garden petals"
            },
            a.length * 0.67,
            a.width * 0.67,
            if grand { 0.34 } else { 0.27 },
            false,
        );
        project_cheek(
            &mut d,
            &mut lib,
            &a,
            "Chased palmette",
            "Paired acanthus escutcheons",
            0.28,
        );
        let mut seat = SeatPadLayer {
            theta_deg: 90.,
            v_mm: mid,
            style: SeatStyle::Bezel,
            bezel_wall_mm: if grand { 0.48 } else { 0.36 },
            recess_mm: 0.65,
            blend_mm: 0.35,
            metal_true: true,
            rot_deg: 90.,
            ..Default::default()
        };
        seat.fit_stone(Gem::calibrated(
            if grand {
                GemCut::Asscher
            } else {
                GemCut::Round
            },
            if grand { 4.4 } else { 2.8 },
        ));
        let mut e = LayerEntry::new(
            if grand {
                "Asscher heart / sculpted bezel"
            } else {
                "Emerald dew / central bezel"
            },
            Layer::SeatPad(seat),
        );
        e.mask = Some("Face reserve".into());
        d.layers.layers.push(e);
        if grand {
            let mut halo = LayerStack::default();
            for i in 0..12 {
                let t = TAU * i as f64 / 12.;
                let x = 5.7 * t.cos();
                let z = 5.7 * t.sin();
                let theta = a.top.atan2(x).to_degrees();
                // Invert the native atlas on this plane for fixed-mm settings.
                let s = a
                    .samples
                    .iter()
                    .filter(|s| (s.theta - theta).abs() < 0.3 && s.p[1] > a.top - 0.5)
                    .min_by(|a, b| (a.p[2] - z).abs().total_cmp(&(b.p[2] - z).abs()))
                    .unwrap();
                let mut seat = SeatPadLayer {
                    theta_deg: theta,
                    v_mm: s.v,
                    style: SeatStyle::Bezel,
                    bezel_wall_mm: 0.25,
                    recess_mm: 0.28,
                    blend_mm: 0.16,
                    metal_true: true,
                    ..Default::default()
                };
                seat.fit_stone(Gem::calibrated(GemCut::Round, 1.15));
                halo.layers.push(LayerEntry::new(
                    format!("Constellation stone {:02}", i + 1),
                    Layer::SeatPad(seat),
                ));
            }
            let mut e = LayerEntry::new(
                "Twelve-stone constellation / editable group",
                Layer::Group(GroupLayer {
                    stack: halo,
                    recipe: None,
                }),
            );
            e.mask = Some("Face reserve".into());
            d.layers.layers.push(e);
        }
        for theta in [26., 154.] {
            let mut seat = SeatPadLayer {
                theta_deg: theta,
                v_mm: mid,
                style: SeatStyle::GypsyMound,
                height_mm: 0.42,
                blend_mm: 0.4,
                metal_true: true,
                dimple_mm: 0.16,
                ..Default::default()
            };
            seat.fit_stone(Gem::calibrated(
                if grand {
                    GemCut::Marquise
                } else {
                    GemCut::Round
                },
                if grand { 1.8 } else { 1.5 },
            ));
            d.layers.layers.push(LayerEntry::new(
                format!("Shoulder gem / {theta} degrees"),
                Layer::SeatPad(seat),
            ));
        }
    }
    if sand {
        let mut e = LayerEntry::new(
            "Graduated shoulder gadroons",
            Layer::Flutes(FlutesLayer {
                count: if grand { 40 } else { 36 },
                width_mm: 1.15,
                height_mm: if grand { 0.3 } else { 0.24 },
                lean: 0.,
                along: false,
                profile: FluteProfile::Round,
            }),
        );
        e.window = Window::except(90., 94.);
        e.window.fade_deg = 12.;
        e.window.v_gate = VGate::Band {
            center_mm: mid,
            span_mm: ctx.band_v_len_mm * 0.64,
            fade_mm: 1.1,
        };
        d.layers.layers.push(e);
    } else {
        project_shoulder(
            &mut d,
            &mut lib,
            &a,
            "Chased palmette",
            "Paired sculpted shoulder palmettes",
            if grand { 0.36 } else { 0.28 },
            false,
        );
    }
    // Deliberately separated grounds, shoulders and edging. Native masks leave
    // the actual face rim and bore transition untouched at every mesh tier.
    d.recipes.push(ProcRecipe {
        name: "Engine turning".into(),
        kind: if sand {
            Procedural::Barleycorn
        } else {
            Procedural::GuillocheWeave
        },
        repeats: 1,
        ..Default::default()
    });
    d.recipes.push(ProcRecipe {
        name: "Silk stipple".into(),
        kind: Procedural::Hammered,
        repeats: 1,
        gamma: 1.3,
        ..Default::default()
    });
    d.bake_all(&mut lib);
    let mut tile = TilingLayer::default_for("Engine turning", &ctx);
    tile.repeats_around = if grand { 28 } else { 24 };
    tile.rows = 4;
    tile.v_span_mm = ctx.band_v_len_mm;
    tile.v_center_mm = mid;
    tile.height_mm = 0.018;
    tile.feather_mm = 0.8;
    tile.warp = Some(WarpField {
        points: (0..8)
            .map(|i| [i as f64 / 8., mid + 0.6 * (TAU * i as f64 / 8.).sin()])
            .collect(),
        strength: 0.7,
        falloff_mm: ctx.band_v_len_mm,
    });
    let mut e = LayerEntry::new("Flowing engine-turned cheek ground", Layer::Tiling(tile));
    e.mask = Some("Cheek reserve".into());
    if !sand {
        e.blend = Blend::Subtract;
    }
    d.layers.layers.push(e);
    for (name, fraction) in [
        ("Upper interior thread", 0.21),
        ("Lower interior thread", 0.27),
    ] {
        let mut e = LayerEntry::new(
            name,
            Layer::Border(BorderLayer {
                v_mm: ctx.band_v_len_mm * fraction,
                width_mm: if sand { 0.6 } else { 0.3 },
                height_mm: 0.12,
                profile: BorderProfile::Round,
                mirror: true,
                ..Default::default()
            }),
        );
        e.mask = Some("Cheek reserve".into());
        d.layers.layers.push(e);
    }
    let mut e = LayerEntry::new(
        "Fine bead procession",
        Layer::Milgrain(MilgrainLayer {
            v_mm: ctx.band_v_len_mm * 0.24,
            bead_diameter_mm: if sand { 0.65 } else { 0.4 },
            beads_around: if grand { 112 } else { 88 },
            height_mm: if sand { 0.12 } else { 0.16 },
            mirror: true,
        }),
    );
    e.mask = Some("Cheek reserve".into());
    d.layers.layers.push(e);
    if !sand {
        let radius = if grand { 7.05 } else { 5.3 };
        let points = (0..=24)
            .map(|i| {
                let t = TAU * i as f64 / 24.;
                let x = radius * t.cos();
                let z = radius * t.sin();
                let theta = a.top.atan2(x).to_degrees();
                let s = a
                    .samples
                    .iter()
                    .filter(|s| (s.theta - theta).abs() < 0.3 && s.p[1] > a.top - 0.5)
                    .min_by(|a, b| (a.p[2] - z).abs().total_cmp(&(b.p[2] - z).abs()))
                    .unwrap();
                [theta / 360., s.v]
            })
            .collect();
        let c = CurveLayer {
            points,
            repeats_around: 1,
            closed: false,
            width_mm: 0.22,
            height_mm: 0.10,
            profile: WireProfile::Round,
            taper: 0.,
            mirror_v: false,
        };
        let mut e = LayerEntry::new("Swept halo thread", Layer::Curve(c));
        e.mask = Some("Face reserve".into());
        d.layers.layers.push(e);
    }
    let mut flutes = LayerEntry::new(
        "Graduated palm reeds",
        Layer::Flutes(FlutesLayer {
            count: if grand { 44 } else { 36 },
            width_mm: 0.8,
            height_mm: 0.13,
            lean: 0.,
            along: false,
            profile: FluteProfile::Round,
        }),
    );
    flutes.window = Window::around(270., 92.);
    flutes.window.v_gate = VGate::Band {
        center_mm: mid,
        span_mm: ctx.band_v_len_mm * 0.26,
        fade_mm: 0.7,
    };
    d.layers.layers.push(flutes);
    // A pressure-drawn four-point highlight, retained as editable strokes.
    let mut star = DrawnAlpha::new("Drawn north star", 512, 512);
    for (u, v) in [(0.12, 0.5), (0.5, 0.1)] {
        let mut s = Stroke::new(0.085, 0.9, false);
        for i in 0..=40 {
            let t = i as f32 / 40.;
            s.push(
                u + (1. - 2. * u) * t,
                v + (1. - 2. * v) * t,
                (PI as f32 * t).sin().max(0.).powf(0.7),
            );
        }
        star.strokes.push(s);
    }
    d.drawn.push(star);
    if !sand {
        for theta in [52., 128.] {
            let mut e = stamp(
                "Drawn shoulder star",
                "Drawn north star",
                theta,
                mid,
                1.8,
                0.19,
            );
            e.mask = Some("Shoulder reserve".into());
            d.layers.layers.push(e);
        }
    }
    if grand {
        add_svg(
            &mut d,
            &mut lib,
            "Crown volutes",
            line(
                "M10 75 C3 40 33 20 39 37 C45 55 20 63 22 45 M90 75 C97 40 67 20 61 37 C55 55 80 63 78 45 M14 81 Q50 102 86 81",
                2.0,
            ),
        );
        project_cheek(
            &mut d,
            &mut lib,
            &a,
            "Crown volutes",
            "Crowned double volute",
            if sand { 0.23 } else { 0.32 },
        );
        if !sand {
            add_svg(
                &mut d,
                &mut lib,
                "Pierced lancets",
                line(
                    "M15 82 Q9 44 25 20 Q42 44 35 82 Z M42 82 Q35 37 50 8 Q65 37 58 82 Z M65 82 Q58 44 75 20 Q91 44 85 82 Z",
                    4.0,
                ),
            );
            project_cheek(
                &mut d,
                &mut lib,
                &a,
                "Pierced lancets",
                "Recessed cathedral gallery / retained floor",
                0.30,
            );
            d.layers.layers.last_mut().unwrap().blend = Blend::Subtract;
        }
    }
    let mut satin = TilingLayer::default_for("Silk stipple", &ctx);
    satin.repeats_around = 18;
    satin.rows = 4;
    satin.v_span_mm = ctx.band_v_len_mm;
    satin.v_center_mm = mid;
    satin.height_mm = 0.018;
    let mut e = LayerEntry::new("Silk ground / bench finish", Layer::Tiling(satin));
    e.mask = Some("Cheek reserve".into());
    e.bench_only = true;
    e.blend = Blend::Subtract;
    d.layers.layers.push(e);
    d.texts.push(TextAlpha {
        name: "Maker signature".into(),
        text: slug.to_uppercase(),
        font: TextFont::Serif,
        tracking: 0.16,
    });
    let mut e = stamp(
        "Palm signature / bench engraving",
        "Maker signature",
        270.,
        mid,
        if grand { 10. } else { 8. },
        0.08,
    );
    e.blend = Blend::Subtract;
    e.bench_only = true;
    d.layers.layers.push(e);
    // Cheek engraving cuts into the stock. It never projects beyond the
    // polished face edge and cannot recreate the screenshot's raised lip.
    for e in &mut d.layers.layers {
        if e.mask.as_deref() == Some("Cheek reserve") {
            e.blend = Blend::Subtract;
            e.opacity *= 0.60;
        }
    }
    if sand {
        // Fine chasing is separately identified; the complete solar emblem,
        // shoulder palmettes, rails, pearls and palm reeds stay in the pattern.
        for e in &mut d.layers.layers {
            if matches!(
                e.name.as_str(),
                "Flowing engine-turned cheek ground"
                    | "Crowned double volute"
                    | "Four sovereign corner cartouches"
            ) {
                e.bench_only = true;
                e.name.push_str(" / bench chasing");
            }
        }
    } else {
        // A painted resist protects raised sculpture from the engine-turned
        // background. Both masks are portable, editable artwork in the graph.
        d.bake_all(&mut lib);
        let mut active = d.layers.clone();
        active
            .layers
            .retain(|e| e.enabled && !matches!(e.blend, Blend::Subtract));
        let current = d.field_context();
        for (name, region) in [
            ("Seal engraving resist", 0),
            ("Shoulder engraving resist", 1),
        ] {
            let mask = a.alpha(name, |s| {
                let gate = if region == 0 {
                    let x = (s.p[0] / (a.length * 0.5)).abs();
                    let z = (s.p[2] / (a.width * 0.5)).abs();
                    a.face(s) * (1. - smoothstep(0.60, 0.65, x.max(z).max((x + z) / 1.68)))
                } else {
                    a.shoulder(s)
                };
                if gate <= 0. {
                    return 0.;
                }
                let uv = ringdesign_core::Uv {
                    u: current.u_of_theta(s.theta),
                    v: s.v,
                };
                let height = active.height(uv, &current, &lib);
                gate * (1. - smoothstep(0.001, 0.012, height))
            });
            portable(&mut lib, mask);
            let mut tex = TilingLayer::default_for("Engine turning", &current);
            tex.repeats_around = if grand { 18 } else { 16 };
            tex.rows = 5;
            tex.v_span_mm = current.band_v_len_mm;
            tex.v_center_mm = current.band_v_len_mm * 0.5;
            tex.height_mm = 0.012;
            tex.shear = 0.25;
            tex.warp = Some(WarpField {
                points: (0..8)
                    .map(|i| [i as f64 / 8., mid + 0.3 * (TAU * i as f64 / 8.).sin()])
                    .collect(),
                strength: 0.6,
                falloff_mm: current.band_v_len_mm,
            });
            let mut e = LayerEntry::new(
                if region == 0 {
                    "Guilloche seal ground / resist mask"
                } else {
                    "Guilloche shoulder ground / resist mask"
                },
                Layer::Tiling(tex),
            );
            e.mask = Some(name.into());
            e.blend = Blend::Subtract;
            d.layers.layers.push(e);
        }
    }
    if slug == "solstice" {
        for e in &mut d.layers.layers {
            if e.name == "Paired cast palmette cartouches" {
                e.name = "Paired palmette cartouches / bench chasing".into();
                e.bench_only = true;
            }
        }
    }
    d.bake_all(&mut lib);
    d.embed_alphas(&lib);
    Ok((d, lib))
}

// ---- One theme, face to palm ------------------------------------------------
//
// Every mark on these two rings is painted into one height map per ring from
// the stock's own three-dimensional samples: distances are millimetres of
// metal, so a scale is the size it says wherever the surface takes it, and
// the regions hand over to each other inside one skin instead of as layers
// laid side by side.

fn hash(i: i64, j: i64) -> f64 {
    let mut h = (i.wrapping_mul(0x9E37_79B9_7F4A_7C15u64 as i64) ^ j.wrapping_mul(0xC2B2_AE3D_27D4_EB4Fu64 as i64)) as u64;
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h % 10_000) as f64 / 10_000.0
}

/// Where a sample stands on the ring, in the terms the painters think in.
struct Spot {
    /// Arc from the head's centre along the ring, mm, signed by shoulder.
    arc: f64,
    /// Degrees round from the head, 0 at the face and 180 at the palm.
    away: f64,
    /// Across the band as a share of its half width here, -1 to 1.
    across: f64,
    /// How squarely the surface faces out from the finger, less its edge rounding.
    outer: f64,
}

struct Skin<'a> {
    a: &'a Atlas,
    half: Vec<f64>,
}

impl<'a> Skin<'a> {
    fn new(a: &'a Atlas) -> Self {
        let half = (0..AW)
            .map(|x| (0..AH).map(|y| a.samples[y * AW + x].p[2].abs()).fold(0.0, f64::max).max(0.5))
            .collect();
        Self { a, half }
    }
    fn spot(&self, s: Sample) -> Spot {
        let r = s.p[0].hypot(s.p[1]).max(1e-6);
        let signed = (s.theta - 90.0 + 180.0).rem_euclid(360.0) - 180.0;
        let x = ((s.theta / 360.0 * AW as f64).round() as usize) % AW;
        let radial = (s.n[0] * s.p[0] + s.n[1] * s.p[1]) / r;
        Spot {
            arc: signed.to_radians() * r,
            away: signed.abs(),
            across: s.p[2] / self.half[x],
            outer: smoothstep(0.12, 0.40, radial) * (1.0 - smoothstep(0.45, 0.72, s.n[2].abs())),
        }
    }
    /// The chart point on the parting line `arc` mm round from the head's centre, signed by shoulder.
    fn on_crest(&self, arc: f64) -> (f64, f64) {
        let mut best = (f64::MAX, 0.0, 0.0);
        for x in 0..AW {
            let Some(s) = (1..AH - 1).map(|y| self.a.samples[y * AW + x]).min_by(|p, q| p.p[2].abs().total_cmp(&q.p[2].abs())) else { continue };
            let miss = (self.spot(s).arc - arc).abs();
            if miss < best.0 { best = (miss, s.theta, s.v); }
        }
        (best.1, best.2)
    }
    /// The chart point on one of the head's walls nearest a place given about the finger: `rho` out from
    /// its axis, `phi` radians round from the head's centre, on the `side` of the parting line asked for.
    fn on_cheek(&self, rho: f64, phi: f64, side: f64) -> Option<(f64, f64)> {
        let (x, y) = (rho * phi.sin(), rho * phi.cos());
        self.a.samples.iter()
            .filter(|s| s.p[2] * side > 0.0 && self.a.cheek(**s) > 0.9)
            .map(|s| ((s.p[0] - x).hypot(s.p[1] - y), s))
            .filter(|(d, _)| *d < 0.12)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, s)| (s.theta, s.v))
    }
    /// The nearest sample on the face to a point of it, as the chart's own `(theta, v)`.
    fn on_face(&self, x: f64, z: f64) -> (f64, f64) {
        let best = self
            .a
            .samples
            .iter()
            .filter(|s| s.p[1] > self.a.top - 1.2)
            .min_by(|p, q| ((p.p[0] - x).powi(2) + (p.p[2] - z).powi(2)).total_cmp(&((q.p[0] - x).powi(2) + (q.p[2] - z).powi(2))))
            .unwrap();
        (best.theta, best.v)
    }
}

/// Make a painted skin pull from a two-part mould by construction. Walking out from the parting line
/// across each section, relief may rise only as fast as the stock's own draft there allows — on a
/// signet's face that is barely at all, on its cheeks without limit. What breaks the rule is cut back,
/// never filled: a filled flank is a ridge to the parting line, and that is not what was drawn.
fn draft_clamp(a: &Atlas, alpha: &mut Alpha, height_mm: f64) {
    for x in 0..AW {
        let at = |y: usize| a.samples[y * AW + x];
        let Some(crest) = (1..AH - 1).min_by(|p, q| at(*p).p[2].abs().total_cmp(&at(*q).p[2].abs())) else { continue };
        for dir in [1i64, -1] {
            let mut y = crest as i64;
            loop {
                let next = y + dir;
                if next < 1 || next >= AH as i64 - 1 { break; }
                let (here, there) = (at(y as usize), at(next as usize));
                let step = (0..3).map(|k| (there.p[k] - here.p[k]).powi(2)).sum::<f64>().sqrt();
                // Draft the mould half sees: the normal's lean toward its own side of the parting line.
                let lean = (there.n[2] * there.p[2].signum()).clamp(0.0, 0.9995);
                let rise = lean / (1.0 - lean * lean).sqrt() * step / height_mm;
                let cap = alpha.data[y as usize * AW + x] + rise as f32;
                let cell = &mut alpha.data[next as usize * AW + x];
                *cell = cell.min(cap.min(1.0));
                y = next;
            }
        }
    }
}

/// A dome: flat-topped inside `0.55 r`, down to nothing at `r`.
fn dome(d: f64, r: f64) -> f64 {
    1.0 - smoothstep(0.55 * r, r, d)
}

/// Pointed scutes: plates the whole width of the band, each rising to a free edge that lies over the
/// next. The edge is a chevron whose point rides the parting line and leads, so its wall faces round the
/// ring and away from the parting line and never back toward it; the plate is arched and keeled along
/// that same line, so everything falls away from it.
fn scutes(along: f64, across: f64, pitch_mm: f64) -> f64 {
    let t = (along + 0.62 * across.abs()).rem_euclid(1.0);
    // The drop off the free edge is a drafted wall as wide as the sand can hold, not a cliff: a cliff
    // under a rising plate is an acute lip, a feather of metal the pour cannot fill and a probe reads
    // as a 0.16 mm wall.
    let fall = (0.45 / pitch_mm).min(0.3);
    let low = 0.20;
    let crest = low + (1.0 - low) * (1.0 - fall).powf(0.9);
    let plate = if t <= 1.0 - fall { low + (1.0 - low) * t.powf(0.9) } else { crest + (low - crest) * smoothstep(1.0 - fall, 1.0, t) };
    let keel = 0.10 * (1.0 - smoothstep(0.0, 0.16, across.abs())) * (plate - low);
    ((plate + keel) * (1.0 - 0.30 * across * across)).min(1.0)
}

/// Small round overlapping scales, for the walls where relief moves along the pull and any shape casts.
fn round_scales(row: f64, col: f64) -> f64 {
    let mut best: f64 = 0.0;
    let i0 = row.floor() as i64;
    for i in [i0 - 1, i0] {
        let stagger = if i.rem_euclid(2) == 0 { 0.0 } else { 0.5 };
        let centre = (col - stagger).round() + stagger;
        let t = (row - i as f64) / 1.55;
        if !(0.0..=1.0).contains(&t) { continue; }
        let dc = (col - centre).abs();
        let half = 0.56 * (1.0 - t.powf(2.4)).max(0.0).powf(0.6);
        if dc >= half { continue; }
        best = best.max(smoothstep(0.0, 0.22, half - dc) * smoothstep(0.0, 0.16, 1.0 - t) * (0.35 + 0.65 * t));
    }
    best
}

fn skin_layer(d: &RingDesign, name: &str, height: f64) -> LayerEntry {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for(name, &ctx);
    t.repeats_around = 1;
    t.rows = 1;
    t.v_center_mm = ctx.band_v_len_mm * 0.5;
    t.v_span_mm = ctx.band_v_len_mm;
    t.height_mm = height;
    t.feather_mm = 0.0;
    let mut e = LayerEntry::new(name, Layer::Tiling(t));
    // The skin stops short of the palm, and so does the layer: the palm is the band's tightest station,
    // and a texture is judged at the tightest one it covers.
    e.window = Window::around(90.0, 300.0);
    e.window.fade_deg = 6.0;
    e
}

fn flush_seat(d: &mut RingDesign, skin: &Skin, name: &str, x: f64, z: f64, gem: Gem, mound: f64) {
    let (theta, v) = skin.on_face(x, z);
    let mut seat = SeatPadLayer {
        theta_deg: theta,
        v_mm: v,
        style: SeatStyle::GypsyMound,
        crown: 1.0,
        blend_mm: 0.55,
        metal_true: true,
        solid: ringdesign_core::setting::SolidKind::Flush,
        through: true,
        ..Default::default()
    };
    seat.fit_stone(gem);
    seat.height_mm = mound;
    let mut e = LayerEntry::new(name, Layer::SeatPad(seat));
    e.blend = Blend::Max;
    d.layers.layers.push(e);
}

fn themed(slug: &str) -> Result<(RingDesign, AlphaLibrary)> {
    let mut d = base(slug)?;
    // The sand envelope stays on as the release's guarantee, but the skin is drawn so that it has nothing
    // to fill: every form falls away from the parting line, and what the graver cuts is laid on afterwards.
    let mut lib = AlphaLibrary::builtin();
    let a = Atlas::new(&d)?;
    let skin = Skin::new(&a);
    if slug == "caiman" {
        caiman(&mut d, &mut lib, &a, &skin)?;
        return Ok((d, lib));
    }
    let setup = d.manufacturing.as_mut().unwrap();
    if slug == "saurian" {
        setup.recipe.alloy = "Silver 925".into();
        setup.bench_notes = "Imported-stock master, one reptile skin from face to palm: pointed, keeled scutes the width of the band, graded from the face down both shoulders, and small round scales on the head's walls. Every wall faces round the ring or away from the parting line, so the skin pulls as drawn. Z=0 parting, opposed Z withdrawal. The stone is flush set at the bench: drill on the raised mark at the face's centre, cut the seat to the measured stone, burnish. Polish the reserved rims and the palm; leave the grooves satin.".into();
        const RELIEF: f64 = 0.60;
        let mut alpha = a.alpha("Saurian skin", |s| {
            let at = skin.spot(s);
            // One form from face to palm: the pointed scute, 2.5 mm deep on the face and graded down the
            // shoulders to 1.5 by the palm, its point leading away from the stone on both sides.
            let (p0, p1, run) = (2.5, 1.5, 24.0);
            let l = at.arc.abs();
            let k = (p0 - p1) / run;
            let row = if l < run { -(1.0 - k * l / p0).ln() / k } else { -(p1 / p0).ln() / k + (l - run) / p1 };
            let top = a.face(s).max(at.outer * (1.0 - smoothstep(0.86, 0.99, at.across.abs())));
            let pitch = if l < run { p0 - k * l } else { p1 };
            let scales = top * (1.0 - smoothstep(112.0, 146.0, at.away)) * scutes(row, at.across, pitch);
            let face = 0.0_f64;
            // Small round scales on the head's walls, flowing back from the face like a flank's.
            let rho = s.p[0].hypot(s.p[1]);
            let walls = a.cheek(s) * round_scales(s.p[0].atan2(s.p[1]).abs() * rho / 2.3, (rho - a.bore) / 1.9);
            face.max(scales).max(walls)
        });
        draft_clamp(&a, &mut alpha, RELIEF);
        portable(&mut lib, alpha);
        d.layers.layers.push(skin_layer(&d, "Saurian skin", RELIEF));
        flush_seat(&mut d, &skin, "Flush stone on the spine", 0.0, 0.0, Gem::calibrated(GemCut::Round, 2.8), 0.50);
    } else {
        setup.recipe.alloy = "Silver 925".into();
        setup.bench_notes = "Imported-stock master, the night sky from face to palm: Orion on the face with his belt as the three stones, the moon's phases struck down each shoulder, star trails on the cheeks, far stars thinning to a polished palm. The full, gibbous and half moons are cast as they are; each crescent is cast as its half moon and cut back to the terminator at the bench, and each new moon is cast a disc and milled to leave its rim. Z=0 parting, opposed Z withdrawal; the belt lies on the parting line. Stones are flush set at the bench: drill on the three raised marks, cut each seat to its measured stone, burnish. The figure's lines are cut with a graver after casting.".into();
        // Orion, belt along the ring on the parting line, the figure across the band. (x along the ring, z across.)
        let belt = [(-2.9, 0.0, 2.0), (0.0, 0.0, 2.2), (2.9, 0.0, 1.8)];
        let stars: [(f64, f64, f64); 9] = [
            (-3.05, 4.15, 0.70), (2.75, 3.95, 0.56), (-2.55, -4.30, 0.54), (3.15, -4.45, 0.72), (-0.2, 5.35, 0.42),
            (-0.35, -1.55, 0.36), (-0.15, -2.3, 0.40), (0.1, -3.05, 0.34), (0.0, 0.0, 0.0),
        ];
        let pleiades = [(6.2, 3.1, 0.40), (6.75, 2.55, 0.34), (5.75, 2.45, 0.36), (6.45, 1.95, 0.32), (5.95, 3.55, 0.30), (6.95, 3.35, 0.30)];
        use ringdesign_core::setting::{Stamp, crescent_cutter, moon_outline};
        let start = a.length * 0.5 + 2.6;
        let ctx = d.field_context();
        const RELIEF: f64 = 0.34;
        const HORN: f64 = 0.86;
        let stamp = |name: String, at: (f64, f64), rot: f64, outline: Vec<[f64; 2]>| Stamp {
            name, theta_deg: at.0, v_mm: at.1, rot_deg: rot, outline, height_mm: RELIEF, sink_mm: 0.3, draft_deg: 4.0, cut: false, bench: false, along_pull: false, tier: 0, top: Default::default(),
        };
        let circle = |r: f64| -> Vec<[f64; 2]> { (0..56).map(|i| { let t = TAU * i as f64 / 56.0; [r * t.cos(), r * t.sin()] }).collect() };
        // A moon the sand can cast is one whose every wall faces away from the parting line or round the
        // ring: the full, the gibbous and the half. A crescent's inner edge faces back across the parting
        // line wherever it is put, so it is cast as the half moon and the bench cuts it to the crescent;
        // the new moon is cast a disc and the bench leaves its rim.
        let mut moons: Vec<Stamp> = Vec::new();
        let phase = |name: &str, at: (f64, f64), rot: f64, r: f64, k: usize, moons: &mut Vec<Stamp>| match k {
            0 => moons.push(stamp(format!("{name}: full moon"), at, rot, moon_outline(r, 1.0, HORN))),
            1 => moons.push(stamp(format!("{name}: gibbous moon"), at, rot, moon_outline(r, 0.76, HORN))),
            2 => moons.push(stamp(format!("{name}: half moon"), at, rot, moon_outline(r, 0.5, HORN))),
            3 => {
                moons.push(stamp(format!("{name}: crescent, cast as the half"), at, rot, moon_outline(r, 0.5, HORN)));
                moons.push(Stamp { height_mm: RELIEF + 0.3, sink_mm: -0.02, draft_deg: 0.0, cut: true, bench: true, ..stamp(format!("{name}: crescent, cut at the bench"), at, rot, crescent_cutter(r, 0.27, HORN, 0.22)) });
            }
            _ => {
                moons.push(stamp(format!("{name}: new moon, cast as a disc"), at, rot, circle(r)));
                moons.push(Stamp { height_mm: RELIEF + 0.3, sink_mm: -0.02, draft_deg: 0.0, cut: true, bench: true, ..stamp(format!("{name}: new moon, its rim left by the bench"), at, rot, circle(r - 0.36)) });
            }
        };
        for (side, shoulder) in [(1.0, "Waning"), (-1.0, "Waxing")] {
            for k in 0..5 {
                let r = 1.75 - 0.2 * k as f64;
                let centre = start + (0..k).map(|q| 2.0 * (1.75 - 0.2 * q as f64) + 0.75).sum::<f64>();
                // The lit side faces the head on both shoulders.
                phase(shoulder, skin.on_crest(side * centre), if side > 0.0 { 0.0 } else { 180.0 }, r, k, &mut moons);
            }
        }
        // The crescent beside the hunter, its limb away from him.
        phase("Beside the hunter", skin.on_face(-5.9, 0.0), 180.0, 1.9, 3, &mut moons);
        // Star trails on the head's walls: arcs about the finger, as a long exposure draws them about the
        // pole, each a tapered stroke. Relief there moves along the pull, so any shape casts.
        let mut trails = 0;
        for side in [1.0, -1.0] {
            for (lane, out) in [1.7, 2.65, 3.55].into_iter().enumerate() {
                let rho = a.bore + out;
                let open: Vec<f64> = (-70..=70).map(|deg| (deg as f64).to_radians()).filter(|phi| skin.on_cheek(rho, *phi, side).is_some()).collect();
                let (Some(lo), Some(hi)) = (open.first().copied(), open.last().copied()) else { continue };
                let (lo, hi) = (lo + 0.07, hi - 0.07);
                let cuts = [0.0, 0.34 + 0.05 * lane as f64, 0.62 - 0.04 * lane as f64, 1.0];
                for w in cuts.windows(2) {
                    let (p0, p1) = (lo + (hi - lo) * w[0] + 0.035, lo + (hi - lo) * w[1] - 0.035);
                    if (p1 - p0) * rho < 1.6 { continue; }
                    let Some(at) = skin.on_cheek(rho, 0.5 * (p0 + p1), side) else { continue };
                    let mut mark = stamp(format!("Star trail {}", trails + 1), at, 0.0, Vec::new());
                    mark.height_mm = 0.3;
                    mark.draft_deg = 0.0;
                    // A head's wall leans; the trail stands along the pull so neither edge tucks under it.
                    mark.along_pull = true;
                    mark.sink_mm = 0.5;
                    let frame = mark.frame(&d, &ctx);
                    let steps = 28;
                    // Tapered to 0.36 mm at the ends, over Delft clay's 0.30 mm detail floor.
                    let half = |t: f64| 0.30 * (std::f64::consts::PI * t).sin().max(0.0).powf(0.55).max(0.6);
                    let local = |r: f64, phi: f64| {
                        let q = [r * phi.sin() - frame.origin[0], r * phi.cos() - frame.origin[1], 0.0];
                        [q[0] * frame.x[0] + q[1] * frame.x[1], q[0] * frame.y[0] + q[1] * frame.y[1]]
                    };
                    let mut outline: Vec<[f64; 2]> = (0..=steps).map(|i| { let t = i as f64 / steps as f64; local(rho + half(t), p0 + (p1 - p0) * t) }).collect();
                    outline.extend((0..=steps).rev().map(|i| { let t = i as f64 / steps as f64; local(rho - half(t), p0 + (p1 - p0) * t) }));
                    let area: f64 = (0..outline.len()).map(|i| { let (p, q) = (outline[i], outline[(i + 1) % outline.len()]); p[0] * q[1] - q[0] * p[1] }).sum();
                    if area < 0.0 { outline.reverse(); }
                    mark.outline = outline;
                    moons.push(mark);
                    trails += 1;
                }
            }
        }
        println!("  stamps: {} ({} star trails)", moons.len(), trails);
        d.stamps = moons;
        // The rest of the sky is the graver's and the drill's, after the pour: the hunter's stars and the
        // lines between them, the Pleiades, and far stars thinning down the shank to a polished palm.
        // A pit or a bump off the parting line would lock in the sand; cut at the bench it costs nothing.
        let lines = [(0usize, 8usize), (1, 8), (2, 8), (3, 8), (0, 4), (1, 4)];
        let belt_ends = [(-2.9, 0.0), (2.9, 0.0)];
        let graver = a.alpha("Graver's sky", |s| {
            let at = skin.spot(s);
            let (x, z) = (s.p[0], s.p[2]);
            let mut cut: f64 = 0.0;
            if a.face(s) > 0.0 {
                for (sx, sz, r) in stars.iter().chain(&pleiades).filter(|t| t.2 > 0.0) {
                    cut = cut.max(dome((x - sx).hypot(z - sz), *r * 1.15));
                }
                for (from, to) in lines {
                    let p = (stars[from].0, stars[from].1);
                    let q = if to == 8 { if p.0 < 0.0 { belt_ends[0] } else { belt_ends[1] } } else { (stars[to].0, stars[to].1) };
                    let (ex, ez) = (q.0 - p.0, q.1 - p.1);
                    let t = (((x - p.0) * ex + (z - p.1) * ez) / (ex * ex + ez * ez)).clamp(0.14, 0.86);
                    cut = cut.max(0.7 * (1.0 - smoothstep(0.06, 0.15, (x - p.0 - ex * t).hypot(z - p.1 - ez * t))));
                }
                cut *= a.face(s);
            }
            let l = at.arc.abs();
            let half = self_half(&skin, s);
            let (ci, cj) = ((l / 2.1).floor() as i64, ((z + 20.0) / 1.9).floor() as i64);
            let here = [(ci as f64 + 0.2 + 0.6 * hash(ci, cj)) * 2.1, (cj as f64 + 0.2 + 0.6 * hash(cj, ci + 17)) * 1.9 - 20.0];
            let r = 0.34 - 0.12 * smoothstep(60.0, 130.0, at.away);
            let far = if hash(ci * 7 + 3, cj * 13 + 1) < 0.55 && here[1].abs() < half - 0.9 { dome((l - here[0]).hypot(z - here[1]), r) } else { 0.0 };
            cut.max(at.outer * far * smoothstep(start + 14.0, start + 15.0, l) * (1.0 - smoothstep(118.0, 150.0, at.away)))
        });
        portable(&mut lib, graver);
        let mut cut = skin_layer(&d, "Graver's sky", 0.24);
        cut.blend = Blend::Subtract;
        cut.bench_only = true;
        d.layers.layers.push(cut);
        for (k, (x, z, w)) in belt.iter().enumerate() {
            flush_seat(&mut d, &skin, ["Alnitak", "Alnilam", "Mintaka"][k], *x, *z, Gem::calibrated(GemCut::Round, *w), 0.40);
        }
    }
    Ok((d, lib))
}

/// The stock's surface measured in millimetres the way a hide is: `along` the parting line from the
/// head's centre (per column, signed by shoulder), `across` the section from the parting line (per
/// sample, signed by side), and `rim`, how far across the outer surface runs on each side before it
/// turns to face the pull. On a head's end wall the parting line runs down the wall, so rows laid by
/// `along` keep their size there instead of crowding into the few degrees the wall spans.
struct Hide {
    along: Vec<f64>,
    across: Vec<f64>,
    rim: Vec<[f64; 2]>,
    /// How far the wall runs down past the rim on each side before the bore's edge.
    wall: Vec<[f64; 2]>,
    /// The atlas row of the parting line in each column.
    crest: Vec<usize>,
}

impl Hide {
    fn new(a: &Atlas) -> Self {
        let at = |x: usize, y: usize| a.samples[y * AW + x];
        let dist = |p: [f64; 3], q: [f64; 3]| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f64>().sqrt();
        let crest: Vec<usize> = (0..AW)
            .map(|x| (1..AH - 1).min_by(|p, q| at(x, *p).p[2].abs().total_cmp(&at(x, *q).p[2].abs())).unwrap_or(AH / 2))
            .collect();
        let head = AW / 4;
        let mut along = vec![0.0; AW];
        for dir in [1i64, -1] {
            let (mut acc, mut prev) = (0.0, head);
            for k in 1..=AW / 2 {
                let x = (head as i64 + dir * k as i64).rem_euclid(AW as i64) as usize;
                acc += dist(at(prev, crest[prev]).p, at(x, crest[x]).p);
                along[x] = dir as f64 * acc;
                prev = x;
            }
        }
        let mut across = vec![0.0; AW * AH];
        let mut rim = vec![[0.0; 2]; AW];
        let mut wall = vec![[0.0; 2]; AW];
        for x in 0..AW {
            let c = crest[x];
            for dir in [1i64, -1] {
                let (mut acc, mut y) = (0.0, c as i64);
                let (mut edge, mut low) = (None, 0.0);
                loop {
                    let next = y + dir;
                    if next < 0 || next >= AH as i64 {
                        break;
                    }
                    let (p, q) = (at(x, y as usize), at(x, next as usize));
                    acc += dist(p.p, q.p);
                    across[next as usize * AW + x] = acc * q.p[2].signum();
                    if edge.is_none() && q.n[2].abs() > 0.6 {
                        edge = Some(acc);
                    }
                    if q.p[0].hypot(q.p[1]) > a.bore + 0.9 {
                        low = acc;
                    }
                    y = next;
                }
                let side = if at(x, (c as i64 + dir * 4).clamp(0, AH as i64 - 1) as usize).p[2] < 0.0 { 0 } else { 1 };
                rim[x][side] = edge.unwrap_or(acc).max(0.5);
                wall[x][side] = (low - rim[x][side]).max(0.0);
            }
        }
        // Each column finds its rim on its own, to within a row of the atlas; averaged along the ring the
        // steps laid from it run straight instead of combing the plates between neighbouring columns.
        let smooth = |v: &Vec<[f64; 2]>| -> Vec<[f64; 2]> {
            (0..AW).map(|x| std::array::from_fn(|k| (-6i64..=6).map(|o| v[(x as i64 + o).rem_euclid(AW as i64) as usize][k]).sum::<f64>() / 13.0)).collect()
        };
        let (rim, wall) = (smooth(&rim), smooth(&wall));
        Self { along, across, rim, wall, crest }
    }
    /// The point of the parting line `along` mm from the head's centre, to the atlas's resolution.
    fn crest_point(&self, a: &Atlas, along: f64) -> [f64; 3] {
        let x = (0..AW).min_by(|p, q| (self.along[*p] - along).abs().total_cmp(&(self.along[*q] - along).abs())).unwrap_or(AW / 4);
        a.samples[self.crest[x] * AW + x].p
    }
    /// The chart point on the parting line `along` mm from the head's centre, signed by shoulder: `v` where
    /// the section actually crosses `z = 0`, not the nearest row, which can stand a fiftieth off it.
    fn crest_at(&self, a: &Atlas, along: f64) -> (f64, f64) {
        let x = (0..AW).min_by(|p, q| (self.along[*p] - along).abs().total_cmp(&(self.along[*q] - along).abs())).unwrap_or(AW / 4);
        let at = |y: usize| a.samples[y * AW + x];
        let c = self.crest[x];
        for (y0, y1) in [(c.saturating_sub(1), c), (c, (c + 1).min(AH - 1))] {
            let (p, q) = (at(y0), at(y1));
            if y0 != y1 && p.p[2] * q.p[2] <= 0.0 && p.p[2] != q.p[2] {
                let f = p.p[2] / (p.p[2] - q.p[2]);
                return (p.theta, p.v + (q.v - p.v) * f);
            }
        }
        (at(c).theta, at(c).v)
    }
    /// A sample's `(along, across, rim, wall)`, the last two on its own side.
    fn at(&self, s: Sample) -> (f64, f64, f64, f64) {
        let (x, side) = (s.i % AW, if s.p[2] < 0.0 { 0 } else { 1 });
        (self.along[x], self.across[s.i], self.rim[x][side], self.wall[x][side])
    }
}

/// Where one series of plates is jointed along the ring, from `start` to `end` mm: lengths scattered
/// about the local pitch, so neighbouring series never line up.
struct Joints(Vec<f64>);

impl Joints {
    fn new(start: f64, end: f64, pitch: impl Fn(f64) -> f64, seed: i64) -> Self {
        let mut g = vec![start];
        let mut j = 0;
        while *g.last().unwrap() < end {
            let l = *g.last().unwrap();
            g.push(l + pitch(l) * (0.72 + 0.56 * hash(seed, j)));
            j += 1;
        }
        // Close on `end`: a stub shorter than half a plate joins the plate before it.
        let n = g.len();
        if n >= 3 && end - g[n - 2] < 0.5 * pitch(end) {
            g.remove(n - 2);
        }
        *g.last_mut().unwrap() = end;
        Self(g)
    }
    /// The plate at `l`: its index, how far along it from 0 to 1, and its length.
    fn at(&self, l: f64) -> Option<(usize, f64, f64)> {
        let g = &self.0;
        if g.len() < 2 || l < g[0] || l > g[g.len() - 1] {
            return None;
        }
        let j = g.partition_point(|x| *x <= l).saturating_sub(1).min(g.len() - 2);
        let len = g[j + 1] - g[j];
        Some((j, (l - g[j]) / len, len))
    }
}

/// Clamp a painted layer to the sand's rule and say how much the rule took: a hide drawn to pull should
/// lose next to nothing.
fn clamped(a: &Atlas, mut alpha: Alpha, height_mm: f64) -> Alpha {
    let before = alpha.data.clone();
    draft_clamp(a, &mut alpha, height_mm);
    let (mut cut, mut worst) = (0usize, 0.0f32);
    for (b, c) in before.iter().zip(&alpha.data) {
        if b - c > 0.02 {
            cut += 1;
        }
        worst = worst.max(b - c);
    }
    println!("  {}: the draft rule cut {} texels, at most {:.3} mm", alpha.name, cut, worst as f64 * height_mm);
    alpha
}

/// A painted layer over the whole chart, one tile, windowed round the ring and joined by `Max`, so that
/// each layer keeping the draft rule keeps the whole hide to it.
fn hide_layer(d: &RingDesign, name: &str, height: f64, centre: f64, span: f64) -> LayerEntry {
    let mut e = skin_layer(d, name, height);
    e.blend = Blend::Max;
    e.window = Window::around(centre, span);
    e.window.fade_deg = 6.0;
    e
}

fn caiman(d: &mut RingDesign, lib: &mut AlphaLibrary, a: &Atlas, skin: &Skin) -> Result<()> {
    let setup = d.manufacturing.as_mut().unwrap();
    setup.bench_notes = "Imported-stock master, one crocodile hide from back to belly: dorsal plates in rows across the face and down both shoulders, grading from the spine to the rim, with a crest of horns struck along the spine and the emerald set as the central plate of the nuchal shield; granular flanks studded with bony tubercles on the head's walls; broad ventral scutes across the palm. Every plate steps down away from the parting line and every groove runs round the ring, so the hide pulls as drawn. Z=0 parting, opposed Z withdrawal. At the bench: drill on the raised mark at the face's centre and cut the emerald's seat to the measured stone; punch the pits into the dorsal plates; cut the two lines down the belly that divide its scutes into tiles. Polish the plates' tops and the palm, leave the grooves and granules satin.".into();
    let hide = Hide::new(a);
    // The palm carries six belly scutes a side at an even 2.1 mm, closing on a joint at its centre; the
    // dorsal mosaic runs from the head's centre to where they begin.
    let lmax = hide.along.iter().fold(0.0_f64, |m, l| m.max(l.abs()));
    const BELLY_PITCH: f64 = 2.1;
    let l_belly = lmax - 6.0 * BELLY_PITCH;
    println!("  hide: {lmax:.2} mm to the palm, the belly from {l_belly:.2}");
    // Across the band, three series a side on the face: a horned spine on the parting line, then two
    // stepping down away from it. Each is sloped outward and none rises, so every wall faces round the ring
    // or away from the parting line.
    let series = |along: f64, rim: f64| -> (f64, f64) {
        let l = along.abs();
        ((0.36 + 0.10 * smoothstep(8.0, 24.0, l)) * rim, (0.70 + 0.30 * smoothstep(14.0, 24.0, l)) * rim)
    };
    // Rows across the band as a crocodile's back has them, the same on both shoulders, their lengths
    // scattered a little about the local pitch. The outer series is jointed twice as often, so the scales
    // shrink toward the flank the way the hide's do. Joints line up across the band, so each can run to the
    // full depth: at a joint every series is at its floor together.
    let pitch = |l: f64| 3.4 - 1.2 * smoothstep(6.0, 28.0, l);
    let rows = Joints::new(3.6, l_belly, pitch, 7);
    // The spine's plates carry the crest: a horn struck on each as a stamp, straddling the parting line
    // like the moons did, graded down both shoulders. Where the parting line folds over the head's end wall
    // onto the shoulder it turns at better than 12 degrees a millimetre, where the ring's own round is
    // under 5, and those millimetres are mapped first.
    let turn = |p: [[f64; 3]; 3]| {
        let (u, w): ([f64; 3], [f64; 3]) = (std::array::from_fn(|k| p[1][k] - p[0][k]), std::array::from_fn(|k| p[2][k] - p[1][k]));
        let dot = u[0] * w[0] + u[1] * w[1] + u[2] * w[2];
        (dot / (u.iter().map(|x| x * x).sum::<f64>() * w.iter().map(|x| x * x).sum::<f64>()).sqrt().max(1e-12)).clamp(-1.0, 1.0).acos().to_degrees()
    };
    for sign in [1.0, -1.0] {
        let folds: Vec<f64> = (0..300)
            .map(|i| i as f64 * 0.1)
            .filter(|l| turn([l - 0.4, *l, l + 0.4].map(|o| hide.crest_point(a, sign * o))) > 12.0 * 0.8)
            .collect();
        for (j, pair) in rows.0.windows(2).enumerate() {
            let (centre, len) = (0.5 * (pair[0] + pair[1]), pair[1] - pair[0]);
            let fade = smoothstep(4.0, 28.0, centre);
            // Clear of the plate's ends by 0.6 mm, where the plate is flat along the ring even as a coarse
            // preview mesh smears its joints; a plate too short for that goes without.
            let (a_half, b_half) = ((0.33 * len).min(0.5 * len - 0.6), 0.55 - 0.2 * fade);
            if a_half < 0.4 {
                continue;
            }
            // Not on the fold, nor within a millimetre of it: a horn's flat bottom cannot follow the parting
            // line round the bend (measured 0.06 mm into the drag from one standing on its last kink). The
            // end wall stays bare between the face's crest and the shoulders'.
            if folds.iter().any(|f| *f >= centre - a_half - 1.0 && *f <= centre + a_half + 0.3) {
                continue;
            }
            let (theta, v) = hide.crest_at(a, sign * centre);
            // An elongated hexagon, pointed along the ring: a keel seen from above. Its points are blunted to
            // a short end square to the ring, so no facet beside the parting line leans across it. Its edges
            // carry a point every tenth of a millimetre, so its walls follow a bend instead of chording it.
            let tip = 0.09;
            let corners = [
                [-a_half, -tip], [-0.4 * a_half, -b_half], [0.4 * a_half, -b_half], [a_half, -tip],
                [a_half, tip], [0.4 * a_half, b_half], [-0.4 * a_half, b_half], [-a_half, tip],
            ];
            let mut outline = Vec::new();
            for (c, n) in corners.iter().zip(corners.iter().cycle().skip(1)) {
                let steps = ((c[0] - n[0]).hypot(c[1] - n[1]) / 0.1).ceil().max(1.0) as usize;
                outline.extend((0..steps).map(|q| {
                    let f = q as f64 / steps as f64;
                    [c[0] + (n[0] - c[0]) * f, c[1] + (n[1] - c[1]) * f]
                }));
            }
            d.stamps.push(ringdesign_core::setting::Stamp {
                name: format!("Dorsal crest, {} {}", if sign > 0.0 { "right" } else { "left" }, j + 1),
                theta_deg: theta,
                v_mm: v,
                rot_deg: 0.0,
                outline,
                height_mm: 0.40 - 0.18 * fade,
                sink_mm: 0.3,
                draft_deg: 4.0,
                cut: false,
                bench: false,
                along_pull: false,
                tier: 0,
                top: Default::default(),
            });
        }
    }
    let plate = move |k: usize, along: f64, _across: f64| -> Option<(usize, f64, f64)> {
        let l = along.abs();
        // Beside the emerald the side series run one plate from the head's centre to the stone's end.
        let (j, t, len) = if l < 3.6 {
            if k == 0 {
                return None;
            }
            (0, l / 3.6, 3.6)
        } else {
            let (j, t, len) = rows.at(l)?;
            (j + 1, t, len)
        };
        if k == 2 {
            return Some(if t < 0.5 { (2 * j, 2.0 * t, 0.5 * len) } else { (2 * j + 1, 2.0 * t - 1.0, 0.5 * len) });
        }
        Some((j, t, len))
    };
    // Floor at the joints, and the plate's height at its inner and outer edge, per series.
    const LEVELS: [(f64, f64, f64); 3] = [(0.0, 0.82, 0.82), (0.0, 0.46, 0.36), (0.0, 0.18, 0.08)];
    const DORSAL: f64 = 1.05;
    let dorsal = a.alpha("Dorsal armour", |s| {
        let (along, across, rim, _) = hide.at(s);
        let l = along.abs();
        if l > l_belly {
            return 0.0;
        }
        let w = across.abs();
        let (e0, e1) = series(along, rim);
        let bounds = [(0.0, e0), (e0, e1), (e1, rim)];
        let mut h = [0.0; 3];
        for k in 0..3 {
            let (floor, inner, outer) = LEVELS[k];
            let f = ((w - bounds[k].0) / (bounds[k].1 - bounds[k].0).max(0.1)).clamp(0.0, 1.0);
            h[k] = if k == 0 && l < 3.6 {
                // The emerald's plate: two rows long, flat, the tallest on the ring.
                floor + (1.0 - floor) * smoothstep(0.10, 0.26, 3.6 - l)
            } else if let Some((_, t, len)) = plate(k, along, across) {
                // A loaf along the ring, rising off the joints and flat through the middle. The spine's is
                // keeled on the parting line, and the horn struck over it takes the keel's gable for its top.
                // Under the horn nothing may vary along the ring: a stamp's top copies the surface at the
                // points it drops, and a surface curving both ways there tilts some of its facets across
                // the parting line (phantom obstructions of 0.03-0.07 mm, at every mesh resolution tried).
                let from_joint = t.min(1.0 - t) * len;
                let end = smoothstep(0.08, 0.2, from_joint);
                let middle = smoothstep(0.0, 0.3, from_joint);
                let loaf = 0.88 + 0.12 * middle;
                let keel = if k == 0 {
                    (0.26 - 0.12 * smoothstep(4.0, 28.0, l)) * (1.0 - (w / 0.52).min(1.0)).powf(1.2) * middle
                } else {
                    0.0
                };
                floor + ((inner + (outer - inner) * f) * loaf + keel - floor) * end
            } else {
                floor
            };
        }
        let s0 = smoothstep(e0 - 0.05, e0 + 0.05, w);
        let s1 = smoothstep(e1 - 0.05, e1 + 0.05, w);
        let s2 = smoothstep(rim - 0.30, rim + 0.05, w);
        // Where the mosaic meets the belly it closes on one joint across the band.
        let close = smoothstep(0.10, 0.26, l_belly - l);
        let top = ((h[0] * (1.0 - s0) + h[1] * s0) * (1.0 - s1) + h[2] * s1) * (1.0 - s2);
        top * close * (1.0 - 0.3 * smoothstep(9.0, 30.0, l)) / DORSAL
    });
    let dorsal = clamped(a, dorsal, DORSAL);
    portable(lib, dorsal);
    d.layers.layers.push(hide_layer(d, "Dorsal armour", DORSAL, 90.0, 290.0));
    // Granules over every wall that faces the pull, where any relief casts.
    const FLANK: f64 = 0.30;
    let flank = a.alpha("Flank granules", |s| {
        let (_, across, rim, _) = hide.at(s);
        let rho = s.p[0].hypot(s.p[1]);
        let side = smoothstep(0.62, 0.80, s.n[2].abs()) * smoothstep(a.bore + 0.3, a.bore + 0.75, rho);
        if side <= 0.0 {
            return 0.0;
        }
        let dd = across.abs() - rim;
        let ua = s.p[0].atan2(s.p[1]) * rho;
        let (du, dv) = (0.86, 0.72);
        let mut best: f64 = 0.0;
        let r0 = (dd / dv).round();
        for ri in [r0 - 1.0, r0, r0 + 1.0] {
            let off = if (ri as i64).rem_euclid(2) == 0 { 0.0 } else { 0.5 * du };
            let ci = ((ua - off) / du).round();
            let (cu, cd) = (ci * du + off + 0.12 * (hash(ri as i64, ci as i64 + 5) - 0.5), ri * dv);
            let q = ((ua - cu) / 0.44).hypot((dd - cd) / 0.37) / (0.9 + 0.2 * hash(ri as i64, ci as i64));
            if q < 1.0 {
                best = best.max(0.5 + 0.5 * (PI * q).cos());
            }
        }
        best * side * 0.26 / FLANK
    });
    let flank = clamped(a, flank, FLANK);
    portable(lib, flank);
    d.layers.layers.push(hide_layer(d, "Flank granules", FLANK, 90.0, 359.0));
    // Bony tubercles in rows along the head's walls, graded down as the walls shorten.
    const HORN: f64 = 0.78;
    let horn = a.alpha("Hornback", |s| {
        let at = skin.spot(s);
        let (_, across, rim, wall) = hide.at(s);
        let side = smoothstep(0.78, 0.9, s.n[2].abs());
        if side <= 0.0 || at.away > 112.0 {
            return 0.0;
        }
        let rho = s.p[0].hypot(s.p[1]);
        let dd = across.abs() - rim;
        let ua = s.p[0].atan2(s.p[1]) * rho;
        let size = 1.0 - 0.3 * smoothstep(25.0, 100.0, at.away);
        let mut best: f64 = 0.0;
        // Two rows of osteoderms under the rim, the upper larger, each a rounded knob keeled along the ring.
        for (k, depth) in [1.25, 3.1].into_iter().enumerate() {
            let (ra, rb) = ([0.95, 0.82][k] * size, [0.78, 0.66][k] * size);
            if depth + rb > wall - 0.25 {
                continue;
            }
            let pitch = [2.45, 2.2][k] * size;
            let off = if k % 2 == 1 { 0.5 * pitch } else { 0.0 };
            let cu = ((ua - off) / pitch).round() * pitch + off;
            let q = ((ua - cu) / ra).hypot((dd - depth) / rb);
            if q < 1.0 {
                let knob = (1.0 - q * q).powf(0.8);
                let ridge = (1.0 - ((dd - depth) / (0.35 * rb)).abs().min(1.0)) * (1.0 - q);
                best = best.max(knob * (0.86 + 0.14 * ridge));
            }
        }
        best * side * 0.72 / HORN
    });
    let horn = clamped(a, horn, HORN);
    portable(lib, horn);
    d.layers.layers.push(hide_layer(d, "Hornback", HORN, 90.0, 230.0));
    // Broad scutes across the palm, in step with the dorsal rows: the graver divides them into tiles.
    const BELLY: f64 = 0.30;
    let belly = a.alpha("Belly scutes", |s| {
        let (along, across, rim, _) = hide.at(s);
        let l = along.abs();
        if l < l_belly {
            return 0.0;
        }
        let t = ((l - l_belly) / BELLY_PITCH).fract();
        let ml = smoothstep(0.10, 0.22, t.min(1.0 - t) * BELLY_PITCH);
        let pillow = 0.86 + 0.14 * (PI * t).sin();
        let edge = 1.0 - smoothstep(rim * 0.9, rim + 0.25, across.abs());
        ml * pillow * edge
    });
    let belly = clamped(a, belly, BELLY);
    portable(lib, belly);
    d.layers.layers.push(hide_layer(d, "Belly scutes", BELLY, 270.0, 150.0));
    // After the pour: pits punched into the dorsal plates, and the two lines that tile the belly.
    const GRAVER: f64 = 0.12;
    let graver = a.alpha("Graver's pits and tiles", |s| {
        let (along, across, rim, _) = hide.at(s);
        let (l, w) = (along.abs(), across.abs());
        let side = if across < 0.0 { 1i64 } else { 2 } + if along < 0.0 { 10 } else { 0 };
        let mut cut: f64 = 0.0;
        if l < l_belly && w < rim - 0.3 {
            let (e0, e1) = series(along, rim);
            // A scatter, not a grid: half the cells of a jittered lattice carry a pit, of three sizes, some
            // drawn out along the ring the way a punch skids.
            let cell = 0.78;
            let (gi, gj) = ((l / cell).floor() as i64, (w / cell).floor() as i64);
            for ci in gi - 1..=gi + 1 {
                for cj in gj - 1..=gj + 1 {
                    if hash(ci * 3 + side, cj + 101) > 0.36 {
                        continue;
                    }
                    let pu = (ci as f64 + 0.1 + 0.8 * hash(ci + 7, cj * 5 + side)) * cell;
                    let pw = (cj as f64 + 0.1 + 0.8 * hash(cj + 13, ci * 3 - side)) * cell;
                    let rad = [0.09, 0.13, 0.17][(hash(ci * 11 + side, cj * 17) * 3.0) as usize % 3];
                    let long = 1.0 + 0.8 * hash(ci * 5 - side, cj * 7 + 3);
                    let depth = 0.6 + 0.4 * hash(ci * 13, cj * 19 + side);
                    // Only on a plate's own top: clear of the joints, the steps, the horn and the stone.
                    let k = if pw < e0 { 0 } else if pw < e1 { 1 } else { 2 };
                    let on_plate = plate(k, pu.copysign(along), across)
                        .is_some_and(|(_, t, len)| t.min(1.0 - t) * len > 0.34 + rad * long && pu < l_belly - 0.4);
                    let clear = on_plate
                        && k < 2
                        && (pw - e0).abs() > 0.2 + rad
                        && (pw - e1).abs() > 0.2 + rad
                        && pw < rim - 0.45
                        && !(k == 0 && pw < 0.7 + rad);
                    let q = ((l - pu) / long).hypot(w - pw);
                    if clear && q < rad {
                        cut = cut.max(depth * dome(q, rad));
                    }
                }
            }
        }
        if l > l_belly {
            let t = ((l - l_belly) / BELLY_PITCH).fract();
            let on = smoothstep(0.30, 0.42, t.min(1.0 - t) * BELLY_PITCH);
            let line = 1.0 - smoothstep(0.035, 0.085, (w - 0.36 * rim).abs());
            cut = cut.max(line * on * (1.0 - smoothstep(rim * 0.85, rim, w)));
        }
        cut * 0.10 / GRAVER
    });
    portable(lib, graver);
    let mut cut = hide_layer(d, "Graver's pits and tiles", GRAVER, 90.0, 359.0);
    cut.blend = Blend::Subtract;
    cut.bench_only = true;
    d.layers.layers.push(cut);
    // The emerald is the central plate of the nuchal shield: a boss the plate's own height, cut flush.
    let (theta, v) = skin.on_face(0.0, 0.0);
    let mut seat = SeatPadLayer {
        theta_deg: theta,
        v_mm: v,
        style: SeatStyle::Boss,
        crown: 0.15,
        blend_mm: 0.4,
        metal_true: true,
        solid: ringdesign_core::setting::SolidKind::Flush,
        through: true,
        ..Default::default()
    };
    seat.fit_stone(Gem::calibrated(GemCut::Emerald, 4.0));
    seat.height_mm = 1.0;
    let mut e = LayerEntry::new("Emerald on the nuchal shield", Layer::SeatPad(seat));
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    Ok(())
}

/// Half the band's width at a sample's own station, mm.
fn self_half(skin: &Skin, s: Sample) -> f64 {
    skin.half[((s.theta / 360.0 * AW as f64).round() as usize) % AW]
}

fn write(out: &Path, slug: &str, draft: bool) -> Result<()> {
    std::fs::create_dir(out)?;
    let (mut d, lib) = decorate(slug)?;
    let params = BuildParams {
        theta_steps: if draft { 900 } else { 1536 },
        profile_steps: 448,
        ..d.build
    };
    d.build = params;
    let built = ringdesign_core::mesh::try_build(&d, &lib, params)?;
    println!(
        "{}: {} triangles, {:.2} mm bore, {:.0} ms",
        d.name,
        built.mesh.faces.len(),
        built.report.inner_diameter_mm,
        built.report.build_ms
    );
    for f in ringdesign_core::dfm::findings_in(&d, &lib) {
        println!("  dfm: {}: {}", f.label, f.message);
    }
    if built.solids.resolved + built.solids.stamped > 0 || !built.solids.notes.is_empty() {
        println!("  made settings: {} resolved, {} stamps, in {} ms {:?}", built.solids.resolved, built.solids.stamped, built.solids.ms, built.solids.notes);
    }
    ringdesign_core::library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let saved = ringdesign_core::library::load_design(out.join("design.ring.json"))?;
    let cold = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
    let rebuilt = ringdesign_core::mesh::try_build(&saved, &cold, params)?;
    ensure!(
        rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces,
        "Saved design changed geometry"
    );
    let gems = ringdesign_core::gems::preview_mesh(&d, &lib);
    let tint = if slug == "solstice" {
        [0.79, 0.79, 0.77]
    } else {
        render::GOLD
    };
    let mut parts = vec![Part::metal(&built.mesh, tint)];
    if let Some(g) = &gems {
        let mut p = Part::stone(g);
        p.tint = if slug == "vesper" {
            [0.16, 0.22, 0.61]
        } else {
            [0.06, 0.45, 0.24]
        };
        parts.push(p);
    }
    for (name, yaw, pitch) in [
        ("hero", 0.48, 1.),
        ("face", 0., PI * 0.5),
        ("edge", 0.92, 0.42),
        ("cheek", 0., 0.22),
        ("shoulder", 1.25, 1.05),
        ("reverse", PI, 0.8),
        ("palm", PI, PI * 0.5),
    ] {
        render::write_png_parts(
            out.join(format!("{name}.png")),
            &parts,
            yaw,
            pitch,
            if draft { 1100 } else { 1600 },
        )?;
    }
    let mut bare = d.clone();
    bare.imported_base.as_mut().unwrap().bare = true;
    let b = ringdesign_core::mesh::try_build(&bare, &lib, params)?;
    render::write_png(out.join("bare.png"), &b.mesh, 0.48, 1., 1100, tint)?;
    let setup = d.manufacturing.as_ref().unwrap();
    let inspection = mf::inspect(&d, &lib, setup, params)?;
    std::fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&mf::package::report(&d, setup, &inspection, false))?,
    )?;
    std::fs::write(
        out.join("mesh.json"),
        serde_json::to_vec_pretty(&built.report)?,
    )?;
    println!(
        "{slug}: {:?}, {} obstructions, {} unresolved",
        inspection.release.status,
        inspection.release.obstructions.len(),
        inspection.release.unresolved_rays
    );
    if setup.recipe.process == CastProcess::SandTwoPart {
        // The template test inspects the preview-sized build, where a coarse mesh can put a ridge a hair
        // off the parting line: the ring has to pull there too.
        let coarse = mf::inspect(&d, &lib, setup, BuildParams { theta_steps: 384, profile_steps: 192, ..params })?;
        println!("{slug} at 384 x 192: {} obstructions {:?}", coarse.release.obstructions.len(), coarse.release.obstructions.iter().map(|o| o.world.map(|v| (v * 100.0).round() / 100.0)).collect::<Vec<_>>());
    }
    if !draft {
        // Mesh files are the pattern: no bench cuts, a raised drill mark on every seat.
        let pattern = ringdesign_core::mesh::try_build_pattern(&d, &lib, params)?;
        // What the founder is handed, seen from above: no seats, a raised dot where each drill starts.
        render::write_png(out.join("pattern-face.png"), &pattern.mesh, 0., PI * 0.5, 1600, tint)?;
        render::write_png(out.join("pattern-hero.png"), &pattern.mesh, 0.48, 1., 1600, tint)?;
        ringdesign_core::stl::write_stl(out.join("nominal.stl"), &pattern.mesh, &d.name)?;
        ringdesign_core::threemf::write_3mf(
            out.join("nominal.3mf"),
            &pattern.mesh,
            &d.name,
            &d.size.display(),
        )?;
        let mut fine = setup.clone();
        fine.sample_pitch_mm = 0.075;
        let release = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
        std::fs::write(
            out.join("release-fine.json"),
            serde_json::to_vec_pretty(&release)?,
        )?;
        if setup.recipe.process == CastProcess::SandTwoPart {
            ensure!(
                release.obstructions.is_empty() && release.unresolved_rays == 0,
                "Sand design has withdrawal obstructions"
            );
        }
        mf::package::export(&out.join("pattern-package"), &d, &lib, setup, params, false)?;
    }
    let art = out.join("artwork");
    std::fs::create_dir(&art)?;
    for svg in &d.svgs {
        std::fs::write(
            art.join(format!("{}.svg", svg.name.replace(' ', "-"))),
            &svg.svg,
        )?;
    }
    for name in d.layers.referenced_alphas() {
        if let Some(a) = lib.get(name) {
            std::fs::write(
                art.join(format!("{}.png", name.replace([' ', '/'], "-"))),
                a.to_png16()?,
            )?;
        }
    }
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        !args.is_empty(),
        "stock_masterworks NEW_DIRECTORY [--draft] [nocturne|solstice|aurelia|vesper]"
    );
    let out = Path::new(&args[0]);
    std::fs::create_dir_all(out)?;
    let draft = args.iter().any(|x| x == "--draft");
    const ALL: [&str; 7] = ["nocturne", "solstice", "aurelia", "vesper", "saurian", "zenith", "caiman"];
    for slug in ALL {
        if args.len() > 1
            && args
                .iter()
                .any(|s| ALL.contains(&s.as_str()))
            && !args.iter().any(|s| s == slug)
        {
            continue;
        }
        write(&out.join(slug), slug, draft)?;
    }
    Ok(())
}
