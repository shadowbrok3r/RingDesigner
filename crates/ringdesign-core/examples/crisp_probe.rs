//! Renders a saved design the way the collection examples do, for judging edge quality.
//! cargo build --release -p ringdesign-core --example crisp_probe
//! target/release/examples/crisp_probe <design.ring.json> <out dir> [options]
//!   --steps WxH        sweep resolution (default: the design's own build)
//!   --edge N           image edge in pixels (default 1600)
//!   --views a,b        named views: hero face palm side shoulder reverse stones (default all)
//!   --zoom x,y,z,r,yaw,pitch[,name]  a close-up of the sphere of radius r round (x, y, z)
//!   --flat             facet shading as well, to tell geometry from normals
//!   --obj x,y,z,r      write the faces within r of (x, y, z) as OBJ
use anyhow::{Context, Result};
use ringdesign_core::{AlphaLibrary, BuildParams, library, manufacturing as mf, mesh, render};
use std::f64::consts::PI;
use std::path::PathBuf;

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.48, 1.0),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.05),
    ("shoulder", -0.9, 0.62),
    ("reverse", 1.6, 0.8),
];

/// The faces whose corners all lie within `radius` of `centre`, as the collection examples crop a close-up.
fn crop(m: &mesh::Mesh, centre: [f64; 3], radius: f64) -> mesh::Mesh {
    let near = |i: u32| {
        let p = m.vertices[i as usize];
        (p.0 as f64 - centre[0]).hypot(p.1 as f64 - centre[1]).hypot(p.2 as f64 - centre[2]) < radius
    };
    let mut index = std::collections::HashMap::new();
    let mut out = mesh::Mesh::default();
    for (fi, f) in m.faces.iter().enumerate().filter(|(_, f)| f.iter().all(|&i| near(i))) {
        let g = f.map(|i| {
            *index.entry(i).or_insert_with(|| {
                out.vertices.push(m.vertices[i as usize]);
                out.normals.push(m.normals.get(i as usize).copied().unwrap_or(mesh::Vec3(0.0, 0.0, 1.0)));
                (out.vertices.len() - 1) as u32
            })
        });
        if let Ok(k) = m.corner_normals.binary_search_by_key(&(fi as u32), |c| c.0) {
            out.corner_normals.push((out.faces.len() as u32, m.corner_normals[k].1));
        }
        out.faces.push(g);
    }
    out
}

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn nums(s: &str) -> Vec<f64> {
    s.split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

fn save(path: PathBuf, rgb: &[u8], edge: usize) -> Result<()> {
    image::save_buffer(&path, rgb, edge as u32, edge as u32, image::ColorType::Rgb8)?;
    println!("  {}", path.display());
    Ok(())
}

/// A plain band carrying one decal of a star with straight walls running every way across the grid.
fn synthetic() -> ringdesign_core::RingDesign {
    use ringdesign_core::field::{Decal, DecalLayer, Layer, LayerEntry};
    let mut d = ringdesign_core::RingDesign::default();
    d.name = "Synthetic star".into();
    d.profile.width_mm = 8.0;
    d.profile.thickness_mm = 2.4;
    let star: String = (0..10)
        .map(|k| {
            let a = std::f64::consts::PI * k as f64 / 5.0 + 0.13;
            let r = if k % 2 == 0 { 480.0 } else { 200.0 };
            format!("{:.2},{:.2} ", 512.0 + r * a.cos(), 512.0 - r * a.sin())
        })
        .collect();
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024"><rect width="1024" height="1024" fill="black"/><polygon points="{star}" fill="white"/></svg>"#);
    d.svgs.push(ringdesign_core::svg::SvgAlpha { name: "Star".into(), svg, invert: false });
    let v = d.reference_loop().surface_len_mm * 0.5;
    let decal = Decal { theta_deg: 90.0, v_mm: v, size_mm: 6.0, rotation_deg: 0.0, height_mm: 0.4, flip: false };
    d.layers.layers.push(LayerEntry::new("Star", Layer::Decals(DecalLayer { alpha: "Star".into(), decals: vec![decal], feather_mm: 0.0, invert: false })));
    d
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let design = args.first().context("design path")?;
    let out = PathBuf::from(args.get(1).context("out dir")?);
    std::fs::create_dir_all(&out)?;
    let mut d = if design == "synthetic" { synthetic() } else { library::load_design(design)? };
    if args.iter().any(|a| a == "--crisp") {
        d.crisp_relief = true;
    }
    let lib = mf::source_library(&d, &AlphaLibrary::default()).into_owned();
    let mut params = d.build;
    if let Some(s) = arg(&args, "--steps") {
        let (w, h) = s.split_once('x').context("--steps WxH")?;
        params = BuildParams { theta_steps: w.parse()?, profile_steps: h.parse()?, refine: None, ..params };
    }
    if let Some(s) = arg(&args, "--soften") {
        params.soften_mm = s.parse()?;
    }
    if let Some(s) = arg(&args, "--refine") {
        let v = nums(s);
        params.refine = Some(ringdesign_core::refine::RefineParams { tolerance_mm: v[0], normal_tolerance_deg: v[1], ..Default::default() });
    }
    let edge: usize = arg(&args, "--edge").map_or(Ok(1600), str::parse)?;
    let flat = args.iter().any(|a| a == "--flat");
    if args.iter().any(|a| a == "--stamps") {
        let ctx = d.field_context();
        for s in &d.stamps {
            let f = s.frame(&d, &ctx);
            println!("  stamp {:?} at ({:.3}, {:.3}, {:.3}) normal ({:.2}, {:.2}, {:.2})", s.name, f.origin[0], f.origin[1], f.origin[2], f.z[0], f.z[1], f.z[2]);
        }
    }
    let clock = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    println!(
        "{}: {} x {} -> {} faces in {:.1} s; notes {:?}",
        d.name,
        params.theta_steps,
        params.profile_steps,
        built.mesh.faces.len(),
        clock.elapsed().as_secs_f64(),
        built.solids.notes
    );
    let fin = render::finished_from(&d, &lib, built);
    let parts = fin.parts(render::GOLD);
    let wanted: Vec<&str> = arg(&args, "--views").map_or_else(|| vec!["hero", "face", "palm", "side", "shoulder", "reverse", "stones"], |v| v.split(',').collect());
    for (name, yaw, pitch) in VIEWS.iter().filter(|(n, ..)| wanted.contains(n)) {
        save(out.join(format!("{name}.png")), &render::render_parts_ss(&parts, *yaw, *pitch, edge, edge, 3), edge)?;
    }
    let mut k = 0;
    while let Some(i) = args.iter().skip(k).position(|a| a == "--cam") {
        let at = k + i;
        k = at + 1;
        let spec = args.get(at + 1).context("--cam name,yaw,pitch")?;
        let name = spec.split(',').next().unwrap_or("cam");
        let v = nums(spec.split_once(',').map_or("", |s| s.1));
        save(out.join(format!("{name}.png")), &render::render_parts_ss(&parts, v[0], v[1], edge, edge, 3), edge)?;
    }
    if wanted.contains(&"stones") {
        // The stones close-up as the vepres examples frame it.
        let frames = ringdesign_core::stones::stone_frames(&d);
        let top = fin.metal.vertices.iter().map(|v| v.1 as f64).fold(0.0, f64::max);
        let centre = if frames.is_empty() { [0.0, top, 0.0] } else { [0.0, top - 1.0, 0.0] };
        let close = crop(&fin.metal, centre, 11.0);
        let mut ps = vec![render::Part::metal(&close, render::GOLD)];
        ps.extend(fin.stones.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
        save(out.join("stones.png"), &render::render_parts_ss(&ps, 0.3, 1.15, edge, edge, 3), edge)?;
    }
    if wanted.contains(&"stones-framed") {
        // The same close-up framed on the whole ring, as `render::write_png_framed` draws it.
        let frames = ringdesign_core::stones::stone_frames(&d);
        let top = fin.metal.vertices.iter().map(|v| v.1 as f64).fold(0.0, f64::max);
        let centre = if frames.is_empty() { [0.0, top, 0.0] } else { [0.0, top - 1.0, 0.0] };
        // Framed where the cropped close-up framed itself, so the two compare pixel for pixel.
        let (lo, hi) = crop(&fin.metal, centre, 11.0).bounds().context("empty crop")?;
        let middle = [(lo.0 + hi.0) as f64 * 0.5, (lo.1 + hi.1) as f64 * 0.5, (lo.2 + hi.2) as f64 * 0.5];
        let ext = ((hi.0 - lo.0).max(hi.1 - lo.1).max(hi.2 - lo.2)) as f64;
        let frame = render::Framing::new(middle, ext * 1.25 * 0.5);
        save(out.join("stones-framed.png"), &render::render_parts_framed(&parts, 0.3, 1.15, frame, edge, edge, 3), edge)?;
    }
    let mut k = 0;
    while let Some(i) = args.iter().skip(k).position(|a| a == "--zoom") {
        let at = k + i;
        k = at + 1;
        let spec = args.get(at + 1).context("--zoom spec")?;
        let v = nums(spec);
        let name = spec.split(',').nth(6).map_or_else(|| format!("zoom{at}"), str::to_owned);
        let frame = render::Framing::new([v[0], v[1], v[2]], v[3]);
        save(out.join(format!("{name}.png")), &render::render_parts_framed(&parts, v[4], v[5], frame, edge, edge, 3), edge)?;
        if flat {
            let ps = [render::Part { smooth: false, ..render::Part::metal(&fin.metal, render::GOLD) }];
            save(out.join(format!("{name}-flat.png")), &render::render_parts_framed(&ps, v[4], v[5], frame, edge, edge, 3), edge)?;
        }
    }
    // A vepres_rubus wedge close-up cropped and framed alike: theta, half_deg, over_mm, yaw, pitch, turned, name.
    let mut k = 0;
    while let Some(i) = args.iter().skip(k).position(|a| a == "--wedge") {
        let at = k + i;
        k = at + 1;
        let spec = args.get(at + 1).context("--wedge theta,half,over,yaw,pitch,turned,name")?;
        let v = nums(spec);
        let name = spec.split(',').nth(6).unwrap_or("wedge");
        let turn = if v[5] > 0.5 { (90.0 - v[0]).to_radians() } else { 0.0 };
        let (ts, tc) = turn.sin_cos();
        let near = |p: &mesh::Vec3| {
            let t = (p.1 as f64).atan2(p.0 as f64).to_degrees();
            ((t - v[0] + 540.0).rem_euclid(360.0) - 180.0).abs() <= v[1] && (p.0 as f64).hypot(p.1 as f64) >= v[2]
        };
        let mut index = std::collections::HashMap::new();
        let mut cut = mesh::Mesh::default();
        for f in fin.metal.faces.iter().filter(|f| f.iter().all(|&i| near(&fin.metal.vertices[i as usize]))) {
            let g = f.map(|i| {
                *index.entry(i).or_insert_with(|| {
                    let (p, n) = (fin.metal.vertices[i as usize], fin.metal.normals[i as usize]);
                    let r = |q: mesh::Vec3| mesh::Vec3((q.0 as f64 * tc - q.1 as f64 * ts) as f32, (q.0 as f64 * ts + q.1 as f64 * tc) as f32, q.2);
                    cut.vertices.push(r(p));
                    cut.normals.push(r(n));
                    (cut.vertices.len() - 1) as u32
                })
            });
            cut.faces.push(g);
        }
        save(out.join(format!("{name}-cropped.png")), &render::render_parts_ss(&[render::Part::metal(&cut, render::GOLD)], v[3], v[4], edge, edge, 3), edge)?;
        let (lo, hi) = cut.bounds().context("empty wedge")?;
        let m = [(lo.0 + hi.0) as f64 * 0.5, (lo.1 + hi.1) as f64 * 0.5, (lo.2 + hi.2) as f64 * 0.5];
        let ext = ((hi.0 - lo.0).max(hi.1 - lo.1).max(hi.2 - lo.2)) as f64;
        let centre = [m[0] * tc + m[1] * ts, -m[0] * ts + m[1] * tc, m[2]];
        let frame = render::Framing::new(centre, ext * 1.25 * 0.5);
        save(out.join(format!("{name}.png")), &render::render_parts_framed(&parts, v[3] + turn, v[4], frame, edge, edge, 3), edge)?;
    }
    // A close-up of the crown at ring angle theta: theta, half-width mm, pitch, name; yaw turns theta to the camera.
    let mut k = 0;
    while let Some(i) = args.iter().skip(k).position(|a| a == "--tzoom") {
        let at = k + i;
        k = at + 1;
        let spec = args.get(at + 1).context("--tzoom theta,half,pitch,name")?;
        let v = nums(spec);
        let name = spec.split(',').nth(3).unwrap_or("tzoom");
        let r = fin
            .metal
            .vertices
            .iter()
            .filter(|p| (p.2 as f64).abs() < 1.0 && (((p.1 as f64).atan2(p.0 as f64).to_degrees() - v[0] + 540.0).rem_euclid(360.0) - 180.0).abs() < 1.0)
            .map(|p| (p.0 as f64).hypot(p.1 as f64))
            .fold(0.0, f64::max);
        let (s, c) = v[0].to_radians().sin_cos();
        let frame = render::Framing::new([r * c, r * s, 0.0], v[1]);
        let yaw = render::yaw_facing(v[0]);
        save(out.join(format!("{name}.png")), &render::render_parts_framed(&parts, yaw, v[2], frame, edge, edge, 3), edge)?;
        if flat {
            let ps = [render::Part { smooth: false, ..render::Part::metal(&fin.metal, render::GOLD) }];
            save(out.join(format!("{name}-flat.png")), &render::render_parts_framed(&ps, yaw, v[2], frame, edge, edge, 3), edge)?;
        }
    }
    // A close-up of a square of a full view: yaw, pitch, the square's centre in that view's pixels and its half-size.
    let mut k = 0;
    while let Some(i) = args.iter().skip(k).position(|a| a == "--pzoom") {
        let at = k + i;
        k = at + 1;
        let spec = args.get(at + 1).context("--pzoom yaw,pitch,px,py,half_px,name")?;
        let v = nums(spec);
        let name = spec.split(',').nth(5).unwrap_or("pzoom");
        let (min, max) = fin.metal.bounds().context("empty mesh")?;
        let c = [(min.0 + max.0) as f64 * 0.5, (min.1 + max.1) as f64 * 0.5, (min.2 + max.2) as f64 * 0.5];
        let ext = ((max.0 - min.0).max(max.1 - min.1).max(max.2 - min.2)) as f64;
        let scale = edge as f64 / (ext * 1.25);
        let (sy, cy) = v[0].sin_cos();
        let (sp, cp) = v[1].sin_cos();
        let (x, y) = ((v[2] - edge as f64 * 0.5) / scale, (edge as f64 * 0.5 - v[3]) / scale);
        let (y1, z) = (y * cp, -y * sp);
        let centre = [c[0] + x * cy + y1 * sy, c[1] - x * sy + y1 * cy, c[2] + z];
        let frame = render::Framing::new(centre, v[4] / scale);
        save(out.join(format!("{name}.png")), &render::render_parts_framed(&parts, v[0], v[1], frame, edge, edge, 3), edge)?;
        if flat {
            let ps = [render::Part { smooth: false, ..render::Part::metal(&fin.metal, render::GOLD) }];
            save(out.join(format!("{name}-flat.png")), &render::render_parts_framed(&ps, v[0], v[1], frame, edge, edge, 3), edge)?;
        }
    }
    if let Some(spec) = arg(&args, "--obj") {
        let v = nums(spec);
        let close = crop(&fin.metal, [v[0], v[1], v[2]], v[3]);
        let mut text = String::new();
        for p in &close.vertices {
            text += &format!("v {} {} {}\n", p.0, p.1, p.2);
        }
        for p in &close.normals {
            text += &format!("vn {} {} {}\n", p.0, p.1, p.2);
        }
        for f in &close.faces {
            text += &format!("f {}//{} {}//{} {}//{}\n", f[0] + 1, f[0] + 1, f[1] + 1, f[1] + 1, f[2] + 1, f[2] + 1);
        }
        std::fs::write(out.join("crop.obj"), text)?;
        println!("  {} faces to crop.obj", close.faces.len());
    }
    Ok(())
}
