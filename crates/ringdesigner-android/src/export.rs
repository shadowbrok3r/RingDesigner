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

/// A file handed to the share sheet and not yet answered: its name, what its export said, and whether the Workshop asked.
#[derive(Clone, Debug, PartialEq)]
pub struct Sharing {
    pub name: String,
    pub said: String,
    pub workshop: bool,
}

impl Sharing {
    pub fn new(name: impl Into<String>, said: impl Into<String>) -> Self {
        Self { name: name.into(), said: said.into(), workshop: false }
    }

    /// The same share, asked for by the Workshop.
    pub fn from_workshop(self) -> Self {
        Self { workshop: true, ..self }
    }

    /// `lead` with what the export said after it.
    fn then_said(&self, lead: String) -> String {
        if self.said.is_empty() { lead } else { format!("{lead} · {}", self.said) }
    }

    /// The status line while the system copies the file.
    pub fn pending(&self) -> String {
        self.then_said(format!("sharing {}", self.name))
    }

    /// The status line once the system answers: the folder the copy landed in, or why nothing was shared.
    pub fn answered(&self, outcome: &Result<String, String>) -> String {
        match outcome {
            Ok(folder) => self.then_said(format!("{} saved to {folder} and handed to the share sheet", self.name)),
            Err(why) => self.then_said(format!("{} not shared: {why}", self.name)),
        }
    }
}

/// A file waiting for the share sheet: where it is, its type, and how it is named on the status line.
#[derive(Clone, Debug, PartialEq)]
pub struct Share {
    pub path: PathBuf,
    pub mime: String,
    pub sharing: Sharing,
}

/// What the system said of one share.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    /// The outcome said against its own file.
    pub line: String,
    /// The status line: every answer of this run of shares, first to last.
    pub status: String,
    pub ok: bool,
    pub workshop: bool,
}

/// Files for the share sheet, handed over one at a time, each outcome said against the file it answers.
#[derive(Debug, Default)]
pub struct Shares {
    waiting: std::collections::VecDeque<Share>,
    handed: Option<Sharing>,
    answers: Vec<String>,
}

impl Shares {
    pub fn push(&mut self, share: Share) {
        self.waiting.push_back(share);
    }

    /// Whether a share waits for the sheet or for its outcome.
    pub fn busy(&self) -> bool {
        self.handed.is_some() || !self.waiting.is_empty()
    }

    /// The next file to hand the share sheet once none awaits its outcome, with the status line to show.
    pub fn next(&mut self) -> Option<(Share, String)> {
        if self.handed.is_some() {
            return None;
        }
        let share = self.waiting.pop_front()?;
        self.handed = Some(share.sharing.clone());
        let status = self.answers.iter().cloned().chain([share.sharing.pending()]).collect::<Vec<_>>().join("; ");
        Some((share, status))
    }

    /// `outcome` said against the share it answers; one with no share handed is said of "the file".
    pub fn answer(&mut self, outcome: &Result<String, String>) -> Answer {
        let sharing = self.handed.take().unwrap_or_else(|| Sharing::new("the file", ""));
        let line = sharing.answered(outcome);
        self.answers.push(line.clone());
        let status = self.answers.join("; ");
        if self.waiting.is_empty() {
            self.answers.clear();
        }
        Answer { line, status, ok: outcome.is_ok(), workshop: sharing.workshop }
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
    /// A STEP file's band as written.
    pub band: Option<ringdesign_core::cad::step::BandFacets>,
}

/// Builds and writes the file; the caller shares it.
pub fn run(job: ExportJob) -> ExportDone {
    let name = job
        .path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("ring{}", job.kind.ext()));
    let (ok, status, band) = match write(&job) {
        Ok((s, band)) => (true, s, band),
        Err(e) => (false, format!("{} failed: {e}", job.kind.label()), None),
    };
    ExportDone { kind: job.kind, path: job.path, name, status, ok, band }
}

