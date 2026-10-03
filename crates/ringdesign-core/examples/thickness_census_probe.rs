//! The wall census on a saved design, or the default band at 2048 x 384, beside the old 384-ray sampler.
use anyhow::Result;
use ringdesign_core::cad::measure::{self, CensusOptions, Thickness};
use ringdesign_core::{AlphaLibrary, BuildParams, Mesh, RingDesign, library, manufacturing as mf, mesh};
use serde_json::json;
use std::time::Instant;

fn arg(args: &[String], name: &str) -> Option<f64> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok())
}

/// The sampler `measure::thickness` ran before the census: 384 face centroids by stride, every face tried.
fn legacy(mesh: &Mesh, limit: f64) -> serde_json::Value {
    if !mesh.validate().watertight || mesh.faces.len() > 250_000 {
        return json!("not assessed: invalid mesh or over 250 000 faces");
    }
    let tri: Vec<_> = mesh.faces.iter().filter_map(|f| mesh.triangle(f)).collect();
    let sub = |a: [f64; 3], b: [f64; 3]| -> [f64; 3] { std::array::from_fn(|i| a[i] - b[i]) };
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let hit = |o: [f64; 3], d: [f64; 3], (a, b, c): ([f64; 3], [f64; 3], [f64; 3])| -> Option<f64> {
        let (e1, e2) = (sub(b, a), sub(c, a));
        let h = cross(d, e2);
        let det = dot(e1, h);
        if det.abs() < 1e-12 {
            return None;
        }
        let s = sub(o, a);
        let u = dot(s, h) / det;
        if !(-1e-8..=1.0 + 1e-8).contains(&u) {
            return None;
        }
        let q = cross(s, e1);
        let v = dot(d, q) / det;
        if v < -1e-8 || u + v > 1.0 + 1e-8 {
            return None;
        }
        let t = dot(e2, q) / det;
        (t > 1e-5).then_some(t)
    };
    let stride = tri.len().div_ceil(384).max(1);
    let (mut min, mut below, mut thin) = (f64::MAX, 0, Vec::new());
    for (i, &(a, b, c)) in tri.iter().enumerate().step_by(stride) {
        let n = cross(sub(b, a), sub(c, a));
        let l = dot(n, n).sqrt();
        if l < 1e-12 {
            continue;
        }
        let o: [f64; 3] = std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0);
        let d = n.map(|v| -v / l);
        if let Some(t) = tri.iter().enumerate().filter(|(j, _)| *j != i).filter_map(|(_, t)| hit(o, d, *t)).min_by(f64::total_cmp) {
            min = min.min(t);
            if t < limit {
                below += 1;
                thin.push(json!({"mm": t, "at": o}));
            }
        }
    }
    json!({"sampled_min_mm": min, "below_limit": below, "thin": thin})
}

/// Each face whose centroid ray leaves the metal within `below` mm, with its exit face's wedge angle, shared vertices and shell.
fn diagnose(mesh: &Mesh, below: f64) -> Vec<serde_json::Value> {
    use ringdesign_core::interaction::bvh::Bvh;
    let bvh = Bvh::build(mesh);
    let mut parent: Vec<usize> = (0..mesh.vertices.len()).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for f in &mesh.faces {
        for (a, b) in [(f[0], f[1]), (f[1], f[2])] {
            let (x, y) = (find(&mut parent, a as usize), find(&mut parent, b as usize));
            parent[x.max(y)] = x.min(y);
        }
    }
    let normal = |f: usize| {
        let (a, b, c) = mesh.triangle(&mesh.faces[f]).unwrap();
        let (u, v): ([f64; 3], [f64; 3]) = (std::array::from_fn(|k| b[k] - a[k]), std::array::from_fn(|k| c[k] - a[k]));
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        (n.map(|x| x / l), 0.5 * l)
    };
    let mut out = Vec::new();
    for f in 0..mesh.faces.len() {
        let (n, area) = normal(f);
        if !(area > 0.0) {
            continue;
        }
        let (a, b, c) = mesh.triangle(&mesh.faces[f]).unwrap();
        let o: [f64; 3] = std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0);
        let Some((g, t)) = bvh.ray(mesh, o, n.map(|x| -x)) else { continue };
        if t >= below {
            continue;
        }
        let (m, _) = normal(g);
        let shared = mesh.faces[f].iter().filter(|v| mesh.faces[g].contains(v)).count();
        let wedge = 180.0 - (n[0] * m[0] + n[1] * m[1] + n[2] * m[2]).clamp(-1.0, 1.0).acos().to_degrees();
        let longest = [(a, b), (b, c), (c, a)].iter().map(|(p, q)| ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()).fold(0.0, f64::max);
        let origin = |i: u32| mesh.origin.get(i as usize).map(|&o| if o >= ringdesign_core::mesh::SOLID_VERTEX { "part" } else { "band" });
        out.push(json!({
            "mm": t, "theta_deg": o[1].atan2(o[0]).to_degrees(), "r_mm": o[0].hypot(o[1]), "z_mm": o[2],
            "wedge_deg": wedge, "shared_vertices": shared, "face_height_mm": 2.0 * area / longest,
            "same_shell": find(&mut parent, mesh.faces[f][0] as usize) == find(&mut parent, mesh.faces[g][0] as usize),
            "from": origin(mesh.faces[f][0]), "to": origin(mesh.faces[g][0]),
        }));
    }
    out
}

