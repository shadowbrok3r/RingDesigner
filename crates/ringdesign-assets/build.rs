//! Packs every bundled asset into one deflated blob beside a generated index.
//!
//! The alternative is `include_bytes!` per file, which is what this crate
//! replaced: the graph templates carry their artwork as base64 inside JSON and
//! came to 99 MB of binary for 17 MB of information. Deflate is chosen over
//! zstd because it is already in the lock through `image`, in a pure-Rust
//! backend — a C codec would have to be cross-compiled for the Windows build
//! as well.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// A family of assets: the index it appears under, where its files live, and
/// the suffix that both selects them and is stripped to make the name.
struct Family {
    ident: &'static str,
    dir: &'static str,
    suffix: &'static str,
}

/// Every directory swept into the payload. `alphas` also takes `.jpg` and
/// `.bmp`, handled below.
const FAMILIES: &[Family] = &[
    Family { ident: "ALPHAS", dir: "bundled/alphas", suffix: ".png" },
    Family { ident: "PROFILES", dir: "bundled/profiles", suffix: ".profile.json" },
    Family { ident: "OUTLINES", dir: "bundled/outlines", suffix: ".outline.json" },
    Family { ident: "GEMS", dir: "bundled/gems", suffix: ".obj" },
    Family { ident: "GRAPHS", dir: "graphs/templates", suffix: ".graph.json" },
    Family { ident: "CLUSTERS", dir: "graphs/clusters", suffix: ".cluster.json" },
    Family { ident: "GRAPH_PRESETS", dir: "graphs/presets", suffix: ".preset.json" },
    Family { ident: "BASES", dir: "bases/signets", suffix: ".ringbase.json" },
];

/// Assets that do not sit in a family's directory, as `(ident, name, path)`.
const LOOSE: &[(&str, &str, &str)] = &[
    ("SIMPLE_GRAPH", "simple", "graphs/simple.graph.json"),
    ("DESIGNS", "aster-botanical", "showcase/aster/design.ring.json"),
    ("DESIGNS", "aster-workshop", "showcase/workshop-collection/aster/design.ring.json"),
    ("DESIGNS", "tide-workshop", "showcase/workshop-collection/tide/design.ring.json"),
    ("DESIGNS", "lantern-workshop", "showcase/workshop-collection/lantern/design.ring.json"),
    ("DESIGNS", "aureole-workshop", "showcase/workshop-collection/aureole/design.ring.json"),
];

/// Compression is kept only when it pays for the decode; a PNG is already
/// deflated and comes back within a percent of its own size.
const WORTH_COMPRESSING: f64 = 0.95;

/// The one drawing every icon on every platform is rendered from.
const ICON_SVG: &str = "bundled/icon/ringdesigner.svg";

/// Sizes inside the Windows `.ico`: the shell picks per context, from the
/// 16 px tray up to the 256 px preview.
const ICO_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];

/// The window icon's edge. Window managers scale down from one image.
const APP_ICON_EDGE: u32 = 256;

/// Sizes written out as loose PNGs for a Linux hicolor icon theme.
const PNG_SIZES: &[u32] = &[32, 48, 64, 128, 256];

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .and_then(Path::parent)
        .expect("the crate sits two levels under the workspace root")
        .to_path_buf();
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let mut payloads: std::collections::HashMap<&str, Vec<u8>> = std::collections::HashMap::new();
    let mut index: Vec<(&str, String)> = Vec::new();
    let mut origins: Vec<(&str, String, PathBuf)> = Vec::new();

    for family in FAMILIES {
        let dir = root.join(family.dir);
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("bundled asset directory {}: {e}", dir.display()))
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && selects(p, family.suffix))
            .collect();
        files.sort();
        for path in files {
            let file = path.file_name().unwrap().to_str().unwrap().to_string();
            let name = file
                .strip_suffix(family.suffix)
                .map(str::to_string)
                .unwrap_or_else(|| path.file_stem().unwrap().to_string_lossy().into_owned());
            let blob = payloads.entry(family.ident).or_default();
            index.push((family.ident, entry(blob, &name, &file, &path)));
            origins.push((family.ident, name, path));
        }
    }

    for (ident, name, rel) in LOOSE {
        let path = root.join(rel);
        println!("cargo:rerun-if-changed={}", path.display());
        let file = path.file_name().unwrap().to_str().unwrap().to_string();
        let blob = payloads.entry(ident).or_default();
        index.push((ident, entry(blob, name, &file, &path)));
        origins.push((ident, name.to_string(), path));
    }

    index.push(("ICON", icon(&root, &out, payloads.entry("ICON").or_default())));

    let mut src = String::new();
    for ident in ["ALPHAS", "PROFILES", "OUTLINES", "GEMS", "GRAPHS", "CLUSTERS", "GRAPH_PRESETS", "BASES", "SIMPLE_GRAPH", "DESIGNS", "ICON"] {
        let blob = payloads.remove(ident).unwrap_or_default();
        let file = format!("{}.bin", ident.to_ascii_lowercase());
        std::fs::write(out.join(&file), &blob).unwrap();
        writeln!(src, "static BLOB_{ident}: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{file}\"));").unwrap();
        writeln!(src, "pub static {ident}: &[Asset] = &[").unwrap();
        for (_, line) in index.iter().filter(|(f, _)| *f == ident) {
            writeln!(src, "    Asset {{ blob: BLOB_{ident}, {line}").unwrap();
        }
        writeln!(src, "];").unwrap();
    }
    std::fs::write(out.join("index.rs"), src).unwrap();

    let mut sources = String::from("pub static SOURCES: &[(&str, &str, &str)] = &[\n");
    for (ident, name, path) in &origins {
        writeln!(sources, "    ({ident:?}, {name:?}, {:?}),", path.display().to_string()).unwrap();
    }
    sources.push_str("];\n");
    std::fs::write(out.join("sources.rs"), sources).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}