fn write(job: &ExportJob) -> Result<(String, Option<ringdesign_core::cad::step::BandFacets>), Box<dyn std::error::Error>> {
    use ringdesign_core::metal;
    if let Some(dir) = job.path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // STEP is the finished ring at its nominal size, kernel parts exact and the rest faceted, the band's facets collapsed within the tolerance.
    if job.kind == ExportKind::Step {
        use ringdesign_core::cad::step;
        let sized = step::ring_sized(&job.design, &job.lib, job.params, step::BAND_TOLERANCE_MM, &job.design.name)?;
        ringdesign_core::library::write_atomic(&job.path, sized.text.as_bytes())?;
        let (exact, faceted) = sized.solids();
        let s = if exact + faceted == 1 { "" } else { "s" };
        return Ok((format!("STEP · {exact} exact and {faceted} faceted solid{s} · {} · {}", step::band_words(sized.band.as_ref()), step::size_words(sized.text.len())), sized.band));
    }
    // Mesh files are patterns: under sand the made settings are left out and each seat carries its drill
    // mark. Everything else shows the finished ring.
    let out = if matches!(job.kind, ExportKind::Stl | ExportKind::ThreeMf) {
        ringdesign_core::mesh::try_build_pattern(&job.design, &job.lib, job.params)?
    } else {
        ringdesign_core::mesh::try_build(&job.design, &job.lib, job.params)?
    };
    let mb = |bytes: usize| bytes as f64 / 1048576.0;
    let status = match job.kind {
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
    };
    Ok((status, None))
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
        let text = std::fs::read_to_string(&done.path).unwrap();
        assert!(done.status.starts_with("STEP · 1 exact and 1 faceted solids · ") && done.status.ends_with(&step::size_words(text.len())), "{}", done.status);
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
        use ringdesign_core::cad::step;
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
        let band = done.band.unwrap();
        assert!(band.deviation_mm <= step::BAND_TOLERANCE_MM, "{band:?}");
        assert_eq!(band.built, 655_360);
        assert!((5_000..8_000).contains(&band.written), "{band:?}");
        assert!((1_000_000..2_500_000).contains(&bytes), "{bytes}");
        assert_eq!(done.status, format!("STEP · 1 exact and 1 faceted solids · {} · {}", step::band_words(Some(&band)), step::size_words(bytes as usize)));
        // The file reads back: the post exact, the band one closed faceted solid.
        let text = std::fs::read_to_string(&done.path).unwrap();
        let (meshes, _) = ringdesign_core::cad::step::faceted_meshes(&text, "court.step").unwrap();
        assert!(meshes.len() == 1 && meshes[0].validate().watertight && meshes[0].faces.len() == band.written);
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
        assert_eq!(ringdesign_core::cad::step::grouped(655_360), "655,360");
        assert_eq!(ringdesign_core::cad::step::grouped(999), "999");
    }

    fn share(name: &str, said: &str) -> Share {
        Share { path: PathBuf::from("/exports").join(name), mime: "model/stl".into(), sharing: Sharing::new(name, said) }
    }

    #[test]
    fn shares_go_to_the_sheet_one_at_a_time_and_each_outcome_names_its_own_file() {
        let mut shares = Shares::default();
        assert!(!shares.busy() && shares.next().is_none());
        // Two exports land in the same frame: only the first is handed over.
        shares.push(share("a.stl", "STL · 2.0 MB"));
        shares.push(share("b.step", "STEP · 1.8 MB"));
        let (a, status) = shares.next().unwrap();
        assert_eq!((a.path.to_str(), status.as_str()), (Some("/exports/a.stl"), "sharing a.stl · STL · 2.0 MB"));
        assert!(shares.next().is_none(), "b waits for a's outcome");
        // a's outcome is taken before b is handed over, and names a.
        let told = shares.answer(&Ok("Download".into()));
        assert_eq!(told.line, "a.stl saved to Download and handed to the share sheet · STL · 2.0 MB");
        assert_eq!(told.status, told.line);
        assert!(told.ok && !told.workshop);
        // b's pending line keeps a's outcome in front of it, and b's answer both.
        let (b, status) = shares.next().unwrap();
        assert_eq!(b.sharing.name, "b.step");
        assert_eq!(status, "a.stl saved to Download and handed to the share sheet · STL · 2.0 MB; sharing b.step · STEP · 1.8 MB");
        let told = shares.answer(&Err("no Java environment to share from".into()));
        assert_eq!(told.line, "b.step not shared: no Java environment to share from · STEP · 1.8 MB");
        assert_eq!(told.status, "a.stl saved to Download and handed to the share sheet · STL · 2.0 MB; b.step not shared: no Java environment to share from · STEP · 1.8 MB");
        assert!(!told.ok && !shares.busy());
        // The run is over: the next share starts a fresh line.
        shares.push(Share { sharing: Sharing::new("c.glb", "").from_workshop(), ..share("c.glb", "") });
        assert_eq!(shares.next().unwrap().1, "sharing c.glb");
        let told = shares.answer(&Ok("Download".into()));
        assert_eq!((told.status.as_str(), told.workshop), ("c.glb saved to Download and handed to the share sheet", true));
        // An outcome with no share handed over is still said.
        assert_eq!(shares.answer(&Ok("Pictures".into())).line, "the file saved to Pictures and handed to the share sheet");
    }
}