fn summary(t: &Thickness, ms: f64) -> serde_json::Value {
    let zone = |z: &measure::ThinZone| json!({"area_mm2": z.area_mm2, "thinnest_mm": z.thinnest_mm, "at": z.point, "samples": z.samples, "depth_mm": z.depth_mm, "span_mm": z.span_mm});
    json!({
        "ms": ms, "clean": t.clean(), "assessed": t.assessed, "pitch_mm": t.pitch_mm, "area_mm2": t.area_mm2,
        "samples": t.rays, "unresolved": t.unresolved, "internal": t.internal, "sampled_min_mm": t.sampled_min_mm, "at": t.point,
        "wall_samples": t.below_limit, "wall_area_mm2": t.wall_area_mm2, "edge_samples": t.edge_below_limit, "edge_area_mm2": t.edge_area_mm2,
        "walls": t.walls.iter().map(zone).collect::<Vec<_>>(),
        "edges": t.edges.iter().map(zone).collect::<Vec<_>>(),
    })
}

fn report(name: &str, m: &Mesh, options: &CensusOptions) -> serde_json::Value {
    let started = Instant::now();
    let t = measure::census(m, options);
    let ms = started.elapsed().as_secs_f64() * 1e3;
    let started = Instant::now();
    let old = legacy(m, options.floor_mm);
    let legacy_ms = started.elapsed().as_secs_f64() * 1e3;
    json!({"mesh": name, "faces": m.faces.len(), "census": summary(&t, ms), "legacy": old, "legacy_ms": legacy_ms})
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let floor = arg(&args, "--floor").unwrap_or(0.8);
    let options = CensusOptions { edge_reach_mm: arg(&args, "--reach"), ..CensusOptions::floor(floor) };
    let Some(path) = args.iter().find(|a| a.ends_with(".json")) else {
        let lib = AlphaLibrary::builtin();
        let started = Instant::now();
        let built = mesh::build(&RingDesign::default(), &lib, BuildParams { theta_steps: 2048, profile_steps: 384, ..BuildParams::default() });
        println!("band built in {:.0} ms", started.elapsed().as_secs_f64() * 1e3);
        println!("{}", serde_json::to_string_pretty(&report("default band 2048 x 384", &built.mesh, &options))?);
        return Ok(());
    };
    let d = library::load_design(path)?;
    let lib = mf::source_library(&d, &AlphaLibrary::default()).into_owned();
    let params = BuildParams {
        theta_steps: arg(&args, "--theta").map_or(320, |v| v as usize),
        profile_steps: arg(&args, "--profile").map_or(112, |v| v as usize),
        ..BuildParams::default()
    };
    let started = Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    println!("{} built at {} x {} in {:.0} ms", d.name, params.theta_steps, params.profile_steps, started.elapsed().as_secs_f64() * 1e3);
    println!("{}", serde_json::to_string_pretty(&report("whole ring", &built.mesh, &options))?);
    if let Some(below) = arg(&args, "--diagnose") {
        println!("{}", serde_json::to_string_pretty(&json!({"diagnose_whole_ring": diagnose(&built.mesh, below)}))?);
    }
    if args.iter().any(|a| a == "--parts") {
        for c in built.parts.evaluated.iter().flat_map(|e| e.components.iter()) {
            println!("{}", serde_json::to_string_pretty(&report(&c.name, &c.mesh, &options))?);
        }
    }
    Ok(())
}
