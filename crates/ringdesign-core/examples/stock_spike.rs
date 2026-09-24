//! Factory stock spikes: the 013 Round taken to a 16 mm face, and the eleven sand-safe plans through the sand master.
//! cargo run -p ringdesign-core --release --example stock_spike -- OUT_DIR
use anyhow::{Context, Result};
use ringdesign_core::{
    AlphaLibrary, BuildParams, Mesh, RingDesign,
    castability,
    imported_base::{ImportedBase, PRESETS, Source},
    mesh, render,
};
use std::{collections::HashMap, path::Path, sync::Arc};

#[path = "common/probe.rs"]
mod probe;

const SAND_SAFE: [&str; 11] = ["001", "002", "003", "005", "006", "007", "012", "013", "015", "016", "017"];

fn seg(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-18)).clamp(0.0, 1.0);
    (p[0] - a[0] - dx * t).hypot(p[1] - a[1] - dy * t)
}

/// Worst distance from a section point's mirror across z = 0 to the section, over 72 stations, and where.
fn mirror_miss(d: &RingDesign) -> (f64, f64) {
    let mut worst = (0.0, 0.0);
    for k in 0..72 {
        let theta = k as f64 * 5.0 + 0.3;
        let l = d.section_at(theta, 384, None, None);
        let pts: Vec<[f64; 2]> = l.pts.iter().map(|p| [p.r, p.z]).collect();
        for p in &pts {
            let m = [p[0], -p[1]];
            let e = (0..pts.len()).map(|i| seg(m, pts[i], pts[(i + 1) % pts.len()])).fold(f64::MAX, f64::min);
            if e > worst.0 {
                worst = (e, theta);
            }
        }
    }
    worst
}

/// Most the outer surface rises walking away from z = 0 over 360 stations, the metal a sand envelope must add, and where.
fn fill_mm(d: &RingDesign) -> (f64, f64) {
    let mut worst = (0.0, 0.0);
    for k in 0..360 {
        let theta = k as f64 + 0.5;
        let l = d.section_at(theta, 512, None, None);
        let n = l.pts.len();
        let mut reach = [vec![f64::MIN; 400], vec![f64::MIN; 400]];
        for i in 0..n {
            let (p, q) = (&l.pts[i], &l.pts[(i + 1) % n]);
            let steps = ((q.z - p.z).abs() / 0.01).ceil().max(1.0) as usize;
            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                let (r, z) = (p.r + (q.r - p.r) * t, p.z + (q.z - p.z) * t);
                let bin = (z.abs() / 0.05) as usize;
                if bin < 400 {
                    let side = &mut reach[usize::from(z < 0.0)];
                    side[bin] = side[bin].max(r);
                }
            }
        }
        for side in &reach {
            let mut beyond = f64::MIN;
            for r in side.iter().rev().filter(|r| **r > f64::MIN) {
                beyond = beyond.max(*r);
                if beyond - r > worst.0 {
                    worst = (beyond - r, theta);
                }
            }
        }
    }
    worst
}

/// Edges turning past `deg` between their two faces, clear of the bore by 0.3 mm: how many, the sharpest, and where it stands.
fn creases(m: &Mesh, bore: f64, deg: f64) -> (usize, f64, [f64; 3]) {
    let p = |i: u32| {
        let v = m.vertices[i as usize];
        [v.0 as f64, v.1 as f64, v.2 as f64]
    };
    let normal = |f: &[u32; 3]| {
        let (a, b, c) = (p(f[0]), p(f[1]), p(f[2]));
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-18);
        n.map(|x| x / l)
    };
    let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (i, f) in m.faces.iter().enumerate() {
        for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
            edges.entry((a.min(b), a.max(b))).or_default().push(i);
        }
    }
    let (mut count, mut sharpest, mut at) = (0, 0.0f64, [0.0; 3]);
    for ((a, b), fs) in &edges {
        if fs.len() != 2 {
            continue;
        }
        let (pa, pb) = (p(*a), p(*b));
        if pa[0].hypot(pa[1]).min(pb[0].hypot(pb[1])) < bore + 0.3 {
            continue;
        }
        let (n0, n1) = (normal(&m.faces[fs[0]]), normal(&m.faces[fs[1]]));
        let turn = (n0[0] * n1[0] + n0[1] * n1[1] + n0[2] * n1[2]).clamp(-1.0, 1.0).acos().to_degrees();
        if turn > deg {
            count += 1;
        }
        if turn > sharpest {
            sharpest = turn;
            at = std::array::from_fn(|k| 0.5 * (pa[k] + pb[k]));
        }
    }
    (count, sharpest, at)
}

