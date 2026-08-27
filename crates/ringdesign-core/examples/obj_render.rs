//! Render an OBJ mesh with the core software renderer.
//!
//! Single file:  obj_render <in.obj> <out.png> [--yaw DEG] [--pitch DEG] [--edge PX] [--cg] [--gif out.gif] [--stones gems.obj]
//! Batch:        obj_render --dir <root> [--cg] [--edge PX] [--force]
//!
//! `--cg` maps a CrossGems mesh (ring in XZ, finger axis Y, head at +Z)
//! into this crate's frame (ring in XY, finger axis Z, top at +Y).
//! Batch mode renders every `*.obj` under the root to a sibling `.png`.
use ringdesign_core::mesh::{Mesh, Vec3};
use ringdesign_core::render;
use std::path::{Path, PathBuf};

struct Opts {
    yaw: f64,
    pitch: f64,
    edge: usize,
    cg: bool,
    gif: Option<String>,
    force: bool,
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut o = Opts { yaw: 0.55, pitch: 1.05, edge: 640, cg: false, gif: None, force: false };
    let mut stones: Option<String> = None;
    let mut positional = Vec::new();
    let mut dir: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--yaw" => { o.yaw = args[i + 1].parse::<f64>()?.to_radians(); i += 2; }
            "--pitch" => { o.pitch = args[i + 1].parse::<f64>()?.to_radians(); i += 2; }
            "--edge" => { o.edge = args[i + 1].parse()?; i += 2; }
            "--cg" => { o.cg = true; i += 1; }
            "--force" => { o.force = true; i += 1; }
            "--gif" => { o.gif = Some(args[i + 1].clone()); i += 2; }
            "--dir" => { dir = Some(args[i + 1].clone()); i += 2; }
            "--stones" => { stones = Some(args[i + 1].clone()); i += 2; }
            other => { positional.push(other.to_string()); i += 1; }
        }
    }
    if let Some(root) = dir {
        let mut files = Vec::new();
        walk(Path::new(&root), &mut files);
        files.sort();
        let mut n = 0;
        for f in files {
            let png = f.with_extension("png");
            if png.exists() && !o.force { continue; }
            let mesh = read_obj(&f, o.cg)?;
            if mesh.faces.is_empty() { continue; }
            render::write_png(&png, &mesh, o.yaw, o.pitch, o.edge, render::GOLD)?;
            n += 1;
        }
        eprintln!("rendered {n} meshes under {root}");
        return Ok(());
    }
    if positional.len() < 2 {
        eprintln!("usage: obj_render <in.obj> <out.png> [--yaw DEG] [--pitch DEG] [--edge PX] [--cg] [--gif out.gif]\n       obj_render --dir <root> [--cg] [--edge PX] [--force]");
        std::process::exit(2);
    }
    let mesh = read_obj(Path::new(&positional[0]), o.cg)?;
    // A second mesh drawn as stones, for a ring exported with its gems.
    let gems = stones.map(|p| read_obj(Path::new(&p), o.cg)).transpose()?;
    let mut parts = vec![render::Part::metal(&mesh, render::GOLD)];
    if let Some(g) = &gems {
        parts.push(render::Part::stone(g));
    }
    render::write_png_parts(&positional[1], &parts, o.yaw, o.pitch, o.edge)?;
    if let Some(g) = o.gif {
        render::write_turntable_gif(g, &mesh, 36, 360, render::GOLD)?;
    }
    Ok(())
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "obj") {
            out.push(p);
        }
    }
}

fn read_obj(path: &Path, cg: bool) -> anyhow::Result<Mesh> {
    let text = std::fs::read_to_string(path)?;
    let mut vertices: Vec<Vec3> = Vec::new();
    let mut faces: Vec<[u32; 3]> = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let mut c = [0f32; 3];
                for k in c.iter_mut() {
                    *k = it.next().unwrap_or("0").parse()?;
                }
                vertices.push(if cg { Vec3(c[0], c[2], -c[1]) } else { Vec3(c[0], c[1], c[2]) });
            }
            Some("f") => {
                let idx: Vec<u32> = it
                    .map(|t| t.split('/').next().unwrap_or("0").parse::<i64>().unwrap_or(0))
                    .map(|k| if k < 0 { (vertices.len() as i64 + k) as u32 } else { (k - 1).max(0) as u32 })
                    .collect();
                for k in 1..idx.len().saturating_sub(1) {
                    faces.push([idx[0], idx[k], idx[k + 1]]);
                }
            }
            _ => {}
        }
    }
    // Inward-wound meshes are flipped so the renderer's backface cull shows the outside.
    let mut vol = 0f64;
    for f in &faces {
        let (a, b, c) = (vertices[f[0] as usize], vertices[f[1] as usize], vertices[f[2] as usize]);
        let (a, b, c) = ([a.0 as f64, a.1 as f64, a.2 as f64], [b.0 as f64, b.1 as f64, b.2 as f64], [c.0 as f64, c.1 as f64, c.2 as f64]);
        vol += a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0]);
    }
    if vol < 0.0 {
        for f in faces.iter_mut() {
            f.swap(1, 2);
        }
    }
    let mut normals = vertex_normals(&vertices, &faces);
    // Two faceless vertices widen the bounds the renderer frames on, so a
    // rotated ring is not cropped at the image edge.
    if let Some((min, max)) = (Mesh { vertices: vertices.clone(), normals: normals.clone(), faces: Vec::new() }).bounds() {
        let pad = 0.35 * (max.0 - min.0).max(max.1 - min.1).max(max.2 - min.2);
        vertices.push(Vec3(min.0 - pad, min.1 - pad, min.2 - pad));
        vertices.push(Vec3(max.0 + pad, max.1 + pad, max.2 + pad));
        normals.push(Vec3(0.0, 0.0, 1.0));
        normals.push(Vec3(0.0, 0.0, 1.0));
    }
    Ok(Mesh { vertices, normals, faces })
}

fn vertex_normals(v: &[Vec3], faces: &[[u32; 3]]) -> Vec<Vec3> {
    let mut acc = vec![[0f64; 3]; v.len()];
    for f in faces {
        let (a, b, c) = (v[f[0] as usize], v[f[1] as usize], v[f[2] as usize]);
        let e1 = [(b.0 - a.0) as f64, (b.1 - a.1) as f64, (b.2 - a.2) as f64];
        let e2 = [(c.0 - a.0) as f64, (c.1 - a.1) as f64, (c.2 - a.2) as f64];
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        for &k in f {
            for d in 0..3 {
                acc[k as usize][d] += n[d];
            }
        }
    }
    acc.iter()
        .map(|n| {
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if l < 1e-12 { Vec3(0.0, 0.0, 1.0) } else { Vec3((n[0] / l) as f32, (n[1] / l) as f32, (n[2] / l) as f32) }
        })
        .collect()
}