/// Rasterize the app icon and hand both forms on: the window icon goes into
/// the payload as straight RGBA, so no runtime decoder is needed on any
/// platform, and a multi-size `.ico` is written for `ringdesign-gui`'s build
/// script to compile into the Windows executable's resources. One SVG is the
/// source of both, so the taskbar and the title bar cannot drift apart.
fn icon(root: &Path, out: &Path, payload: &mut Vec<u8>) -> String {
    let path = root.join(ICON_SVG);
    println!("cargo:rerun-if-changed={}", path.display());
    let svg = std::fs::read(&path).unwrap_or_else(|e| panic!("app icon {}: {e}", path.display()));
    let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default())
        .unwrap_or_else(|e| panic!("app icon {}: {e}", path.display()));

    let render = |edge: u32| -> Vec<u8> {
        let mut map = resvg::tiny_skia::Pixmap::new(edge, edge).expect("a pixmap for the icon");
        let scale = edge as f32 / tree.size().width();
        resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut map.as_mut());
        // tiny-skia paints premultiplied; an icon is read as straight RGBA.
        map.take()
            .chunks_exact(4)
            .flat_map(|p| {
                let a = p[3] as u32;
                let un = |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a / 2) / a).min(255) as u8 };
                [un(p[0]), un(p[1]), un(p[2]), p[3]]
            })
            .collect()
    };

    let ico: Vec<image::codecs::ico::IcoFrame> = ICO_SIZES
        .iter()
        .map(|edge| {
            image::codecs::ico::IcoFrame::as_png(&render(*edge), *edge, *edge, image::ExtendedColorType::Rgba8)
                .unwrap_or_else(|e| panic!("icon frame at {edge} px: {e}"))
        })
        .collect();
    let ico_path = out.join("ringdesigner.ico");
    let file = std::io::BufWriter::new(std::fs::File::create(&ico_path).unwrap());
    image::codecs::ico::IcoEncoder::new(file)
        .encode_images(&ico)
        .unwrap_or_else(|e| panic!("writing {}: {e}", ico_path.display()));
    // Read by ringdesign-gui's build script as DEP_RINGDESIGN_ASSETS_ICO.
    println!("cargo:ico={}", ico_path.display());

    // Loose PNGs for a Linux hicolor icon theme, found by packaging/package.sh.
    for edge in PNG_SIZES {
        let buf = image::RgbaImage::from_raw(*edge, *edge, render(*edge)).expect("icon pixels");
        buf.save(out.join(format!("icon-{edge}.png"))).unwrap();
    }

    let rgba = render(APP_ICON_EDGE);
    let offset = payload.len();
    payload.extend_from_slice(&rgba);
    let len = payload.len() - offset;
    format!("name: \"app\", file: \"ringdesigner.rgba\", offset: {offset}, len: {len}, raw_len: {len} }},")
}

/// Whether a file belongs to a family. Alphas take the three image formats
/// [`ringdesign_core::Alpha::load`] reads, not only the `.png` that names them.
fn selects(path: &Path, suffix: &str) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if suffix == ".png" {
        let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        return matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp");
    }
    name.ends_with(suffix)
}

/// Append one file to the payload and return its index line.
fn entry(payload: &mut Vec<u8>, name: &str, file: &str, path: &Path) -> String {
    let raw = std::fs::read(path).unwrap_or_else(|e| panic!("bundled asset {}: {e}", path.display()));
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(&raw).unwrap();
    let packed = encoder.finish().unwrap();

    let offset = payload.len();
    let stored = packed.len() as f64 > raw.len() as f64 * WORTH_COMPRESSING;
    payload.extend_from_slice(if stored { &raw } else { &packed });
    let len = payload.len() - offset;
    format!(
        "name: {name:?}, file: {file:?}, offset: {offset}, len: {len}, raw_len: {} }},",
        if stored { len } else { raw.len() }
    )
}
