//! What the app remembers between launches.
//!
//! The design itself is autosaved; this is everything around it — where the
//! desktop is, how the brush is set, which shading you were looking at. The
//! sync host and token were documented as "both remembered between launches"
//! and were not: nothing wrote them, so a Tailscale address and a secret had to
//! be retyped on a phone keyboard at every launch, which is the friction that
//! makes a feature go unused.
//!
//! Written on the same debounce that autosaves, so it costs nothing extra, and
//! every field is `#[serde(default)]` so a file from an older build loads
//! rather than resetting everything it does not mention.
//!
//! Pure — no egui, no Android — so the round trip is host-testable.

use serde::{Deserialize, Serialize};

/// Filename beside the autosave in the app's data root.
pub const FILE: &str = "prefs.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub sync_host: String,
    /// Stored in the app's private data directory, which is not readable by
    /// other apps without root — the same place the designs live.
    pub sync_token: String,
    pub brush_frac: f32,
    pub brush_depth: f64,
    pub brush_erase: bool,
    pub stylus_only: bool,
    /// `ShadeMode` as its index in `ShadeMode::ALL`; the enum is not `Serialize`
    /// and lives in the viewport, which has no business knowing about serde.
    pub shade: usize,
    pub wireframe: bool,
    /// Every CAD part's edges drawn over the metal.
    pub part_edges: bool,
    /// The work planes drawn over the ring, each named.
    pub work_planes: bool,
    pub navigation: ringdesign_workbench::navigation::Settings,
    pub preview_quality: crate::ring::PreviewQuality,
    pub finish: usize,
    pub polish: usize,
    pub as_cast: bool,
    pub show_gems: bool,
    /// Resolve made settings into the preview as they are edited.
    pub live_cuts: bool,
    /// Draw the seats' cutters over the ring as a ghost.
    pub show_cutters: bool,
    pub editor_mode: usize,
    pub editor_guides: bool,
    pub editor_inspector: bool,
    pub editor_debug_layout: bool,
    pub workspace: Workspace,
    /// Index into `ringdesign_core::metal::METALS`; `None` is nominal.
    pub shrink_metal: Option<usize>,
    pub pattern_repeats: u32,
    pub pattern_height_mm: f64,
    /// Most recently opened or saved design files, newest first.
    pub recent: Vec<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            sync_host: String::new(),
            sync_token: String::new(),
            brush_frac: 0.012,
            brush_depth: 1.0,
            brush_erase: false,
            stylus_only: false,
            shade: 0,
            wireframe: false,
            part_edges: true,
            work_planes: true,
            navigation: Default::default(),
            preview_quality: Default::default(),
            finish: 0,
            polish: 0,
            as_cast: false,
            show_gems: true,
            live_cuts: true,
            show_cutters: false,
            editor_mode: 0,
            editor_guides: true,
            editor_inspector: true,
            editor_debug_layout: false,
            workspace: Workspace::default(),
            shrink_metal: None,
            pattern_repeats: 24,
            pattern_height_mm: 0.35,
            recent: Vec::new(),
        }
    }
}

/// How many recent files are kept, matching the desktop's `push_recent`.
pub const MAX_RECENT: usize = 10;

impl Prefs {
    /// Put `path` at the front, dropping any earlier mention of it and anything
    /// past the cap. Most-recent-first with no duplicates.
    pub fn push_recent(&mut self, path: &str) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_string());
        self.recent.truncate(MAX_RECENT);
    }

    pub fn forget_recent(&mut self, path: &str) {
        self.recent.retain(|p| p != path);
    }

    /// Clamp anything a hand-edited or older file could put out of range.
    ///
    /// Every one of these feeds a slider or an index; egui panics on a slider
    /// whose value is outside its range, and a stale `shade` index would panic
    /// the shader lookup.
    pub fn sanitize(&mut self, shade_modes: usize, metals: usize) {
        self.workspace.sanitize();
        self.brush_frac = self.brush_frac.clamp(0.002, 0.08);
        self.brush_depth = self.brush_depth.clamp(0.05, 1.0);
        self.pattern_repeats = self.pattern_repeats.clamp(1, 200);
        self.pattern_height_mm = self.pattern_height_mm.clamp(0.02, 1.6);
        if self.shade >= shade_modes {
            self.shade = 0;
        }
        if self.shrink_metal.is_some_and(|i| i >= metals) {
            self.shrink_metal = None;
        }
        self.recent.truncate(MAX_RECENT);
    }
}

/// Normalized workspace geometry survives screen size and keyboard changes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Workspace {
    pub mode_panels: [Option<crate::editor::Sheet>; 4],
    pub graph_fullscreen: bool,
    /// Portrait height and landscape width as fractions of the available editor.
    pub inspector_fraction: [f32; 2],
    /// The same, while the recipe graph rides under (or beside) the ring: a
    /// canvas has no height of its own to hug.
    pub graph_fraction: [f32; 2],
    /// Turn the ring to face what the chosen node does.
    pub graph_follow: bool,
    pub rail_position: Option<[f32; 2]>,
    pub palette_position: Option<[f32; 2]>,
    pub rail_collapsed: bool,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            mode_panels: [Some(crate::editor::Sheet::Edit), Some(crate::editor::Sheet::Layers), Some(crate::editor::Sheet::Edit), Some(crate::editor::Sheet::Findings)],
            graph_fullscreen: true,
            inspector_fraction: [0.32, 0.36],
            graph_fraction: [0.5, 0.5],
            graph_follow: true,
            rail_position: None,
            palette_position: None,
            rail_collapsed: false,
        }
    }
}

