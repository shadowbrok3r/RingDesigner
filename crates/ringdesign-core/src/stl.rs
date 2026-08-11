//! Binary and ASCII STL export.

use std::path::Path;

use crate::mesh::Mesh;

/// The 80-byte binary header: generator, design name, units. Never starts with
/// "solid", which some parsers sniff as ASCII.
fn binary_header(name: &str) -> [u8; 80] {
    let text = format!(
        "RingDesigner {}; {}; UNITS=mm",
        env!("CARGO_PKG_VERSION"),
        name
    );
    let mut header = [0u8; 80];
    let n = text.len().min(80);
    header[..n].copy_from_slice(&text.as_bytes()[..n]);
    header
}

/// Faces whose three indices are all in range; a partial facet would corrupt
/// the fixed 50-byte record stream.
fn whole_faces<'a>(mesh: &'a Mesh) -> impl Iterator<Item = &'a [u32; 3]> {
    mesh.faces
        .iter()
        .filter(|f| f.iter().all(|&i| (i as usize) < mesh.vertices.len()))
}

/// Serialize to binary STL with per-facet normals.
pub fn to_stl_binary(mesh: &Mesh, name: &str) -> Vec<u8> {
    let faces: Vec<&[u32; 3]> = whole_faces(mesh).collect();
    let mut out = Vec::with_capacity(84 + faces.len() * 50);
    out.extend_from_slice(&binary_header(name));
    out.extend_from_slice(&(faces.len() as u32).to_le_bytes());
    for f in faces {
        let n = mesh.face_normal(f).unwrap_or([0.0, 0.0, 0.0]);
        for c in n {
            out.extend_from_slice(&(c as f32).to_le_bytes());
        }
        for &i in f {
            let v = mesh.vertices[i as usize];
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
    for f in whole_faces(mesh) {
        let n = mesh.face_normal(f).unwrap_or([0.0, 0.0, 0.0]);
        let _ = writeln!(s, "  facet normal {:e} {:e} {:e}", n[0], n[1], n[2]);
        let _ = writeln!(s, "    outer loop");
        for &i in f {
            let v = mesh.vertices[i as usize];
            let _ = writeln!(s, "      vertex {:e} {:e} {:e}", v.0, v.1, v.2);
        }
        let _ = writeln!(s, "    endloop");
        let _ = writeln!(s, "  endfacet");
    }
    let _ = writeln!(s, "endsolid {name}");
    s
}

/// Write a binary STL, returning the byte count.
pub fn write_stl(path: impl AsRef<Path>, mesh: &Mesh, name: &str) -> anyhow::Result<usize> {
    let bytes = to_stl_binary(mesh, name);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{self, BuildParams, Vec3};
    use std::collections::HashMap;

    fn small_build() -> Mesh {
        let design = crate::RingDesign::default();
        let lib = crate::AlphaLibrary::builtin();
        let params = BuildParams { theta_steps: 96, profile_steps: 64, ..Default::default() };
        mesh::build(&design, &lib, params).mesh
    }

    /// Parse a binary STL back into a welded mesh, asserting record layout.
    fn read_stl_bytes(b: &[u8]) -> Mesh {
        assert!(b.len() >= 84, "too short for a binary STL");
        let n = u32::from_le_bytes(b[80..84].try_into().unwrap()) as usize;
        assert_eq!(b.len(), 84 + n * 50, "byte length disagrees with the facet count");
        let f32_at = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());

        let mut index: HashMap<[i64; 3], u32> = HashMap::new();
        let mut vertices: Vec<Vec3> = Vec::new();
        let mut faces: Vec<[u32; 3]> = Vec::new();
        for t in 0..n {
            let base = 84 + t * 50 + 12;
            let mut tri = [0u32; 3];
            for (k, slot) in tri.iter_mut().enumerate() {
                let p = [
                    f32_at(base + k * 12),
                    f32_at(base + k * 12 + 4),
                    f32_at(base + k * 12 + 8),
                ];
                let key = [
                    (p[0] as f64 * 1e4).round() as i64,
                    (p[1] as f64 * 1e4).round() as i64,
                    (p[2] as f64 * 1e4).round() as i64,
                ];
                *slot = *index.entry(key).or_insert_with(|| {
                    vertices.push(Vec3(p[0], p[1], p[2]));
                    (vertices.len() - 1) as u32
                });
            }
            faces.push(tri);
        }
        Mesh { vertices, normals: Vec::new(), faces }
    }

    #[test]
    fn the_header_names_the_generator_and_the_units() {
        let bytes = to_stl_binary(&small_build(), "Test Ring");
        let header = String::from_utf8_lossy(&bytes[..80]);
        assert!(header.starts_with("RingDesigner "), "{header}");
        assert!(header.contains("Test Ring"), "{header}");
        assert!(header.contains("UNITS=mm"), "{header}");
        assert!(!header.starts_with("solid"));
    }

    #[test]
    fn a_binary_round_trip_preserves_the_solid() {
        let mesh = small_build();
        let bytes = to_stl_binary(&mesh, "rt");
        let back = read_stl_bytes(&bytes);
        assert_eq!(back.faces.len(), mesh.faces.len());
        let v = back.validate();
        assert!(v.watertight, "welded round trip should close: {v:?}");
        let dv = (back.volume_mm3() - mesh.volume_mm3()).abs();
        assert!(dv < mesh.volume_mm3() * 1e-4, "volume drifted by {dv} mm3");
    }

    #[test]
    fn an_out_of_range_face_drops_whole_not_partial() {
        let mut mesh = small_build();
        let count = mesh.faces.len();
        mesh.faces.push([9_000_000, 1, 2]);
        let bytes = to_stl_binary(&mesh, "broken");
        let n = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
        assert_eq!(n, count, "the malformed facet is skipped entirely");
        assert_eq!(bytes.len(), 84 + n * 50);
    }

    #[test]
    fn ascii_grammar_holds() {
        let mesh = small_build();
        let s = to_stl_ascii(&mesh, "grammar");
        assert!(s.starts_with("solid grammar\n"));
        assert!(s.trim_end().ends_with("endsolid grammar"));
        assert_eq!(s.matches("facet normal").count(), mesh.faces.len());
        assert_eq!(s.matches("vertex").count(), mesh.faces.len() * 3);
    }

    #[test]
    fn obj_indices_are_base_1_and_in_range() {
        let mesh = small_build();
        let path = std::env::temp_dir().join("ringdesign_stl_test.obj");
        write_obj(&path, &mesh, "obj test").unwrap();
        let s = std::fs::read_to_string(&path).unwrap();
        assert_eq!(s.lines().filter(|l| l.starts_with("v ")).count(), mesh.vertices.len());
        assert_eq!(s.lines().filter(|l| l.starts_with("vn ")).count(), mesh.normals.len());
        let mut min_i = u32::MAX;
        let mut max_i = 0u32;
        for line in s.lines().filter(|l| l.starts_with("f ")) {
            for part in line.split_whitespace().skip(1) {
                let i: u32 = part.split('/').next().unwrap().parse().unwrap();
                min_i = min_i.min(i);
                max_i = max_i.max(i);
            }
        }
        assert!(min_i >= 1, "OBJ indices are 1-based");
        assert!(max_i as usize <= mesh.vertices.len());
    }
}
