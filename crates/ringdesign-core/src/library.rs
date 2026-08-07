//! Persisting designs and locating the alpha library on disk.

use std::path::{Path, PathBuf};

use crate::RingDesign;

/// File extension for saved designs.
pub const DESIGN_EXT: &str = "ring.json";

pub fn save_design(path: impl AsRef<Path>, design: &RingDesign) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(design)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn load_design(path: impl AsRef<Path>) -> anyhow::Result<RingDesign> {
    let text = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

/// Alphas bundled with the source tree: `<workspace>/assets/alphas`.
pub fn bundled_alpha_dir() -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("assets").join("alphas"))
        .filter(|p| p.is_dir())
}

/// The user's own alpha library in the platform data directory. This is where
/// imports land and where a converted collection belongs.
pub fn user_alpha_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ringdesigner")
        .join("alphas")
}

/// Every directory scanned at startup, bundled first so a user file of the same
/// name wins.
pub fn alpha_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    out.extend(bundled_alpha_dir());
    let user = user_alpha_dir();
    if !out.contains(&user) {
        out.push(user);
    }
    out
}

/// Where an import writes by default.
pub fn default_alpha_dir() -> PathBuf {
    user_alpha_dir()
}

/// Designs directory, created on demand.
pub fn default_design_dir() -> PathBuf {
    default_alpha_dir()
        .parent()
        .map(|p| p.join("designs"))
        .unwrap_or_else(|| PathBuf::from("designs"))
}
