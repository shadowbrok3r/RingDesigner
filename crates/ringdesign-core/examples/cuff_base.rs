//! Base body for a cuff after Meshy's "Trident Crest of the Golden Knight": a flat band, a pointed crest, a strap behind.
//! cargo run --release -p ringdesign-core --example cuff_base -- [OUT_DIR] [--bore MM] [--gap MM] [--theta N] [--rows N]
use anyhow::{Context, Result, bail, ensure};
use ringdesign_core::{
    AlphaLibrary, BandProfile, BuildParams, BuildResult, DraftSettings, ProfileStyle, RingDesign,
    cad::{self, Attach, Component, ComponentRole, Document, Feature, Operation, SurfaceKind, stored},
    castability::CastProcess,
    csg, library, mesh,
    profile::ShankMod,
    render, stl,
};
use serde_json::json;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const NAME: &str = "Trident cuff base";
const SILVER_G_PER_MM3: f64 = 0.01036;
const GOLD14_G_PER_MM3: f64 = 0.01307;
const CUFF: u64 = 1;
const OPENING_TOOL: u64 = 2;
/// Ring angle of the crest's centre.
const CREST_THETA: f64 = 90.0;

/// The cuff's dimensions in mm; angles in degrees from the crest's centre.
#[derive(Clone, Copy, Debug)]
struct Cuff {
    /// Inside diameter of the band.
    bore_mm: f64,
    /// Axial width of the band.
    band_mm: f64,
    /// Radial thickness of the band and strap.
    thick_mm: f64,
    /// Fillet on the section's outer corners.
    edge_mm: f64,
    /// Axial height at the crest's tip.
    crest_mm: f64,
    /// Angle from the tip at which the crest's flank would meet the band unrounded.
    crest_deg: f64,
    /// Exponent of the flank's fall from tip to band.
    crest_pow: f64,
    /// Rounding of the tip, degrees.
    tip_deg: f64,
    /// Extra thickness at the crest's tip, outward.
    crest_rise_mm: f64,
    /// Axial width of the strap.
    strap_mm: f64,
    /// How far the strap sits inside the band, bore and outer face alike.
    strap_inset_mm: f64,
    /// Arc length past the wall's foot over which the strap steps in.
    inset_ramp_mm: f64,
    /// Angle from the crest's centre to the top of each wall down to the strap.
    wall_deg: f64,
    /// Radius of the band's edge rounding into each wall.
    corner_mm: f64,
    /// Arc length each wall leans over on the outer face.
    wall_mm: f64,
    /// Width of the open cuff's opening.
    gap_mm: f64,
}

impl Default for Cuff {
    fn default() -> Self {
        Self {
            bore_mm: 63.5,
            band_mm: 32.45,
            thick_mm: 3.9,
            edge_mm: 0.45,
            crest_mm: 50.0,
            crest_deg: 59.25,
            crest_pow: 1.58,
            tip_deg: 1.25,
            crest_rise_mm: 0.4,
            strap_mm: 15.8,
            strap_inset_mm: 0.5,
            inset_ramp_mm: 1.0,
            wall_deg: 156.0,
            corner_mm: 2.5,
            wall_mm: 0.25,
            gap_mm: 31.0,
        }
    }
}

/// One section of the sweep: axial width, radial thickness and bore radius.
#[derive(Clone, Copy, Debug)]
struct Section {
    width: f64,
    thick: f64,
    inner_r: f64,
}

impl Cuff {
    fn inner_r(&self) -> f64 {
        self.bore_mm * 0.5
    }

    fn outer_r(&self) -> f64 {
        self.inner_r() + self.thick_mm
    }

    /// The band's section: flat outer face, filleted outer corners, no crown, draft or comfort fit.
    fn profile(&self) -> BandProfile {
        let mut p = BandProfile { width_mm: self.band_mm, thickness_mm: self.thick_mm, ..BandProfile::default() };
        p.apply_style(ProfileStyle::Flat);
        p.crown_mm = 0.0;
        p.edge_round_mm = self.edge_mm;
        p.comfort_fit_mm = 0.0;
        p.side_draft_deg = 0.0;
        p
    }

    /// Share of the crest at `phi`: 1 at the tip, 0 on the band.
    fn crest_share(&self, phi: f64) -> f64 {
        let soft = (phi * phi + self.tip_deg * self.tip_deg).sqrt() - self.tip_deg;
        (1.0 - soft / self.crest_deg).max(0.0).powf(self.crest_pow)
    }

