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
/// Faces whose three vertices all exist and are all finite.
///
/// STL, OBJ and PLY all write coordinates verbatim, so a non-finite vertex
/// leaves the app as `nan` in a file a caster feeds to a slicer. 3MF and GLB
/// have always dropped such faces; this is what brings the other three into
/// line, and it is the last line of defence behind the guards in `mesh::build`,
/// `refine::point` and `castability::section_at`.
fn whole_faces<'a>(mesh: &'a Mesh) -> impl Iterator<Item = &'a [u32; 3]> {
    mesh.faces.iter().filter(|f| {
        f.iter().all(|&i| {
            mesh.vertices
                .get(i as usize)
                .is_some_and(|v| v.0.is_finite() && v.1.is_finite() && v.2.is_finite())
        })
    })
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

/// A vertex list writes positionally, so a non-finite one cannot simply be
/// dropped without renumbering every face. 3MF's rule is the one that works:
/// write it as the origin and drop the faces that touch it, which keeps the
/// indices stable and keeps `nan` out of the file.
fn finite_or_origin(v: crate::mesh::Vec3) -> crate::mesh::Vec3 {
    if v.is_finite() { v } else { crate::mesh::Vec3(0.0, 0.0, 0.0) }
}

/// Write an OBJ with smooth vertex normals, returning the byte count.
pub fn write_obj(path: impl AsRef<Path>, mesh: &Mesh, name: &str) -> anyhow::Result<usize> {
    use std::fmt::Write;
    let mut s = String::with_capacity(mesh.vertices.len() * 40 + mesh.faces.len() * 30);
    let _ = writeln!(s, "o {name}");
    for v in &mesh.vertices {
        let v = finite_or_origin(*v);
        let _ = writeln!(s, "v {} {} {}", v.0, v.1, v.2);
    }
    for n in &mesh.normals {
        let n = finite_or_origin(*n);
        let _ = writeln!(s, "vn {} {} {}", n.0, n.1, n.2);
    }
    let has_normals = mesh.normals.len() == mesh.vertices.len();
    for f in whole_faces(mesh) {
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

/// Binary little-endian PLY: positions and normals per vertex, index lists
/// per face. The format scan and measurement tools speak.
pub fn write_ply(path: impl AsRef<Path>, mesh: &Mesh, name: &str) -> anyhow::Result<usize> {
    let has_normals = mesh.normals.len() == mesh.vertices.len();
    let mut header = format!(
        "ply\nformat binary_little_endian 1.0\ncomment {}\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\n",
        name.replace(['\n', '\r'], " "),
        mesh.vertices.len()
    );
    if has_normals {
        header.push_str("property float nx\nproperty float ny\nproperty float nz\n");
    }
    // The count is in the header, so the kept faces have to be known first.
    let faces: Vec<&[u32; 3]> = whole_faces(mesh).collect();
    header.push_str(&format!(
        "element face {}\nproperty list uchar uint vertex_indices\nend_header\n",
        faces.len()
    ));
    let mut out = header.into_bytes();
    out.reserve(mesh.vertices.len() * 24 + faces.len() * 13);
    for (i, v) in mesh.vertices.iter().enumerate() {
        let v = finite_or_origin(*v);
        for c in [v.0, v.1, v.2] {
            out.extend_from_slice(&c.to_le_bytes());
        }
        if has_normals {
            let n = finite_or_origin(mesh.normals[i]);
            for c in [n.0, n.1, n.2] {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
    }
    for f in faces {
        out.push(3);
        for &i in f.iter() {
            out.extend_from_slice(&i.to_le_bytes());
        }
    }
    std::fs::write(path, &out)?;
    Ok(out.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{self, BuildParams, Vec3};
    use std::collections::HashMap;


    /// A design file is data, and `opacity` and `height_mm` are unbounded
    /// `f64`s read straight out of it. Two ways they reach a writer:
    /// a NaN opacity makes the height itself NaN, and a merely enormous
    /// height is a perfectly finite `f64` that overflows on the way to `f32`.
    /// The first is caught in `mesh::build`, the second only at the writer —
    /// so both are checked here, on every format that writes coordinates
    /// verbatim.
    #[test]
    fn no_export_carries_a_non_finite_coordinate() {
        use crate::field::{BorderLayer, BorderProfile, Layer, LayerEntry};

        let hostile = |opacity: f64, height_mm: f64| {
            let mut d = crate::RingDesign::default();
            let mut e = LayerEntry::new(
                "hostile",
                Layer::Border(BorderLayer {
                    v_mm: d.field_context().band_v_len_mm * 0.5,
                    width_mm: 0.8,
                    height_mm,
                    profile: BorderProfile::Round,
                    mirror: false,
                    rope_twists: 0,
                }),
            );
            e.opacity = opacity;
            d.layers.layers.push(e);
            let lib = crate::AlphaLibrary::builtin();
            let params = BuildParams { theta_steps: 64, profile_steps: 48, ..Default::default() };
            mesh::build(&d, &lib, params).mesh
        };

        let dir = std::env::temp_dir().join("ringdesign-finite-test");
        std::fs::create_dir_all(&dir).unwrap();

        // `mesh::build` zeroes a non-finite height, so a NaN opacity produces a
        // clean mesh; an overflowing height is finite as an `f64` and only goes
        // bad in the cast to `f32`, so that one must reach the writers dirty or
        // this test is checking nothing.
        let overflow = hostile(1.0, 1e308);
        assert!(
            overflow.vertices.iter().any(|v| !v.is_finite()),
            "the overflow case must actually put a non-finite vertex in the mesh"
        );
        assert!(
            hostile(f64::NAN, 0.4).vertices.iter().all(|v| v.is_finite()),
            "a NaN height is caught in mesh::build, before any writer sees it"
        );

        for (label, m) in [
            ("nan opacity", hostile(f64::NAN, 0.4)),
            ("overflowing height", overflow),
            ("infinite opacity", hostile(f64::INFINITY, 0.4)),
        ] {
            // Binary STL: 84-byte header, then 50 bytes per facet, of which
            // the first 48 are twelve f32s. Every one must be finite.
            let stl = to_stl_binary(&m, "hostile");
            let count = u32::from_le_bytes(stl[80..84].try_into().unwrap()) as usize;
            assert!(count > 0, "{label}: the whole mesh was dropped");
            for f in 0..count {
                let base = 84 + f * 50;
                for k in 0..12 {
                    let o = base + k * 4;
                    let c = f32::from_le_bytes(stl[o..o + 4].try_into().unwrap());
                    assert!(c.is_finite(), "{label}: STL facet {f} coord {k} is {c}");
                }
            }

            let obj = dir.join("h.obj");
            write_obj(&obj, &m, "hostile").unwrap();
            let text = std::fs::read_to_string(&obj).unwrap();
            assert!(
                !text.contains("NaN") && !text.contains("nan") && !text.contains("inf"),
                "{label}: OBJ carries a non-finite coordinate"
            );
            assert!(text.contains("\nf "), "{label}: OBJ kept no faces at all");

            // ASCII STL goes through the same face filter.
            let ascii = to_stl_ascii(&m, "hostile");
            assert!(
                !ascii.contains("NaN") && !ascii.contains("nan") && !ascii.contains("inf"),
                "{label}: ASCII STL carries a non-finite coordinate"
            );

            // PLY declares its face count in the header, so the header and the
            // body have to agree after filtering.
            let ply = dir.join("h.ply");
            write_ply(&ply, &m, "hostile").unwrap();
            let bytes = std::fs::read(&ply).unwrap();
            let end = b"end_header\n";
            let at = bytes
                .windows(end.len())
                .position(|w| w == end)
                .expect("a header end")
                + end.len();
            let header = String::from_utf8_lossy(&bytes[..at]).to_string();
            let nv: usize = header
                .lines()
                .find_map(|l| l.strip_prefix("element vertex "))
                .unwrap()
                .parse()
                .unwrap();
            let nf: usize = header
                .lines()
                .find_map(|l| l.strip_prefix("element face "))
                .unwrap()
                .parse()
                .unwrap();
            let stride = if m.normals.len() == m.vertices.len() { 24 } else { 12 };
            assert_eq!(
                bytes.len(),
                at + nv * stride + nf * 13,
                "{label}: PLY header face count disagrees with the body"
            );
            for i in 0..nv * (stride / 4) {
                let o = at + i * 4;
                let c = f32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
                assert!(c.is_finite(), "{label}: PLY value {i} is {c}");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn small_build() -> Mesh {
        let design = crate::RingDesign::default();
        let lib = crate::AlphaLibrary::builtin();
        let params = BuildParams { theta_steps: 96, profile_steps: 64, ..Default::default() };
        mesh::build(&design, &lib, params).mesh
    }

    #[test]
    fn the_ply_header_and_byte_length_agree() {
        let mesh = small_build();
        let dir = std::env::temp_dir().join("ringdesign-ply-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ring.ply");
        let bytes = write_ply(&path, &mesh, "Test ring").unwrap();
        let b = std::fs::read(&path).unwrap();
        assert_eq!(b.len(), bytes);
        let header_end = b.windows(11).position(|w| w == b"end_header\n").unwrap() + 11;
        let header = std::str::from_utf8(&b[..header_end]).unwrap();
        assert!(header.contains(&format!("element vertex {}", mesh.vertices.len())));
        assert!(header.contains(&format!("element face {}", mesh.faces.len())));
        assert!(header.contains("property float nx"));
        assert_eq!(
            b.len() - header_end,
            mesh.vertices.len() * 24 + mesh.faces.len() * 13,
            "body length disagrees with the counts"
        );
        let _ = std::fs::remove_dir_all(&dir);
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
