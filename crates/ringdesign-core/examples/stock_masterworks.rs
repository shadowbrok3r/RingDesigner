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
    imported_base::{ImportedBase, PRESETS, Source, SurfaceChart},
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
// Add vertices before shaping the drafted master: a flat source face may
// contain very large triangles, which would otherwise turn a smooth crown
// into two planes meeting at a ridge.
fn refine_master(s: &mut Source) -> Result<()> {
    for _ in 0..10 {
        let mut mids = std::collections::HashMap::new();
        for f in &s.faces {
            for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                let key = (a.min(b), a.max(b));
                if mids.contains_key(&key) {
                    continue;
                }
                let p = s.vertices[a as usize];
                let q = s.vertices[b as usize];
                if p.iter().zip(q).map(|(a, b)| (a - b).powi(2)).sum::<f64>() > 0.55_f64.powi(2) {
                    mids.insert(key, s.vertices.len() as u32);
                    s.vertices
                        .push(std::array::from_fn(|i| (p[i] + q[i]) * 0.5));
                }
            }
        }
        if mids.is_empty() {
            break;
        }
        let mut faces = Vec::new();
        for [a, b, c] in s.faces.drain(..) {
            let get = |a: u32, b: u32| mids.get(&(a.min(b), a.max(b))).copied();
            match (get(a, b), get(b, c), get(c, a)) {
                (None, None, None) => faces.push([a, b, c]),
                (Some(d), None, None) => faces.extend([[a, d, c], [d, b, c]]),
                (None, Some(e), None) => faces.extend([[b, e, a], [e, c, a]]),
                (None, None, Some(f)) => faces.extend([[c, f, b], [f, a, b]]),
                (Some(d), Some(e), None) => faces.extend([[d, b, e], [a, d, c], [d, e, c]]),
                (None, Some(e), Some(f)) => faces.extend([[e, c, f], [b, e, a], [e, f, a]]),
                (Some(d), None, Some(f)) => faces.extend([[f, a, d], [c, f, b], [f, d, b]]),
                (Some(d), Some(e), Some(f)) => {
                    faces.extend([[a, d, f], [d, b, e], [f, e, c], [d, e, f]])
                }
            }
        }
        s.faces = faces;
        ensure!(
            s.faces.len() <= 400_000,
            "Drafted master exceeds source budget"
        );
    }
    Ok(())
}
fn sand_stock(source: std::sync::Arc<Source>) -> Result<std::sync::Arc<Source>> {
    let mut s: Source = serde_json::from_value(serde_json::to_value(&*source)?)?;
    let bore = s.calibration.bore_radius_mm;
    let min_z = s
        .vertices
        .iter()
        .map(|p| p[2])
        .fold(f64::INFINITY, f64::min);
    let max_z = s
        .vertices
        .iter()
        .map(|p| p[2])
        .fold(f64::NEG_INFINITY, f64::max);
    let z0 = (min_z + max_z) * 0.5;
    // A sand master needs an actual edge loop at the parting plane. Merely
    // adding draft to triangles crossing it puts their maxima off-plane.
    // Clip the upper half and reflect it, retaining the imported contour.
    let old = s.vertices.clone();
    let old_faces = s.faces.clone();
    s.vertices.clear();
    s.faces.clear();
    let mut welded = std::collections::HashMap::new();
    let mut index = |p: [f64; 3], vertices: &mut Vec<[f64; 3]>| {
        let key = p.map(|x| (x * 1e6).round() as i64);
        *welded.entry(key).or_insert_with(|| {
            let id = vertices.len() as u32;
            vertices.push(p);
            id
        })
    };
    for f in old_faces {
        let tri = f.map(|i| {
            let p = old[i as usize];
            [p[0], p[1], p[2] - z0]
        });
        let mut poly = Vec::new();
        for i in 0..3 {
            let a = tri[i];
            let b = tri[(i + 1) % 3];
            if a[2] >= 0. {
                poly.push(a);
            }
            if (a[2] >= 0.) != (b[2] >= 0.) {
                let t = -a[2] / (b[2] - a[2]);
                poly.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, 0.]);
            }
        }
        for k in 1..poly.len().saturating_sub(1) {
            let tri = [poly[0], poly[k], poly[k + 1]];
            let upper = tri.map(|p| index(p, &mut s.vertices));
            let lower = tri.map(|p| index([p[0], p[1], -p[2]], &mut s.vertices));
            if upper[0] != upper[1] && upper[1] != upper[2] && upper[0] != upper[2] {
                s.faces.push(upper);
                s.faces.push([lower[0], lower[2], lower[1]]);
            }
        }
    }
    refine_master(&mut s)?;
    let mut half = vec![0_f64; 360];
    for p in &s.vertices {
        let t = p[1].atan2(p[0]).to_degrees().rem_euclid(360.).round() as usize % 360;
        for k in 0..=8 {
            for i in [(t + k) % 360, (t + 360 - k) % 360] {
                half[i] = half[i].max(p[2].abs());
            }
        }
    }
    for p in &mut s.vertices {
        let r = p[0].hypot(p[1]);
        let t = p[1].atan2(p[0]).to_degrees().rem_euclid(360.);
        let j = t.floor() as usize;
        let f = t - j as f64;
        let h = half[j] * (1. - f) + half[(j + 1) % 360] * f;
        let skin = smoothstep(bore + 0.12, bore + 0.65, r);
        let head = smoothstep(
            bore + s.calibration.head_height_mm * 0.35,
            bore + s.calibration.head_height_mm * 0.8,
            p[1],
        );
        let drafted = r + (0.045 + 0.16 * head) * (h - (p[2] * p[2] + 0.36).sqrt()).max(0.);
        let inner = bore + 0.012 * p[2].abs();
        let rr = if r < bore + 0.65 {
            inner * (1. - skin) + drafted * skin
        } else {
            drafted
        };
        p[0] *= rr / r;
        p[1] *= rr / r;
    }
    s.name = format!("{} / drafted workshop master", source.name);
    s.calibration.head_height_mm = s.vertices.iter().map(|p| p[1]).fold(0_f64, f64::max) - bore;
    s.calibration.palm_thickness_mm = s
        .vertices
        .iter()
        .filter(|p| p[1] < -bore && p[0].abs() < 0.12)
        .map(|p| p[0].hypot(p[1]) - bore)
        .fold(0_f64, f64::max);
    s.calibration.shoulder_end_mm = bore + s.calibration.head_height_mm * 0.8;
    Source::from_json(&serde_json::to_string(&s)?)
}
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
        _ => anyhow::bail!("Unknown design {slug}"),
    };
    let mut d = RingDesign::default();
    let source = PRESETS
        .iter()
        .find(|p| p.name.starts_with(id))
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
    if !draft {
        ringdesign_core::stl::write_stl(out.join("nominal.stl"), &built.mesh, &d.name)?;
        ringdesign_core::threemf::write_3mf(
            out.join("nominal.3mf"),
            &built.mesh,
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
    for slug in ["nocturne", "solstice", "aurelia", "vesper"] {
        if args.len() > 1
            && args
                .iter()
                .any(|s| ["nocturne", "solstice", "aurelia", "vesper"].contains(&s.as_str()))
            && !args.iter().any(|s| s == slug)
        {
            continue;
        }
        write(&out.join(slug), slug, draft)?;
    }
    Ok(())
}
