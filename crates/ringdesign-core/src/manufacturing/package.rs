//! Identifiable pattern packages and a self-contained printable molding sheet.

use super::{Inspection, Setup, inspect, release::Status};
use crate::{AlphaLibrary, BuildParams, RingDesign};
use serde_json::json;
use std::fmt::Write;
use std::path::Path;

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Stable non-cryptographic identity for diagnostics and stale-report detection.
pub fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}

pub fn report(
    d: &RingDesign,
    setup: &Setup,
    i: &Inspection,
    diagnostic: bool,
) -> serde_json::Value {
    let stl = crate::stl::to_stl_binary(&i.prepared.mesh, "pattern");
    let mesh = &i.prepared.mesh;
    let bounds = mesh
        .bounds()
        .map(|(lo, hi)| [hi.0 - lo.0, hi.1 - lo.1, hi.2 - lo.2]);
    json!({"format_version":1,"design":d.name,"nominal_size":d.size.display(),"nominal_bore_mm":d.size.inner_diameter_mm(),
        "diagnostic_only":diagnostic,"pattern_fingerprint":format!("{:016x}",fingerprint(&stl[80..])),"units":"millimeter",
        "setup":setup,"pattern_scale":i.prepared.scale,"release":i.release,"radial_wall_mm":i.field.as_ref().map(|f|f.thinnest_wall_mm),
        "radial_wall_limit_mm":setup.recipe.min_section_mm,"sampled_local_wall":i.local_wall,"detail_findings":i.details,"bench_layers":i.prepared.bench_layers,
        "geometry":{"validation":mesh.validate(),"volume_mm3":mesh.volume_mm3(),"surface_area_mm2":mesh.surface_area_mm2(),"bounds_mm":bounds},
        "source_build":i.prepared.build,"cast_ring_grams":i.ring_grams,"estimated_channel_grams":i.channel_grams,
        "estimated_charge_grams":i.ring_grams+i.channel_grams,"feeding_proxy":i.hot_spot,
        "limits":["Release is sampled; features between rays can be missed","Sand-slot widths are a screening check, not a sand-strength model","Radial wall is not the minimum thickness in every direction","Channel volumes overlap at junctions and omit melt/handling losses","Separately cored geometry needs a core and assembly plan"]})
}

pub fn mold_svg(setup: &Setup, i: &Inspection) -> String {
    let f = &setup.flask;
    let w = f.width_mm;
    let h = f.length_mm;
    let r = &i.release;
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{}mm\" viewBox=\"{} {} {w} {}\"><rect x=\"{}\" y=\"{}\" width=\"{w}\" height=\"{h}\" fill=\"#eee7d7\" stroke=\"#665d4a\" stroke-width=\"0.2\"/>",
        h + 8.0,
        -w / 2.0,
        -h / 2.0,
        h + 8.0,
        -w / 2.0,
        -h / 2.0
    );
    for c in &r.columns {
        if c.intervals.is_empty() {
            continue;
        }
        let blocked = c.upper_block_mm > 0.0 || c.lower_block_mm > 0.0;
        let _ = write!(
            s,
            "<rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" fill=\"{}\"/>",
            c.x - r.cell_mm[0] / 2.0,
            -c.y - r.cell_mm[1] / 2.0,
            r.cell_mm[0] + 0.003,
            r.cell_mm[1] + 0.003,
            if blocked { "#db365b" } else { "#6b7489" }
        );
    }
    for c in &setup.channels {
        let _ = write!(
            s,
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#9c6b18\" stroke-width=\"{}\" stroke-linecap=\"round\"/><text x=\"{}\" y=\"{}\" font-size=\"1.7\">{}</text>",
            c.start[0],
            -c.start[1],
            c.end[0],
            -c.end[1],
            c.diameter_mm,
            c.end[0],
            -c.end[1] - 1.0,
            c.kind.label()
        );
    }
    let _ = write!(
        s,
        "<path d=\"M {} {} h 10\" stroke=\"black\" stroke-width=\"0.25\"/><text x=\"{}\" y=\"{}\" font-size=\"2\">10 mm • print at 100%</text></svg>",
        -w / 2.0 + 2.0,
        h / 2.0 + 3.0,
        -w / 2.0 + 2.0,
        h / 2.0 + 6.0
    );
    s
}

