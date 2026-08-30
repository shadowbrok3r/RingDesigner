//! The golden corpus: what every shipped starting point measures, pinned.
//!
//! The templates test asserted only `verdict != NotCastable`, so a change that
//! took all nine from Castable to Marginal, or that moved `volume_mm3`, failed
//! nothing. This walks the shop window — the nine templates, every
//! `ProfileStyle` as a bare band, and every `ShankKind` as a bare band — and
//! compares the numbers a jeweller reads off the report against a committed
//! table.
//!
//! It is a *regression* net, not a specification: a row moving is not
//! automatically wrong. It means the diff has to say why, and the table is
//! rewritten in the same commit.
//!
//! ```text
//! RD_WRITE_GOLDEN=1 cargo test -p ringdesign-core --test golden
//! ```

use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability;
use ringdesign_core::mesh::{build, BuildParams};
use ringdesign_core::profile::{ShankKind, ShankStyle};
use ringdesign_core::{ProfileStyle, RingDesign};
use serde::{Deserialize, Serialize};

/// Fixed for the corpus. Not the preview's resolution and not export's — the
/// point is that it never moves, so a row that changes means the geometry did.
const THETA: usize = 192;
const PROFILE: usize = 96;
const FIELD_THETA: usize = 160;
const FIELD_PROFILE: usize = 96;

/// Relative slack on a summed f64. Tight enough to catch a real geometry
/// change, loose enough to survive a different summation order.
const REL: f64 = 1e-9;
/// Absolute slack on a millimetre figure.
const ABS_MM: f64 = 1e-6;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Row {
    name: String,
    verdict: String,
    undercut_pct: f64,
    worst_draft_deg: f64,
    thinnest_wall_mm: f64,
    volume_mm3: f64,
    surface_area_mm2: f64,
    max_relief_mm: f64,
    triangles: usize,
    watertight: bool,
}

fn row(name: &str, d: &RingDesign, lib: &AlphaLibrary) -> Row {
    let out = build(
        d,
        lib,
        BuildParams { theta_steps: THETA, profile_steps: PROFILE, ..Default::default() },
    );
    let f = castability::analyze_field(d, lib, &d.draft, FIELD_THETA, FIELD_PROFILE);
    Row {
        name: name.to_string(),
        verdict: format!("{:?}", f.verdict),
        undercut_pct: f.undercut_fraction() * 100.0,
        worst_draft_deg: f.worst_draft_deg,
        thinnest_wall_mm: f.thinnest_wall_mm,
        volume_mm3: out.report.volume_mm3,
        surface_area_mm2: out.report.surface_area_mm2,
        max_relief_mm: out.report.max_relief_mm,
        triangles: out.report.validation.triangle_count,
        watertight: out.report.validation.watertight,
    }
}

fn bare_band() -> RingDesign {
    RingDesign::default()
}

fn corpus() -> Vec<Row> {
    let lib = AlphaLibrary::builtin();
    let mut rows = Vec::new();

    for t in ringdesign_core::templates::all() {
        rows.push(row(&format!("template/{}", t.name), &t.design(), &lib));
    }
    for &style in ProfileStyle::ALL {
        let mut d = bare_band();
        d.profile.apply_style(style);
        rows.push(row(&format!("profile/{}", style.label()), &d, &lib));
    }
    for &kind in ShankKind::ALL {
        let mut d = bare_band();
        d.shank = ShankStyle { kind, ..ShankStyle::default() };
        if kind == ShankKind::Signet {
            d.shank.apply_signet(d.profile.width_mm);
        }
        rows.push(row(&format!("shank/{}", kind.label()), &d, &lib));
    }
    rows
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= ABS_MM + REL * a.abs().max(b.abs())
}

#[test]
fn the_shop_window_still_measures_what_it_measured() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/corpus.json");
    let got = corpus();

    if std::env::var("RD_WRITE_GOLDEN").is_ok() {
        std::fs::write(&path, serde_json::to_string_pretty(&got).unwrap() + "\n").unwrap();
        eprintln!("wrote {} rows to {}", got.len(), path.display());
        return;
    }

    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("{}: {e}. Seed it with RD_WRITE_GOLDEN=1.", path.display())
    });
    let want: Vec<Row> = serde_json::from_str(&text).expect("the corpus parses");

    let names = |r: &[Row]| r.iter().map(|x| x.name.clone()).collect::<Vec<_>>();
    assert_eq!(
        names(&got),
        names(&want),
        "the corpus gained or lost rows — rewrite it with RD_WRITE_GOLDEN=1 and say why in the commit"
    );

    let mut moved = Vec::new();
    for (g, w) in got.iter().zip(&want) {
        let mut why = Vec::new();
        if g.verdict != w.verdict {
            why.push(format!("verdict {} -> {}", w.verdict, g.verdict));
        }
        if g.watertight != w.watertight {
            why.push(format!("watertight {} -> {}", w.watertight, g.watertight));
        }
        if g.triangles != w.triangles {
            why.push(format!("triangles {} -> {}", w.triangles, g.triangles));
        }
        for (label, a, b) in [
            ("undercut_pct", g.undercut_pct, w.undercut_pct),
            ("worst_draft_deg", g.worst_draft_deg, w.worst_draft_deg),
            ("thinnest_wall_mm", g.thinnest_wall_mm, w.thinnest_wall_mm),
            ("volume_mm3", g.volume_mm3, w.volume_mm3),
            ("surface_area_mm2", g.surface_area_mm2, w.surface_area_mm2),
            ("max_relief_mm", g.max_relief_mm, w.max_relief_mm),
        ] {
            if !close(a, b) {
                why.push(format!("{label} {b} -> {a}"));
            }
        }
        if !why.is_empty() {
            moved.push(format!("  {}: {}", g.name, why.join(", ")));
        }
    }

    assert!(
        moved.is_empty(),
        "{} of {} rows moved:\n{}\n\nIf the change is intended, rewrite the table with \
         RD_WRITE_GOLDEN=1 and say why in the commit message.",
        moved.len(),
        want.len(),
        moved.join("\n")
    );
}
