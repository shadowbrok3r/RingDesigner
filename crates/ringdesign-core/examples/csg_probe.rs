//! What the M2 join path costs on a real band: the closure census every `combine` now pays, the grid-culled
//! self-crossing count, a traced bezel join with its seam, an uncancelled band-against-itself union (the
//! workload the mid-way cancel test interrupts), and the contact matrix a part's foot can land in.
//!
//!     cargo run --release --example csg_probe
use ringdesign_core::{
    AlphaLibrary, BuildParams,
    csg::{self, Frame, Op, Solid, P3},
    interaction::picking::raycast,
    mesh, templates,
};
use std::f64::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

fn sub(a: P3, b: P3) -> P3 { std::array::from_fn(|k| a[k] - b[k]) }
fn scale(a: P3, s: f64) -> P3 { a.map(|v| v * s) }
fn cross(a: P3, b: P3) -> P3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn unit(a: P3) -> P3 { let l = a.iter().map(|v| v * v).sum::<f64>().sqrt().max(1e-300); scale(a, 1.0 / l) }

fn cuboid(lo: P3, hi: P3) -> Solid {
    let v: Vec<P3> = (0..8).map(|i: usize| std::array::from_fn(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] })).collect();
    let quads: [[u32; 4]; 6] = [[0, 4, 6, 2], [1, 3, 7, 5], [0, 1, 5, 4], [2, 6, 7, 3], [0, 2, 3, 1], [4, 5, 7, 6]];
    Solid { v, f: quads.iter().flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect() }
}

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
    for j in 0..n {
        let (b0, b1, t0, t1) = (j, (j + 1) % n, n + j, n + (j + 1) % n);
        s.f.extend([[b0, b1, t1], [b0, t1, t0], [2 * n, b1, b0], [2 * n + 1, t0, t1]]);
    }
    s
}

fn sphere(c: P3, r: f64, rings: usize, around: usize) -> Solid {
    let mut s = Solid::default();
    s.v.push([c[0], c[1], c[2] + r]);
    for i in 1..rings {
        let phi = PI * i as f64 / rings as f64;
        for j in 0..around {
            let th = 2.0 * PI * (j as f64 + 0.37) / around as f64;
            s.v.push([c[0] + r * phi.sin() * th.cos(), c[1] + r * phi.sin() * th.sin(), c[2] + r * phi.cos()]);
        }
    }
    s.v.push([c[0], c[1], c[2] - r]);
    let ring = |i: usize, j: usize| (1 + (i - 1) * around + j % around) as u32;
    let last = (s.v.len() - 1) as u32;
    for j in 0..around {
        s.f.push([0, ring(1, j), ring(1, j + 1)]);
        for i in 1..rings - 1 {
            s.f.push([ring(i, j), ring(i + 1, j), ring(i + 1, j + 1)]);
            s.f.push([ring(i, j), ring(i + 1, j + 1), ring(i, j + 1)]);
        }
        s.f.push([ring(rings - 1, j), last, ring(rings - 1, j + 1)]);
    }
    s
}

fn court_band(theta_steps: usize, profile_steps: usize) -> (mesh::Mesh, Solid) {
    let t = templates::all().iter().find(|t| t.name == "Court band").unwrap();
    let params = BuildParams { theta_steps, profile_steps, ..BuildParams::default() };
    let built = mesh::try_build(&t.design(), &AlphaLibrary::builtin(), params).unwrap();
    let solid = Solid { v: built.mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: built.mesh.faces.clone() };
    (built.mesh, solid)
}

fn seat_on_crest(mesh: &mesh::Mesh, sink: f64) -> Frame {
    let theta = 90.0_f64.to_radians();
    let from = [(40.0 * theta.cos()) as f32, (40.0 * theta.sin()) as f32, 0.0];
    let (face, hit) = raycast(mesh, from, [(-theta.cos()) as f32, (-theta.sin()) as f32, 0.0]).unwrap();
    let (a, b, c) = mesh.triangle(&mesh.faces[face]).unwrap();
    let normal = unit(cross(sub(b, a), sub(c, a)));
    Frame::from_normal(sub(hit.map(f64::from), scale(normal, sink)), normal, [0.0, 0.0, 1.0])
}

