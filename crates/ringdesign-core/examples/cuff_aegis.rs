//! The trident cuff carrying the Aegis of Olympus ornament: a composed relief map on the swept body, the figures joined as solids.
//! cargo run --release -p ringdesign-core --example cuff_aegis -- WORK_DIR [OUT_DIR] [--step MM] [--keep FRACTION] [--no-figures]
use anyhow::{Context, Result, bail, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, DraftSettings, RingDesign,
    cad::{self, Component, ComponentRole, Document, Feature, Operation, stored},
    castability::CastProcess,
    csg::{self, Op, Solid},
    library,
    mesh::{self, Mesh, Vec3},
    render, sculpt, stl,
};
use serde_json::Value;
use std::f64::consts::FRAC_PI_2;
use std::path::{Path, PathBuf};
use std::time::Instant;

const NAME: &str = "Trident cuff, Aegis of Olympus";
const SILVER_G_PER_MM3: f64 = 0.01036;
const GOLD14_G_PER_MM3: f64 = 0.01307;
const FIGURES: [&str; 3] = ["center", "female", "snake"];

/// The cuff's outline and thickness laws, read from the layout; `s` is arc length on the outer face from the crest's centre.
struct Cuff {
    r_bore: f64,
    t0: f64,
    band: f64,
    crest: f64,
    crest_deg: f64,
    crest_pow: f64,
    tip_deg: f64,
    rise: f64,
    r_ref: f64,
    s_end: f64,
    corner: f64,
    end_round: f64,
    edge: f64,
    bore_round: f64,
}

fn num(v: &Value, key: &str) -> Result<f64> {
    v.get(key).and_then(Value::as_f64).with_context(|| format!("layout has no number `{key}`"))
}

impl Cuff {
    fn from(v: &Value) -> Result<Self> {
        let r_bore = 0.5 * num(v, "bore_mm")?;
        let t0 = num(v, "thick_mm")?;
        let r_ref = r_bore + t0;
        let phi_end = std::f64::consts::PI - (0.5 * num(v, "gap_mm")? / r_bore).asin();
        Ok(Self {
            r_bore,
            t0,
            band: num(v, "band_mm")?,
            crest: num(v, "crest_mm")?,
            crest_deg: num(v, "crest_deg")?,
            crest_pow: num(v, "crest_pow")?,
            tip_deg: num(v, "tip_deg")?,
            rise: num(v, "crest_rise_mm")?,
            r_ref,
            s_end: phi_end * r_ref,
            corner: num(v, "end_corner_mm")?,
            end_round: num(v, "end_round_mm")?,
            edge: num(v, "edge_mm")?,
            bore_round: num(v, "bore_round_mm")?,
        })
    }

    fn share(&self, s: f64) -> f64 {
        let phi = (s / self.r_ref).to_degrees();
        let soft = (phi * phi + self.tip_deg * self.tip_deg).sqrt() - self.tip_deg;
        (1.0 - soft / self.crest_deg).max(0.0).powf(self.crest_pow)
    }

    /// Half the band's axial width, with the opening's corners rounded in plan.
    fn half_width(&self, s: f64) -> f64 {
        let base = 0.5 * (self.band + (self.crest - self.band) * self.share(s));
        let x = s.abs() - (self.s_end - self.corner);
        if x > 0.0 { base - self.corner + (self.corner * self.corner - x * x).max(0.0).sqrt() } else { base }
    }

    fn thick(&self, s: f64) -> f64 {
        self.t0 + self.rise * self.share(s)
    }

    /// How far the section draws in on every side over the last `end_round` of each end.
    fn end_inset(&self, s: f64) -> f64 {
        let x = s.abs() - (self.s_end - self.end_round);
        if x > 0.0 { self.end_round - (self.end_round * self.end_round - x * x).max(0.0).sqrt() } else { 0.0 }
    }

    fn point(&self, s: f64, r: f64, z: f64) -> [f64; 3] {
        let th = FRAC_PI_2 + s / self.r_ref;
        [r * th.cos(), r * th.sin(), z]
    }
}

