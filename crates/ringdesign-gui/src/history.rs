//! Undo history over the design, with a name for every step.
//!
//! Entries are whole-design snapshots. A `RingDesign` is a few kilobytes of
//! plain data, so keeping a couple of hundred costs less than the mesh built
//! from one of them, and restoring is exact by construction — there is no
//! inverse operation per edit to get wrong.
//!
//! # Where the names come from
//!
//! Not from the call sites. Panels mutate `app.design` directly and call
//! `mark_dirty()`, and threading a label through every one of those would be a
//! large edit that still misses anything mutated another way — an MCP client,
//! a loaded file, a future panel. Instead the label is read out of the
//! difference between the snapshots: serialize both and report the first field
//! that moved. Every edit gets named, including ones nobody thought to label.
//!
//! # Coalescing
//!
//! A slider drag fires `mark_dirty()` on every frame it moves. Committing each
//! one would bury the history, so a snapshot is only taken once the design has
//! been *still* for [`SETTLE`] — the drag lands as the single entry it reads as.

use std::time::{Duration, Instant};

use ringdesign_core::RingDesign;
use serde_json::Value;

/// How long the design must sit unchanged before the edit is committed.
const SETTLE: Duration = Duration::from_millis(400);

/// Entries kept before the oldest is dropped.
const MAX_ENTRIES: usize = 200;

/// Depth the diff walks before giving up and naming the section instead.
const MAX_DEPTH: usize = 6;

/// Fields that name the edit when several move at once.
///
/// Serialized objects come back in alphabetical order, so without this a style
/// change reads as whichever of the values it set sorts first — picking a Flat
/// profile would report the crown it happened to adjust.
const HEADLINE_KEYS: &[&str] = &["style", "kind", "outline", "blend", "enabled", "name"];

#[derive(Clone)]
struct Snapshot {
    /// What the edit *out of* this state was called.
    label: String,
    design: RingDesign,
}

pub struct History {
    past: Vec<Snapshot>,
    future: Vec<Snapshot>,
    /// The last committed state, and what undo compares against.
    baseline: RingDesign,
    /// When the design last changed, while an edit is still settling.
    touched: Option<Instant>,
}

impl History {
    pub fn new(design: &RingDesign) -> Self {
        Self {
            past: Vec::new(),
            future: Vec::new(),
            baseline: design.clone(),
            touched: None,
        }
    }

    /// Note that the design may have changed. Cheap; the comparison happens
    /// once the edit settles.
    pub fn touch(&mut self) {
        self.touched = Some(Instant::now());
    }

    /// Forget everything and start from this design — a new or opened file.
    pub fn reset(&mut self, design: &RingDesign) {
        self.past.clear();
        self.future.clear();
        self.baseline = design.clone();
        self.touched = None;
    }

    /// Commit the pending edit if it has settled. Returns the label recorded.
    pub fn commit_if_settled(&mut self, design: &RingDesign) -> Option<String> {
        if !self.touched.is_some_and(|t| t.elapsed() >= SETTLE) {
            return None;
        }
        self.touched = None;
        self.commit(design)
    }

