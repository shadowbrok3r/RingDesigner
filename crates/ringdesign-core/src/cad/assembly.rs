//! Component checks and an atomic assembly package. Reference stones are
//! included in the manifest and clearance check, excluded from metal exports.
use super::{AlphaLibrary, BuildParams, Evaluated, RingDesign, brep};
use crate::manufacturing::{
    self as mf,
    package::{escape, fingerprint},
};
use anyhow::{Result, ensure};
use serde::Serialize;
use serde_json::json;
use std::{fmt::Write, path::Path};

#[derive(Clone, Debug, Serialize)]
pub struct PairReport {
    pub a: u64,
    pub b: u64,
    pub interference_mm3: Option<f64>,
    pub required_clearance_mm: f64,
    pub clearance_estimate_mm: Option<f64>,
    pub note: String,
}
fn aabb_distance(a: &crate::Mesh, b: &crate::Mesh) -> f64 {
    let Some((al, ah)) = a.bounds() else {
        return f64::INFINITY;
    };
    let Some((bl, bh)) = b.bounds() else {
        return f64::INFINITY;
    };
    [
        (bl.0 - ah.0).max(al.0 - bh.0).max(0.0) as f64,
        (bl.1 - ah.1).max(al.1 - bh.1).max(0.0) as f64,
        (bl.2 - ah.2).max(al.2 - bh.2).max(0.0) as f64,
    ]
    .iter()
    .map(|v| v * v)
    .sum::<f64>()
    .sqrt()
}
pub fn inspect(d: &RingDesign, e: &Evaluated) -> Vec<PairReport> {
    let joints = d.cad.as_ref().map(|d| d.joints.as_slice()).unwrap_or(&[]);
    let mut reports = Vec::new();
    for (i, a) in e.components.iter().enumerate() {
        for b in e.components.iter().skip(i + 1) {
            let required = joints
                .iter()
                .find(|j| (j.a == a.id && j.b == b.id) || (j.a == b.id && j.b == a.id))
                .map_or(0.0, |j| j.clearance_mm);
            let lower = aabb_distance(&a.mesh, &b.mesh);
            let (interference, note) = if lower > 1e-6 {
                (Some(0.0), "Disjoint component bounds".to_string())
            } else if a.body.faces.len() + b.body.faces.len() > 2000 {
                (
                    None,
                    "Complex surfaces: interference requires manual inspection".into(),
                )
            } else {
                match brep::combine(
                    a.body.clone(),
                    b.body.clone(),
                    brep::Operation::Intersection,
                    1e-6,
                ) {
                    Ok(body) if body.roots.is_empty() => (
                        Some(0.0),
                        "No solid intersection; touching surfaces may remain".into(),
                    ),
                    Ok(body) => match super::tessellate(&body, 0.02) {
                        Ok(m) => (
                            Some(m.volume_mm3()),
                            "Solid intersection measured by tessellation".into(),
                        ),
                        Err(err) => (None, format!("Intersection is unresolved: {err}")),
                    },
                    Err(err) => (None, format!("Intersection is unresolved: {err:?}")),
                }
            };
            reports.push(PairReport {a:a.id,b:b.id,interference_mm3:interference,required_clearance_mm:required,clearance_estimate_mm:Some(lower),note:format!("{note}. Clearance is an AABB lower bound: a bound below the target requires closer inspection.")});
        }
    }
    reports
}
pub fn manifest(d: &RingDesign, e: &Evaluated) -> serde_json::Value {
    let parts=e.components.iter().map(|c|{
        let bytes=crate::stl::to_stl_binary(&c.mesh,&c.name);let mass=if c.settings.reference {None} else {crate::metal::find(&c.settings.material).map(|m|c.mesh.volume_mm3()*m.density/1000.0)};
        json!({"id":c.id,"name":c.name,"settings":c.settings,"units":"millimeter","stage":"nominal","volume_mm3":c.mesh.volume_mm3(),"grams":mass,"bounds":c.mesh.bounds().map(|(lo,hi)|[[lo.0,lo.1,lo.2],[hi.0,hi.1,hi.2]]),"geometry_fingerprint":format!("{:016x}",fingerprint(&bytes[80..])),"mesh":c.mesh.validate(),"local_wall":(!c.settings.reference).then(||super::measure::thickness(&c.mesh,c.settings.manufacturing.as_ref().map_or(d.draft.min_section_mm,|s|s.recipe.min_section_mm)))})
    }).collect::<Vec<_>>();
    json!({"format":"ringdesigner-assembly-v1","design":d.name,"nominal_size":d.size.display(),"units":"millimeter","components":parts,"pairs":inspect(d,e),"joints":d.cad.as_ref().map(|c|&c.joints),"features":e.features,"limits":["Components remain separate; total volume sums overlaps","Clearance bounds do not prove a seat fits a stone","A component recipe does not alter its nominal mesh; pattern files are separately identified"]})
}
fn threemf(e: &Evaluated, name: &str) -> Vec<u8> {
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\"><metadata name=\"Title\">{}</metadata><resources>",
        escape(name)
    );
    let mut ids = Vec::new();
    for (index, c) in e
        .components
        .iter()
        .filter(|c| !c.settings.reference)
        .enumerate()
    {
        let id = index + 1;
        ids.push(id);
        let _ = write!(
            xml,
            "<object id=\"{id}\" type=\"model\" name=\"{}\"><mesh><vertices>",
            escape(&c.name)
        );
        for p in &c.mesh.vertices {
            let _ = write!(xml, "<vertex x=\"{}\" y=\"{}\" z=\"{}\"/>", p.0, p.1, p.2);
        }
        xml.push_str("</vertices><triangles>");
        for f in &c.mesh.faces {
            let _ = write!(
                xml,
                "<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>",
                f[0], f[1], f[2]
            );
        }
        xml.push_str("</triangles></mesh></object>");
    }
    xml.push_str("</resources><build>");
    for id in ids {
        let _ = write!(xml, "<item objectid=\"{id}\"/>");
    }
    xml.push_str("</build></model>");
    crate::threemf::zip_store(&[
        crate::threemf::Entry {name:"[Content_Types].xml".into(),data:b"<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/></Types>".to_vec()},
        crate::threemf::Entry {name:"_rels/.rels".into(),data:b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/></Relationships>".to_vec()},
        crate::threemf::Entry {name:"3D/3dmodel.model".into(),data:xml.into_bytes()},
    ])
}
fn section_svg(mesh: &crate::Mesh, axis: usize, offset: f64) -> String {
    let lines = super::measure::section(mesh, axis, offset);
    if lines.is_empty() {
        return "<p>Section plane misses this part.</p>".into();
    }
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for p in lines.iter().flatten() {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let width = hi[0] - lo[0] + 4.0;
    let height = hi[1] - lo[1] + 7.0;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{height}mm\" viewBox=\"{} {} {width} {height}\">",
        lo[0] - 2.0,
        -hi[1] - 2.0
    );
    for line in lines {
        let _ = write!(
            svg,
            "<path d=\"M {} {} L {} {}\" fill=\"none\" stroke=\"#222\" stroke-width=\"0.1\"/>",
            line[0][0], -line[0][1], line[1][0], -line[1][1]
        );
    }
    let _ = write!(
        svg,
        "<text x=\"{}\" y=\"{}\" font-size=\"1.2\">{:.3} × {:.3} mm</text></svg>",
        lo[0],
        -lo[1] + 3.0,
        hi[0] - lo[0],
        hi[1] - lo[1]
    );
    svg
}
pub fn files(
    d: &RingDesign,
    lib: &AlphaLibrary,
    params: BuildParams,
) -> Result<mf::package::Package> {
    let evaluated = super::evaluate(d, lib, params)?;
    let manifest = manifest(d, &evaluated);
    let mut entries = Vec::new();
    let mut sheet = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>{}</title><style>body{{font:15px system-ui;margin:24px}}table{{border-collapse:collapse}}td,th{{padding:8px;border:1px solid #bbb}}</style><h1>{}</h1><p>Nominal assembly • {} • all dimensions millimeters</p><table><tr><th>Component</th><th>Material</th><th>Dimensions</th><th>Bench instructions</th></tr>",
        escape(&d.name),
        escape(&d.name),
        escape(&d.size.display())
    );
    for c in &evaluated.components {
        if !c.settings.reference {
            entries.push(crate::threemf::Entry {
                name: format!("component-{}-nominal.stl", c.id),
                data: crate::stl::to_stl_binary(&c.mesh, &c.name),
            });
            if let Some(setup) = &c.settings.manufacturing {
                let mut setup = setup.clone();
                setup.component = Some(c.id);
                let package = mf::package::files(d, lib, &setup, params, true)?;
                entries.extend(package.entries.into_iter().map(|e| crate::threemf::Entry {
                    name: format!("component-{}-pattern-review/{}", c.id, e.name),
                    data: e.data,
                }));
            }
        }
        let (lo, hi) = c.mesh.bounds().unwrap();
        write!(
            sheet,
            "<tr><td>#{} {}{}</td><td>{}</td><td>{:.3} × {:.3} × {:.3}</td><td>{}</td></tr>",
            c.id,
            escape(&c.name),
            if c.settings.reference {
                " (reference stone)"
            } else {
                ""
            },
            escape(&c.settings.material),
            hi.0 - lo.0,
            hi.1 - lo.1,
            hi.2 - lo.2,
            escape(&c.settings.bench_notes)
        )?;
    }
    sheet.push_str("</table><p>Reference stones are excluded from metal files. See manifest.json for component identities, measured intersections, clearance bounds, and joints.</p>");
    for pair in inspect(d, &evaluated) {
        write!(
            sheet,
            "<p>#{} / #{}: {}; intersection {:?} mm³</p>",
            pair.a,
            pair.b,
            escape(&pair.note),
            pair.interference_mm3
        )?;
    }
    if let Some(doc) = &d.cad {
        for joint in &doc.joints {
            write!(
                sheet,
                "<p>Joint #{} → #{}: {}; clearance {:.3} mm. {}</p>",
                joint.a,
                joint.b,
                escape(&joint.method),
                joint.clearance_mm,
                escape(&joint.notes)
            )?;
        }
    }
    for c in &evaluated.components {
        let (lo, hi) = c.mesh.bounds().unwrap();
        write!(
            sheet,
            "<h2>#{} {}</h2><p>XY cut at Z={:.3} mm; YZ cut at X={:.3} mm. Print at 100%.</p>{}{}",
            c.id,
            escape(&c.name),
            (lo.2 + hi.2) / 2.0,
            (lo.0 + hi.0) / 2.0,
            section_svg(&c.mesh, 2, ((lo.2 + hi.2) / 2.0) as f64),
            section_svg(&c.mesh, 0, ((lo.0 + hi.0) / 2.0) as f64)
        )?;
    }
    entries.push(crate::threemf::Entry {
        name: "assembly-nominal.3mf".into(),
        data: threemf(&evaluated, &d.name),
    });
    entries.push(crate::threemf::Entry {
        name: "assembly-nominal.step".into(),
        data: super::step::export(&evaluated, &d.name)?.into_bytes(),
    });
    entries.push(crate::threemf::Entry {
        name: "manifest.json".into(),
        data: serde_json::to_vec_pretty(&manifest)?,
    });
    entries.push(crate::threemf::Entry {
        name: "assembly-sheet.html".into(),
        data: sheet.into_bytes(),
    });
    let mut saved = d.clone();
    saved.embed_alphas(&mf::source_library(d, lib));
    entries.push(crate::threemf::Entry {
        name: "design.ring.json".into(),
        data: crate::library::design_json(&saved)?.into_bytes(),
    });
    Ok(mf::package::Package {
        report: manifest,
        entries,
    })
}
pub fn export(
    dir: &Path,
    d: &RingDesign,
    lib: &AlphaLibrary,
    params: BuildParams,
) -> Result<serde_json::Value> {
    ensure!(!dir.exists(), "Assembly destination already exists");
    let package = files(d, lib, params)?;
    package.write(dir)?;
    Ok(package.report)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::{Component, Document, Feature, Operation};
    #[test]
    fn reference_stones_are_excluded_and_actual_overlap_is_located() {
        let mut doc = Document::default();
        for id in 1..=2 {
            doc.append(Feature {
                id,
                name: format!("Part {id}"),
                enabled: true,
                operation: Operation::Box { size: [4.0; 3] },
                component: Component {
                    reference: id == 2,
                    ..Default::default()
                },
            })
            .unwrap();
        }
        let mut d = RingDesign::default();
        d.cad = Some(doc);
        let e =
            super::super::evaluate(&d, &AlphaLibrary::builtin(), BuildParams::default()).unwrap();
        let r = inspect(&d, &e);
        assert!(r[0].interference_mm3.unwrap() > 63.9);
        assert!((super::super::combined(&e, false).volume_mm3() - 64.0).abs() < 1e-6);
        let zip = threemf(&e, "Test");
        assert!(zip.windows(12).any(|w| w == b"objectid=\"1\""));
        assert!(!zip.windows(12).any(|w| w == b"objectid=\"2\""));
    }
}