    /// Half-width of the band's edge at `phi`: a rounded corner, a steep wall, then the strap.
    fn edge_half(&self, phi: f64) -> f64 {
        let (hi, lo) = (0.5 * self.band_mm, 0.5 * self.strap_mm);
        let rho = self.corner_mm.clamp(0.0, hi - lo);
        let s = (phi.abs() - self.wall_deg).to_radians() * self.outer_r();
        if s <= -rho {
            hi
        } else if s < 0.0 {
            hi - rho + (rho * rho - (s + rho).powi(2)).max(0.0).sqrt()
        } else if s < self.wall_mm {
            hi - rho + (lo - hi + rho) * s / self.wall_mm.max(1e-6)
        } else {
            lo
        }
    }

    /// Share of the strap's inset at `phi`: 0 up to the wall's foot, 1 once `inset_ramp_mm` past it.
    fn inset_share(&self, phi: f64) -> f64 {
        let s = (phi.abs() - self.wall_deg).to_radians() * self.outer_r() - self.wall_mm;
        let t = (s / self.inset_ramp_mm.max(1e-6)).clamp(0.0, 1.0);
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    fn section(&self, phi: f64) -> Section {
        let crest = self.crest_share(phi);
        Section {
            width: 2.0 * self.edge_half(phi) + (self.crest_mm - self.band_mm) * crest,
            thick: self.thick_mm + self.crest_rise_mm * crest,
            inner_r: self.inner_r() - self.strap_inset_mm * self.inset_share(phi),
        }
    }

    fn json(&self) -> serde_json::Value {
        json!({
            "bore_mm": self.bore_mm, "band_mm": self.band_mm, "thick_mm": self.thick_mm, "edge_mm": self.edge_mm,
            "crest_mm": self.crest_mm, "crest_deg": self.crest_deg, "crest_pow": self.crest_pow, "tip_deg": self.tip_deg,
            "crest_rise_mm": self.crest_rise_mm, "strap_mm": self.strap_mm, "strap_inset_mm": self.strap_inset_mm,
            "inset_ramp_mm": self.inset_ramp_mm, "wall_deg": self.wall_deg, "corner_mm": self.corner_mm, "wall_mm": self.wall_mm,
        })
    }
}

/// The swept torus grid: `n_theta` slices of `rows` points, slice-major.
struct Grid {
    pos: Vec<[f64; 3]>,
    rows: usize,
    n_theta: usize,
    bore: Vec<bool>,
}

impl Grid {
    fn idx(&self, i: usize, j: usize) -> u32 {
        ((i % self.n_theta) * self.rows + j % self.rows) as u32
    }

    /// Two triangles per grid cell, each named outer face (0) or bore (1).
    fn packed(&self) -> Result<stored::Packed> {
        let cells = self.n_theta * self.rows;
        let (mut tri, mut face) = (Vec::with_capacity(2 * cells), Vec::with_capacity(2 * cells));
        for i in 0..self.n_theta {
            for j in 0..self.rows {
                let (a, b, c, d) = (self.idx(i, j), self.idx(i + 1, j), self.idx(i + 1, j + 1), self.idx(i, j + 1));
                tri.push([a, b, c]);
                tri.push([a, c, d]);
                let bore = u32::from(self.bore[j] && self.bore[(j + 1) % self.rows]);
                face.extend([bore, bore]);
            }
        }
        stored::Packed::encode(&self.pos, &tri, &face, &[SurfaceKind::Freeform, SurfaceKind::Cylinder])
    }

