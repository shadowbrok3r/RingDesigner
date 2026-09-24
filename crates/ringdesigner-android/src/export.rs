//! Exports built off the UI thread: one thread per job, the file written
//! there, the share sheet opened on the UI thread once it lands. Export
//! jobs are never queued behind preview builds, so none is ever dropped as
//! stale.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::mesh::BuildParams;
use ringdesign_core::RingDesign;

use crate::ring::{EXPORT, METAL_TINT, PREVIEW};

/// A file handed to the share sheet and not yet answered: its name and what its export said.
#[derive(Clone, Debug, PartialEq)]
pub struct Sharing {
    pub name: String,
    pub said: String,
}

impl Sharing {
    pub fn new(name: impl Into<String>, said: impl Into<String>) -> Self {
        Self { name: name.into(), said: said.into() }
    }

    /// `lead` with what the export said after it.
    fn then_said(&self, lead: String) -> String {
        if self.said.is_empty() { lead } else { format!("{lead} · {}", self.said) }
    }

    /// The status line while the system copies the file.
    pub fn pending(&self) -> String {
        self.then_said(format!("sharing {}", self.name))
    }

    /// The status line once the system answers, the outcome first: the folder the copy landed in with the sheet open, or why nothing was shared.
    pub fn answered(&self, outcome: &Result<String, String>) -> String {
        match outcome {
            Ok(folder) => self.then_said(format!("{} saved to {folder} and handed to the share sheet", self.name)),
            Err(why) => self.then_said(format!("{} not shared: {why}", self.name)),
        }
    }
}

/// `n` with its thousands grouped.
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// What a sized STEP file's band holds: its facets, and how near every vertex of the build stands to them.
fn band_words(band: Option<&ringdesign_core::cad::step::BandFacets>) -> String {
    match band {
        Some(b) if b.written < b.built => format!("band {} facets from {}, every vertex within {:.3} mm", grouped(b.written), grouped(b.built), b.deviation_mm),
        Some(b) => format!("band {} facets as built", grouped(b.written)),
        None => "no band".into(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportKind {
    Stl,
    ThreeMf,
    Glb,
    Sheet,
    Render,
    Turntable,
    StoneMap,
    Step,
}

impl ExportKind {
    pub fn ext(self) -> &'static str {
        match self {
            ExportKind::StoneMap => "_stones.svg",
            ExportKind::Step => ".step",
            ExportKind::Stl => ".stl",
            ExportKind::ThreeMf => ".3mf",
            ExportKind::Glb => ".glb",
            ExportKind::Sheet => "_sheet.html",
            ExportKind::Render => ".png",
            ExportKind::Turntable => ".gif",
        }
    }

    /// Explicit types: MediaProvider renames a file whose extension disagrees
    /// with its type, and the generic table has no `stl` entry.
    pub fn mime(self) -> &'static str {
        match self {
            ExportKind::StoneMap => "image/svg+xml",
            ExportKind::Step => "model/step",
            ExportKind::Stl => "model/stl",
            ExportKind::ThreeMf => "model/3mf",
            ExportKind::Glb => "model/gltf-binary",
            ExportKind::Sheet => "text/html",
            ExportKind::Render => "image/png",
            ExportKind::Turntable => "image/gif",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ExportKind::StoneMap => "stone map",
            ExportKind::Step => "STEP",
            ExportKind::Stl => "STL",
            ExportKind::ThreeMf => "3MF",
            ExportKind::Glb => "GLB",
            ExportKind::Sheet => "casting sheet",
            ExportKind::Render => "render",
            ExportKind::Turntable => "turntable",
        }
    }

    /// The grid a kind is built on: the turntable's frames on the preview grid, every file on the export grid, STEP's band collapsed from it.
    pub fn params(self) -> BuildParams {
        match self {
            ExportKind::Turntable => PREVIEW,
            _ => EXPORT,
        }
    }
}

pub struct ExportJob {
    pub kind: ExportKind,
    pub path: PathBuf,
    pub design: RingDesign,
    pub lib: Arc<AlphaLibrary>,
    pub params: BuildParams,
    /// Patternmaker's shrink as `(percent, metal name)`; the file is cut
    /// oversize and named as a pattern.
    pub shrink: Option<(f64, String)>,
    /// The app's name and version, on the casting sheet.
    pub generator: String,
}

pub struct ExportDone {
    pub kind: ExportKind,
    pub path: PathBuf,
    pub name: String,
    pub status: String,
    pub ok: bool,
}

/// Builds and writes the file; the caller shares it.
pub fn run(job: ExportJob) -> ExportDone {
    let name = job
        .path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("ring{}", job.kind.ext()));
    let result = write(&job).map_err(|e| e.to_string());
    let (ok, status) = match result {
        Ok(s) => (true, s),
        Err(e) => (false, format!("{} failed: {e}", job.kind.label())),
    };
    ExportDone { kind: job.kind, path: job.path, name, status, ok }
}

