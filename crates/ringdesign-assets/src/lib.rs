//! Every asset the program ships, inside the binary.
//!
//! One jeweller's install is one file. Nothing here is looked up on disk, so a
//! build carries the same alphas, profiles, signet plans, gem meshes, graph
//! templates and factory stock wherever it is copied — a machine with no
//! source tree included.
//!
//! The user's own library is still read, and still wins: every consumer
//! overlays the data root's directory over these by name
//! ([`ringdesign_core::library`]), so an imported alpha or a saved profile
//! shadows a bundled one of the same name.
//!
//! Assets are deflated and decoded on the first read of each. That matters
//! most for the graph templates, which carry their artwork as base64 PNG
//! inside JSON: 99 MB of asset for 27 MB of binary.
//!
//! Each family is its own blob rather than all of them being one, so a build
//! that never touches a family does not carry it — a wasm target that lists
//! no templates should not pay 27 MB for them.

use std::borrow::Cow;

/// One bundled file: a range of its family's blob.
#[derive(Clone, Copy, Debug)]
pub struct Asset {
    blob: &'static [u8],
    /// The library name — the file's own, with the family suffix removed.
    pub name: &'static str,
    /// The file name as it was bundled, so a consumer can write it back out.
    pub file: &'static str,
    offset: usize,
    len: usize,
    raw_len: usize,
}

impl Asset {
    /// The file's contents, decompressed on demand. Stored entries borrow the
    /// payload; the rest allocate once per call, so callers that read the same
    /// asset repeatedly should hold onto the result.
    pub fn bytes(&self) -> Cow<'static, [u8]> {
        let packed = &self.blob[self.offset..self.offset + self.len];
        if self.raw_len == self.len {
            return Cow::Borrowed(packed);
        }
        let mut out = Vec::with_capacity(self.raw_len);
        std::io::copy(&mut flate2::read::DeflateDecoder::new(packed), &mut out)
            .unwrap_or_else(|e| panic!("bundled asset {}: {e}", self.name));
        Cow::Owned(out)
    }

    /// The file's contents as text. Every bundled text asset is UTF-8.
    pub fn text(&self) -> Cow<'static, str> {
        match self.bytes() {
            Cow::Borrowed(b) => Cow::Borrowed(std::str::from_utf8(b).expect("bundled asset is UTF-8")),
            Cow::Owned(b) => Cow::Owned(String::from_utf8(b).expect("bundled asset is UTF-8")),
        }
    }

    /// The asset's size once decoded.
    pub fn size(&self) -> usize {
        self.raw_len
    }
}

/// Look one asset up by name within a family.
pub fn find(family: &'static [Asset], name: &str) -> Option<&'static Asset> {
    family.iter().find(|a| a.name == name)
}

include!(concat!(env!("OUT_DIR"), "/index.rs"));

/// The starter graph the editor opens on.
pub fn simple_graph() -> Cow<'static, str> {
    SIMPLE_GRAPH[0].text()
}

/// The window icon's edge in pixels.
pub const APP_ICON_EDGE: u32 = 256;

/// The window icon as straight RGBA, `APP_ICON_EDGE` square — the shape
/// `eframe::IconData` and every window manager want, with no image decoder in
/// the path. Rendered from `bundled/icon/ringdesigner.svg` at build time,
/// the same drawing the Windows `.ico` is cut from.
pub fn app_icon_rgba() -> Vec<u8> {
    ICON[0].bytes().into_owned()
}

/// Bytes across every family, decoded — what the bundle would weigh on disk.
pub fn bundled_size() -> usize {
    [ALPHAS, PROFILES, OUTLINES, GEMS, GRAPHS, CLUSTERS, GRAPH_PRESETS, BASES, SIMPLE_GRAPH, DESIGNS, ICON]
        .iter()
        .flat_map(|f| f.iter())
        .map(Asset::size)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    include!(concat!(env!("OUT_DIR"), "/sources.rs"));

    /// Every asset comes back byte for byte as the file it was packed from.
    /// The whole crate is a compression layer between a file and the program
    /// that reads it; nothing else here would catch a family's entries
    /// sliding against its blob, or a name bound to the wrong file.
    #[test]
    fn every_asset_round_trips_to_its_source_file() {
        for (family, name, path) in SOURCES {
            let assets = match *family {
                "ALPHAS" => ALPHAS,
                "PROFILES" => PROFILES,
                "OUTLINES" => OUTLINES,
                "GEMS" => GEMS,
                "GRAPHS" => GRAPHS,
                "CLUSTERS" => CLUSTERS,
                "GRAPH_PRESETS" => GRAPH_PRESETS,
                "BASES" => BASES,
                "SIMPLE_GRAPH" => SIMPLE_GRAPH,
                "DESIGNS" => DESIGNS,
                other => panic!("{other} is not a family"),
            };
            let asset = find(assets, name).unwrap_or_else(|| panic!("{family}/{name} is not bundled"));
            let source = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert!(asset.bytes() == source, "{family}/{name} differs from {path}");
        }
    }

    /// Every family is populated, every asset decodes to the length the build
    /// recorded, and names are unique within a family — a duplicate would make
    /// [`find`] answer by bundle order rather than by intent.
    #[test]
    fn every_bundled_asset_decodes_to_its_recorded_length() {
        let families: [(&str, &[Asset]); 11] = [
            ("alphas", ALPHAS),
            ("profiles", PROFILES),
            ("outlines", OUTLINES),
            ("gems", GEMS),
            ("graphs", GRAPHS),
            ("clusters", CLUSTERS),
            ("graph presets", GRAPH_PRESETS),
            ("bases", BASES),
            ("simple graph", SIMPLE_GRAPH),
            ("designs", DESIGNS),
            ("icon", ICON),
        ];
        for (label, family) in families {
            assert!(!family.is_empty(), "{label} is empty — the build swept nothing");
            let mut names: Vec<&str> = family.iter().map(|a| a.name).collect();
            names.sort_unstable();
            let count = names.len();
            names.dedup();
            assert_eq!(names.len(), count, "{label} has a duplicate name");
            for asset in family {
                assert_eq!(asset.bytes().len(), asset.size(), "{label}/{}", asset.name);
            }
        }
    }

    /// The window icon is the size the runtime promises, so a caller can hand
    /// it straight to a window manager without measuring it.
    #[test]
    fn the_window_icon_is_square_rgba_at_the_declared_edge() {
        assert_eq!(app_icon_rgba().len(), (APP_ICON_EDGE * APP_ICON_EDGE * 4) as usize);
    }

    /// The families whose contents this program parses are valid on their own,
    /// checked here so a bad bundle fails the build rather than a jeweller's
    /// menu click.
    #[test]
    fn bundled_json_parses_and_images_carry_a_header() {
        for family in [PROFILES, OUTLINES, GRAPHS, CLUSTERS, GRAPH_PRESETS, BASES, SIMPLE_GRAPH, DESIGNS] {
            for asset in family {
                let text = asset.text();
                assert!(text.trim_start().starts_with('{'), "{} is not a JSON document", asset.file);
            }
        }
        for asset in ALPHAS {
            let bytes = asset.bytes();
            assert!(
                bytes.starts_with(b"\x89PNG") || bytes.starts_with(b"\xff\xd8\xff") || bytes.starts_with(b"BM"),
                "{} is not a PNG, JPEG or BMP",
                asset.file
            );
        }
        for asset in GEMS {
            assert!(asset.text().contains("\nv "), "{} carries no vertices", asset.file);
        }
    }
}
