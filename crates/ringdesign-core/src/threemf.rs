//! 3MF export: a zip package with the mesh as XML, units stated.
//!
//! 3MF ends STL's units ambiguity — the model says `unit="millimeter"` and
//! carries the design's name and size as metadata, so a slicer or CAD package
//! opens it right without being told. The package is written store-only (no
//! compression) with zeroed timestamps: byte-identical output for identical
//! input, and no dependency bought for a container three files big.

use std::borrow::Cow;
use std::path::Path;

use crate::mesh::{BuildResult, Mesh};

/// One entry queued for the package.
/// One stored file in a package.
pub struct Entry {
    pub name: String,
    pub data: Vec<u8>,
}

/// CRC-32 (IEEE), the zip checksum.
fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *slot = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

/// Store-only zip: local headers, central directory, end record.
/// A store-only zip of `entries`, zeroed timestamps, deterministic bytes.
pub fn zip_store(entries: &[Entry]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    let mut offsets = Vec::with_capacity(entries.len());

    let u16le = |v: usize| (v as u16).to_le_bytes();
    let u32le = |v: usize| (v as u32).to_le_bytes();

    for e in entries {
        offsets.push(out.len());
        let crc = crc32(&e.data);
        out.extend_from_slice(&0x0403_4B50u32.to_le_bytes());
        out.extend_from_slice(&u16le(20)); // version needed
        out.extend_from_slice(&u16le(0)); // flags
        out.extend_from_slice(&u16le(0)); // method: store
        out.extend_from_slice(&u16le(0)); // time
        out.extend_from_slice(&u16le(0x21)); // date: 1980-01-01
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&u32le(e.data.len()));
        out.extend_from_slice(&u32le(e.data.len()));
        out.extend_from_slice(&u16le(e.name.len()));
        out.extend_from_slice(&u16le(0)); // extra
        out.extend_from_slice(e.name.as_bytes());
        out.extend_from_slice(&e.data);
    }

    for (e, &off) in entries.iter().zip(&offsets) {
        let crc = crc32(&e.data);
        central.extend_from_slice(&0x0201_4B50u32.to_le_bytes());
        central.extend_from_slice(&u16le(20)); // made by
        central.extend_from_slice(&u16le(20)); // needed
        central.extend_from_slice(&u16le(0));
        central.extend_from_slice(&u16le(0));
        central.extend_from_slice(&u16le(0));
        central.extend_from_slice(&u16le(0x21));
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&u32le(e.data.len()));
        central.extend_from_slice(&u32le(e.data.len()));
        central.extend_from_slice(&u16le(e.name.len()));
        central.extend_from_slice(&u16le(0)); // extra
        central.extend_from_slice(&u16le(0)); // comment
        central.extend_from_slice(&u16le(0)); // disk
        central.extend_from_slice(&u16le(0)); // internal attrs
        central.extend_from_slice(&u32le(0)); // external attrs
        central.extend_from_slice(&u32le(off));
        central.extend_from_slice(e.name.as_bytes());
    }

    let central_off = out.len();
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
    out.extend_from_slice(&u16le(0));
    out.extend_from_slice(&u16le(0));
    out.extend_from_slice(&u16le(entries.len()));
    out.extend_from_slice(&u16le(entries.len()));
    out.extend_from_slice(&u32le(central.len()));
    out.extend_from_slice(&u32le(central_off));
    out.extend_from_slice(&u16le(0)); // comment
    out
}

fn xml_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            c => c.to_string(),
        })
        .collect()
}

/// Trailing-zero-trimmed coordinate, micron precision.
fn coord(v: f32) -> String {
    // Shortest round-trippable decimal preserves the analyzed mesh exactly.
    if v == 0.0 { "0".into() } else { v.to_string() }
}

/// One named object of a package: the band, or the part `feature` names.
pub struct Object<'a> {
    pub name: String,
    pub feature: Option<crate::sketch::Id>,
    pub mesh: Cow<'a, Mesh>,
}

impl Object<'_> {
    /// This object scaled uniformly, as the patternmaker's shrink scales it.
    pub fn scaled(&self, factor: f64) -> Object<'static> {
        Object { name: self.name.clone(), feature: self.feature, mesh: Cow::Owned(self.mesh.scaled(factor)) }
    }
}

