//! Spike R4: the rolling-ball seam bead (`blend.rs`) measured — a post on a plane against the
//! analytic torus fillet, a bezel on the Court band and the Heart signet, a 0.8 mm wire lying on
//! the low dome, and 200 random bezel placements round the band for closure, folds and time.
//!
//!     cargo run --release --example bead_probe
use ringdesign_core::{
    AlphaLibrary, BuildParams, Mesh,
    blend::{self, Surfaces},
    csg::{self, Frame, Op, Solid},
    interaction::picking::raycast,
    mesh, templates,
};
use std::f64::consts::PI;
use std::time::Instant;

type P3 = [f64; 3];

fn sub(a: P3, b: P3) -> P3 { std::array::from_fn(|k| a[k] - b[k]) }
fn add(a: P3, b: P3) -> P3 { std::array::from_fn(|k| a[k] + b[k]) }
fn scale(a: P3, s: f64) -> P3 { a.map(|v| v * s) }
fn cross(a: P3, b: P3) -> P3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn norm(a: P3) -> f64 { a.iter().map(|v| v * v).sum::<f64>().sqrt() }
fn unit(a: P3) -> P3 { scale(a, 1.0 / norm(a).max(1e-300)) }

/// An outward-wound box.
fn cuboid(lo: P3, hi: P3) -> Solid {
    let v: Vec<P3> = (0..8).map(|i: usize| std::array::from_fn(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] })).collect();
    let quads: [[u32; 4]; 6] = [[0, 4, 6, 2], [1, 3, 7, 5], [0, 1, 5, 4], [2, 6, 7, 3], [0, 2, 3, 1], [4, 5, 7, 6]];
    let f = quads.iter().flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect();
    Solid { v, f }
}

/// An outward-wound cylinder about z from `z0` to `z1`, caps fanned from their centres.
fn cylinder(r: f64, z0: f64, z1: f64, n: usize) -> Solid {
    let mut s = Solid::default();
    for z in [z0, z1] {
        for j in 0..n {
            let th = 2.0 * PI * (j as f64 + 0.37) / n as f64;
            s.v.push([r * th.cos(), r * th.sin(), z]);
        }
    }
    s.v.push([0.0, 0.0, z0]);
    s.v.push([0.0, 0.0, z1]);
    let n = n as u32;
    let (cb, ct) = (2 * n, 2 * n + 1);
    for j in 0..n {
        let (b0, b1, t0, t1) = (j, (j + 1) % n, n + j, n + (j + 1) % n);
        s.f.push([b0, b1, t1]);
        s.f.push([b0, t1, t0]);
        s.f.push([cb, b1, b0]);
        s.f.push([ct, t0, t1]);
    }
    s
}

/// Volume of the fillet round a post of radius `big` with a bead of radius `r`, by Pappus.
fn torus_fillet_volume(big: f64, r: f64) -> f64 {
    let area = r * r * (1.0 - PI / 4.0);
    let centroid = (r * r * (big + r / 2.0) - PI * r * r / 4.0 * (big + r - 4.0 * r / (3.0 * PI))) / area;
    2.0 * PI * centroid * area
}

/// Worst and rms distance of the result's vertices in the fillet zone from the torus fillet round a
/// post of radius `big` on the plane `z = 0`, and how many vertices were read.
fn fillet_deviation(s: &Solid, big: f64, r: f64) -> (f64, f64, usize) {
    let (mut worst, mut sum, mut count) = (0.0_f64, 0.0, 0usize);
    for p in &s.v {
        let rho = (p[0] * p[0] + p[1] * p[1]).sqrt();
        let (dr, dz) = (rho - (big + r), p[2] - r);
        if dr > 1e-6 || dz > 1e-6 || rho < big - 0.02 || p[2] < -0.02 {
            continue;
        }
        let d = (dr * dr + dz * dz).sqrt() - r;
        worst = worst.max(d.abs());
        sum += d * d;
        count += 1;
    }
    (worst, if count > 0 { (sum / count as f64).sqrt() } else { 0.0 }, count)
}

