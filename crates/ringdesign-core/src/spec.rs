//! The tech sheet: one self-contained HTML page a caster can hold.
//!
//! Everything the pour needs to know without opening the app — dimensions,
//! weight in every alloy, the field-sampled verdict with its notes, the
//! stones and their bench checks, the DFM findings — inline CSS, no external
//! anything, printable. The caller supplies the provenance line so this
//! stays deterministic.

use crate::castability::{FieldReport, Verdict};
use crate::dfm::DfmFinding;
use crate::mesh::Report;
use crate::stones::{SeatFooting, StonesReport};
use crate::RingDesign;

fn esc(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            c => c.to_string(),
        })
        .collect()
}

/// Render the sheet. `provenance` is a freeform line — app, version, date.
pub fn html(
    design: &RingDesign,
    report: &Report,
    field: &FieldReport,
    stones: Option<&StonesReport>,
    dfm: &[DfmFinding],
    provenance: &str,
) -> String {
    let mut h = String::with_capacity(8 * 1024);
    let name = esc(&design.name);
    let verdict_color = match field.verdict {
        Verdict::Castable => "#2e8b57",
        Verdict::Marginal => "#b8860b",
        Verdict::NotCastable => "#b03040",
    };

    h.push_str(&format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>{name} — casting sheet</title>
<style>
 body {{ font: 13px/1.45 system-ui, sans-serif; color: #222; max-width: 720px; margin: 24px auto; padding: 0 16px; }}
 h1 {{ font-size: 20px; margin: 0 0 2px; }}
 h2 {{ font-size: 14px; margin: 18px 0 6px; border-bottom: 1px solid #ccc; padding-bottom: 2px; }}
 table {{ border-collapse: collapse; width: 100%; }}
 td, th {{ text-align: left; padding: 2px 10px 2px 0; vertical-align: top; }}
 th {{ font-weight: 600; color: #555; white-space: nowrap; }}
 .verdict {{ display: inline-block; padding: 3px 10px; border-radius: 4px; color: #fff; background: {verdict_color}; font-weight: 600; }}
 .dim {{ color: #777; }}
 .warn {{ color: #a06000; }}
 ul {{ margin: 4px 0; padding-left: 18px; }}
 @media print {{ body {{ margin: 8px; }} }}
</style></head><body>
<h1>{name}</h1>
<div class="dim">Size {size} • sand-cast pattern • all lengths mm</div>
"#,
        size = esc(&design.size.display()),
    ));

    // --- Cast check ---------------------------------------------------------
    h.push_str(&format!(
        r#"<h2>Cast check</h2>
<p><span class="verdict">{}</span></p>
<table>
<tr><th>Undercut</th><td>{:.3}% of the surface, worst {:+.1}&deg;</td></tr>
<tr><th>Thinnest wall</th><td>{:.2} mm at {:.0}&deg; (minimum fill {:.1} mm)</td></tr>
<tr><th>Parting plane</th><td>z = {:+.2} mm; cope pulls +Z, drag &minus;Z</td></tr>
<tr><th>Min draft</th><td>{:.1}&deg; • detail floor {:.2} mm</td></tr>
</table><ul>"#,
        esc(field.verdict.label()),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm,
        field.thinnest_wall_theta_deg,
        design.draft.min_section_mm,
        field.parting_z_mm,
        design.draft.min_draft_deg,
        design.draft.min_detail_mm,
    ));
    for n in &field.notes {
        h.push_str(&format!("<li>{}</li>", esc(n)));
    }
    for f in dfm {
        h.push_str(&format!(
            "<li class=\"warn\">{}: {}</li>",
            esc(&f.label),
            esc(&f.message)
        ));
    }
    h.push_str("</ul>");

    // --- Dimensions ---------------------------------------------------------
    h.push_str(&format!(
        r#"<h2>Dimensions</h2>
<table>
<tr><th>Inside dia</th><td>{:.2}</td><th>Outside dia</th><td>{:.2}</td></tr>
<tr><th>Band width</th><td>{:.2}</td><th>Overall</th><td>{:.2} × {:.2} × {:.2}</td></tr>
<tr><th>Relief</th><td>+{:.2} / {:+.2}</td><th>Volume</th><td>{:.2} mm&sup3;</td></tr>
</table>"#,
        report.inner_diameter_mm,
        report.outer_diameter_mm,
        report.band_width_mm,
        report.bounds_mm[0],
        report.bounds_mm[1],
        report.bounds_mm[2],
        report.max_relief_mm,
        report.min_relief_mm,
        report.volume_mm3,
    ));

    // --- Weights ------------------------------------------------------------
    h.push_str("<h2>Casting weight</h2><table><tr><th>Metal</th><th>grams</th><th>dwt</th><th>pattern scale</th></tr>");
    for m in &report.metals {
        let shrink = crate::metal::find(m.metal)
            .map(|mm| format!("&times;{:.4} (+{:.1}%)", crate::metal::pattern_scale(mm.shrink_pct), mm.shrink_pct))
            .unwrap_or_default();
        h.push_str(&format!(
            "<tr><td>{}</td><td>{:.2}</td><td>{:.2}</td><td class=\"dim\">{}</td></tr>",
            esc(m.metal),
            m.grams,
            m.dwt,
            shrink
        ));
    }
    h.push_str("</table><div class=\"dim\">Casting weight only — no sprue, button, or finishing loss. Pattern scale is the oversize to cut for that alloy's shrink.</div>");

    // --- Stones -------------------------------------------------------------
    if let Some(s) = stones {
        h.push_str(&format!(
            "<h2>Stones</h2><div>{} stones • {:.2} ct total — set at the bench; the ring casts the stock.</div><table><tr><th>Seat</th><th>Stone</th><th>Sits on</th><th>Clearance</th><th>Depth</th></tr>",
            s.stone_count, s.total_carats
        ));
        for seat in &s.seats {
            let footing = match seat.footing {
                SeatFooting::SideFace => "side face".to_string(),
                SeatFooting::Crown(d) => format!("crown {d:+.1}&deg;"),
            };
            h.push_str(&format!(
                "<tr><td>{}{}</td><td>{}</td><td>{}</td><td>{:.2}</td><td>{:.2}</td></tr>",
                esc(&seat.label),
                if seat.count > 1 { format!(" ×{}", seat.count) } else { String::new() },
                seat.gem.map(|g| esc(&g.display())).unwrap_or_else(|| "—".into()),
                footing,
                seat.edge_clearance_mm,
                seat.depth_available_mm,
            ));
            if let Some((pairs, dia, proud)) = seat.shared_prongs {
                h.push_str(&format!(
                    "<tr><td></td><td colspan=\"4\">shared prongs: {pairs} pairs, \
                     &Oslash;{dia:.2} mm posts, {proud:.2} mm proud</td></tr>"
                ));
            }
            for w in &seat.warnings {
                h.push_str(&format!(
                    "<tr><td></td><td colspan=\"4\" class=\"warn\">{}</td></tr>",
                    esc(w)
                ));
            }
        }
        h.push_str("</table>");
    }

    // --- Provenance ---------------------------------------------------------
    h.push_str(&format!(
        r#"<h2>Build</h2>
<table>
<tr><th>Mesh</th><td>{} triangles, watertight: {}; worst corner {:.1}&deg;, aspect {:.0}</td></tr>
<tr><th>Provenance</th><td>{}</td></tr>
</table>
</body></html>
"#,
        report.validation.triangle_count,
        report.validation.watertight,
        report.quality.min_angle_deg,
        report.quality.worst_aspect,
        esc(provenance),
    ));
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpha::AlphaLibrary;
    use crate::mesh::{build, BuildParams};

    #[test]
    fn the_sheet_carries_the_verdict_the_weights_and_escapes_the_name() {
        let lib = AlphaLibrary::builtin();
        let mut d = RingDesign::default();
        d.name = "Logan's <Heart> & Band".into();
        let out = build(
            &d,
            &lib,
            BuildParams { theta_steps: 96, profile_steps: 64, ..Default::default() },
        );
        let field = crate::castability::analyze_field(&d, &lib, &d.draft, 96, 64);
        let stones = crate::stones::report(&d, field.parting_z_mm);
        let dfm = crate::dfm::findings(&d);
        let page = html(&d, &out.report, &field, stones.as_ref(), &dfm, "test build");

        assert!(page.contains("Logan's &lt;Heart&gt; &amp; Band"));
        assert!(!page.contains("<Heart>"));
        assert!(page.contains(field.verdict.label()));
        assert!(page.contains("Silver 925"));
        assert!(page.contains("pattern scale"));
        assert!(page.contains("test build"));
        assert!(page.contains("Thinnest wall"));
        // Self-contained: no external references at all.
        assert!(!page.contains("http"));
        assert!(!page.contains("src="));
    }
}