impl Workspace {
    pub fn sanitize(&mut self) {
        for (i, fraction) in self.inspector_fraction.iter_mut().enumerate() {
            *fraction = if fraction.is_finite() {
                fraction.clamp(0.08, 0.75)
            } else {
                Self::default().inspector_fraction[i]
            };
        }
        for (i, fraction) in self.graph_fraction.iter_mut().enumerate() {
            *fraction = if fraction.is_finite() {
                fraction.clamp(0.2, 0.75)
            } else {
                Self::default().graph_fraction[i]
            };
        }
        for position in [&mut self.rail_position, &mut self.palette_position] {
            if let Some(point) = position {
                if point.iter().all(|v| v.is_finite()) {
                    *point = point.map(|v| v.clamp(0.0, 1.0));
                } else {
                    *position = None;
                }
            }
        }
    }
}

/// Read prefs from `dir`, falling back to defaults on anything unreadable — a
/// corrupt preferences file must never stop the app starting.
pub fn load(dir: &std::path::Path) -> Prefs {
    std::fs::read_to_string(dir.join(FILE))
        .ok()
        .and_then(|t| serde_json::from_str::<Prefs>(&t).ok())
        .unwrap_or_default()
}

pub fn save(dir: &std::path::Path, prefs: &Prefs) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(prefs)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(dir.join(FILE), text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_round_trip_keeps_every_field() {
        let mut p = Prefs::default();
        p.sync_host = "100.64.0.1".into();
        p.sync_token = "hunter2".into();
        p.shade = 3;
        p.shrink_metal = Some(4);
        p.push_recent("/a/b.ring.json");
        let text = serde_json::to_string(&p).expect("serializes");
        let back: Prefs = serde_json::from_str(&text).expect("deserializes");
        assert_eq!(p, back);
    }

    /// A file written by an older build mentions fewer fields; the ones it does
    /// not name must keep their defaults rather than reset the lot.
    #[test]
    fn an_older_file_keeps_its_fields_and_defaults_the_rest() {
        let p: Prefs = serde_json::from_str(r#"{"sync_host":"host.ts.net"}"#).expect("loads");
        assert_eq!(p.sync_host, "host.ts.net");
        assert_eq!(p.brush_depth, Prefs::default().brush_depth);
        assert!(p.show_gems, "and a default that is not false survives");
        assert!(p.work_planes, "a file from before the switch draws the work planes");
        let hidden: Prefs = serde_json::from_str(r#"{"work_planes":false}"#).expect("loads");
        assert!(!hidden.work_planes && hidden.part_edges);
    }

    #[test]
    fn unreadable_prefs_fall_back_rather_than_failing() {
        let dir = std::env::temp_dir().join("rd-prefs-none");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::remove_file(dir.join(FILE));
        assert_eq!(load(&dir), Prefs::default());
        std::fs::write(dir.join(FILE), "{ not json").expect("write");
        assert_eq!(
            load(&dir),
            Prefs::default(),
            "corrupt must not stop the app starting"
        );
    }

    #[test]
    fn recents_are_newest_first_without_duplicates() {
        let mut p = Prefs::default();
        for f in ["a", "b", "c"] {
            p.push_recent(f);
        }
        assert_eq!(p.recent, ["c", "b", "a"]);
        p.push_recent("a");
        assert_eq!(
            p.recent,
            ["a", "c", "b"],
            "reopening moves it to the front, not a second copy"
        );
        p.forget_recent("c");
        assert_eq!(p.recent, ["a", "b"]);
    }

    #[test]
    fn recents_are_capped() {
        let mut p = Prefs::default();
        for i in 0..(MAX_RECENT + 5) {
            p.push_recent(&format!("f{i}"));
        }
        assert_eq!(p.recent.len(), MAX_RECENT);
        assert_eq!(
            p.recent[0],
            format!("f{}", MAX_RECENT + 4),
            "newest survives"
        );
    }

    /// Every sanitized field feeds a slider or an index, and egui panics on a
    /// slider whose value is outside its range.
    #[test]
    fn sanitize_pulls_everything_back_into_range() {
        let mut p = Prefs::default();
        p.brush_frac = 99.0;
        p.brush_depth = -1.0;
        p.pattern_repeats = 100_000;
        p.pattern_height_mm = 0.0;
        p.shade = 42;
        p.shrink_metal = Some(999);
        p.sanitize(5, 10);
        assert!((0.002..=0.08).contains(&p.brush_frac));
        assert!((0.05..=1.0).contains(&p.brush_depth));
        assert!((1..=200).contains(&p.pattern_repeats));
        assert!((0.02..=1.6).contains(&p.pattern_height_mm));
        assert_eq!(
            p.shade, 0,
            "a stale shade index would panic the shader lookup"
        );
        assert_eq!(p.shrink_metal, None);
    }

    #[test]
    fn sanitize_leaves_valid_values_alone() {
        let mut p = Prefs::default();
        p.shade = 3;
        p.shrink_metal = Some(2);
        let before = p.clone();
        p.sanitize(5, 10);
        assert_eq!(p, before);
    }
}