/// The stock as `d` builds it bare, baked into a stock of its own whose face is its new size.
fn bake(d: &RingDesign, name: &str) -> Result<Arc<Source>> {
    let mut bare = d.clone();
    bare.imported_base.as_mut().unwrap().bare = true;
    let built = mesh::try_build(&bare, &AlphaLibrary::default(), BuildParams { theta_steps: 900, profile_steps: 448, refine: None, ..Default::default() })?;
    let c = &d.imported_base.as_ref().unwrap().source.calibration;
    let json = serde_json::json!({
        "version": 1,
        "name": name,
        "calibration": {
            "bore_radius_mm": c.bore_radius_mm,
            "face_length_mm": d.shank.head.length_mm,
            "face_width_mm": d.profile.width_mm,
            "palm_thickness_mm": c.palm_thickness_mm,
            "head_height_mm": c.head_height_mm,
            "shoulder_start_mm": c.shoulder_start_mm,
            "shoulder_end_mm": c.shoulder_end_mm,
        },
        "vertices": built.mesh.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect::<Vec<_>>(),
        "faces": built.mesh.faces,
    });
    Source::from_json(&json.to_string())
}

/// Build `d` bare, judge it, render it, and print one line.
fn study(out: &Path, slug: &str, d: &RingDesign, renders: &[(&str, f64, f64)]) -> Result<()> {
    let lib = AlphaLibrary::builtin();
    let mut bare = d.clone();
    bare.imported_base.as_mut().unwrap().bare = true;
    let params = BuildParams { theta_steps: 900, profile_steps: 448, refine: None, ..Default::default() };
    let built = mesh::try_build(&bare, &lib, params)?;
    let field = castability::attributed_field_report(d, &lib, &d.draft, 256, 128);
    let (count, sharpest, at) = creases(&built.mesh, d.inner_radius_mm(), 45.0);
    let q = built.mesh.quality();
    let (miss, miss_at) = mirror_miss(d);
    let top = built.mesh.vertices.iter().map(|v| v.1 as f64).fold(0.0, f64::max);
    let table = built.mesh.vertices.iter().filter(|v| v.1 as f64 > top - 0.15);
    let (x, z) = table.fold((0.0f64, 0.0f64), |(x, z), v| (x.max(v.0.abs() as f64), z.max(v.2.abs() as f64)));
    let (fill, fill_at) = fill_mm(d);
    println!(
        "{slug}: {} faces, watertight {}, {} degenerate, min angle {:.1}°, table {:.2} x {:.2} mm, {count} creases > 45° (sharpest {sharpest:.1}° at [{:.2}, {:.2}, {:.2}]), mirror miss {miss:.3} mm at {miss_at:.1}°, envelope fill {fill:.3} mm at {fill_at:.1}°",
        built.mesh.faces.len(), built.report.validation.watertight, q.degenerate_faces, q.min_angle_deg, 2.0 * x, 2.0 * z, at[0], at[1], at[2]
    );
    println!("    field {:?} ({:?}): {:.4}% undercut, worst {:+.1}°, thinnest wall {:.2} mm at {:.0}°", field.verdict, field.process, field.undercut_fraction() * 100.0, field.worst_draft_deg, field.thinnest_wall_mm, field.thinnest_wall_theta_deg);
    for n in field.notes.iter().take(3) {
        println!("      {n}");
    }
    let mut pinned = d.clone();
    pinned.draft.auto_parting = false;
    pinned.draft.parting_z_mm = 0.0;
    let at_zero = castability::attributed_field_report(&pinned, &lib, &pinned.draft, 256, 128);
    println!("    field parted at z = 0: {:?} {:.4}% undercut, worst {:+.1}° (chosen plane {:+.3} mm)", at_zero.verdict, at_zero.undercut_fraction() * 100.0, at_zero.worst_draft_deg, field.parting_z_mm);
    if d.draft.process == castability::CastProcess::SandTwoPart {
        match probe::pull(d, &lib, BuildParams { theta_steps: 384, profile_steps: 192, ..params }, 0.075) {
            Ok((inspection, fine)) => println!("    pull at 384 x 192: {}; at 0.075 mm: {}", probe::release_line(&inspection.release), probe::release_line(&fine)),
            Err(e) => println!("    pull refused: {e:#}"),
        }
        let mut raw = d.clone();
        raw.imported_base.as_mut().unwrap().sand_envelope = false;
        match probe::pull(&raw, &lib, BuildParams { theta_steps: 384, profile_steps: 192, ..params }, 0.075) {
            Ok((inspection, _)) => println!("    pull without the envelope: {}", probe::release_line(&inspection.release)),
            Err(e) => println!("    pull without the envelope refused: {e:#}"),
        }
    }
    for (view, yaw, pitch) in renders {
        render::write_png(out.join(format!("{slug}-{view}.png")), &built.mesh, *yaw, *pitch, 900, render::GOLD)?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let out = std::env::args().nth(1).context("stock_spike OUT_DIR")?;
    let out = Path::new(&out);
    std::fs::create_dir_all(out)?;
    let views: [(&str, f64, f64); 3] = [("hero", 0.48, 1.0), ("face", 0.0, std::f64::consts::FRAC_PI_2), ("side", 0.0, 0.05)];
    println!("== 013 Round to a 16 mm face");
    for sand in [false, true] {
        let tag = if sand { "sand" } else { "wax" };
        let mut native = probe::stock("013", sand, None)?;
        if !sand {
            probe::cast_in(&mut native, &probe::wax_setup(0.1));
        }
        let (l0, w0) = (native.shank.head.length_mm, native.profile.width_mm);
        println!("013 {tag}: native face {l0:.2} x {w0:.2} mm, bore {:.2} mm", native.inner_radius_mm() * 2.0);
        study(out, &format!("013-{tag}-native"), &native, &views)?;
        let mut direct = native.clone();
        direct.shank.head.length_mm = 16.0;
        direct.profile.width_mm = 16.0;
        match mesh::try_build(&direct, &AlphaLibrary::default(), direct.build) {
            Ok(_) => println!("013 {tag} straight to 16 mm: builds"),
            Err(e) => println!("013 {tag} straight to 16 mm: refused: {e:#}"),
        }
        let mut step = native.clone();
        step.shank.head.length_mm = l0 * 1.3;
        step.profile.width_mm = w0 * 1.3;
        study(out, &format!("013-{tag}-130"), &step, &[])?;
        let baked = bake(&step, &format!("Signet 013 at {:.1} mm", l0 * 1.3))?;
        let mut sixteen = RingDesign::default();
        ImportedBase::attach(&mut sixteen, baked)?;
        let base = sixteen.imported_base.as_mut().unwrap();
        base.sand_envelope = sand;
        sixteen.profile = native.profile.clone();
        sixteen.shank.head = native.shank.head.clone();
        sixteen.size = native.size;
        sixteen.shank.head.length_mm = 16.0;
        sixteen.profile.width_mm = 16.0;
        sixteen.imported_base.as_mut().unwrap().chart = native.imported_base.as_ref().unwrap().chart.clone();
        sixteen.draft = native.draft.clone();
        sixteen.manufacturing = native.manufacturing.clone();
        sixteen.build = native.build;
        match study(out, &format!("013-{tag}-16mm"), &sixteen, &views) {
            Ok(()) => {}
            Err(e) => println!("013 {tag} baked at 130% then to 16 mm: refused: {e:#}"),
        }
        let mut tall = sixteen.clone();
        tall.shank.head.rise_mm += 1.2;
        match study(out, &format!("013-{tag}-16mm-tall"), &tall, &views) {
            Ok(()) => {}
            Err(e) => println!("013 {tag} at 16 mm raised 1.2 mm: refused: {e:#}"),
        }
    }
    println!("== The eleven sand-safe plans through the sand master, bare, at their own size");
    for id in SAND_SAFE {
        let preset = PRESETS.iter().find(|p| p.id == id).unwrap();
        let raw = probe::stock(id, false, None)?;
        let (raw_miss, raw_at) = mirror_miss(&raw);
        println!("{id} {}: the stock itself misses its mirror by {raw_miss:.3} mm at {raw_at:.1}°", preset.name);
        let d = probe::stock(id, true, None)?;
        study(out, &format!("{id}-{}-master", preset.name.to_lowercase()), &d, &views[..2])?;
    }
    Ok(())
}