/// The composed relief, mm of displacement off the outer face, on a regular `(s, z)` grid.
struct Relief {
    s0: f64,
    z0: f64,
    ds: f64,
    ns: usize,
    nz: usize,
    h: Vec<f32>,
}

impl Relief {
    fn load(dir: &Path) -> Result<Self> {
        let meta: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("target_relief.json"))?)?;
        let ns = num(&meta, "ns")? as usize;
        let nz = num(&meta, "nz")? as usize;
        let bytes = std::fs::read(dir.join("target_relief.f32"))?;
        ensure!(bytes.len() == 4 * ns * nz, "the relief map holds {} bytes, not {}", bytes.len(), 4 * ns * nz);
        let h = bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        Ok(Self { s0: num(&meta, "s0")?, z0: num(&meta, "z0")?, ds: num(&meta, "ds")?, ns, nz, h })
    }

    fn at(&self, s: f64, z: f64) -> f64 {
        let fx = ((s - self.s0) / self.ds).clamp(0.0, (self.ns - 1) as f64 - 1e-9);
        let fy = ((z - self.z0) / self.ds).clamp(0.0, (self.nz - 1) as f64 - 1e-9);
        let (i, j) = (fx as usize, fy as usize);
        let (u, v) = (fx - i as f64, fy - j as f64);
        let g = |a: usize, b: usize| f64::from(self.h[b * self.ns + a]);
        (1.0 - v) * ((1.0 - u) * g(i, j) + u * g(i + 1, j)) + v * ((1.0 - u) * g(i, j + 1) + u * g(i + 1, j + 1))
    }
}

/// Points along an arc of `n` steps about `c`, from `a0` to `a1` radians, endpoints as asked.
fn arc(c: [f64; 2], rad: f64, a0: f64, a1: f64, n: usize, first: bool, last: bool) -> Vec<[f64; 2]> {
    let lo = if first { 0 } else { 1 };
    let hi = if last { n } else { n - 1 };
    (lo..=hi).map(|k| a0 + (a1 - a0) * k as f64 / n as f64).map(|a| [c[0] + rad * a.cos(), c[1] + rad * a.sin()]).collect()
}

/// One section's `(r, z)` points in loop order, split where the outer face's own rows begin and end.
struct Section {
    lead: Vec<[f64; 2]>,
    face: Vec<[f64; 2]>,
    rest: Vec<[f64; 2]>,
}

/// One section at `s`: the run up to the outer face, the outer face's inner rows, and the rest of the loop.
fn section(c: &Cuff, relief: &Relief, s: f64, step: f64) -> Section {
    const N_SIDE: usize = 6;
    const N_ARC: usize = 8;
    const N_BORE_ARC: usize = 6;
    const N_BORE: usize = 24;
    let e = c.end_inset(s);
    let hw = c.half_width(s) - e;
    let (r_in, r_out) = (c.r_bore + e, c.r_bore + c.thick(s) - e);
    let ec = c.edge.min(0.45 * (r_out - r_in)).min(0.45 * hw);
    let eb = c.bore_round.min(0.45 * (r_out - r_in)).min(0.45 * hw);
    let mut a: Vec<[f64; 2]> = (0..=N_SIDE).map(|k| [r_in + eb + (r_out - ec - r_in - eb) * k as f64 / N_SIDE as f64, -hw]).collect();
    a.extend(arc([r_out - ec, -hw + ec], ec, -FRAC_PI_2, 0.0, N_ARC, false, true));
    let span = 2.0 * (hw - ec);
    let m = ((span / step).round() as usize).max(2);
    let b: Vec<[f64; 2]> = (1..m)
        .map(|k| {
            let z = -hw + ec + span * k as f64 / m as f64;
            [r_out + relief.at(s, z), z]
        })
        .collect();
    let mut rest = arc([r_out - ec, hw - ec], ec, 0.0, FRAC_PI_2, N_ARC, true, true);
    rest.extend((1..=N_SIDE).map(|k| [r_out - ec - (r_out - ec - r_in - eb) * k as f64 / N_SIDE as f64, hw]));
    rest.extend(arc([r_in + eb, hw - eb], eb, FRAC_PI_2, std::f64::consts::PI, N_BORE_ARC, false, true));
    rest.extend((1..=N_BORE).map(|k| [r_in, hw - eb - (2.0 * (hw - eb)) * k as f64 / N_BORE as f64]));
    rest.extend(arc([r_in + eb, -hw + eb], eb, std::f64::consts::PI, 1.5 * std::f64::consts::PI, N_BORE_ARC, false, false));
    Section { lead: a, face: b, rest }
}

