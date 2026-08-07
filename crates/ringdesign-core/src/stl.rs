//! Binary and ASCII STL export.

use std::path::Path;

use crate::mesh::Mesh;

/// Serialize to binary STL with per-facet normals.
pub fn to_stl_binary(mesh: &Mesh) -> Vec<u8> {
    let mut out = Vec::with_capacity(84 + mesh.faces.len() * 50);
    out.extend_from_slice(&[0u8; 80]);
    out.extend_from_slice(&(mesh.faces.len() as u32).to_le_bytes());
    for f in &mesh.faces {
        let n = mesh.face_normal(f).unwrap_or([0.0, 0.0, 0.0]);
        for c in n {
            out.extend_from_slice(&(c as f32).to_le_bytes());
        }
        for &i in f {
            let v = match mesh.vertices.get(i as usize) {
                Some(v) => *v,
                None => continue,
            };
            out.extend_from_slice(&v.0.to_le_bytes());
            out.extend_from_slice(&v.1.to_le_bytes());
            out.extend_from_slice(&v.2.to_le_bytes());
        }
        out.extend_from_slice(&[0u8, 0u8]);
    }
    out
}

/// Serialize to ASCII STL.
pub fn to_stl_ascii(mesh: &Mesh, name: &str) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(mesh.faces.len() * 180);
    let _ = writeln!(s, "solid {name}");
    for f in &mesh.faces {
        let n = mesh.face_normal(f).unwrap_or([0.0, 0.0, 0.0]);
        let _ = writeln!(s, "  facet normal {:e} {:e} {:e}", n[0], n[1], n[2]);
        let _ = writeln!(s, "    outer loop");
        for &i in f {
            if let Some(v) = mesh.vertices.get(i as usize) {
                let _ = writeln!(s, "      vertex {:e} {:e} {:e}", v.0, v.1, v.2);
            }
        }
        let _ = writeln!(s, "    endloop");
        let _ = writeln!(s, "  endfacet");
    }
    let _ = writeln!(s, "endsolid {name}");
    s
}

/// Write a binary STL, returning the byte count.
pub fn write_stl(path: impl AsRef<Path>, mesh: &Mesh) -> anyhow::Result<usize> {
    let bytes = to_stl_binary(mesh);
    std::fs::write(path, &bytes)?;
    Ok(bytes.len())
}

/// Write an OBJ with smooth vertex normals, returning the byte count.
pub fn write_obj(path: impl AsRef<Path>, mesh: &Mesh, name: &str) -> anyhow::Result<usize> {
    use std::fmt::Write;
    let mut s = String::with_capacity(mesh.vertices.len() * 40 + mesh.faces.len() * 30);
    let _ = writeln!(s, "o {name}");
    for v in &mesh.vertices {
        let _ = writeln!(s, "v {} {} {}", v.0, v.1, v.2);
    }
    for n in &mesh.normals {
        let _ = writeln!(s, "vn {} {} {}", n.0, n.1, n.2);
    }
    let has_normals = mesh.normals.len() == mesh.vertices.len();
    for f in &mesh.faces {
        let (a, b, c) = (f[0] + 1, f[1] + 1, f[2] + 1);
        if has_normals {
            let _ = writeln!(s, "f {a}//{a} {b}//{b} {c}//{c}");
        } else {
            let _ = writeln!(s, "f {a} {b} {c}");
        }
    }
    std::fs::write(path, s.as_bytes())?;
    Ok(s.len())
}
