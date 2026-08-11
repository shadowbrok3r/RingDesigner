//! glTF binary (.glb) export: the shareable-render format.
//!
//! One node, one mesh, smooth normals, a PBR metal material tinted to the
//! chosen alloy. glTF's units are **metres**, so the millimetre mesh is
//! scaled by 0.001 on the way out — the one convention every viewer agrees
//! on, and the reason a ring pasted into a scene is ring-sized rather than
//! room-sized. Hand-rolled like the 3MF writer: two chunks and some
//! byte-alignment do not earn a dependency.

use std::path::Path;

use crate::mesh::Mesh;

/// Millimetres to glTF metres.
const MM_TO_M: f32 = 0.001;

/// The GLB bytes: JSON chunk + binary chunk, both 4-byte aligned.
///
/// `base_color` is linear RGB 0..1 — the alloy's tint; metalness 1.0 and a
/// polished roughness are what a cast-and-polished ring is.
pub fn to_glb(mesh: &Mesh, name: &str, base_color: [f32; 3]) -> Vec<u8> {
    let ok = |i: u32| mesh.vertices.get(i as usize).map(|v| v.is_finite()).unwrap_or(false);
    let faces: Vec<&[u32; 3]> = mesh.faces.iter().filter(|f| f.iter().all(|&i| ok(i))).collect();

    // --- Binary chunk: positions, normals, indices ---------------------------
    let n_verts = mesh.vertices.len();
    let mut bin: Vec<u8> = Vec::with_capacity(n_verts * 24 + faces.len() * 12);
    let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in &mesh.vertices {
        let p = if v.is_finite() { [v.0, v.1, v.2] } else { [0.0; 3] };
        for k in 0..3 {
            let m = p[k] * MM_TO_M;
            min[k] = min[k].min(m);
            max[k] = max[k].max(m);
            bin.extend_from_slice(&m.to_le_bytes());
        }
    }
    let normals_off = bin.len();
    for i in 0..n_verts {
        let n = mesh
            .normals
            .get(i)
            .filter(|n| n.is_finite())
            .copied()
            .unwrap_or(crate::mesh::Vec3(0.0, 0.0, 1.0));
        for c in [n.0, n.1, n.2] {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    let indices_off = bin.len();
    for f in &faces {
        for &i in f.iter() {
            bin.extend_from_slice(&i.to_le_bytes());
        }
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }

    // --- JSON chunk -----------------------------------------------------------
    let json = format!(
        r#"{{"asset":{{"version":"2.0","generator":"RingDesigner {} (mm scaled to m)"}},"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0,"name":{name:?}}}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"NORMAL":1}},"indices":2,"material":0}}]}}],"materials":[{{"name":{name:?},"pbrMetallicRoughness":{{"baseColorFactor":[{r},{g},{b},1.0],"metallicFactor":1.0,"roughnessFactor":0.22}}}}],"buffers":[{{"byteLength":{blen}}}],"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":{plen},"target":34962}},{{"buffer":0,"byteOffset":{normals_off},"byteLength":{plen},"target":34962}},{{"buffer":0,"byteOffset":{indices_off},"byteLength":{ilen},"target":34963}}],"accessors":[{{"bufferView":0,"componentType":5126,"count":{nv},"type":"VEC3","min":[{minx},{miny},{minz}],"max":[{maxx},{maxy},{maxz}]}},{{"bufferView":1,"componentType":5126,"count":{nv},"type":"VEC3"}},{{"bufferView":2,"componentType":5125,"count":{ni},"type":"SCALAR"}}]}}"#,
        env!("CARGO_PKG_VERSION"),
        r = base_color[0],
        g = base_color[1],
        b = base_color[2],
        blen = bin.len(),
        plen = n_verts * 12,
        ilen = faces.len() * 12,
        nv = n_verts,
        ni = faces.len() * 3,
        minx = min[0],
        miny = min[1],
        minz = min[2],
        maxx = max[0],
        maxy = max[1],
        maxz = max[2],
    );
    let mut json = json.into_bytes();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }

    // --- Container ------------------------------------------------------------
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}

pub fn write_glb(
    path: impl AsRef<Path>,
    mesh: &Mesh,
    name: &str,
    base_color: [f32; 3],
) -> anyhow::Result<usize> {
    let bytes = to_glb(mesh, name, base_color);
    std::fs::write(path, &bytes)?;
    Ok(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpha::AlphaLibrary;
    use crate::mesh::{build, BuildParams};
    use crate::RingDesign;

    #[test]
    fn the_glb_is_well_formed_metre_scaled_and_complete() {
        let d = RingDesign::default();
        let out = build(
            &d,
            &AlphaLibrary::default(),
            BuildParams { theta_steps: 48, profile_steps: 32, ..Default::default() },
        );
        let glb = to_glb(&out.mesh, "Ring \"7\"", [0.9, 0.8, 0.5]);

        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize, glb.len());
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        assert_eq!(&glb[16..20], b"JSON");
        let json: serde_json::Value =
            serde_json::from_slice(&glb[20..20 + json_len]).expect("valid JSON chunk");

        assert_eq!(json["asset"]["version"], "2.0");
        assert_eq!(
            json["accessors"][0]["count"].as_u64().unwrap() as usize,
            out.mesh.vertices.len()
        );
        assert_eq!(
            json["accessors"][2]["count"].as_u64().unwrap() as usize,
            out.mesh.faces.len() * 3
        );
        // Metres: a ~22 mm ring spans ~0.022, not 22.
        let span = json["accessors"][0]["max"][0].as_f64().unwrap()
            - json["accessors"][0]["min"][0].as_f64().unwrap();
        assert!((0.005..0.1).contains(&span), "span {span} is not ring-sized in metres");

        // The BIN chunk is where the JSON says it is, aligned.
        let bin_at = 20 + json_len;
        assert_eq!(&glb[bin_at + 4..bin_at + 8], b"BIN\0");
        assert_eq!(json_len % 4, 0);
        let bin_len = u32::from_le_bytes(glb[bin_at..bin_at + 4].try_into().unwrap()) as usize;
        assert_eq!(
            json["buffers"][0]["byteLength"].as_u64().unwrap() as usize,
            bin_len
        );
        assert_eq!(bin_at + 8 + bin_len, glb.len());
    }
}