    /// Commit now, whatever the timer says — for a save, an export, or anything
    /// that wants the history settled before it acts.
    pub fn commit(&mut self, design: &RingDesign) -> Option<String> {
        let label = describe(&self.baseline, design)?;
        self.past.push(Snapshot {
            label: label.clone(),
            design: self.baseline.clone(),
        });
        if self.past.len() > MAX_ENTRIES {
            self.past.remove(0);
        }
        self.future.clear();
        self.baseline = design.clone();
        Some(label)
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// What undo would take back, for a button's tooltip.
    pub fn undo_label(&self) -> Option<&str> {
        self.past.last().map(|s| s.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.future.last().map(|s| s.label.as_str())
    }

    pub fn undo(&mut self) -> Option<RingDesign> {
        let s = self.past.pop()?;
        self.future.push(Snapshot {
            label: s.label,
            design: self.baseline.clone(),
        });
        self.baseline = s.design;
        self.touched = None;
        Some(self.baseline.clone())
    }

    pub fn redo(&mut self) -> Option<RingDesign> {
        let s = self.future.pop()?;
        self.past.push(Snapshot {
            label: s.label,
            design: self.baseline.clone(),
        });
        self.baseline = s.design;
        self.touched = None;
        Some(self.baseline.clone())
    }

    /// The timeline as `(label, is_present)`, oldest first.
    ///
    /// Row 0 is the state the session opened in; row `k` is the state edit `k`
    /// produced, so the row indices line up with [`History::present`] and drop
    /// straight into [`History::jump_to`]. Undone steps stay on the list after
    /// the present, which is what makes it a timeline rather than a stack.
    pub fn timeline(&self) -> Vec<(String, bool)> {
        let mut out: Vec<(String, bool)> =
            Vec::with_capacity(self.past.len() + self.future.len() + 1);
        out.push(("Opened".to_string(), self.past.is_empty()));
        for (i, s) in self.past.iter().enumerate() {
            out.push((s.label.clone(), i + 1 == self.past.len()));
        }
        for s in self.future.iter().rev() {
            out.push((s.label.clone(), false));
        }
        out
    }

    /// Index of the present in [`History::timeline`].
    pub fn present(&self) -> usize {
        self.past.len()
    }

    /// Step to a point on the timeline, undoing or redoing as far as needed.
    pub fn jump_to(&mut self, index: usize) -> Option<RingDesign> {
        let mut out = None;
        while self.present() > index {
            out = self.undo().or(out);
        }
        while self.present() < index && self.can_redo() {
            out = self.redo().or(out);
        }
        out
    }
}

// --- Naming an edit --------------------------------------------------------

/// Name the first field that moved between two designs, or `None` when nothing
/// did.
fn describe(old: &RingDesign, new: &RingDesign) -> Option<String> {
    let (a, b) = (
        serde_json::to_value(old).ok()?,
        serde_json::to_value(new).ok()?,
    );
    if a == b {
        return None;
    }
    let mut path: Vec<String> = Vec::new();
    match first_difference(&a, &b, &mut path, 0) {
        Some((from, to)) => Some(phrase(&path, &from, &to)),
        None => Some("Edit".to_string()),
    }
}

/// Walk to the first differing leaf, recording the path taken.
fn first_difference(
    a: &Value,
    b: &Value,
    path: &mut Vec<String>,
    depth: usize,
) -> Option<(Value, Value)> {
    if a == b {
        return None;
    }
    if depth >= MAX_DEPTH {
        return Some((a.clone(), b.clone()));
    }
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let ordered = HEADLINE_KEYS
                .iter()
                .filter_map(|k| x.get_key_value(*k))
                .chain(
                    x.iter()
                        .filter(|(k, _)| !HEADLINE_KEYS.contains(&k.as_str())),
                );
            for (k, va) in ordered {
                let vb = y.get(k)?;
                if va != vb {
                    path.push(k.clone());
                    return first_difference(va, vb, path, depth + 1)
                        .or(Some((va.clone(), vb.clone())));
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            // A different length is the interesting fact, not whichever
            // element happens to have shifted under the insertion.
            if x.len() != y.len() {
                return Some((Value::from(x.len()), Value::from(y.len())));
            }
            for (i, (va, vb)) in x.iter().zip(y).enumerate() {
                if va != vb {
                    path.push(format!("#{}", i + 1));
                    return first_difference(va, vb, path, depth + 1)
                        .or(Some((va.clone(), vb.clone())));
                }
            }
            None
        }
        _ => Some((a.clone(), b.clone())),
    }
}

/// Turn a path and a pair of values into something a person would say.
fn phrase(path: &[String], from: &Value, to: &Value) -> String {
    let section = path.first().map(String::as_str).unwrap_or("");
    let leaf = path.last().map(String::as_str).unwrap_or("");

    // A layer count moving is an add or a remove, whatever field it surfaced on.
    if section == "layers" && from.is_u64() && to.is_u64() {
        let (a, b) = (from.as_u64().unwrap_or(0), to.as_u64().unwrap_or(0));
        return if b > a {
            "Added a layer".into()
        } else {
            "Removed a layer".into()
        };
    }

    let name = pretty(leaf);
    let unit = unit_of(leaf);
    let scope = match section {
        "profile" => "Profile",
        "shank" => "Shank",
        "draft" => "Casting",
        "build" => "Mesh",
        "size" => "Size",
        "layers" => "Layer",
        _ => "",
    };

    // Where the path went through a layer index, say which one.
    let index = path.iter().find(|p| p.starts_with('#')).cloned();
    let head = match (scope, index) {
        ("Layer", Some(i)) => format!(
            "Layer {} {}",
            i.trim_start_matches('#'),
            name.to_lowercase()
        ),
        ("", _) => name,
        // A newtype like `size` has no field under it, so the leaf *is* the
        // section and repeating it reads as "Size size".
        (s, _) if s.eq_ignore_ascii_case(&name) => name,
        (s, _) => format!("{s} {}", name.to_lowercase()),
    };

    match (show(from, &unit), show(to, &unit)) {
        (Some(a), Some(b)) => format!("{head} {a} -> {b}"),
        (_, Some(b)) => format!("{head} {b}"),
        _ => head,
    }
}

/// `width_mm` -> `Width`, `side_draft_deg` -> `Side draft`.
fn pretty(key: &str) -> String {
    let base = key
        .trim_end_matches("_mm")
        .trim_end_matches("_deg")
        .trim_end_matches("_frac")
        .replace('_', " ");
    let mut c = base.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => base,
    }
}

fn unit_of(key: &str) -> &'static str {
    if key.ends_with("_mm") {
        " mm"
    } else if key.ends_with("_deg") {
        "\u{00b0}"
    } else {
        ""
    }
}

