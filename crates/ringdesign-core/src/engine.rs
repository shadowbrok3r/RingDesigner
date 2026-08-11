//! Transport-agnostic design state.
//!
//! Holds the design, the alpha library, and the most recent build. Every
//! mutation bumps a generation counter, so a GUI sharing this engine with an
//! MCP server can notice edits made from the other side and refresh.

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::alpha::AlphaLibrary;
use crate::castability::{self, CastReport, Section};
use crate::mesh::{self, BuildParams, BuildResult, Mesh, Report};
use crate::{RingDesign, library, stl};

pub struct DesignEngine {
    design: RingDesign,
    lib: Arc<AlphaLibrary>,
    build: Option<Arc<BuildResult>>,
    cast: Option<CastReport>,
    generation: u64,
}

pub type SharedEngine = Arc<Mutex<DesignEngine>>;

impl Default for DesignEngine {
    fn default() -> Self {
        Self::new(AlphaLibrary::builtin())
    }
}

impl DesignEngine {
    pub fn new(lib: AlphaLibrary) -> Self {
        Self {
            design: RingDesign::default(),
            lib: Arc::new(lib),
            build: None,
            cast: None,
            generation: 0,
        }
    }

    /// Built-in patterns plus every alpha found in the standard directories.
    pub fn with_disk_library() -> Self {
        let mut lib = AlphaLibrary::builtin();
        for dir in library::alpha_dirs() {
            match lib.load_dir(&dir) {
                Ok(n) if n > 0 => log::info!("loaded {n} alphas from {}", dir.display()),
                Ok(_) => {}
                Err(e) => log::warn!("alpha library {}: {e}", dir.display()),
            }
        }
        Self::new(lib)
    }

    pub fn shared(lib: AlphaLibrary) -> SharedEngine {
        Arc::new(Mutex::new(Self::new(lib)))
    }

    pub fn shared_with_disk_library() -> SharedEngine {
        Arc::new(Mutex::new(Self::with_disk_library()))
    }

    // --- State -------------------------------------------------------------

    pub fn design(&self) -> &RingDesign {
        &self.design
    }

    /// Mutable access. Bumps the generation and drops the stale build, so the
    /// next report or export rebuilds.
    pub fn design_mut(&mut self) -> &mut RingDesign {
        self.invalidate();
        &mut self.design
    }

    pub fn set_design(&mut self, design: RingDesign) {
        self.design = design;
        self.invalidate();
    }

    pub fn library(&self) -> &AlphaLibrary {
        &self.lib
    }

    pub fn library_arc(&self) -> Arc<AlphaLibrary> {
        self.lib.clone()
    }

    pub fn library_mut(&mut self) -> &mut AlphaLibrary {
        self.invalidate();
        Arc::make_mut(&mut self.lib)
    }