/// A build's metal as package objects: the band with its joined and cut parts named `name`, then each Separate part named for its feature.
pub fn objects<'a>(built: &'a BuildResult, name: &str) -> Vec<Object<'a>> {
    let mesh = &built.mesh;
    let whole = || vec![Object { name: name.to_string(), feature: None, mesh: Cow::Borrowed(mesh) }];
    let separate: Vec<&crate::cad::EvaluatedComponent> = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| &e.components)
        .filter(|c| !c.settings.reference && c.attach == crate::cad::Attach::Separate)
        .collect();
    if built.band.is_none() || separate.is_empty() || mesh.origin.len() != mesh.vertices.len() {
        return whole();
    }
    // The separate part each vertex belongs to, by its position in `separate`.
    let owner: Vec<Option<usize>> = mesh
        .origin
        .iter()
        .map(|o| built.parts.feature_of(*o).and_then(|id| separate.iter().position(|c| c.id == id)))
        .collect();
    let mut faces: Vec<Vec<[u32; 3]>> = vec![Vec::new(); separate.len() + 1];
    for f in &mesh.faces {
        let first = owner.get(f[0] as usize).copied().flatten();
        let own = first.filter(|_| f.iter().all(|v| owner.get(*v as usize).copied().flatten() == first));
        faces[own.map_or(0, |k| k + 1)].push(*f);
    }
    let named = std::iter::once((name.to_string(), None)).chain(separate.iter().map(|c| (c.name.clone(), Some(c.id))));
    named.zip(faces).filter(|(_, f)| !f.is_empty()).map(|((name, feature), f)| Object { name, feature, mesh: Cow::Owned(sub_mesh(mesh, &f)) }).collect()
}

/// The vertices `faces` use, renumbered in first use, with their normals when the mesh has one per vertex.
fn sub_mesh(mesh: &Mesh, faces: &[[u32; 3]]) -> Mesh {
    let mut index = vec![u32::MAX; mesh.vertices.len()];
    let mut out = Mesh::default();
    let normals = mesh.normals.len() == mesh.vertices.len();
    for f in faces {
        let g = f.map(|v| {
            let slot = &mut index[v as usize];
            if *slot == u32::MAX {
                *slot = out.vertices.len() as u32;
                out.vertices.push(mesh.vertices[v as usize]);
                if normals {
                    out.normals.push(mesh.normals[v as usize]);
                }
            }
            *slot
        });
        out.faces.push(g);
    }
    out
}

/// The 3MF package bytes for a mesh. Faces touching a missing or non-finite
/// vertex are dropped whole, the same rule STL applies.
pub fn to_3mf(mesh: &Mesh, name: &str, size_label: &str) -> Vec<u8> {
    to_3mf_objects(&[Object { name: name.to_string(), feature: None, mesh: Cow::Borrowed(mesh) }], name, size_label)
}