/// Arc positions of the sweep's slices: even steps, then each end's rounding walked by angle.
fn slices(c: &Cuff, step: f64) -> Vec<f64> {
    let inner = c.s_end - c.end_round;
    let n = ((2.0 * inner) / step).ceil() as usize;
    let mut s: Vec<f64> = (0..=n).map(|k| -inner + 2.0 * inner * k as f64 / n as f64).collect();
    let n_end = 16;
    let tail: Vec<f64> = (1..=n_end).map(|k| inner + c.end_round * (FRAC_PI_2 * k as f64 / n_end as f64).sin()).collect();
    let mut head: Vec<f64> = tail.iter().map(|x| -x).collect();
    head.reverse();
    head.append(&mut s);
    head.extend(tail);
    head
}

/// The swept open cuff with the relief on its outer face, capped at both ends.
fn body(c: &Cuff, relief: &Relief, step: f64) -> Solid {
    let ss = slices(c, step);
    let mut v: Vec<[f64; 3]> = Vec::new();
    let mut cols: Vec<(usize, usize, usize, usize)> = Vec::with_capacity(ss.len());
    for &s in &ss {
        let sec = section(c, relief, s, step);
        let start = v.len();
        for p in sec.lead.iter().chain(&sec.face).chain(&sec.rest) {
            v.push(c.point(s, p[0], p[1]));
        }
        cols.push((start, sec.lead.len(), sec.face.len(), sec.rest.len()));
    }
    let mut f: Vec<[u32; 3]> = Vec::new();
    for w in cols.windows(2) {
        let ((p0, pa, pb, pc), (q0, qa, qb, qc)) = (w[0], w[1]);
        let (pn, qn) = (pa + pb + pc, qa + qb + qc);
        let mut quad = |i0: usize, i1: usize, j0: usize, j1: usize| {
            f.push([i0 as u32, j0 as u32, j1 as u32]);
            f.push([i0 as u32, j1 as u32, i1 as u32]);
        };
        for k in 0..pa - 1 {
            quad(p0 + k, p0 + k + 1, q0 + k, q0 + k + 1);
        }
        for k in 0..pc - 1 {
            quad(p0 + pa + pb + k, p0 + pa + pb + k + 1, q0 + qa + qb + k, q0 + qa + qb + k + 1);
        }
        quad(p0 + pn - 1, p0, q0 + qn - 1, q0);
        let p: Vec<usize> = (p0 + pa - 1..=p0 + pa + pb).collect();
        let q: Vec<usize> = (q0 + qa - 1..=q0 + qa + qb).collect();
        let (mut i, mut j) = (0, 0);
        while i + 1 < p.len() || j + 1 < q.len() {
            let take_q = i + 1 >= p.len() || (j + 1 < q.len() && v[q[j + 1]][2] <= v[p[i + 1]][2]);
            if take_q {
                f.push([p[i] as u32, q[j] as u32, q[j + 1] as u32]);
                j += 1;
            } else {
                f.push([p[i] as u32, q[j] as u32, p[i + 1] as u32]);
                i += 1;
            }
        }
    }
    for (end, (o, a, b, r)) in [(false, cols[0]), (true, cols[cols.len() - 1])] {
        let n = a + b + r;
        let mut centre = [0.0; 3];
        for k in 0..n {
            for d in 0..3 {
                centre[d] += v[o + k][d] / n as f64;
            }
        }
        let ci = v.len() as u32;
        v.push(centre);
        for k in 0..n {
            let (x, y) = ((o + k) as u32, (o + (k + 1) % n) as u32);
            f.push(if end { [ci, y, x] } else { [ci, x, y] });
        }
    }
    Solid { v, f }
}