fn write(job: &ExportJob) -> Result<String, Box<dyn std::error::Error>> {
    use ringdesign_core::metal;
    if let Some(dir) = job.path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // STEP is the finished ring at its nominal size, kernel parts exact and the rest faceted, the band's facets collapsed within the tolerance.
    if job.kind == ExportKind::Step {
        use ringdesign_core::cad::step;
        let sized = step::ring_sized(&job.design, &job.lib, job.params, step::BAND_TOLERANCE_MM, &job.design.name)?;
        let text = &sized.text;
        let exact = text.matches("=MANIFOLD_SOLID_BREP(").count() + text.matches("=BREP_WITH_VOIDS(").count();
        let faceted = text.matches("=FACETED_BREP(").count();
        ringdesign_core::library::write_atomic(&job.path, text.as_bytes())?;
        let s = if exact + faceted == 1 { "" } else { "s" };
        return Ok(format!("STEP · {exact} exact and {faceted} faceted solid{s} · {} · {:.1} MB", band_words(sized.band.as_ref()), text.len() as f64 / 1048576.0));
    }
    // Mesh files are patterns: under sand the made settings are left out and each seat carries its drill
    // mark. Everything else shows the finished ring.
    let out = if matches!(job.kind, ExportKind::Stl | ExportKind::ThreeMf) {
        ringdesign_core::mesh::try_build_pattern(&job.design, &job.lib, job.params)?
    } else {
        ringdesign_core::mesh::try_build(&job.design, &job.lib, job.params)?
    };
    let mb = |bytes: usize| bytes as f64 / 1048576.0;
    Ok(match job.kind {
        ExportKind::Stl | ExportKind::ThreeMf => {
            let (mesh, name) = match &job.shrink {
                Some((pct, metal)) => (
                    out.mesh.scaled(metal::pattern_scale(*pct)),
                    format!("{} [pattern +{pct:.1}% for {metal}]", job.design.name),
                ),
                None => (out.mesh.clone(), job.design.name.clone()),
            };
            let bytes = if job.kind == ExportKind::ThreeMf {
                let k = job.shrink.as_ref().map_or(1.0, |(pct, _)| metal::pattern_scale(*pct));
                let objects: Vec<ringdesign_core::threemf::Object> = ringdesign_core::threemf::objects(&out, &name).iter().map(|o| o.scaled(k)).collect();
                ringdesign_core::threemf::write_3mf_objects(&job.path, &objects, &name, &job.design.size.display())?
            } else {
                ringdesign_core::stl::write_stl(&job.path, &mesh, &name)?
            };
            format!("{} tris · {:.1} MB", out.report.validation.triangle_count, mb(bytes))
        }
        ExportKind::Glb => {
            let bytes = ringdesign_core::gltf::write_glb(&job.path, &out.mesh, &job.design.name, METAL_TINT)?;
            format!("GLB · {:.1} MB", mb(bytes))
        }
        ExportKind::Sheet => {
            let field = ringdesign_core::castability::judged_field_report(
                &job.design,
                &job.lib,
                &job.design.draft,
                160,
                112,
                Some(&out),
            );
            let stones = ringdesign_core::stones::report(&job.design, field.parting_z_mm);
            let dfm = ringdesign_core::dfm::findings_in(&job.design, &job.lib);
            let page = ringdesign_core::spec::html(&job.design, &out.report, &field, stones.as_ref(), &dfm, &job.generator);
            std::fs::write(&job.path, page)?;
            "casting sheet".into()
        }
        ExportKind::Render => {
            ringdesign_core::render::write_png(&job.path, &out.mesh, 0.55, 1.12, 1280, METAL_TINT)?;
            "render".into()
        }
        ExportKind::Turntable => {
            ringdesign_core::render::write_turntable_gif(&job.path, &out.mesh, 36, 480, METAL_TINT)?;
            "turntable".into()
        }
        ExportKind::StoneMap => {
            let field = ringdesign_core::castability::analyze_field(&job.design, &job.lib, &job.design.draft, 96, 64);
            let stones = ringdesign_core::stones::report(&job.design, field.parting_z_mm);
            ringdesign_core::stonemap::write_stone_map_svg(&job.path, &job.design, stones.as_ref())?;
            "stone map".into()
        }
        ExportKind::Step => unreachable!("written above, before any mesh is built"),
    })
}