/// The 3MF package bytes for named objects, one build item each, in order.
pub fn to_3mf_objects(objects: &[Object], name: &str, size_label: &str) -> Vec<u8> {
    let mut model = String::with_capacity(objects.iter().map(|o| o.mesh.vertices.len()).sum::<usize>() * 40);
    model.push_str(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\" \
         xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n",
    );
    model.push_str(&format!(
        " <metadata name=\"Title\">{}</metadata>\n <metadata name=\"Application\">RingDesigner {}</metadata>\n <metadata name=\"Description\">Ring size {}; sand-cast pattern; units mm</metadata>\n",
        xml_escape(name),
        env!("CARGO_PKG_VERSION"),
        xml_escape(size_label),
    ));
    model.push_str(" <resources>\n");
    for (k, object) in objects.iter().enumerate() {
        let mesh = &object.mesh;
        let ok = |i: u32| mesh.vertices.get(i as usize).map(|v| v.is_finite()).unwrap_or(false);
        model.push_str(&format!("  <object id=\"{}\" type=\"model\" name=\"", k + 1));
        model.push_str(&xml_escape(&object.name));
        model.push_str("\">\n   <mesh>\n    <vertices>\n");
        for v in &mesh.vertices {
            let v = if v.is_finite() { *v } else { crate::mesh::Vec3(0.0, 0.0, 0.0) };
            model.push_str(&format!(
                "     <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
                coord(v.0),
                coord(v.1),
                coord(v.2)
            ));
        }
        model.push_str("    </vertices>\n    <triangles>\n");
        for f in &mesh.faces {
            if f.iter().all(|&i| ok(i)) {
                model.push_str(&format!(
                    "     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>\n",
                    f[0], f[1], f[2]
                ));
            }
        }
        model.push_str("    </triangles>\n   </mesh>\n  </object>\n");
    }
    model.push_str(" </resources>\n <build>\n");
    for k in 0..objects.len() {
        model.push_str(&format!("  <item objectid=\"{}\"/>\n", k + 1));
    }
    model.push_str(" </build>\n</model>\n");

    let entries = [
        Entry {
            name: "[Content_Types].xml".into(),
            data: b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\n <Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/>\n</Types>\n".to_vec(),
        },
        Entry {
            name: "_rels/.rels".into(),
            data: b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n <Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/>\n</Relationships>\n".to_vec(),
        },
        Entry { name: "3D/3dmodel.model".into(), data: model.into_bytes() },
    ];
    zip_store(&entries)
}

/// Write the mesh as 3MF; returns the bytes written.
pub fn write_3mf(
    path: impl AsRef<Path>,
    mesh: &Mesh,
    name: &str,
    size_label: &str,
) -> anyhow::Result<usize> {
    let bytes = to_3mf(mesh, name, size_label);
    std::fs::write(path, &bytes)?;
    Ok(bytes.len())
}

