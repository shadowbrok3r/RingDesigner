//! Shared workshop planning. All coordinates and allowances are millimeters.
//!
//! Release is a sampled geometric check, not a sand-strength or fluid simulation.
//! The same prepared pattern is used for inspection and package export.

pub mod package;
pub mod release;
pub mod repair;
pub mod stages;
#[cfg(test)]
mod tests;
pub mod trials;

use crate::castability::{CastProcess, SandProcess};
use crate::cad::{Attach, Document, Operation, Stage};
#[cfg(test)]
use crate::Layer;
use crate::sketch::Id;
use crate::{AlphaLibrary, BuildParams, Mesh, RingDesign};
use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Recipe {
    pub name: String,
    pub process: CastProcess,
    pub sand: Option<SandProcess>,
    pub alloy: String,
    pub shrink_pct: f64,
    pub min_draft_deg: f64,
    pub min_section_mm: f64,
    pub min_detail_mm: f64,
    pub min_sand_web_mm: f64,
    pub sand_margin_mm: f64,
    pub calibration_note: String,
}

impl Default for Recipe {
    fn default() -> Self {
        Self::sand(SandProcess::DelftClay)
    }
}

impl Recipe {
    pub fn sand(sand: SandProcess) -> Self {
        let mut draft = crate::DraftSettings::default();
        sand.apply(&mut draft);
        Self {
            name: format!("{} / Silver 925", sand.label()),
            process: CastProcess::SandTwoPart,
            sand: Some(sand),
            alloy: "Silver 925".into(),
            shrink_pct: 1.9,
            min_draft_deg: draft.min_draft_deg,
            min_section_mm: draft.min_section_mm,
            min_detail_mm: draft.min_detail_mm,
            min_sand_web_mm: 0.6,
            sand_margin_mm: 5.0,
            calibration_note: "Starting values; verify with shop trials".into(),
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.name.trim().is_empty(), "Recipe needs a name");
        anyhow::ensure!(
            crate::metal::find(&self.alloy).is_some(),
            "Unknown alloy: {}",
            self.alloy
        );
        for (name, value, lo, hi) in [
            ("shrink", self.shrink_pct, 0.0, 15.0),
            ("draft", self.min_draft_deg, 0.0, 30.0),
            ("metal section", self.min_section_mm, 0.05, 10.0),
            ("detail", self.min_detail_mm, 0.01, 5.0),
            ("sand web", self.min_sand_web_mm, 0.05, 10.0),
            ("sand margin", self.sand_margin_mm, 0.0, 50.0),
        ] {
            anyhow::ensure!(
                value.is_finite() && (lo..=hi).contains(&value),
                "Invalid {name}: expected {lo}–{hi}"
            );
        }
        Ok(())
    }

    pub fn save(&self, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
        self.validate()?;
        crate::library::write_atomic(path.as_ref(), &serde_json::to_vec_pretty(self)?)
    }