pub fn sheet(d: &RingDesign, setup: &Setup, i: &Inspection, diagnostic: bool) -> String {
    let r = &i.release;
    let mut h = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{} manufacturing sheet</title><style>body{{font:15px system-ui;max-width:900px;margin:24px;color:#222}}table{{border-collapse:collapse;width:100%}}th,td{{text-align:left;padding:6px;border-bottom:1px solid #ccc}}.alert{{font-weight:700;color:#a31537}}svg{{max-width:100%;height:auto}}@media print{{body{{margin:0}}}}</style></head><body><h1>{}</h1><p class=\"alert\">{}: {}</p>",
        escape(&d.name),
        escape(&d.name),
        if diagnostic {
            "DIAGNOSTIC PATTERN — REVIEW BEFORE MANUFACTURE"
        } else {
            "Manufacturing review"
        },
        r.status.label()
    );
    h.push_str("<table>");
    for (label, value) in [
        (
            "Nominal fit",
            format!(
                "{}; bore {:.3} mm",
                d.size.display(),
                d.size.inner_diameter_mm()
            ),
        ),
        (
            "Recipe",
            format!("{}; {}", setup.recipe.name, setup.recipe.calibration_note),
        ),
        ("Alloy", setup.recipe.alloy.clone()),
        (
            "Pattern scale",
            format!(
                "×{:.6}; {:.3}% shrink, applied once",
                i.prepared.scale, setup.recipe.shrink_pct
            ),
        ),
        (
            "Profile stock",
            format!(
                "radial {:.3}; each side {:.3}; bore {:.3} mm",
                setup.radial_stock_mm, setup.axial_stock_mm, setup.bore_stock_mm
            ),
        ),
        (
            "Pull direction",
            format!(
                "[{:.5}, {:.5}, {:.5}]",
                r.frame.z[0], r.frame.z[1], r.frame.z[2]
            ),
        ),
        (
            "Parting plane",
            format!("{:+.3} mm along pull, in pattern coordinates", r.parting_mm),
        ),
        ("Bore strategy", setup.bore.label().into()),
        (
            "Flask",
            format!(
                "{} × {} mm; upper {} mm; lower {} mm; margin {} mm",
                setup.flask.width_mm,
                setup.flask.length_mm,
                setup.flask.cope_mm,
                setup.flask.drag_mm,
                setup.recipe.sand_margin_mm
            ),
        ),
        (
            "Charge estimate",
            format!(
                "ring {:.2} g + channels {:.2} g = {:.2} g; excludes handling losses",
                i.ring_grams,
                i.channel_grams,
                i.ring_grams + i.channel_grams
            ),
        ),
        (
            "Radial wall",
            i.field.as_ref().map_or_else(
                || "Not applicable to this CAD component; inspect local thickness".into(),
                |f| {
                    format!(
                        "{:.3} mm, target ≥ {:.3} mm; other wall directions need inspection",
                        f.thinnest_wall_mm, setup.recipe.min_section_mm
                    )
                },
            ),
        ),
    ] {
        let _ = write!(
            h,
            "<tr><th>{}</th><td>{}</td></tr>",
            escape(label),
            escape(&value)
        );
    }
    let sand = setup.recipe.process == crate::castability::CastProcess::SandTwoPart;
    h.push_str(if sand {
        "</table><h2>Mold plan</h2>"
    } else {
        "</table><h2>Pattern reference view</h2>"
    });
    h.push_str(&mold_svg(setup, i));
    if let Some(wall) = &i.local_wall {
        let _ = write!(
            h,
            "<p>Local wall samples: minimum {:?} mm; {} below {:.3} mm; {} unresolved. {}</p>",
            wall.sampled_min_mm,
            wall.below_limit,
            wall.limit_mm,
            wall.unresolved,
            escape(wall.note)
        );
    }
    if sand {
        h.push_str("<p>Plan viewed down the pull direction. Red marks sampled obstructions. Channels are cut into the sand and are not attached to the printed pattern.</p><h2>Opening and pattern withdrawal</h2><ol><li>Orient the pattern using the pull direction and parting plane above.</li><li>Prepare the lower mold, then the upper mold and any separately planned core.</li><li>Lift the upper mold straight along the positive pull direction.</li><li>Withdraw the pattern from the lower mold along the positive pull direction. Inspect the finger-hole sand and narrow projections.</li><li>Cut the planned channels, inspect both impressions, and reassemble the mold.</li></ol><h2>Findings</h2><ul>");
    } else {
        h.push_str("<p>Pull direction, parting, flask, and any channels shown above are geometric reference data. Two-part sand withdrawal is not required for this investment-casting recipe.</p><h2>Investment pattern preparation</h2><ol><li>Produce the compensated pattern using the caster's chosen wax or castable material.</li><li>Verify dimensions, minimum sections, and access for removing investment from cavities.</li><li>Have the caster plan the sprue tree, investment, and burnout process for this material and alloy.</li><li>Inspect the casting and complete the bench and assembly operations below.</li></ol><h2>Findings</h2><ul>");
    }

    for o in &r.obstructions {
        let _ = write!(
            h,
            "<li class=\"alert\">{}: obstruction depth {:.3} mm at [{:.2}, {:.2}, {:.2}]</li>",
            o.half.label(),
            o.depth_mm,
            o.point[0],
            o.point[1],
            o.point[2]
        );
    }
    for note in r.notes.iter().chain(i.details.iter()) {
        let _ = write!(h, "<li>{}</li>", escape(note));
    }
    for f in &r.sand_findings {
        let _ = write!(h, "<li>{}</li>", escape(&f.message));
    }
    h.push_str("</ul><h2>Bench operations</h2><ul>");
    for name in &i.prepared.bench_layers {
        let _ = write!(
            h,
            "<li>{}: omitted from pattern; plan the required cutting or added component at the bench.</li>",
            escape(name)
        );
    }
    let _ = write!(
        h,
        "</ul><p>{}</p><p>Metal-flow and sand-strength predictions require workshop verification. Review the JSON report for sampling and mesh-quality details.</p></body></html>",
        escape(&setup.bench_notes)
    );
    h
}

/// Portable package contents. Browser and native exports run identical checks.
pub struct Package {
    pub report: serde_json::Value,
    pub entries: Vec<crate::threemf::Entry>,
}
impl Package {
    pub fn zip(&self) -> Vec<u8> {
        crate::threemf::zip_store(&self.entries)
    }
    /// Write to staging, then rename. Never overwrite an existing destination.
    pub fn write(&self, dir: &Path) -> anyhow::Result<()> {
        anyhow::ensure!(!dir.exists(), "Package destination already exists");
        let parent = dir
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let temp = parent.join(format!(".package-{}-{nonce}.building", std::process::id()));
        std::fs::create_dir(&temp)?;
        let result = (|| -> anyhow::Result<()> {
            for e in &self.entries {
                let path = Path::new(&e.name);
                anyhow::ensure!(
                    !path.as_os_str().is_empty()
                        && path
                            .components()
                            .all(|c| matches!(c, std::path::Component::Normal(_))),
                    "Invalid package entry"
                );
                let path = temp.join(path);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(path, &e.data)?;
            }
            anyhow::ensure!(!dir.exists(), "Destination appeared during export");
            std::fs::rename(&temp, dir)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&temp);
        }
        result
    }
}

pub fn files(
    d: &RingDesign,
    lib: &AlphaLibrary,
    setup: &Setup,
    params: BuildParams,
    diagnostic: bool,
) -> anyhow::Result<Package> {
    let i = inspect(d, lib, setup, params)?;
    anyhow::ensure!(
        i.release.status != Status::Invalid,
        "Cannot export invalid geometry"
    );
    anyhow::ensure!(
        diagnostic
            || (i.release.status != Status::Blocked
                && (i.release.fits_flask
                    || setup.recipe.process == crate::castability::CastProcess::LostWax)),
        "Pattern is obstructed or does not fit the flask. Correct it or request a diagnostic package."
    );
    if let Some(r) = i.prepared.build.refine {
        anyhow::ensure!(
            diagnostic || (!r.hit_cap && r.saturated_leaves == 0),
            "Export refinement did not reach its limits; use diagnostic export to inspect it"
        );
    }
    let manifest = report(d, setup, &i, diagnostic);
    let name = format!(
        "{} [{}pattern ×{:.6}]",
        d.name,
        if diagnostic { "DIAGNOSTIC " } else { "" },
        i.prepared.scale
    );
    let filename = if diagnostic {
        "DIAGNOSTIC-pattern"
    } else {
        "pattern"
    };
    let mut saved = d.clone();
    saved.manufacturing = Some(setup.clone());
    saved.embed_alphas(&super::source_library(d, lib));
    let entries = vec![
        crate::threemf::Entry {
            name: format!("{filename}.stl"),
            data: crate::stl::to_stl_binary(&i.prepared.mesh, &name),
        },
        crate::threemf::Entry {
            name: format!("{filename}.3mf"),
            data: crate::threemf::to_3mf(&i.prepared.mesh, &name, &d.size.display()),
        },
        crate::threemf::Entry {
            name: "report.json".into(),
            data: serde_json::to_vec_pretty(&manifest)?,
        },
        crate::threemf::Entry {
            name: "recipe.json".into(),
            data: serde_json::to_vec_pretty(&setup.recipe)?,
        },
        crate::threemf::Entry {
            name: "mold.svg".into(),
            data: mold_svg(setup, &i).into_bytes(),
        },
        crate::threemf::Entry {
            name: "molding-sheet.html".into(),
            data: sheet(d, setup, &i, diagnostic).into_bytes(),
        },
        crate::threemf::Entry {
            name: "design.ring.json".into(),
            data: crate::library::design_json(&saved)?.into_bytes(),
        },
    ];
    Ok(Package {
        report: manifest,
        entries,
    })
}

pub fn export(
    dir: &Path,
    d: &RingDesign,
    lib: &AlphaLibrary,
    setup: &Setup,
    params: BuildParams,
    diagnostic: bool,
) -> anyhow::Result<serde_json::Value> {
    anyhow::ensure!(
        !dir.exists(),
        "Package directory already exists; choose a new directory"
    );
    let package = files(d, lib, setup, params, diagnostic)?;
    package.write(dir)?;
    Ok(package.report)
}