/// Write named objects as one 3MF package; returns the bytes written.
pub fn write_3mf_objects(path: impl AsRef<Path>, objects: &[Object], name: &str, size_label: &str) -> anyhow::Result<usize> {
    let bytes = to_3mf_objects(objects, name, size_label);
    std::fs::write(path, &bytes)?;
    Ok(bytes.len())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::alpha::AlphaLibrary;
    use crate::mesh::{build, BuildParams};
    use crate::RingDesign;

    fn ring_bytes() -> (Mesh, Vec<u8>) {
        let d = RingDesign::default();
        let out = build(
            &d,
            &AlphaLibrary::default(),
            BuildParams { theta_steps: 48, profile_steps: 32, ..Default::default() },
        );
        let bytes = to_3mf(&out.mesh, "Test & Ring", "7");
        (out.mesh, bytes)
    }

    /// Walk the local headers of a store-only zip, returning (name, data).
    fn read_store_zip(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let mut at = 0usize;
        let u16at = |b: &[u8], o: usize| u16::from_le_bytes([b[o], b[o + 1]]) as usize;
        let u32at = |b: &[u8], o: usize| {
            u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as usize
        };
        while at + 30 <= bytes.len() && u32at(bytes, at) == 0x0403_4B50 {
            let crc = u32at(bytes, at + 14) as u32;
            let size = u32at(bytes, at + 18);
            let name_len = u16at(bytes, at + 26);
            let extra_len = u16at(bytes, at + 28);
            let name =
                String::from_utf8(bytes[at + 30..at + 30 + name_len].to_vec()).unwrap();
            let start = at + 30 + name_len + extra_len;
            let data = bytes[start..start + size].to_vec();
            assert_eq!(crc32(&data), crc, "{name}: stored CRC wrong");
            out.push((name, data));
            at = start + size;
        }
        out
    }

    #[test]
    fn the_package_holds_the_model_with_units_and_the_whole_mesh() {
        let (mesh, bytes) = ring_bytes();
        assert_eq!(&bytes[0..4], &0x0403_4B50u32.to_le_bytes(), "not a zip");

        let entries = read_store_zip(&bytes);
        let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["[Content_Types].xml", "_rels/.rels", "3D/3dmodel.model"]);

        let model = String::from_utf8(entries[2].1.clone()).unwrap();
        assert!(model.contains("unit=\"millimeter\""));
        assert!(model.contains("Test &amp; Ring"));
        assert!(model.contains("Ring size 7"));
        assert_eq!(model.matches("<vertex ").count(), mesh.vertices.len());
        assert_eq!(model.matches("<triangle ").count(), mesh.faces.len());

        // End record agrees with the entry count.
        let eocd = bytes.len() - 22;
        assert_eq!(&bytes[eocd..eocd + 4], &0x0605_4B50u32.to_le_bytes());
        assert_eq!(u16::from_le_bytes([bytes[eocd + 10], bytes[eocd + 11]]), 3);
    }

    #[test]
    fn identical_input_writes_identical_bytes() {
        let (_, a) = ring_bytes();
        let (_, b) = ring_bytes();
        assert_eq!(a, b);
    }

    #[test]
    fn a_store_zip_of_named_entries_reads_back() {
        let entries = vec![
            Entry { name: "a/one.txt".into(), data: b"one".to_vec() },
            Entry { name: "two.bin".into(), data: vec![0, 1, 2, 255] },
        ];
        let files = read_store_zip(&zip_store(&entries));
        assert_eq!(files.len(), 2);
        assert_eq!(files[0], ("a/one.txt".to_string(), b"one".to_vec()));
        assert_eq!(files[1], ("two.bin".to_string(), vec![0, 1, 2, 255]));
    }

    /// Every object of a package's model: its name and its mesh.
    fn read_objects(model: &str) -> Vec<(String, Mesh)> {
        let attr = |tag: &str, key: &str| -> String {
            let at = tag.find(&format!("{key}=\"")).unwrap() + key.len() + 2;
            tag[at..at + tag[at..].find('"').unwrap()].to_string()
        };
        let tags = |body: &str, open: &str| -> Vec<String> { body.split(open).skip(1).map(|t| t[..t.find("/>").unwrap()].to_string()).collect() };
        model
            .split("<object ")
            .skip(1)
            .map(|o| {
                let body = &o[..o.find("</object>").unwrap()];
                let head = &body[..body.find('>').unwrap()];
                let mut mesh = Mesh::default();
                for v in tags(body, "<vertex ") {
                    let c = |k: &str| attr(&v, k).parse::<f32>().unwrap();
                    mesh.vertices.push(crate::mesh::Vec3(c("x"), c("y"), c("z")));
                }
                for t in tags(body, "<triangle ") {
                    mesh.faces.push(["v1", "v2", "v3"].map(|k| attr(&t, k).parse::<u32>().unwrap()));
                }
                (attr(head, "name"), mesh)
            })
            .collect()
    }

    #[test]
    fn a_one_object_package_writes_the_model_it_always_has() {
        let mesh = Mesh {
            vertices: vec![crate::mesh::Vec3(0.0, 0.0, 0.0), crate::mesh::Vec3(1.0, 0.0, 0.0), crate::mesh::Vec3(0.0, 1.5, 0.0), crate::mesh::Vec3(0.0, 0.0, 2.0)],
            faces: vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
            ..Default::default()
        };
        let entries = read_store_zip(&to_3mf(&mesh, "Tetra", "7"));
        let model = String::from_utf8(entries[2].1.clone()).unwrap();
        let expected = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n <metadata name=\"Title\">Tetra</metadata>\n <metadata name=\"Application\">RingDesigner {}</metadata>\n <metadata name=\"Description\">Ring size 7; sand-cast pattern; units mm</metadata>\n <resources>\n  <object id=\"1\" type=\"model\" name=\"Tetra\">\n   <mesh>\n    <vertices>\n     <vertex x=\"0\" y=\"0\" z=\"0\"/>\n     <vertex x=\"1\" y=\"0\" z=\"0\"/>\n     <vertex x=\"0\" y=\"1.5\" z=\"0\"/>\n     <vertex x=\"0\" y=\"0\" z=\"2\"/>\n    </vertices>\n    <triangles>\n     <triangle v1=\"0\" v2=\"2\" v3=\"1\"/>\n     <triangle v1=\"0\" v2=\"1\" v3=\"3\"/>\n     <triangle v1=\"1\" v2=\"2\" v3=\"3\"/>\n     <triangle v1=\"2\" v2=\"0\" v3=\"3\"/>\n    </triangles>\n   </mesh>\n  </object>\n </resources>\n <build>\n  <item objectid=\"1\"/>\n </build>\n</model>\n",
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(model, expected);
    }

    /// The Court band carrying a joined post, a cut pilot, a spacer kept beside it and a reference stone.
    pub(crate) fn parted() -> RingDesign {
        use crate::cad::{Attach, Component, Document, Feature, Operation, Placement, builders};
        let mut d = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        let mut add = |id, name: &str, operation, attach, placement| {
            let component = Component { attach, placement, ..Component::default() };
            doc.append(Feature { id, name: name.into(), enabled: true, operation, component }).unwrap();
        };
        add(1, "Procedural shank", Operation::Band, Attach::Separate, Placement::Free);
        add(2, "Post", Operation::Cylinder { radius_mm: 0.8, height_mm: 2.0 }, Attach::Join, Placement::ring(90.0, 0.6));
        add(3, "Pilot", Operation::Cylinder { radius_mm: 0.4, height_mm: 3.0 }, Attach::Cut, Placement::ring(0.0, 0.0));
        add(4, "Spacer", Operation::Box { size: [1.5, 1.5, 1.5] }, Attach::Separate, Placement::ring(270.0, 2.0));
        let gem = crate::gem::Gem::calibrated(crate::gem::GemCut::Round, 3.0);
        doc.append(builders::stone_feature(5, gem, Placement::ring(180.0, builders::stand_off_mm("claw4", gem)))).unwrap();
        d.cad = Some(doc);
        d
    }

    #[test]
    fn the_band_and_each_separate_part_are_named_objects_and_the_stone_is_left_out() {
        let built = crate::mesh::try_build(&parted(), &AlphaLibrary::builtin(), BuildParams { theta_steps: 192, profile_steps: 96, ..Default::default() }).unwrap();
        assert_eq!((built.parts.joined, built.parts.cut, built.parts.separate, built.parts.references), (1, 1, 1, 1), "{:?}", built.parts.notes);
        let bytes = to_3mf_objects(&objects(&built, "Court band"), "Court band", "7");
        assert_eq!(bytes, to_3mf_objects(&objects(&built, "Court band"), "Court band", "7"), "the same build writes the same bytes");
        let entries = read_store_zip(&bytes);
        let model = String::from_utf8(entries[2].1.clone()).unwrap();
        let read = read_objects(&model);
        assert_eq!(read.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["Court band", "Spacer"]);
        assert_eq!(model.matches("<item objectid=").count(), 2);
        for (name, m) in &read {
            let v = m.validate();
            assert!(v.watertight, "{name}: {v:?}");
        }
        // Face for face the whole build, the spacer its own 1.5 mm cube and the band everything else.
        assert_eq!(read.iter().map(|(_, m)| m.faces.len()).sum::<usize>(), built.mesh.faces.len());
        assert!((read[1].1.volume_mm3() - 3.375).abs() < 1e-4, "{}", read[1].1.volume_mm3());
        let total: f64 = read.iter().map(|(_, m)| m.volume_mm3()).sum();
        assert!((total - built.report.volume_mm3).abs() < 1e-3 * built.report.volume_mm3, "{total} against {}", built.report.volume_mm3);
        // Scaled for shrink each object keeps its name and its closure.
        let shrunk: Vec<Object> = objects(&built, "Court band").iter().map(|o| o.scaled(1.02)).collect();
        assert!((shrunk[1].mesh.volume_mm3() - 3.375 * 1.02f64.powi(3)).abs() < 1e-3);
        // A ring without a separate part stays one object, byte for byte the single-mesh package.
        let mut joined = parted();
        joined.cad.as_mut().unwrap().features[3].component.attach = crate::cad::Attach::Join;
        let built = crate::mesh::try_build(&joined, &AlphaLibrary::builtin(), BuildParams { theta_steps: 192, profile_steps: 96, ..Default::default() }).unwrap();
        assert_eq!(to_3mf_objects(&objects(&built, "Court band"), "Court band", "7"), to_3mf(&built.mesh, "Court band", "7"));
    }

    #[test]
    fn known_crc_vector_holds() {
        // The classic test vector: CRC-32("123456789") = 0xCBF43926.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }
}