    /// The grid as an OBJ of quads.
    fn quad_obj(&self, name: &str) -> String {
        let mut s = format!("# {name}: {} quads, mm\no {name}\n", self.n_theta * self.rows);
        for p in &self.pos {
            let _ = writeln!(s, "v {:.5} {:.5} {:.5}", p[0], p[1], p[2]);
        }
        for i in 0..self.n_theta {
            for j in 0..self.rows {
                let q = [self.idx(i, j), self.idx(i + 1, j), self.idx(i + 1, j + 1), self.idx(i, j + 1)];
                let _ = writeln!(s, "f {} {} {} {}", q[0] + 1, q[1] + 1, q[2] + 1, q[3] + 1);
            }
        }
        s
    }
}

/// One band section per slice from the profile, modulated to the cuff, swept round the bore.
fn sweep(c: &Cuff, n_theta: usize, n_prof: usize) -> Result<Grid> {
    let profile = c.profile();
    let reference = profile.sample(c.inner_r(), n_prof);
    let rows = reference.pts.len();
    let mut pos: Vec<[f64; 3]> = Vec::with_capacity(n_theta * rows);
    for i in 0..n_theta {
        let theta = 360.0 * i as f64 / n_theta as f64;
        let phi = (theta - CREST_THETA + 180.0).rem_euclid(360.0) - 180.0;
        let sec = c.section(phi);
        let m = ShankMod { width_scale: sec.width / c.band_mm, thickness_scale: sec.thick / c.thick_mm, ..ShankMod::identity() };
        let loop_i = profile.sample_spaced(sec.inner_r, n_prof, &m, None, Some(&reference));
        ensure!(loop_i.pts.len() == rows, "slice {i} has {} rows, not {rows}", loop_i.pts.len());
        let (s, co) = theta.to_radians().sin_cos();
        pos.extend(loop_i.pts.iter().map(|p| [p.r * co, p.r * s, p.z]));
    }
    Ok(Grid { pos, rows, n_theta, bore: reference.pts.iter().map(|p| !p.surface).collect() })
}

fn feature(id: u64, name: &str, operation: Operation, role: ComponentRole) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component: Component { role, ..Component::default() } }
}

/// The cuff as a stored part, and for `open` two box cuts through the strap, each face on a radius.
fn design(c: &Cuff, body: &stored::Packed, open: bool) -> Result<RingDesign> {
    let mut doc = Document::default();
    let recipe = stored::Recipe { kernel: "cuff_base".into(), op: "import".into(), params: c.json(), digest: String::new() };
    doc.append(feature(CUFF, "Cuff", Operation::Stored { recipe, sources: Vec::new(), mesh: body.clone() }, ComponentRole::Shank))?;
    if open {
        let bore = c.inner_r() - c.strap_inset_mm;
        ensure!(0.5 * c.gap_mm < bore - 1.0, "a {:.1} mm opening is wider than the bore", c.gap_mm);
        // End angle from the back's centre line, with the gap measured across the band's bore.
        let beta = (0.5 * c.gap_mm / c.inner_r()).asin();
        let far = c.outer_r() + 4.0;
        let w = far * beta.sin() + 1.0;
        ensure!(w < bore * (2.0 * beta).sin(), "a {:.1} mm opening is too narrow for two radial cuts", c.gap_mm);
        let near = (bore * bore - w * w).sqrt() - 2.0;
        let size = [w, far - near, c.band_mm + 4.0];
        for (k, sign) in [(0u64, 1.0f64), (1, -1.0)] {
            let tool = OPENING_TOOL + 2 * k;
            doc.append(feature(tool, "Opening tool", Operation::Box { size }, ComponentRole::Other))?;
            let (lx, ly) = (-sign * 0.5 * w, -0.5 * (near + far));
            let (s, co) = (sign * beta).sin_cos();
            let cut = Operation::Transform {
                source: tool,
                translation: [lx * co - ly * s, lx * s + ly * co, 0.0],
                rotation_deg: [0.0, 0.0, (sign * beta).to_degrees()],
            };
            let mut f = feature(tool + 1, "Opening", cut, ComponentRole::Other);
            f.component.attach = Attach::Cut;
            doc.append(f)?;
        }
    }
    Ok(RingDesign {
        name: if open { format!("{NAME}, open") } else { NAME.into() },
        draft: DraftSettings { process: CastProcess::LostWax, ..DraftSettings::default() },
        cad: Some(doc),
        ..RingDesign::default()
    })
}