/// A figure as written by the extraction: vertices in its source frame `(s, z, height)`, faces wound as meshed.
fn read_figure(path: &Path) -> Result<Solid> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    ensure!(bytes.len() >= 16, "{} is too short", path.display());
    let nv = u64::from_le_bytes(bytes[0..8].try_into()?) as usize;
    let nf = u64::from_le_bytes(bytes[8..16].try_into()?) as usize;
    ensure!(bytes.len() == 16 + 24 * nv + 12 * nf, "{} has the wrong length", path.display());
    let f64s = |o: usize| f64::from_le_bytes(bytes[o..o + 8].try_into().unwrap_or([0; 8]));
    let u32s = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap_or([0; 4]));
    let v = (0..nv).map(|i| [f64s(16 + 24 * i), f64s(24 + 24 * i), f64s(32 + 24 * i)]).collect();
    let fo = 16 + 24 * nv;
    let f = (0..nf).map(|i| [u32s(fo + 12 * i), u32s(fo + 12 * i + 4), u32s(fo + 12 * i + 8)]).collect();
    Ok(Solid { v, f })
}

/// A figure carried from its source frame, `(s, z, height over its own ground)`, onto the cuff's panel floor.
fn figure(c: &Cuff, dir: &Path, name: &str, hero: &Value, floor: f64, sink: f64) -> Result<Solid> {
    let Solid { v: src, f } = read_figure(&dir.join(format!("motif_{name}.bin")))?;
    let k = num(hero, "k")?;
    let kh = num(hero, "kh")?;
    let pair = |key: &str| -> Result<[f64; 2]> {
        let a = hero.get(key).and_then(Value::as_array).with_context(|| format!("figure {name} has no `{key}`"))?;
        Ok([a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0)])
    };
    let (from, to) = (pair("src")?, pair("dst")?);
    let v = src
        .iter()
        .map(|p| {
            let s = to[0] + k * (p[0] - from[0]);
            let z = to[1] + k * (p[1] - from[1]);
            c.point(s, c.r_bore + c.thick(s) + floor - sink + kh * p[2], z)
        })
        .collect();
    let mut solid = Solid { v, f };
    if solid.volume() < 0.0 {
        for t in &mut solid.f {
            t.swap(1, 2);
        }
    }
    Ok(solid)
}

