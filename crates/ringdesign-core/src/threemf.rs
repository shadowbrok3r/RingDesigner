//! 3MF export: a zip package with the mesh as XML, units stated.
//!
//! 3MF ends STL's units ambiguity — the model says `unit="millimeter"` and
//! carries the design's name and size as metadata, so a slicer or CAD package
//! opens it right without being told. The package is written store-only (no
//! compression) with zeroed timestamps: byte-identical output for identical
//! input, and no dependency bought for a container three files big.

use std::path::Path;

use crate::mesh::Mesh;

/// One entry queued for the package.
struct Entry {
    name: &'static str,
    data: Vec<u8>,
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
fn zip_store(entries: &[Entry]) -> Vec<u8> {
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
    let s = format!("{:.3}", v);
    let t = s.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-" { "0".into() } else { t.to_string() }
}

/// The 3MF package bytes for a mesh. Faces touching a missing or non-finite
/// vertex are dropped whole, the same rule STL applies.
pub fn to_3mf(mesh: &Mesh, name: &str, size_label: &str) -> Vec<u8> {
    let ok =
        |i: u32| mesh.vertices.get(i as usize).map(|v| v.is_finite()).unwrap_or(false);
    let mut model = String::with_capacity(mesh.vertices.len() * 40);
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
    model.push_str(" <resources>\n  <object id=\"1\" type=\"model\" name=\"");
    model.push_str(&xml_escape(name));
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
    model.push_str("    </triangles>\n   </mesh>\n  </object>\n </resources>\n <build>\n  <item objectid=\"1\"/>\n </build>\n</model>\n");

    let entries = [
        Entry {
            name: "[Content_Types].xml",
            data: b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\n <Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/>\n</Types>\n".to_vec(),
        },
        Entry {
            name: "_rels/.rels",
            data: b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n <Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/>\n</Relationships>\n".to_vec(),
        },
        Entry { name: "3D/3dmodel.model", data: model.into_bytes() },
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

#[cfg(test)]
mod tests {
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
    fn known_crc_vector_holds() {
        // The classic test vector: CRC-32("123456789") = 0xCBF43926.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }
}
