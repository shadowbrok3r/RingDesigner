//! Mesh readers: OBJ and STL (binary or ASCII) into the core's mesh. The
//! core writes these formats; Free mode also has to take them in.

use std::collections::HashMap;
use std::path::Path;

use ringdesign_core::mesh::{Mesh, Vec3};

use crate::mesh_from;

/// Read an OBJ: `v` and `f` lines, polygons fanned into triangles,
/// negative indices resolved, texture and normal indices ignored.
pub fn read_obj(path: impl AsRef<Path>) -> anyhow::Result<Mesh> {
    let text = std::fs::read_to_string(path)?;
    parse_obj(&text)
}

pub fn parse_obj(text: &str) -> anyhow::Result<Mesh> {
    let mut vertices: Vec<Vec3> = Vec::new();
    let mut faces: Vec<[u32; 3]> = Vec::new();
    for (ln, line) in text.lines().enumerate() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let xyz: Vec<f32> = it.take(3).map(|t| t.parse::<f32>()).collect::<Result<_, _>>().map_err(|e| anyhow::anyhow!("line {}: {e}", ln + 1))?;
                if xyz.len() != 3 {
                    anyhow::bail!("line {}: a vertex needs three coordinates", ln + 1);
                }
                vertices.push(Vec3(xyz[0], xyz[1], xyz[2]));
            }
            Some("f") => {
                let idx: Vec<u32> = it
                    .map(|t| {
                        let first = t.split('/').next().unwrap_or("");
                        let i: i64 = first.parse().map_err(|_| anyhow::anyhow!("line {}: bad index {t:?}", ln + 1))?;
                        let n = vertices.len() as i64;
                        let resolved = if i < 0 { n + i } else { i - 1 };
                        if resolved < 0 || resolved >= n {
                            anyhow::bail!("line {}: index {i} is outside the {n} vertices read so far", ln + 1);
                        }
                        Ok(resolved as u32)
                    })
                    .collect::<anyhow::Result<_>>()?;
                if idx.len() < 3 {
                    anyhow::bail!("line {}: a face needs three vertices", ln + 1);
                }
                for k in 1..idx.len() - 1 {
                    faces.push([idx[0], idx[k], idx[k + 1]]);
                }
            }
            _ => {}
        }
    }
    if faces.is_empty() {
        anyhow::bail!("no faces");
    }
    Ok(mesh_from(vertices, faces))
}

/// Read an STL, binary or ASCII, welding identical vertices so the result
/// can be a manifold.
pub fn read_stl(path: impl AsRef<Path>) -> anyhow::Result<Mesh> {
    let bytes = std::fs::read(path)?;
    parse_stl(&bytes)
}

pub fn parse_stl(bytes: &[u8]) -> anyhow::Result<Mesh> {
    let looks_ascii = bytes.len() >= 5 && bytes.starts_with(b"solid") && !is_binary_sized(bytes);
    let tris: Vec<[[f32; 3]; 3]> = if looks_ascii { parse_stl_ascii(std::str::from_utf8(bytes)?)? } else { parse_stl_binary(bytes)? };
    weld(&tris)
}

fn is_binary_sized(bytes: &[u8]) -> bool {
    if bytes.len() < 84 {
        return false;
    }
    let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    bytes.len() == 84 + n * 50
}

fn parse_stl_binary(bytes: &[u8]) -> anyhow::Result<Vec<[[f32; 3]; 3]>> {
    if bytes.len() < 84 {
        anyhow::bail!("too short for a binary STL");
    }
    let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    if bytes.len() < 84 + n * 50 {
        anyhow::bail!("binary STL truncated: {n} triangles announced");
    }
    let f = |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    Ok((0..n)
        .map(|k| {
            let base = 84 + k * 50 + 12;
            [[f(base), f(base + 4), f(base + 8)], [f(base + 12), f(base + 16), f(base + 20)], [f(base + 24), f(base + 28), f(base + 32)]]
        })
        .collect())
}

fn parse_stl_ascii(text: &str) -> anyhow::Result<Vec<[[f32; 3]; 3]>> {
    let mut tris = Vec::new();
    let mut cur: Vec<[f32; 3]> = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if it.next() == Some("vertex") {
            let xyz: Vec<f32> = it.take(3).map(|t| t.parse::<f32>()).collect::<Result<_, _>>()?;
            if xyz.len() == 3 {
                cur.push([xyz[0], xyz[1], xyz[2]]);
                if cur.len() == 3 {
                    tris.push([cur[0], cur[1], cur[2]]);
                    cur.clear();
                }
            }
        }
    }
    if tris.is_empty() {
        anyhow::bail!("no triangles");
    }
    Ok(tris)
}

fn weld(tris: &[[[f32; 3]; 3]]) -> anyhow::Result<Mesh> {
    let mut index: HashMap<[u32; 3], u32> = HashMap::new();
    let mut vertices: Vec<Vec3> = Vec::new();
    let mut faces: Vec<[u32; 3]> = Vec::with_capacity(tris.len());
    for t in tris {
        let mut f = [0u32; 3];
        for (k, p) in t.iter().enumerate() {
            let key = [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
            let i = *index.entry(key).or_insert_with(|| {
                vertices.push(Vec3(p[0], p[1], p[2]));
                (vertices.len() - 1) as u32
            });
            f[k] = i;
        }
        if f[0] != f[1] && f[1] != f[2] && f[0] != f[2] {
            faces.push(f);
        }
    }
    if faces.is_empty() {
        anyhow::bail!("no triangles");
    }
    Ok(mesh_from(vertices, faces))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obj_and_stl_read_back_what_the_core_writes() {
        let mut d = ringdesign_core::RingDesign::default();
        d.profile.width_mm = 4.0;
        let lib = ringdesign_core::AlphaLibrary::default();
        let out = ringdesign_core::mesh::build(&d, &lib, ringdesign_core::mesh::BuildParams { theta_steps: 48, profile_steps: 24, ..Default::default() });
        let dir = std::env::temp_dir().join(format!("ringdesign-solid-io-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let obj = dir.join("r.obj");
        let stl = dir.join("r.stl");
        ringdesign_core::stl::write_obj(&obj, &out.mesh, "r").unwrap();
        ringdesign_core::stl::write_stl(&stl, &out.mesh, "r").unwrap();
        let a = read_obj(&obj).unwrap();
        let b = read_stl(&stl).unwrap();
        assert_eq!(a.faces.len(), out.mesh.faces.len());
        assert_eq!(b.faces.len(), out.mesh.faces.len());
        assert!(a.validate().watertight && b.validate().watertight, "{:?} {:?}", a.validate(), b.validate());
        assert!((a.volume_mm3() - out.mesh.volume_mm3()).abs() < 1e-2 * out.mesh.volume_mm3());
        assert!((b.volume_mm3() - out.mesh.volume_mm3()).abs() < 1e-2 * out.mesh.volume_mm3());
        let ascii = "solid t\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid t\n";
        assert_eq!(parse_stl(ascii.as_bytes()).unwrap().faces.len(), 1);
        assert!(parse_obj("v 0 0 0\nv 1 0 0\nf 1 2 9\n").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