fn report(key: &str, built: &BuildResult) -> Result<()> {
    let r = &built.report;
    let v = &r.validation;
    let b = r.bounds_mm;
    println!("\n{key}: {} triangles, watertight {}, boundary {}, non-manifold {}", v.triangle_count, v.watertight, v.boundary_edges, v.non_manifold_edges);
    for n in &built.parts.notes {
        println!("  note: {n}");
    }
    println!(
        "  {:.2} x {:.2} x {:.2} mm, {:.0} mm³: {:.1} g sterling, {:.1} g 14k",
        b[0],
        b[1],
        b[2],
        r.volume_mm3,
        r.volume_mm3 * SILVER_G_PER_MM3,
        r.volume_mm3 * GOLD14_G_PER_MM3
    );
    println!("  triangles: smallest corner {:.2}°, worst aspect {:.1}, {} degenerate", r.quality.min_angle_deg, r.quality.worst_aspect, r.quality.degenerate_faces);
    let wall = cad::measure::thickness(&built.mesh, 0.5);
    println!(
        "  wall census at 0.5 mm: thinnest {:.3} mm, {} wall samples under, {} edge samples under, {} unresolved",
        wall.sampled_min_mm.unwrap_or(f64::NAN),
        wall.below_limit,
        wall.edge_below_limit,
        wall.unresolved
    );
    let solid = csg::Solid {
        v: built.mesh.vertices.iter().map(|p| [f64::from(p.0), f64::from(p.1), f64::from(p.2)]).collect(),
        f: built.mesh.faces.clone(),
    };
    let crossings = csg::self_crossings(&solid);
    println!("  self-crossing face pairs: {crossings}");
    for z in wall.walls.iter().take(6).chain(wall.edges.iter().take(4)) {
        let theta = z.point[1].atan2(z.point[0]).to_degrees();
        println!("  thin {:?} {:.4} mm² down to {:.3} mm at theta {theta:.2}, r {:.2}, z {:.2}", z.kind, z.area_mm2, z.thinnest_mm, z.point[0].hypot(z.point[1]), z.point[2]);
    }
    ensure!(v.watertight, "the {key} mesh is not closed");
    ensure!(crossings == 0, "the {key} mesh crosses itself at {crossings} face pairs");
    Ok(())
}

fn write(out: &Path, stem: &str, d: &RingDesign, built: &BuildResult, lib: &AlphaLibrary, params: BuildParams) -> Result<()> {
    stl::write_stl(out.join(format!("{stem}.stl")), &built.mesh, &d.name)?;
    let path = out.join(format!("{stem}.ring.json"));
    library::save_design(&path, d)?;
    let again = mesh::try_build(&library::load_design(&path)?, lib, params).with_context(|| format!("reopening {}", path.display()))?;
    ensure!(
        again.mesh.faces == built.mesh.faces && again.mesh.vertices == built.mesh.vertices,
        "{} does not rebuild to the mesh it was saved from",
        path.display()
    );
    let parts = [render::Part::metal(&built.mesh, render::GOLD)];
    let side = 90f64.to_radians();
    for (view, yaw, pitch) in [("hero", 0.55, 1.05), ("front", 0.0, side), ("plan", 0.0, 0.0), ("back", 2.0 * side, side)] {
        render::write_png_parts(out.join(format!("{stem}-{view}.png")), &parts, yaw, pitch, 900)?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut out = PathBuf::from("target/cuff-base");
    let mut c = Cuff::default();
    let (mut n_theta, mut n_prof) = (1080, 320);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--bore" => c.bore_mm = args.next().context("--bore needs mm")?.parse()?,
            "--gap" => c.gap_mm = args.next().context("--gap needs mm")?.parse()?,
            "--theta" => n_theta = args.next().context("--theta needs a count")?.parse()?,
            "--rows" => n_prof = args.next().context("--rows needs a count")?.parse()?,
            flag if flag.starts_with("--") => bail!("unknown option {flag}"),
            other => out = PathBuf::from(other),
        }
    }
    ensure!(n_theta >= 360, "--theta below 360 cannot hold the crest's tip or the strap's walls");
    std::fs::create_dir_all(&out)?;
    println!("{NAME}: bore Ø{:.2}, band {:.2} x {:.2}, crest {:.2} tall, strap {:.2} wide", c.bore_mm, c.band_mm, c.thick_mm, c.crest_mm, c.strap_mm);
    println!("from the crest   width   thick   bore r  outer r");
    for phi in [0.0, 10.0, 20.0, 30.0, 45.0, 60.0, 90.0, 120.0, 150.0, 154.0, 156.0, 156.5, 160.0, 180.0] {
        let s = c.section(phi);
        println!("  {phi:6.1}°       {:6.2}  {:6.2}  {:6.2}  {:7.2}", s.width, s.thick, s.inner_r, s.inner_r + s.thick);
    }
    let grid = sweep(&c, n_theta, n_prof)?;
    std::fs::write(out.join("trident-cuff-base-quads.obj"), grid.quad_obj("trident_cuff_base"))?;
    let body = grid.packed()?;
    let lib = AlphaLibrary::builtin();
    let params = BuildParams::default();
    for (open, stem) in [(false, "trident-cuff-base"), (true, "trident-cuff-base-open")] {
        let d = design(&c, &body, open)?;
        let built = mesh::try_build(&d, &lib, params).with_context(|| format!("building {stem}"))?;
        report(stem, &built)?;
        write(&out, stem, &d, &built, &lib, params)?;
    }
    println!("\nwritten to {}", out.display());
    Ok(())
}