    pub fn load(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let recipe: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        recipe.validate()?;
        Ok(recipe)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoreStrategy {
    /// Both sand halves forming the bore must pass the same release check.
    #[default]
    Molded,
    /// A separately prepared core: still report geometric obstruction, require review.
    SeparateCore,
    /// Stock is left for reaming; the actual pattern bore is still checked.
    ReamAtBench,
}

impl BoreStrategy {
    pub const ALL: [Self; 3] = [Self::Molded, Self::SeparateCore, Self::ReamAtBench];
    pub fn label(self) -> &'static str {
        match self {
            Self::Molded => "Molded bore",
            Self::SeparateCore => "Separate core (review)",
            Self::ReamAtBench => "Ream at bench",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Flask {
    pub width_mm: f64,
    pub length_mm: f64,
    pub cope_mm: f64,
    pub drag_mm: f64,
}
impl Default for Flask {
    fn default() -> Self {
        Self {
            width_mm: 60.0,
            length_mm: 60.0,
            cope_mm: 20.0,
            drag_mm: 20.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelKind {
    #[default]
    Gate,
    Sprue,
    Vent,
    Feeder,
}
impl ChannelKind {
    pub const ALL: [Self; 4] = [Self::Gate, Self::Sprue, Self::Vent, Self::Feeder];
    pub fn label(self) -> &'static str {
        match self {
            Self::Gate => "Gate",
            Self::Sprue => "Sprue",
            Self::Vent => "Vent",
            Self::Feeder => "Feeder",
        }
    }
}

/// A circular channel cut into sand, in the pull frame. Kept out of the printed ring.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Channel {
    pub kind: ChannelKind,
    pub start: [f64; 3],
    pub end: [f64; 3],
    pub diameter_mm: f64,
}
impl Channel {
    pub fn volume_mm3(&self) -> f64 {
        let len = self
            .start
            .iter()
            .zip(self.end)
            .map(|(a, b)| (b - a).powi(2))
            .sum::<f64>()
            .sqrt();
        std::f64::consts::PI * (self.diameter_mm * 0.5).powi(2) * len
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Setup {
    /// The CAD part to cast: on a band None, its anchor or a joined or cut part casts the ring, any other part casts alone.
    pub component: Option<u64>,
    pub recipe: Recipe,
    pub pull: [f64; 3],
    /// Plane distance from the origin along normalized pull; pattern-space mm.
    pub parting_mm: f64,
    pub auto_parting: bool,
    pub bore: BoreStrategy,
    pub flask: Flask,
    /// Extra band stock before shrink compensation. These are profile allowances,
    /// not a general solid offset or an automatic prediction of polishing loss.
    pub radial_stock_mm: f64,
    pub axial_stock_mm: f64,
    pub bore_stock_mm: f64,
    pub sample_pitch_mm: f64,
    pub tolerance_mm: f64,
    pub channels: Vec<Channel>,
    pub bench_notes: String,
}
impl Default for Setup {
    fn default() -> Self {
        Self {
            component: None,
            recipe: Recipe::default(),
            pull: [0.0, 0.0, 1.0],
            parting_mm: 0.0,
            auto_parting: true,
            bore: BoreStrategy::Molded,
            flask: Flask::default(),
            radial_stock_mm: 0.0,
            axial_stock_mm: 0.0,
            bore_stock_mm: 0.0,
            sample_pitch_mm: 0.15,
            tolerance_mm: 0.02,
            channels: Vec::new(),
            bench_notes: String::new(),
        }
    }
}
impl Setup {
    pub fn from_design(d: &RingDesign) -> Self {
        let mut s = Self::default();
        if let Some(sand) = d.draft.sand {
            s.recipe = Recipe::sand(sand);
        } else {
            s.recipe.name = "Design limits / Silver 925".into();
            s.recipe.sand = None;
            s.recipe.calibration_note =
                "Limits inherited from this design; choose a sand recipe to replace them".into();
        }
        s.recipe.process = d.draft.process;
        s.recipe.min_draft_deg = d.draft.min_draft_deg;
        s.recipe.min_section_mm = d.draft.min_section_mm;
        s.recipe.min_detail_mm = d.draft.min_detail_mm;
        s.parting_mm = d.draft.parting_z_mm;
        s.auto_parting = d.draft.auto_parting;
        s
    }
    pub fn scale(&self) -> f64 {
        1.0 / (1.0 - self.recipe.shrink_pct / 100.0)
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        self.recipe.validate()?;
        release::Frame::new(self.pull)?;
        anyhow::ensure!(self.parting_mm.is_finite(), "Parting plane must be finite");
        for v in [
            self.flask.width_mm,
            self.flask.length_mm,
            self.flask.cope_mm,
            self.flask.drag_mm,
        ] {
            anyhow::ensure!(
                v.is_finite() && (1.0..=1000.0).contains(&v),
                "Flask dimensions must be 1–1000 mm"
            );
        }
        for v in [
            self.radial_stock_mm,
            self.axial_stock_mm,
            self.bore_stock_mm,
        ] {
            anyhow::ensure!(
                v.is_finite() && (0.0..=2.0).contains(&v),
                "Stock allowance must be 0–2 mm"
            );
        }
        anyhow::ensure!(
            self.sample_pitch_mm.is_finite() && (0.025..=2.0).contains(&self.sample_pitch_mm),
            "Sample pitch must be 0.025–2 mm"
        );
        anyhow::ensure!(
            self.tolerance_mm.is_finite() && (0.001..=0.25).contains(&self.tolerance_mm),
            "Tolerance must be 0.001–0.25 mm"
        );
        anyhow::ensure!(self.channels.len() <= 128, "At most 128 channels");
        for c in &self.channels {
            anyhow::ensure!(
                c.start
                    .iter()
                    .chain(c.end.iter())
                    .all(|v| v.is_finite() && v.abs() <= 1000.0),
                "Invalid channel coordinates"
            );
            anyhow::ensure!(
                c.diameter_mm.is_finite()
                    && (0.1..=20.0).contains(&c.diameter_mm)
                    && c.volume_mm3() > 0.0,
                "Channel needs a diameter and distinct endpoints"
            );
        }
        Ok(())
    }
}

/// What a pattern pours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Casting {
    /// The band with every part joined to it or cut from it.
    #[default]
    Ring,
    /// One CAD part on its own, standing where the finished ring stands it.
    Part(Id),
}

pub struct Prepared {
    pub design: RingDesign,
    pub mesh: Mesh,
    pub build: crate::mesh::Report,
    pub scale: f64,
    pub bench_layers: Vec<String>,
    /// What the pattern pours.
    pub casting: Casting,
    /// Every part cast on its own and so not in this pattern, one line each.
    pub notes: Vec<String>,
}

/// The outputs the ring pours: every part joined to or cut from the band, in output order.
fn ring_parts(doc: &Document) -> Vec<Id> {
    let listed: std::collections::HashMap<Id, Attach> = doc.attachments().into_iter().map(|(id, attach, _)| (id, attach)).collect();
    doc.outputs
        .iter()
        .copied()
        .filter(|id| {
            doc.feature(*id).is_some_and(|f| {
                !f.component.reference && f.operation.has_body() && !matches!(f.operation, Operation::Band) && listed.get(id).copied().unwrap_or(f.component.attach) != Attach::Separate
            })
        })
        .collect()
}

/// What a band's pattern pours for `component`: the ring for none, the band's anchor or a ring part; anything else alone.
fn chosen(doc: &Document, component: Option<Id>) -> anyhow::Result<Casting> {
    let Some(id) = component else { return Ok(Casting::Ring) };
    anyhow::ensure!(doc.feature(id).is_some_and(|f| !f.component.reference), "Selected casting component is missing or a reference stone");
    Ok(if doc.band() == Some(id) || ring_parts(doc).contains(&id) { Casting::Ring } else { Casting::Part(id) })
}

/// The document's outputs cut down to the ring, and a line for every separate part left out.
fn keep_ring(doc: &mut Document) -> Vec<String> {
    let ring = ring_parts(doc);
    let apart = doc
        .outputs
        .iter()
        .filter_map(|id| doc.feature(*id))
        .filter(|f| f.enabled && !f.component.reference && f.operation.has_body() && !matches!(f.operation, Operation::Band) && !ring.contains(&f.id))
        .map(|f| format!("#{} {} is its own casting and not in this pattern: choose it as the casting component to prepare it", f.id, f.name))
        .collect();
    doc.outputs = ring;
    apart
}

/// The document's one output part `id`, set beside the band and poured.
fn cast_alone(doc: &mut Document, id: Id) {
    doc.outputs = vec![id];
    if let Some(f) = doc.feature_mut(id) {
        f.component.attach = Attach::Separate;
        f.component.stage = Stage::Cast;
    }
}

/// The closed shell part `id` stands as in `built`, on its own.
fn part_mesh(built: &crate::mesh::BuildResult, id: Id, who: &str) -> anyhow::Result<Mesh> {
    crate::threemf::objects(built, who)
        .into_iter()
        .find(|o| o.feature == Some(id))
        .map(|o| o.mesh.into_owned())
        .with_context(|| format!("{who} did not build as a part of its own: {}", built.parts.notes.join("; ")))
}

/// A build report for `mesh` alone, measured the way a ring of parts reports its assembly.
fn report_of(mesh: &Mesh, nominal_bore_mm: f64, build_ms: u128) -> crate::mesh::Report {
    let (lo, hi) = mesh.bounds().unwrap_or_default();
    let bounds_mm = [(hi.0 - lo.0) as f64, (hi.1 - lo.1) as f64, (hi.2 - lo.2) as f64];
    let volume = mesh.volume_mm3();
    let reach = mesh.vertices.iter().filter(|v| v.is_finite()).map(|v| (v.0 as f64).hypot(v.1 as f64)).fold(f64::MAX, f64::min);
    crate::mesh::Report {
        validation: mesh.validate(),
        volume_mm3: volume,
        surface_area_mm2: mesh.surface_area_mm2(),
        bounds_mm,
        inner_diameter_mm: if reach.is_finite() && reach > 0.0 { 2.0 * reach } else { nominal_bore_mm },
        outer_diameter_mm: bounds_mm[0].max(bounds_mm[1]),
        band_width_mm: bounds_mm[2],
        max_relief_mm: 0.0,
        min_relief_mm: 0.0,
        metals: crate::metal::metal_table(volume),
        build_ms,
        refine: None,
        quality: mesh.quality(),
    }
}

/// Nominal-to-pattern conversion. No saved geometry is ever scaled in place.
pub fn prepare(
    d: &RingDesign,
    lib: &AlphaLibrary,
    setup: &Setup,
    params: BuildParams,
) -> anyhow::Result<Prepared> {
    let resolved = source_library(d, lib);
    prepare_with_library(d, &resolved, setup, params).map(|(prepared, _)| prepared)
}

/// Resolve portable source artwork before building nominal or pattern geometry.
/// Borrow the supplied library when the design has no embedded/generated art.
pub fn source_library<'a>(
    d: &RingDesign,
    lib: &'a AlphaLibrary,
) -> std::borrow::Cow<'a, AlphaLibrary> {
    if d.embedded.is_empty()
        && d.texts.is_empty()
        && d.svgs.is_empty()
        && d.drawn.is_empty()
        && d.recipes.is_empty()
    {
        return std::borrow::Cow::Borrowed(lib);
    }
    let mut resolved = lib.clone();
    d.unpack_embedded(&mut resolved);
    d.bake_all(&mut resolved);
    std::borrow::Cow::Owned(resolved)
}
/// The pattern prepared, with the unscaled build it was cut from.
fn prepare_with_library(
    d: &RingDesign,
    lib: &AlphaLibrary,
    setup: &Setup,
    params: BuildParams,
) -> anyhow::Result<(Prepared, crate::mesh::BuildResult)> {
    setup.validate()?;
    anyhow::ensure!(
        d.size.0.is_finite()
            && (crate::sizing::MIN_SIZE..=crate::sizing::MAX_SIZE).contains(&d.size.0),
        "Nominal ring size is outside the supported range"
    );
    anyhow::ensure!(
        d.size.inner_diameter_mm() > 2.0 * setup.bore_stock_mm + 1.0,
        "Bore allowance closes the finger hole"
    );
    let mut pattern = d.clone();
    let mut casting = Casting::Ring;
    let mut notes = Vec::new();
    let stock = setup.radial_stock_mm + setup.axial_stock_mm + setup.bore_stock_mm;
    if let Some(doc) = &mut pattern.cad {
        if doc.band().is_some() {
            casting = chosen(doc, setup.component)?;
            match casting {
                Casting::Ring => notes = keep_ring(doc),
                Casting::Part(id) => {
                    let who = doc.feature(id).map_or_else(|| format!("#{id}"), |f| format!("#{id} {}", f.name));
                    // Profile stock goes on the swept band, which a part cast alone leaves behind.
                    anyhow::ensure!(stock == 0.0, "Profile stock goes on the band, and this pattern casts {who} on its own; model stock into it explicitly");
                    cast_alone(doc, id);
                }
            }
        } else {
            if let Some(id) = setup.component {
                anyhow::ensure!(
                    doc.features
                        .iter()
                        .any(|f| f.id == id && !f.component.reference),
                    "Selected casting component is missing or a reference stone"
                );
                doc.outputs = vec![id];
            } else {
                doc.outputs.retain(|id| {
                    doc.features
                        .iter()
                        .any(|f| f.id == *id && !f.component.reference)
                });
                anyhow::ensure!(
                    doc.outputs.len() == 1,
                    "Select one CAD component in the casting recipe"
                );
            }
            casting = Casting::Part(doc.outputs[0]);
            anyhow::ensure!(
                stock == 0.0,
                "Profile stock requires a procedural shank; model stock into this CAD component explicitly"
            );
        }
    }
    // A part cast alone off a band is cut out of a build that seats it where the finished ring does.
    let alone = match casting {
        Casting::Part(id) if d.band_is_procedural() => Some(id),
        _ => None,
    };
    pattern.size.0 -= 2.0 * std::f64::consts::PI * setup.bore_stock_mm / 2.55;
    anyhow::ensure!(
        pattern.size.0 >= crate::sizing::MIN_SIZE,
        "Bore stock puts the pattern below the supported bore range; use a separately modeled CAD pattern"
    );
    pattern.profile.thickness_mm += setup.bore_stock_mm + setup.radial_stock_mm;
    pattern.profile.width_mm += 2.0 * setup.axial_stock_mm;
    pattern.draft.process = setup.recipe.process;
    pattern.draft.min_draft_deg = setup.recipe.min_draft_deg;
    pattern.draft.min_section_mm = setup.recipe.min_section_mm;
    pattern.draft.min_detail_mm = setup.recipe.min_detail_mm;
    // What the pattern leaves to the bench, by the one rule the live verdict uses; nothing for a part cast alone.
    let (pattern, bench_layers) = if alone.is_some() {
        (pattern, Vec::new())
    } else {
        let left = crate::castability::pattern_parts(&pattern, lib);
        let mut bench_layers = left.layers;
        bench_layers.extend(left.seats.into_iter().map(|s| format!("{s} (setting)")));
        bench_layers.extend(left.parts.into_iter().map(|s| format!("{s} (part)")));
        (left.design.into_owned(), bench_layers)
    };
    // Parts stand where the finished ring has them, sunk into any stock and never on the pattern's marks.
    let clock = crate::mesh::BuildClock::start();
    let out = crate::mesh::try_build_poured(&pattern, d, lib, params)?;
    let scale = setup.scale();
    let (mesh, build) = match alone {
        Some(id) => {
            let who = pattern.cad.as_ref().and_then(|doc| doc.feature(id)).map_or_else(|| format!("#{id}"), |f| format!("#{id} {}", f.name));
            let own = part_mesh(&out, id, &who)?;
            let report = report_of(&own, d.size.inner_diameter_mm(), clock.ms());
            (own, report)
        }
        None => (out.mesh.clone(), out.report.clone()),
    };
    let prepared = Prepared {
        design: pattern,
        mesh: mesh.scaled(scale),
        build,
        scale,
        bench_layers,
        casting,
        notes,
    };
    Ok((prepared, out))
}

pub struct Inspection {
    pub prepared: Prepared,
    pub release: release::ReleaseReport,
    pub details: Vec<String>,
    pub field: Option<crate::castability::FieldReport>,
    pub local_wall: Option<crate::cad::measure::Thickness>,
    pub hot_spot: Option<(f64, f64)>,
    pub ring_grams: f64,
    pub channel_grams: f64,
}
pub fn inspect(
    d: &RingDesign,
    lib: &AlphaLibrary,
    setup: &Setup,
    params: BuildParams,
) -> anyhow::Result<Inspection> {
    let resolved = source_library(d, lib);
    let lib = resolved.as_ref();
    let (prepared, built) = prepare_with_library(d, lib, setup, params)?;
    let release = release::analyze(&prepared.mesh, setup)?;
    // The band is judged by its field only when it is what the pattern pours.
    let ring = d.band_is_procedural() && prepared.casting == Casting::Ring;
    // The pattern's CAD parts are judged on the build the pattern was cut from.
    let field = ring.then(|| {
        let mut f = crate::castability::analyze_field(&prepared.design, lib, &prepared.design.draft, 192, 128);
        crate::castability::judge_parts(&mut f, &prepared.design, &built);
        f
    });
    let local_wall = (!ring).then(|| {
        crate::cad::measure::thickness(
            &prepared.mesh.scaled(1.0 / prepared.scale),
            setup.recipe.min_section_mm,
        )
    });
    let details = if ring {
        crate::dfm::findings_in(&prepared.design, lib)
            .into_iter()
            .map(|f| format!("{}: {}", f.label, f.message))
            .chain(prepared.notes.iter().cloned())
            .collect()
    } else {
        vec!["CAD component: inspect local wall and detail dimensions; the band-only radial wall and ornament checks do not apply".into()]
    };
    let density = crate::metal::find(&setup.recipe.alloy).unwrap().density / 1000.0;
    let ring_grams = prepared.build.volume_mm3 * density;
    let channel_grams = setup
        .channels
        .iter()
        .filter(|c| c.kind != ChannelKind::Vent)
        .map(Channel::volume_mm3)
        .sum::<f64>()
        / prepared.scale.powi(3)
        * density;
    let scan = if ring {
        crate::castability::modulus_scan(&prepared.design, lib, 64)
    } else {
        Vec::new()
    };
    let min = scan.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let hot_spot = scan
        .into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .filter(|p| p.1 > min * 1.05);
    Ok(Inspection {
        prepared,
        release,
        details,
        field,
        local_wall,
        hot_spot,
        ring_grams,
        channel_grams,
    })
}