/// A closed torus: centre, two unit axes spanning its plane, ring radius `big` and tube radius `small`.
fn torus(c: [f64; 3], a: [f64; 3], b: [f64; 3], big: f64, small: f64, n: usize, m: usize) -> Solid {
    let n_out = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let mut v = Vec::with_capacity(n * m);
    for i in 0..n {
        let (su, cu) = (std::f64::consts::TAU * i as f64 / n as f64).sin_cos();
        let radial = [a[0] * cu + b[0] * su, a[1] * cu + b[1] * su, a[2] * cu + b[2] * su];
        for j in 0..m {
            let (sv, cv) = (std::f64::consts::TAU * j as f64 / m as f64).sin_cos();
            v.push(std::array::from_fn(|k| c[k] + radial[k] * (big + small * cv) + n_out[k] * small * sv));
        }
    }
    let at = |i: usize, j: usize| ((i % n) * m + j % m) as u32;
    let mut f = Vec::with_capacity(2 * n * m);
    for i in 0..n {
        for j in 0..m {
            f.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            f.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    let mut t = Solid { v, f };
    if t.volume() < 0.0 {
        for x in &mut t.f {
            x.swap(1, 2);
        }
    }
    t
}

/// A closed cylinder from `p0` to `p1`.
fn cylinder(p0: [f64; 3], p1: [f64; 3], radius: f64, n: usize) -> Solid {
    let ax = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let len = (ax[0] * ax[0] + ax[1] * ax[1] + ax[2] * ax[2]).sqrt().max(1e-9);
    let w = [ax[0] / len, ax[1] / len, ax[2] / len];
    let seed = if w[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let dotp = seed[0] * w[0] + seed[1] * w[1] + seed[2] * w[2];
    let u0 = [seed[0] - dotp * w[0], seed[1] - dotp * w[1], seed[2] - dotp * w[2]];
    let ul = (u0[0] * u0[0] + u0[1] * u0[1] + u0[2] * u0[2]).sqrt();
    let u = [u0[0] / ul, u0[1] / ul, u0[2] / ul];
    let vv = [w[1] * u[2] - w[2] * u[1], w[2] * u[0] - w[0] * u[2], w[0] * u[1] - w[1] * u[0]];
    let mut v = Vec::with_capacity(2 * n + 2);
    for end in [p0, p1] {
        for k in 0..n {
            let (sa, ca) = (std::f64::consts::TAU * k as f64 / n as f64).sin_cos();
            v.push(std::array::from_fn(|d| end[d] + radius * (u[d] * ca + vv[d] * sa)));
        }
    }
    v.push(p0);
    v.push(p1);
    let (c0, c1) = ((2 * n) as u32, (2 * n + 1) as u32);
    let mut f = Vec::with_capacity(4 * n);
    for k in 0..n {
        let (a, b) = (k as u32, ((k + 1) % n) as u32);
        let (a1, b1) = (a + n as u32, b + n as u32);
        f.push([a, b, b1]);
        f.push([a, b1, a1]);
        f.push([c0, b, a]);
        f.push([c1, a1, b1]);
    }
    let mut t = Solid { v, f };
    if t.volume() < 0.0 {
        for x in &mut t.f {
            x.swap(1, 2);
        }
    }
    t
}

/// The chain loop at a crest tip: a ring lying in the band's own surface, on a neck sunk into the tip.
fn chain_loop(c: &Cuff, spec: &Value) -> Result<Solid> {
    let sign = if spec.get("edge").and_then(Value::as_str) == Some("bottom") { -1.0 } else { 1.0 };
    let big = num(spec, "ring_mm")?;
    let small = 0.5 * num(spec, "wire_mm")?;
    let neck = num(spec, "neck_mm")?;
    let tip = c.half_width(0.0);
    let r_mid = c.r_bore + 0.5 * c.thick(0.0);
    let z_ring = sign * (tip + 0.6 + big);
    let ring = torus([0.0, r_mid, z_ring], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], big, small, 96, 28);
    let stem = cylinder([0.0, r_mid, sign * (tip - 1.6)], [0.0, r_mid, sign * (tip + 0.6 + 0.4 * small)], 0.5 * neck, 48);
    csg::combine(&stem, &ring, Op::Union).map_err(|e| anyhow::anyhow!("the chain loop's neck and ring would not join: {e}"))
}

fn to_mesh(s: &Solid) -> Mesh {
    let mut acc = vec![[0f64; 3]; s.v.len()];
    for t in &s.f {
        let [a, b, c] = t.map(|i| s.v[i as usize]);
        let (u, w) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
        for &i in t {
            for d in 0..3 {
                acc[i as usize][d] += n[d];
            }
        }
    }
    let normals = acc
        .iter()
        .map(|n| {
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-300);
            Vec3((n[0] / l) as f32, (n[1] / l) as f32, (n[2] / l) as f32)
        })
        .collect();
    let vertices = s.v.iter().map(|p| Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect();
    Mesh { vertices, normals, faces: s.f.clone(), ..Mesh::default() }
}

fn check(key: &str, s: &Solid) -> Result<()> {
    let (open, repeated) = s.open_edges();
    let crossings = csg::self_crossings(s);
    println!("  {key}: {} faces, {} vertices, open {open}, repeated {repeated}, self-crossings {crossings}, volume {:.0} mm³", s.f.len(), s.v.len(), s.volume());
    ensure!(open == 0 && repeated == 0, "{key} is not closed");
    ensure!(crossings == 0, "{key} crosses itself at {crossings} face pairs");
    Ok(())
}

fn design(solid: &Solid, layout: &Value) -> Result<RingDesign> {
    let mut doc = Document::default();
    let recipe = stored::Recipe { kernel: "cuff_aegis".into(), op: "import".into(), params: layout.clone(), digest: String::new() };
    let mesh = sculpt::packed(solid)?;
    let feature = Feature {
        id: 1,
        name: "Aegis cuff".into(),
        enabled: true,
        operation: Operation::Stored { recipe, sources: Vec::new(), mesh },
        component: Component { role: ComponentRole::Shank, ..Component::default() },
    };
    doc.append(feature)?;
    Ok(RingDesign {
        name: NAME.into(),
        draft: DraftSettings { process: CastProcess::LostWax, ..DraftSettings::default() },
        cad: Some(doc),
        ..RingDesign::default()
    })
}

/// The cuff turned upright for the camera, then a full front, a close front of the crest, three quarters, a side, a plan and the chain loop.
fn renders(out: &Path, stem: &str, mesh: &Mesh) -> Result<()> {
    let mut up = mesh.clone();
    for p in up.vertices.iter_mut().chain(up.normals.iter_mut()) {
        *p = Vec3(-p.0, p.1, -p.2);
    }
    let parts = [render::Part::metal(&up, render::GOLD)];
    let side = 90f64.to_radians();
    render::write_png_parts(out.join(format!("{stem}-front.png")), &parts, 0.0, side, 1600)?;
    render::write_png_framed(out.join(format!("{stem}-crest.png")), &parts, 0.0, side, render::Framing::new([0.0, 36.0, 0.0], 27.0), 1600)?;
    render::write_png_framed(out.join(format!("{stem}-figures.png")), &parts, 0.0, side, render::Framing::new([0.0, 36.0, 1.0], 17.0), 1600)?;
    render::write_png_parts(out.join(format!("{stem}-hero.png")), &parts, 0.5, 1.15, 1600)?;
    render::write_png_parts(out.join(format!("{stem}-side.png")), &parts, 1.25, side, 1600)?;
    render::write_png_parts(out.join(format!("{stem}-plan.png")), &parts, 0.0, 0.0, 1600)?;
    render::write_png_framed(out.join(format!("{stem}-loop.png")), &parts, 0.35, 1.25, render::Framing::new([0.0, 36.0, -27.0], 7.0), 1200)?;
    Ok(())
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut work: Option<PathBuf> = None;
    let mut out = PathBuf::from("target/cuff-aegis");
    let mut step = 0.05;
    let mut keep = 0.3;
    let mut figures = true;
    let mut only: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--step" => step = args.next().context("--step needs mm")?.parse()?,
            "--keep" => keep = args.next().context("--keep needs a fraction")?.parse()?,
            "--no-figures" => figures = false,
            "--only" => only = Some(args.next().context("--only needs a figure")?),
            flag if flag.starts_with("--") => bail!("unknown option {flag}"),
            other if work.is_none() => work = Some(PathBuf::from(other)),
            other => out = PathBuf::from(other),
        }
    }
    let work = work.context("usage: cuff_aegis WORK_DIR [OUT_DIR] [--step MM] [--keep FRACTION] [--no-figures]")?;
    ensure!(step > 0.005 && step < 1.0, "--step {step} is out of range");
    std::fs::create_dir_all(&out)?;
    let layout: Value = serde_json::from_str(&std::fs::read_to_string(work.join("layout.json"))?)?;
    let c = Cuff::from(layout.get("cuff").context("layout has no `cuff`")?)?;
    let floor = num(layout.get("levels").context("layout has no `levels`")?, "panel_floor")?;
    let sink = num(&layout, "sink_mm")?;
    let relief = Relief::load(&work)?;
    println!("{NAME}: bore Ø{:.2}, band {:.2} to {:.2}, {:.2} thick, ends at ±{:.2} mm of arc", 2.0 * c.r_bore, c.band, c.crest, c.t0, c.s_end);
    let t = Instant::now();
    let mut solid = body(&c, &relief, step);
    println!("body swept in {:.1}s", t.elapsed().as_secs_f64());
    check("body", &solid)?;
    if figures {
        let heroes = layout.get("heroes").context("layout has no `heroes`")?;
        for name in FIGURES.into_iter().filter(|n| only.as_deref().is_none_or(|o| o == *n)) {
            let t = Instant::now();
            let raw = figure(&c, &work, name, heroes.get(name).with_context(|| format!("no figure {name}"))?, floor, sink)?;
            let target = ((raw.f.len() as f64) * keep) as usize;
            let part = match sculpt::clean_decimate_or_sites(&raw, target) {
                Ok(p) => p,
                Err(sites) => {
                    for p in sites.iter().take(12) {
                        let s = (p[1].atan2(p[0]) - FRAC_PI_2) * c.r_ref;
                        println!("  {name} crosses itself after decimation at s {s:.2}, z {:.2}, r {:.2}", p[2], p[0].hypot(p[1]));
                    }
                    println!("  {name}: {} crossing sites; kept undecimated", sites.len());
                    raw.clone()
                }
            };
            println!("{name}: {} faces decimated to {} in {:.1}s", raw.f.len(), part.f.len(), t.elapsed().as_secs_f64());
            check(name, &part)?;
            let t = Instant::now();
            solid = csg::combine(&solid, &part, Op::Union).map_err(|e| anyhow::anyhow!("joining {name}: {e}"))?;
            println!("  joined in {:.1}s", t.elapsed().as_secs_f64());
        }
        if let Some(spec) = layout.get("loop") {
            let ring = chain_loop(&c, spec)?;
            check("chain loop", &ring)?;
            solid = csg::combine(&solid, &ring, Op::Union).map_err(|e| anyhow::anyhow!("joining the chain loop: {e}"))?;
            println!("  chain loop joined at the {} tip", spec.get("edge").and_then(Value::as_str).unwrap_or("top"));
        }
        let cleaned = csg::clean(&mut solid, 2e-5);
        println!("cleaned {cleaned} sliver faces");
    }
    check("cuff", &solid)?;
    let mesh = to_mesh(&solid);
    let vol = solid.volume();
    let (lo, hi) = solid.bounds().context("empty cuff")?;
    println!(
        "{:.2} x {:.2} x {:.2} mm, {vol:.0} mm³: {:.1} g sterling, {:.1} g 14k",
        hi[0] - lo[0],
        hi[1] - lo[1],
        hi[2] - lo[2],
        vol * SILVER_G_PER_MM3,
        vol * GOLD14_G_PER_MM3
    );
    let wall = cad::measure::thickness(&mesh, 0.5);
    println!(
        "wall census at 0.5 mm: thinnest {:.3} mm, {} wall samples under, {} edge samples under, {} unresolved",
        wall.sampled_min_mm.unwrap_or(f64::NAN),
        wall.below_limit,
        wall.edge_below_limit,
        wall.unresolved
    );
    for z in wall.walls.iter().take(8) {
        let theta = z.point[1].atan2(z.point[0]).to_degrees();
        println!("  thin wall {:.4} mm² down to {:.3} mm at theta {theta:.2}, r {:.2}, z {:.2}", z.area_mm2, z.thinnest_mm, z.point[0].hypot(z.point[1]), z.point[2]);
    }
    let stem = if figures { "trident-cuff-aegis" } else { "trident-cuff-aegis-relief" };
    stl::write_stl(out.join(format!("{stem}.stl")), &mesh, NAME)?;
    let d = design(&solid, &layout)?;
    let path = out.join(format!("{stem}.ring.json"));
    library::save_design(&path, &d)?;
    let lib = AlphaLibrary::builtin();
    let again = mesh::try_build(&library::load_design(&path)?, &lib, BuildParams::default()).with_context(|| format!("reopening {}", path.display()))?;
    println!("{} reopens: {} faces, watertight {}", path.display(), again.mesh.faces.len(), again.report.validation.watertight);
    renders(&out, stem, &mesh)?;
    println!("written to {}", out.display());
    Ok(())
}