/// Runs the job on its own thread; the receiver yields once.
pub fn spawn(job: ExportJob, ctx: egui::Context) -> Receiver<ExportDone> {
    let (tx, rx) = channel();
    let name = format!("ring-export-{}", job.kind.label());
    std::thread::Builder::new()
        .name(name)
        .spawn(move || {
            let _ = tx.send(run(job));
            ctx.request_repaint();
        })
        .expect("spawn export thread");
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_name_their_files_and_types() {
        let all = [ExportKind::Stl, ExportKind::ThreeMf, ExportKind::Glb, ExportKind::Sheet, ExportKind::Render, ExportKind::Turntable, ExportKind::StoneMap, ExportKind::Step];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.ext(), b.ext());
                assert_ne!(a.mime(), b.mime());
            }
        }
        assert_eq!(ExportKind::Stl.mime(), "model/stl");
        assert_eq!(ExportKind::Sheet.ext(), "_sheet.html");
    }

    #[test]
    fn an_export_writes_its_file_and_reports() {
        let dir = std::env::temp_dir().join(format!("rd-export-{}", std::process::id()));
        let lib = Arc::new(AlphaLibrary::builtin());
        let small = BuildParams { theta_steps: 96, profile_steps: 48, ..Default::default() };
        let job = |kind: ExportKind, shrink: Option<(f64, String)>| ExportJob {
            kind,
            path: dir.join(format!("ring{}", kind.ext())),
            design: RingDesign::default(),
            lib: lib.clone(),
            params: small,
            shrink,
            generator: "test".into(),
        };
        let stl = run(job(ExportKind::Stl, None));
        assert!(stl.ok, "{}", stl.status);
        assert!(std::fs::metadata(&stl.path).unwrap().len() > 84);
        assert_eq!(stl.name, "ring.stl");
        let pattern = run(job(ExportKind::Stl, Some((1.9, "Silver 925".into()))));
        assert!(pattern.ok);
        assert!(std::fs::read(&pattern.path).unwrap().len() >= std::fs::read(&stl.path).unwrap().len());
        let tmf = run(job(ExportKind::ThreeMf, None));
        assert!(tmf.ok && std::fs::read(&tmf.path).unwrap().starts_with(b"PK"));
        let glb = run(job(ExportKind::Glb, None));
        assert!(glb.ok && std::fs::read(&glb.path).unwrap().starts_with(b"glTF"));
        let sheet = run(job(ExportKind::Sheet, None));
        assert!(sheet.ok);
        assert!(std::fs::read_to_string(&sheet.path).unwrap().to_lowercase().contains("<html"));
        let bad = run(ExportJob { path: PathBuf::from("/proc/no/such/dir/ring.stl"), ..job(ExportKind::Stl, None) });
        assert!(!bad.ok && bad.status.starts_with("STL failed"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_step_export_carries_the_band_faceted_and_a_joined_kernel_part_exact_at_nominal_size() {
        use ringdesign_core::cad::step;
        let dir = std::env::temp_dir().join(format!("rd-step-{}", std::process::id()));
        let mut design = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let (edits, _) = ringdesign_workbench::touch::parts::part_here(&design, "Cylinder", 90.0, 0.0).unwrap();
        design = ringdesign_workbench::touch::prepare(&design, &edits, None).unwrap().unwrap().design;
        let job = ExportJob {
            kind: ExportKind::Step,
            path: dir.join(format!("ring{}", ExportKind::Step.ext())),
            design,
            lib: Arc::new(AlphaLibrary::builtin()),
            params: BuildParams { theta_steps: 96, profile_steps: 48, ..Default::default() },
            shrink: Some((1.9, "Silver 925".into())),
            generator: "test".into(),
        };
        let done = run(job);
        assert!(done.ok, "{}", done.status);
        assert_eq!(done.name, "ring.step");
        assert!(done.status.starts_with("STEP · 1 exact and 1 faceted solids · ") && done.status.ends_with(" MB"), "{}", done.status);
        let text = std::fs::read_to_string(&done.path).unwrap();
        assert!(text.starts_with("ISO-10303-21;"));
        // Read back, the band is one faceted solid and the post the exact one beside it.
        let solids = step::read_solids(&text).unwrap();
        assert_eq!(solids.iter().map(|s| s.faceted).collect::<Vec<_>>(), [false, true]);
        let (meshes, notes) = step::faceted_meshes(&text, "ring.step").unwrap();
        assert!(meshes.len() == 1 && meshes[0].validate().watertight && notes.len() == 1, "{notes:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_step_is_collapsed_from_the_export_grid_and_says_its_band_and_size() {
        let grid = |p: BuildParams| (p.theta_steps, p.profile_steps);
        assert_eq!(grid(ExportKind::Step.params()), grid(EXPORT));
        assert_eq!(grid(ExportKind::Turntable.params()), grid(PREVIEW));
        assert_eq!(grid(ExportKind::Stl.params()), grid(EXPORT));
        // The Court band with a post as the phone shares it.
        let dir = std::env::temp_dir().join(format!("rd-step-sized-{}", std::process::id()));
        let mut design = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let (edits, _) = ringdesign_workbench::touch::parts::part_here(&design, "Cylinder", 90.0, 0.0).unwrap();
        design = ringdesign_workbench::touch::prepare(&design, &edits, None).unwrap().unwrap().design;
        let started = std::time::Instant::now();
        let done = run(ExportJob {
            kind: ExportKind::Step,
            path: dir.join("court.step"),
            design,
            lib: Arc::new(AlphaLibrary::builtin()),
            params: ExportKind::Step.params(),
            shrink: None,
            generator: "test".into(),
        });
        let bytes = std::fs::metadata(&done.path).map_or(0, |m| m.len());
        eprintln!("{} ({bytes} bytes, {:.1} s)", done.status, started.elapsed().as_secs_f64());
        assert!(done.ok, "{}", done.status);
        // Measured: 5,922 facets from 655,360 and 1.8 MB, where the preview grid's uncollapsed band wrote 34.9 MB and the export grid's 217 MB.
        assert!(done.status.starts_with("STEP · 1 exact and 1 faceted solids · band ") && done.status.contains(" facets from 655,360, every vertex within 0.010 mm · "), "{}", done.status);
        let facets: usize = done.status.split("band ").nth(1).and_then(|s| s.split(' ').next()).map(|s| s.replace(',', "")).and_then(|s| s.parse().ok()).unwrap();
        assert!((5_000..8_000).contains(&facets), "{facets}");
        assert!((1_000_000..2_500_000).contains(&bytes), "{bytes}");
        assert!(done.status.ends_with(&format!(" · {:.1} MB", bytes as f64 / 1048576.0)), "{}", done.status);
        // The file reads back: the post exact, the band one closed faceted solid.
        let text = std::fs::read_to_string(&done.path).unwrap();
        let (meshes, _) = ringdesign_core::cad::step::faceted_meshes(&text, "court.step").unwrap();
        assert!(meshes.len() == 1 && meshes[0].validate().watertight && meshes[0].faces.len() == facets);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_share_says_where_its_copy_landed_or_why_nothing_was_shared() {
        // The outcome leads the line, the export's words after it.
        let step = Sharing::new("court.step", "STEP · 1.2 MB");
        assert_eq!(step.pending(), "sharing court.step · STEP · 1.2 MB");
        assert_eq!(step.answered(&Ok("Download".into())), "court.step saved to Download and handed to the share sheet · STEP · 1.2 MB");
        assert_eq!(step.answered(&Err("cannot read court.step: No such file".into())), "court.step not shared: cannot read court.step: No such file · STEP · 1.2 MB");
        // A design shared from Files has no export to report.
        let design = Sharing::new("Court.ring.json", "");
        assert_eq!(design.pending(), "sharing Court.ring.json");
        assert_eq!(design.answered(&Ok("Download".into())), "Court.ring.json saved to Download and handed to the share sheet");
        assert_eq!(grouped(655_360), "655,360");
        assert_eq!(grouped(999), "999");
    }
}
