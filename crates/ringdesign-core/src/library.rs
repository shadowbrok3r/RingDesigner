//! Persisting designs and locating the alpha library on disk.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::RingDesign;

/// Overrides where the user's own files live, for platforms with no `$HOME`.
static DATA_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Point the library and design directories at `root`, once, before anything reads them.
///
/// Android has neither `XDG_DATA_HOME` nor `HOME`, so [`user_alpha_dir`] would otherwise fall back
/// to `"."` — which is `/` for an app process, and unwritable. The host hands us
/// `getFilesDir()` instead.
pub fn set_data_root(root: impl Into<PathBuf>) {
    let _ = DATA_ROOT.set(root.into());
}

/// The configured root, or the XDG-derived one.
pub fn data_root() -> PathBuf {
    if let Some(root) = DATA_ROOT.get() {
        return root.clone();
    }
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ringdesigner")
}

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
    data_root().join("alphas")
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
    data_root().join("designs")
}
