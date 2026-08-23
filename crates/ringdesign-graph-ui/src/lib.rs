//! The node editor: an egui-snarl view over a [`ringdesign_graph::graph::Graph`].
//!
//! The graph is the truth. Every frame the editor builds a `Snarl` from it,
//! lets snarl draw and edit, and extracts the graph back — WireLab's cycle —
//! so nothing about a design ever lives only in the view. Positions are the
//! one thing the view owns, and they are stored on the nodes.
//!
//! The vendored `egui-snarl` under `patches/` is the egui-0.36 repin; the
//! root `[patch.crates-io]` table reaches it, so exactly one egui is in the
//! tree. [`editor`] is filled in by M3.2.

pub mod editor;
pub mod style;
pub mod widgets;

pub use editor::{Editor, EditorResponse, NodeCard, build_snarl, extract_graph};

/// The egui-snarl this editor is built on, for sanity checks and the diff
/// guard against the sibling copies.
pub const SNARL_VERSION: &str = "0.11.0";

#[cfg(test)]
mod tests {
    /// The vendored forks are byte-identical to the sibling trees' copies
    /// when those are checked out beside this repository.
    #[test]
    fn vendored_copies_match_the_siblings() {
        let here = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../patches");
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
        let siblings = [home.join("Documents/Rust/Mobile/EguiMobile/patches"), home.join("Documents/Rust/EmbeddedApps/wirelab/patches")];
        for sib in siblings.iter().filter(|s| s.join("egui-snarl").is_dir()) {
            for crate_ in ["egui-snarl", "egui-scale"] {
                if !sib.join(crate_).is_dir() {
                    continue;
                }
                let out = std::process::Command::new("diff")
                    .args(["-rq", "--exclude=target", "--exclude=.git"])
                    .arg(here.join(crate_))
                    .arg(sib.join(crate_))
                    .output()
                    .expect("diff runs");
                assert!(out.status.success(), "{crate_} differs from {}: {}", sib.display(), String::from_utf8_lossy(&out.stdout));
            }
        }
        assert_eq!(egui_snarl_version(), super::SNARL_VERSION);
    }

    fn egui_snarl_version() -> String {
        let toml = std::fs::read_to_string(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../patches/egui-snarl/Cargo.toml")).unwrap();
        toml.lines().find_map(|l| l.strip_prefix("version = \"").map(|v| v.trim_end_matches('"').to_string())).unwrap_or_default()
    }
}