    /// Increments on every mutation. A GUI polls this to detect outside edits.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn is_built(&self) -> bool {
        self.build.is_some()
    }

    fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.build = None;
        self.cast = None;
    }

    // --- Build -------------------------------------------------------------

    /// Rebuild from the current design. `params` defaults to `design.build`.
    pub fn build(&mut self, params: Option<BuildParams>) -> Report {
        let params = params.unwrap_or(self.design.build);
        let built = mesh::build(&self.design, &self.lib, params);
        let report = built.report.clone();
        self.cast = Some(castability::analyze(
            &built.mesh,
            &self.design.draft,
            self.design.inner_radius_mm(),
        ));
        self.build = Some(Arc::new(built));
        self.generation = self.generation.wrapping_add(1);
        report
    }

    /// The current build, building first if the design has changed since.
    pub fn ensure_built(&mut self) -> Arc<BuildResult> {
        if self.build.is_none() {
            self.build(None);
        }
        self.build.clone().expect("build populated above")
    }

    pub fn report(&mut self) -> Report {
        self.ensure_built().report.clone()
    }

    pub fn castability(&mut self) -> CastReport {
        self.ensure_built();
        self.cast.clone().unwrap_or_else(|| {
            let built = self.ensure_built();
            castability::analyze(&built.mesh, &self.design.draft, self.design.inner_radius_mm())
        })
    }

    pub fn mesh(&mut self) -> Arc<BuildResult> {
        self.ensure_built()
    }

    /// A displaced cross-section at a ring angle. Does not need a built mesh.
    pub fn section(&self, theta_deg: f64, steps: usize) -> Section {
        castability::section_at(&self.design, &self.lib, theta_deg, steps)
    }

    // --- Files -------------------------------------------------------------

    pub fn export_stl(&mut self, path: impl AsRef<Path>) -> anyhow::Result<usize> {
        let built = self.ensure_built();
        let name = self.design.name.clone();
        stl::write_stl(path, &built.mesh, &name)
    }

    pub fn export_obj(&mut self, path: impl AsRef<Path>) -> anyhow::Result<usize> {
        let built = self.ensure_built();
        let name = self.design.name.clone();
        stl::write_obj(path, &built.mesh, &name)
    }

    pub fn save_design(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        library::save_design_embedded(path, &self.design, &self.lib)
    }

    pub fn load_design(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let design = library::load_design(path)?;
        design.unpack_embedded(Arc::make_mut(&mut self.lib));
        design.bake_drawn(Arc::make_mut(&mut self.lib));
        design.bake_texts(Arc::make_mut(&mut self.lib));
        self.set_design(design);
        Ok(())
    }

    /// Borrow the last built mesh without rebuilding, if one exists.
    pub fn peek_mesh(&self) -> Option<&Mesh> {
        self.build.as_ref().map(|b| &b.mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_bumps_the_generation_and_drops_the_build() {
        let mut e = DesignEngine::new(AlphaLibrary::builtin());
        e.design_mut().build = BuildParams { theta_steps: 96, profile_steps: 64, ..Default::default() };
        let g0 = e.generation();
        let r = e.build(None);
        assert!(r.validation.watertight);
        assert!(e.is_built());
        assert!(e.generation() > g0);

        let g1 = e.generation();
        e.design_mut().profile.width_mm = 8.0;
        assert!(e.generation() > g1, "generation did not advance");
        assert!(!e.is_built(), "stale build survived a design change");
    }

    #[test]
    fn ensure_built_is_idempotent() {
        let mut e = DesignEngine::new(AlphaLibrary::builtin());
        e.design_mut().build = BuildParams { theta_steps: 96, profile_steps: 64, ..Default::default() };
        let a = e.ensure_built();
        let g = e.generation();
        let b = e.ensure_built();
        assert_eq!(e.generation(), g, "a no-op rebuild bumped the generation");
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn castability_and_section_agree_on_a_plain_band() {
        let mut e = DesignEngine::new(AlphaLibrary::builtin());
        e.design_mut().build = BuildParams { theta_steps: 128, profile_steps: 96, ..Default::default() };
        let cast = e.castability();
        assert_eq!(cast.undercut, 0, "a plain domed band should not undercut");
        let sec = e.section(90.0, 128);
        assert!(!sec.points.is_empty());
        assert_eq!(sec.undercut_count, 0);
    }

    #[test]
    fn exports_round_trip_through_the_engine() {
        let dir = std::env::temp_dir().join("ringdesign_engine_test");
        std::fs::create_dir_all(&dir).unwrap();
        let mut e = DesignEngine::new(AlphaLibrary::builtin());
        e.design_mut().build = BuildParams { theta_steps: 64, profile_steps: 48, ..Default::default() };

        let stl = dir.join("r.stl");
        assert!(e.export_stl(&stl).unwrap() > 84);

        let json = dir.join("r.json");
        e.design_mut().name = "Engine test".into();
        e.save_design(&json).unwrap();
        let mut other = DesignEngine::new(AlphaLibrary::builtin());
        other.load_design(&json).unwrap();
        assert_eq!(other.design().name, "Engine test");

        let _ = std::fs::remove_file(&stl);
        let _ = std::fs::remove_file(&json);
    }
}