fn ms(from: Instant) -> f64 { from.elapsed().as_secs_f64() * 1e3 }

fn main() {
    for (label, theta, profile) in [("preview 256×128", 256, 128), ("export 1024×384", 1024, 384)] {
        let (mesh, band) = court_band(theta, profile);
        println!("== {label}: {} faces", band.f.len());
        let t = Instant::now();
        let (open, repeated) = band.open_edges();
        println!("  closure census        {:>8.1} ms  open {open} repeated {repeated}", ms(t));
        let t = Instant::now();
        let check = band.check(true);
        println!("  check with crossings  {:>8.1} ms  {check:?}", ms(t));
        let tool = cylinder(3.0, 0.0, 2.5, 48).placed(&seat_on_crest(&mesh, 0.05));
        let t = Instant::now();
        let traced = csg::combine_traced(&band, &tool, Op::Union, None).unwrap();
        let took = ms(t);
        let loops = csg::seam_loops(&traced);
        println!(
            "  traced union bezel    {:>8.1} ms  {} faces, {} seam vertices, {} loop(s) of {:?} vertices, Δvolume {:+.3} mm³",
            took, traced.solid.f.len(), traced.seam.len(), loops.len(), loops.iter().map(Vec::len).collect::<Vec<_>>(), traced.solid.volume() - band.volume()
        );
        let t = Instant::now();
        let plain = csg::combine(&band, &tool, Op::Union).unwrap();
        println!("  plain union bezel     {:>8.1} ms  same bytes: {}", ms(t), plain.v == traced.solid.v && plain.f == traced.solid.f);
        if theta == 256 {
            let shifted = band.translated([0.011, 0.007, 0.013]);
            let t = Instant::now();
            let r = csg::combine(&band, &shifted, Op::Union);
            println!("  band ∪ shifted band   {:>8.1} ms  {}", ms(t), r.as_ref().map(|s| format!("{} faces", s.f.len())).unwrap_or_else(|e| e.to_string()));
            let flag = AtomicBool::new(false);
            let t = Instant::now();
            let r = std::thread::scope(|s| {
                s.spawn(|| { std::thread::sleep(Duration::from_millis(60)); flag.store(true, Ordering::Relaxed); });
                csg::combine_with(&band, &shifted, Op::Union, Some(&flag))
            });
            println!("  same, flag at 60 ms   {:>8.1} ms  {}", ms(t), r.as_ref().map(|s| format!("{} faces", s.f.len())).unwrap_or_else(|e| e.to_string()));
        } else {
            let flag = AtomicBool::new(true);
            let t = Instant::now();
            let r = csg::combine_with(&band, &tool, Op::Union, Some(&flag));
            println!("  flag raised first     {:>8.1} ms  {}", ms(t), r.as_ref().map(|s| format!("{} faces", s.f.len())).unwrap_or_else(|e| e.to_string()));
        }
    }
    println!("== contact matrix: a 3 mm post on a 10 mm block");
    let block = cuboid([-5.0, -5.0, 0.0], [5.0, 5.0, 10.0]);
    let post = |foot: f64| cylinder(1.5, 10.0 + foot, 13.0 + foot, 32);
    for (label, tool) in [("foot coplanar", post(0.0)), ("foot sunk 0.05", post(-0.05)), ("foot raised 0.05", post(0.05)), ("sphere tangent", sphere([0.0, 0.0, 11.5], 1.5, 12, 16))] {
        let t = Instant::now();
        match csg::combine(&block, &tool, Op::Union) {
            Ok(mut s) => {
                let (open, repeated) = s.open_edges();
                let volume = s.volume();
                let cleaned = csg::clean(&mut s, 2e-5);
                println!("  {label:<18} {:>6.2} ms  Ok: {} faces, open {open} repeated {repeated}, volume {volume:.6} against {:.6}, cleaned {cleaned}", ms(t), s.f.len() + cleaned, block.volume() + tool.volume());
            }
            Err(e) => println!("  {label:<18} {:>6.2} ms  {e}", ms(t)),
        }
    }
}