/// An outward-wound torus about z, major radius `big`, tube radius `small`.
fn torus(big: f64, small: f64, around: usize, tube: usize) -> Solid {
    let mut s = Solid::default();
    for i in 0..around {
        let th = 2.0 * PI * i as f64 / around as f64;
        for j in 0..tube {
            let ph = 2.0 * PI * (j as f64 + 0.5) / tube as f64;
            let rho = big + small * ph.cos();
            s.v.push([rho * th.cos(), rho * th.sin(), small * ph.sin()]);
        }
    }
    let at = |i: usize, j: usize| ((i % around) * tube + j % tube) as u32;
    for i in 0..around {
        for j in 0..tube {
            s.f.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            s.f.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    if s.volume() < 0.0 {
        for f in &mut s.f {
            f.swap(1, 2);
        }
    }
    s
}

fn band_solid(mesh: &Mesh) -> Solid {
    Solid { v: mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: mesh.faces.clone() }
}

fn build(name: &str, params: BuildParams) -> Option<(Mesh, Solid)> {
    let t = templates::all().iter().find(|t| t.name == name)?;
    let built = mesh::try_build(&t.design(), &AlphaLibrary::builtin(), params).ok()?;
    let solid = band_solid(&built.mesh);
    Some((built.mesh, solid))
}

/// A frame on the built surface at `theta`, `across` mm along the finger, sunk `sink` under it and
/// tilted `tilt` about the ring's tangent, x along the tangent.
fn on_band(mesh: &Mesh, theta_deg: f64, across: f64, sink: f64, tilt_deg: f64) -> Option<Frame> {
    let theta = theta_deg.to_radians();
    let from = [(40.0 * theta.cos()) as f32, (40.0 * theta.sin()) as f32, across as f32];
    let (face, hit) = raycast(mesh, from, [(-theta.cos()) as f32, (-theta.sin()) as f32, 0.0])?;
    let (a, b, c) = mesh.triangle(&mesh.faces[face])?;
    let normal = unit(cross(sub(b, a), sub(c, a)));
    let tangent = [-theta.sin(), theta.cos(), 0.0];
    let (sn, cs) = tilt_deg.to_radians().sin_cos();
    let tilted = add(scale(normal, cs), scale(cross(tangent, normal), sn));
    Some(Frame::from_normal(sub(hit.map(f64::from), scale(tilted, sink)), tilted, tangent))
}

struct Row {
    ok: bool,
    ms: f64,
    faces: usize,
    open: usize,
    repeated: usize,
    loops: usize,
    stations: usize,
    clamped: usize,
    folded: usize,
    min_r: f64,
    unrefined: usize,
    bead_crossings: usize,
    dv: f64,
    note: String,
}

/// Join `tool` into `base`, bead the seam, and measure.
fn junction(base: &Solid, tool: &Solid, radius: f64, concave: bool, check_result: bool) -> Row {
    let op = if concave { Op::Union } else { Op::Subtract };
    let t = match csg::combine_traced(base, tool, op, None) {
        Ok(t) => t,
        Err(e) => return Row { ok: false, ms: 0.0, faces: 0, open: 0, repeated: 0, loops: 0, stations: 0, clamped: 0, folded: 0, min_r: 0.0, unrefined: 0, bead_crossings: 0, dv: 0.0, note: format!("join: {e}") },
    };
    let before = t.solid.volume();
    let started = Instant::now();
    let seams = blend::seams(&t, base, tool, concave);
    let mut bead_crossings = 0;
    for s in &seams {
        if let Ok(b) = blend::bead_against(&s.points, &s.normals_a, &s.normals_b, radius, Some(&Surfaces::near(&t, &s.points, (blend::REACH_MAX + 1.5) * radius + 0.05))) {
            bead_crossings += csg::self_crossings(&b.solid);
        }
    }
    match blend::fillet_junction_report(&t, base, tool, radius, concave) {
        Ok(j) => {
            let ms = started.elapsed().as_secs_f64() * 1e3;
            let (open, repeated) = j.solid.open_edges();
            let note = if check_result {
                let c = Instant::now();
                let n = csg::self_crossings(&j.solid);
                format!("result crossings {n} ({:.0} ms)", c.elapsed().as_secs_f64() * 1e3)
            } else {
                String::new()
            };
            Row { ok: true, ms, faces: j.solid.f.len(), open, repeated, loops: j.loops, stations: j.stations, clamped: j.clamped, folded: j.folded, min_r: j.min_radius_mm, unrefined: j.unrefined, bead_crossings, dv: j.solid.volume() - before, note }
        }
        Err(e) => Row { ok: false, ms: started.elapsed().as_secs_f64() * 1e3, faces: 0, open: 0, repeated: 0, loops: seams.len(), stations: 0, clamped: 0, folded: 0, min_r: 0.0, unrefined: 0, bead_crossings, dv: 0.0, note: e },
    }
}

fn print(label: &str, r: &Row) {
    if r.ok {
        println!(
            "  {label:<34} {:>7.1} ms  {:>7} faces  open {} rep {}  loops {}  stations {:>4}  clamped {:>4}  folded {:>3}  min r {:.3}  unrefined {:>3}  bead x {}  Δvol {:+.4}  {}",
            r.ms, r.faces, r.open, r.repeated, r.loops, r.stations, r.clamped, r.folded, r.min_r, r.unrefined, r.bead_crossings, r.dv, r.note
        );
    } else {
        println!("  {label:<34} {:>7.1} ms  FAILED after {} loops, bead x {}: {}", r.ms, r.loops, r.bead_crossings, r.note);
    }
}

fn main() {
    println!("== post r 3 h 2.5 on a plane, bead r 0.3: deviation from the analytic torus fillet");
    let slab = cuboid([-8.0, -8.0, -4.0], [8.0, 8.0, 0.0]);
    for n in [48usize, 96, 180] {
        let post = cylinder(3.0, -0.05, 2.45, n);
        let t = csg::combine_traced(&slab, &post, Op::Union, None).unwrap();
        let before = t.solid.volume();
        let started = Instant::now();
        let j = blend::fillet_junction_report(&t, &slab, &post, 0.3, true).unwrap();
        let ms = started.elapsed().as_secs_f64() * 1e3;
        let (worst, rms, count) = fillet_deviation(&j.solid, 3.0, 0.3);
        let sag = 3.0 * (1.0 - (PI / n as f64).cos());
        println!(
            "  {n:>3}-gon  {ms:>6.1} ms  {:>6} faces  open {:?}  crossings {}  stations {}  clamped {}  worst {:.4} mm  rms {:.4}  over {count} vertices  added {:.4} mm³ (analytic {:.4}; facet sag {:.4})",
            j.solid.f.len(), j.solid.open_edges(), csg::self_crossings(&j.solid), j.stations, j.clamped, worst, rms, j.solid.volume() - before, torus_fillet_volume(3.0, 0.3), sag
        );
    }
    println!("  other radii on the 96-gon:");
    for r in [0.1, 0.5, 1.0] {
        let post = cylinder(3.0, -0.05, 2.45, 96);
        let t = csg::combine_traced(&slab, &post, Op::Union, None).unwrap();
        let before = t.solid.volume();
        let started = Instant::now();
        let j = blend::fillet_junction_report(&t, &slab, &post, r, true).unwrap();
        let ms = started.elapsed().as_secs_f64() * 1e3;
        let (worst, rms, count) = fillet_deviation(&j.solid, 3.0, r);
        println!(
            "  r {r:.1}   {ms:>6.1} ms  {:>6} faces  open {:?}  crossings {}  stations {}  worst {:.4} mm  rms {:.4}  over {count}  added {:.4} mm³ (analytic {:.4})",
            j.solid.f.len(), j.solid.open_edges(), csg::self_crossings(&j.solid), j.stations, worst, rms, j.solid.volume() - before, torus_fillet_volume(3.0, r)
        );
    }
    println!("  a plus-shaped post (four re-entrant seam corners), r 0.3:");
    let plus = csg::combine(&cuboid([-2.0, -1.0, -0.05], [2.0, 1.0, 2.45]), &cuboid([-1.0, -2.0, -0.05], [1.0, 2.0, 2.45]), Op::Union).unwrap();
    print("slab ∪ plus", &junction(&slab, &plus, 0.3, true, true));
    println!("  a post r 1.5 sunk 0.5 on a torus 10 × 2 (the section curves under the seam):");
    let ring = torus(10.0, 2.0, 360, 96);
    let post = cylinder(1.5, 1.5, 4.0, 48).translated([10.0, 0.0, 0.0]);
    print("torus ∪ post", &junction(&ring, &post, 0.3, true, true));
    println!("  a drilled rim, r 0.3 as a cut:");
    let block = cuboid([-8.0, -8.0, -5.0], [8.0, 8.0, 0.0]);
    let drill = cylinder(3.0, -6.0, 1.0, 96);
    print("block − drill", &junction(&block, &drill, 0.3, false, true));

    let preview = BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() };
    let export = BuildParams { theta_steps: 1024, profile_steps: 384, ..BuildParams::default() };
    println!("\n== bezel on the templates at the top of the ring, bead r 0.3");
    for (label, params) in [("preview 256×128", preview), ("export 1024×384", export)] {
        println!(" {label}");
        for name in ["Court band", "Heart signet"] {
            let Some((mesh, band)) = build(name, params) else { continue };
            // The plan's part: r 3, sunk 0.05, which on a 4 mm low dome is a lens on the crown.
            let frame = on_band(&mesh, 90.0, 0.0, 0.05, 0.0).unwrap();
            let bezel = cylinder(3.0, 0.0, 2.5, 48).placed(&frame);
            print(&format!("{name}: r 3 sunk 0.05"), &junction(&band, &bezel, 0.3, true, label.starts_with("preview")));
            // A bezel that fits the band: r 1.5 sunk 0.5, the wall meeting the crown all round.
            let frame = on_band(&mesh, 90.0, 0.0, 0.5, 0.0).unwrap();
            let bezel = cylinder(1.5, 0.0, 2.5, 48).placed(&frame);
            print(&format!("{name}: r 1.5 sunk 0.5"), &junction(&band, &bezel, 0.3, true, label.starts_with("preview")));
            if name == "Heart signet" {
                let frame = on_band(&mesh, 90.0, 0.0, 0.5, 0.0).unwrap();
                let bezel = cylinder(3.0, 0.0, 2.5, 48).placed(&frame);
                print(&format!("{name}: r 3 sunk 0.5"), &junction(&band, &bezel, 0.3, true, label.starts_with("preview")));
            }
        }
    }

    println!("\n== 0.8 mm wire (cylinder r 0.4) on the Court band's low dome, bead r 0.3, preview");
    let (mesh, band) = build("Court band", preview).unwrap();
    let frame = on_band(&mesh, 90.0, 0.0, 0.0, 0.0).unwrap();
    for (label, sink, along) in [("along the ring, sunk 0.05 (lens, no caps)", 0.05, true), ("along the ring, sunk 0.40 (caps cut the crown)", 0.40, true), ("across the band, sunk 0.05", 0.05, false), ("across the band, sunk 0.40", 0.40, false)] {
        let axis = add(frame.origin, scale(frame.z, 0.4 - sink));
        let dir = if along { frame.x } else { frame.y };
        let lying = Frame::from_normal(axis, dir, frame.z);
        let wire = cylinder(0.4, -1.5, 1.5, 48).placed(&lying);
        print(label, &junction(&band, &wire, 0.3, true, true));
    }
    println!("  the same wire standing on its end (a post r 0.4), sunk 0.3:");
    let frame = on_band(&mesh, 90.0, 0.0, 0.3, 0.0).unwrap();
    print("post r 0.4", &junction(&band, &cylinder(0.4, 0.0, 3.0, 48).placed(&frame), 0.3, true, true));

    println!("\n== 200 random placements of a bezel r 1.5 sunk 0.5 round the Court band, bead r 0.3, preview");
    let mut seed = 0x9e37_79b9_u64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut ok, mut closed, mut clean_beads, mut clamped_stations, mut stations, mut clamped_cases, mut unrefined) = (0, 0, 0, 0, 0, 0, 0);
    let mut times = Vec::new();
    let mut failures = Vec::new();
    let mut result_crossings = Vec::new();
    for k in 0..200 {
        let theta = 360.0 * next();
        let across = 0.8 * (next() - 0.5);
        let tilt = 6.0 * (next() - 0.5);
        let spin = 360.0 * next();
        let Some(frame) = on_band(&mesh, theta, across, 0.5, tilt) else { continue };
        let (sn, cs) = spin.to_radians().sin_cos();
        let frame = Frame { origin: frame.origin, x: add(scale(frame.x, cs), scale(frame.y, sn)), y: add(scale(frame.x, -sn), scale(frame.y, cs)), z: frame.z };
        let bezel = cylinder(1.5, 0.0, 2.5, 48).placed(&frame);
        let row = junction(&band, &bezel, 0.3, true, k < 5);
        if k < 5 {
            print(&format!("#{k} θ {theta:.1}° v {across:+.2} tilt {tilt:+.1}°"), &row);
        }
        if row.ok {
            ok += 1;
            times.push(row.ms);
            stations += row.stations;
            clamped_stations += row.clamped;
            unrefined += row.unrefined;
            if row.clamped > 0 {
                clamped_cases += 1;
            }
            if row.open == 0 && row.repeated == 0 {
                closed += 1;
            }
            if k < 5 {
                result_crossings.push(row.note.clone());
            }
        } else {
            failures.push(format!("#{k} θ {theta:.1}° v {across:+.2} tilt {tilt:+.1}° spin {spin:.0}°: {}", row.note));
        }
        if row.bead_crossings == 0 {
            clean_beads += 1;
        }
    }
    times.sort_by(|a, b| a.total_cmp(b));
    let median = times.get(times.len() / 2).copied().unwrap_or(0.0);
    println!(
        "  succeeded {ok}/200, closed {closed}, beads without self-crossings {clean_beads}, cases with a clamped station {clamped_cases}, clamped stations {clamped_stations} of {stations}, unrefined stations {unrefined}"
    );
    println!("  time min {:.0} ms  median {median:.0} ms  max {:.0} ms", times.first().copied().unwrap_or(0.0), times.last().copied().unwrap_or(0.0));
    for f in &failures {
        println!("  FAILED {f}");
    }
}