/// A value as a person would read it, or `None` when it says nothing useful.
fn show(v: &Value, unit: &str) -> Option<String> {
    match v {
        Value::Number(n) => {
            let f = n.as_f64()?;
            Some(if f.fract().abs() < 1e-9 && f.abs() < 1e6 {
                format!("{f:.0}{unit}")
            } else {
                format!("{f:.2}{unit}")
            })
        }
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(if *b { "on".into() } else { "off".into() }),
        // An object or array changing wholesale has no short reading.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::field::{Layer, LayerEntry, MilgrainLayer};

    fn design() -> RingDesign {
        RingDesign::default()
    }

    #[test]
    fn a_scalar_edit_is_named_with_both_values() {
        let a = design();
        let mut b = a.clone();
        b.profile.width_mm = 6.5;
        let label = describe(&a, &b).expect("a change should be named");
        assert!(label.contains("Profile"), "{label}");
        assert!(
            !label.contains("Profile profile"),
            "the section is doubled: {label}"
        );
        assert!(label.contains("width"), "{label}");
        assert!(label.contains("6.50 mm"), "{label}");
    }

    #[test]
    fn a_newtype_field_does_not_repeat_its_own_name() {
        let a = design();
        let mut b = a.clone();
        b.size = ringdesign_core::RingSize(9.0);
        let label = describe(&a, &b).expect("named");
        assert!(!label.to_lowercase().contains("size size"), "{label}");
        assert!(label.contains('9'), "{label}");
    }

    #[test]
    fn an_unchanged_design_has_nothing_to_name() {
        assert!(describe(&design(), &design()).is_none());
    }

    #[test]
    fn adding_and_removing_a_layer_read_as_such() {
        let a = design();
        let mut b = a.clone();
        b.layers.layers.push(LayerEntry::new(
            "Milgrain",
            Layer::Milgrain(MilgrainLayer::default()),
        ));
        assert_eq!(describe(&a, &b).as_deref(), Some("Added a layer"));
        assert_eq!(describe(&b, &a).as_deref(), Some("Removed a layer"));
    }

    #[test]
    fn an_edit_inside_a_layer_says_which_one() {
        let mut a = design();
        for n in ["one", "two"] {
            a.layers.layers.push(LayerEntry::new(
                n,
                Layer::Milgrain(MilgrainLayer::default()),
            ));
        }
        let mut b = a.clone();
        b.layers.layers[1].opacity = 0.4;
        let label = describe(&a, &b).expect("named");
        assert!(label.contains("Layer 2"), "{label}");
    }

    #[test]
    fn an_enum_change_names_the_variant() {
        let a = design();
        let mut b = a.clone();
        b.profile.apply_style(ringdesign_core::ProfileStyle::Flat);
        let label = describe(&a, &b).expect("named");
        assert!(label.contains("Flat"), "{label}");
    }

    #[test]
    fn undo_and_redo_walk_back_and_forth() {
        let mut d = design();
        let mut h = History::new(&d);
        assert!(!h.can_undo() && !h.can_redo());

        d.profile.width_mm = 7.0;
        h.commit(&d).expect("committed");
        d.profile.thickness_mm = 3.0;
        h.commit(&d).expect("committed");
        assert!(h.can_undo());

        let back = h.undo().expect("undo");
        assert_eq!(back.profile.thickness_mm, 2.0);
        assert_eq!(
            back.profile.width_mm, 7.0,
            "undo took back more than one edit"
        );

        let back = h.undo().expect("undo");
        assert_eq!(back.profile.width_mm, 6.0);
        assert!(!h.can_undo());

        let fwd = h.redo().expect("redo");
        assert_eq!(fwd.profile.width_mm, 7.0);
        let fwd = h.redo().expect("redo");
        assert_eq!(fwd.profile.thickness_mm, 3.0);
        assert!(!h.can_redo());
    }

    #[test]
    fn a_new_edit_after_undo_drops_the_redo_branch() {
        let mut d = design();
        let mut h = History::new(&d);
        d.profile.width_mm = 7.0;
        h.commit(&d);
        let mut d = h.undo().expect("undo");
        assert!(h.can_redo());

        d.profile.thickness_mm = 3.0;
        h.commit(&d);
        assert!(!h.can_redo(), "the abandoned branch survived a new edit");
    }

    #[test]
    fn committing_nothing_records_nothing() {
        let d = design();
        let mut h = History::new(&d);
        assert!(h.commit(&d).is_none());
        assert!(!h.can_undo());
    }

    #[test]
    fn the_timeline_marks_where_the_present_is() {
        let mut d = design();
        let mut h = History::new(&d);
        for w in [6.5, 7.0, 7.5] {
            d.profile.width_mm = w;
            h.commit(&d);
        }
        assert_eq!(h.timeline().len(), 4, "three edits plus the start");
        assert_eq!(h.present(), 3);
        // The present is the last row, and no row repeats the one above it.
        assert!(h.timeline()[3].1, "the present is not the newest row");
        let rows = h.timeline();
        let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
        assert!(
            labels.windows(2).all(|w| w[0] != w[1]),
            "a row repeats its neighbour: {labels:?}"
        );

        h.undo();
        assert_eq!(h.present(), 2);
        assert_eq!(h.timeline().iter().filter(|(_, now)| *now).count(), 1);
        assert_eq!(h.timeline().len(), 4, "the redo branch left the timeline");
        assert!(h.timeline()[2].1, "the present marker did not move back");
    }

    #[test]
    fn jumping_lands_on_the_state_that_step_produced() {
        let mut d = design();
        let mut h = History::new(&d);
        for w in [6.5, 7.0, 7.5] {
            d.profile.width_mm = w;
            h.commit(&d);
        }
        let at = h.jump_to(1).expect("jumped");
        assert_eq!(at.profile.width_mm, 6.5);
        assert_eq!(h.present(), 1);

        let at = h.jump_to(3).expect("jumped");
        assert_eq!(at.profile.width_mm, 7.5);
    }

    #[test]
    fn the_history_cannot_grow_without_bound() {
        let mut d = design();
        let mut h = History::new(&d);
        for i in 0..MAX_ENTRIES + 40 {
            d.profile.width_mm = 4.0 + i as f64 * 0.01;
            h.commit(&d);
        }
        assert!(h.past.len() <= MAX_ENTRIES, "{} entries", h.past.len());
    }
}
