//! Curated starter designs — the File menu's "New from template".
//!
//! Each template is built in code from the same API the panels drive, so it
//! can never go stale against the file format and every part of it stays
//! editable. Only builtin alphas are referenced — a template must open
//! identically on a machine with an empty library.

use crate::field::{
    Layer, LayerEntry, MilgrainLayer, SeatPadLayer, SeatStyle, SignetOutline, Window,
    SIDE_FACE_MIN_DRAFT_DEG,
};
use crate::gem::{Gem, GemCut};
use crate::profile::{ShankKind, SignetHead, TOP_DEG};
use crate::tiling::TilingLayer;
use crate::{ProfileStyle, RingDesign};

mod fixtures;
pub mod settings;
pub use fixtures::{fixture, NAMES as FIXTURE_NAMES};

pub struct Template {
    pub name: &'static str,
    /// One sentence of what it teaches, shown as the menu item's hover.
    pub blurb: &'static str,
    pub view: (f64, f64),
    build: fn() -> RingDesign,
}

impl Template {
    /// A fresh design named after the template.
    pub fn design(&self) -> RingDesign {
        let mut d = (self.build)();
        d.name = self.name.into();
        d
    }
}

pub fn all() -> &'static [Template] {
    &TEMPLATES
}

/// Flat profile with squared side faces — castable ground for relief.
fn squared(width: f64, thickness: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d
}

fn signet(outline: SignetOutline, width: f64, thickness: f64) -> RingDesign {
    let mut d = squared(width, thickness);
    d.shank.apply_signet(width);
    d.shank.head.outline = outline;
    d.shank.head.fit_length_to(width);
    d
}

/// Builtin tiling fitted onto the side faces, mirrored when both exist.
fn side_tiling(d: &RingDesign, alpha: &str, height: f64) -> TilingLayer {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for(alpha, &ctx);
    t.height_mm = height;
    t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG);
    t.repeats_around = t.repeats_for_square_cells(&ctx);
    t
}

static TEMPLATES: [Template; 13] = [
    Template {
        name: "Court band",
        blurb: "A plain comfort-fit court — the blank everything else starts from.",
        view: (0.55, 1.12),
        build: || {
            let mut d = RingDesign::default();
            d.profile.apply_style(ProfileStyle::LowDome);
            d.profile.width_mm = 4.0;
            d.profile.thickness_mm = 2.0;
            d
        },
    },
    Template {
        name: "Braided band",
        blurb: "Original braid and crest milgrain; cords merge softly in the pour, with marginal sand release and sampled obstructions.",
        view: (0.55, 0.6),
        build: || {
            let mut d = squared(7.5, 2.4);
            let mut t = side_tiling(&d, "Braid", 0.30);
            // Three cords per tile; 8 around keeps a cord ~2 mm at the crest.
            t.repeats_around = 8;
            t.rows = 1;
            d.layers.layers.push(LayerEntry::new("Braid", Layer::Tiling(t)));
            let ctx = d.field_context();
            d.layers.layers.push(LayerEntry::new(
                "Milgrain",
                Layer::Milgrain(MilgrainLayer {
                    v_mm: ctx.band_v_len_mm * 0.5,
                    bead_diameter_mm: 0.5,
                    beads_around: 130,
                    height_mm: 0.22,
                    mirror: false,
                }),
            ));
            d
        },
    },
    Template {
        name: "Split shank",
        blurb: "Two open rails meet at each shoulder on a fine band; pour in lost wax.",
        view: (0.0, 1.08),
        build: settings::split_shank,
    },
    Template {
        name: "Split gallery",
        blurb: "Daylight between two arched rails along the sand pull; mould trial required.",
        view: (0.15, 0.1),
        build: settings::split_gallery,
    },
    Template {
        name: "Shouldered cushion signet",
        blurb: "Original Chevron shoulders off the blank table; fine gaps soften in the pour and the sand verdict is marginal.",
        view: (0.55, 1.12),
        build: || {
            let mut d = signet(SignetOutline::Cushion, 14.5, 2.2);
            let mut t = side_tiling(&d, "Chevron", 0.28);
            // Four zigzag bands per tile read at ~0.5 mm each on the face.
            t.repeats_around = 9;
            t.rows = 1;
            let mut e = LayerEntry::new("Shoulder ornament", Layer::Tiling(t));
            e.window = Window::except(TOP_DEG, 120.0);
            d.layers.layers.push(e);
            d
        },
    },
    Template {
        name: "Cathedral solitaire",
        blurb: "Six claws carry a round over two cathedral arches; a local flared bed holds four teardrop azures above the fine palm. Lost wax.",
        view: (0.55, 1.12),
        build: settings::cathedral_solitaire,
    },
    Template {
        name: "Bezel solitaire",
        blurb: "An oval collet soldered onto a Delft-cast band; the pattern carries raised locating dots, with the head and bur left to the bench.",
        view: (0.55, 1.12),
        build: settings::bezel_solitaire,
    },
    Template {
        name: "Halo",
        blurb: "A cushion in four claws framed by bezel-set melee and bead-set shoulder pavé; lost wax.",
        view: (0.55, 1.12),
        build: settings::halo,
    },
    Template {
        name: "Trilogy",
        blurb: "A round centre between two oval side stones in individual claw heads; lost wax.",
        view: (0.55, 1.12),
        build: settings::trilogy,
    },
    Template {
        name: "Toi et moi",
        blurb: "Two ovals pass each other on crossing arms, each in its own claw head; lost wax.",
        view: (0.55, 1.25),
        build: settings::toi_et_moi,
    },
    Template {
        name: "Split-shank basket",
        blurb: "An oval basket stands over two open rails; lost wax.",
        view: (0.55, 1.12),
        build: settings::split_shank_basket,
    },
    Template {
        name: "Half eternity",
        blurb: "Eight bead-set stones along the parting line; Delft drill-start dots, with a tight 0.85 mm bridge for the setter to review.",
        view: (0.55, 1.0),
        build: settings::half_eternity,
    },
    Template {
        name: "Gypsy trio",
        blurb: "Three rounds flush-set in domed stock; pour in Delft and cut the seats at the bench.",
        view: (0.55, 1.0),
        build: settings::gypsy_trio,
    },
];

/// The menu name of a factory blank, retaining the two cushion sizes.
pub fn stock_name(preset: &crate::imported_base::Preset) -> String {
    let size = match preset.id { "001" => " (20 mm)", "012" => " (10 mm)", _ => "" };
    format!("{} signet · {}{size}", preset.name, preset.id)
}

/// Factory plans verified Castable with clear release rays at their calibrated dimensions.
pub fn stock_sand_ready(preset: &crate::imported_base::Preset) -> bool {
    matches!(preset.id, "002" | "006" | "015" | "017")
}

/// Why a symmetric plan opens in native lost wax after its sand-master trial.
pub fn stock_process_note(preset: &crate::imported_base::Preset) -> Option<&'static str> {
    match preset.id {
        "001" | "012" | "013" => Some("Native lost wax: the drafted sand master has a Marginal verdict."),
        "003" | "007" => Some("Native lost wax: sand withdrawal support would add more than 1 mm."),
        "005" | "016" => Some("Native lost wax: the drafted sand master has a NotCastable verdict."),
        _ => None,
    }
}

/// A factory blank on its calibrated chart, using only verified sand masters.
pub fn stock(preset: &'static crate::imported_base::Preset) -> anyhow::Result<RingDesign> {
    stock_as(preset, stock_sand_ready(preset))
}

/// A factory blank as a Delft sand master with its envelope, or as native stock in lost wax.
fn stock_as(preset: &'static crate::imported_base::Preset, sand: bool) -> anyhow::Result<RingDesign> {
    use crate::imported_base::{ImportedBase, SurfaceChart, sand_master};
    use crate::castability::{CastProcess, SandProcess};
    let source = preset.load()?;
    let mut d = RingDesign::default();
    ImportedBase::attach(&mut d, if sand { sand_master(source)? } else { source })?;
    d.imported_base.as_mut().unwrap().sand_envelope = sand;
    let (length, width) = (d.shank.head.length_mm, d.profile.width_mm);
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.shank.head.length_mm = length;
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    let chart = SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() };
    d.imported_base.as_mut().unwrap().chart = Some(chart);
    if sand { SandProcess::DelftClay.apply(&mut d.draft); }
    else { CastProcess::LostWax.apply(&mut d.draft); }
    d.name = stock_name(preset);
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AlphaLibrary, BuildParams, castability::{self, CastProcess, Verdict}, csg, dfm, manufacturing, mesh, stones};

    fn params() -> BuildParams {
        BuildParams { theta_steps: 192, profile_steps: 96, ..Default::default() }
    }

    fn geometry(d: &RingDesign, lib: &AlphaLibrary) -> mesh::BuildResult {
        let out = mesh::try_build(d, lib, params()).unwrap_or_else(|e| panic!("{}: {e:#}", d.name));
        assert!(out.mesh.validate().watertight, "{} not watertight", d.name);
        assert_eq!(out.mesh.quality().degenerate_faces, 0, "{} has degenerate faces", d.name);
        assert!(out.solids.notes.is_empty(), "{}: {:?}", d.name, out.solids.notes);
        assert!(out.parts.notes.is_empty(), "{}: {:?}", d.name, out.parts.notes);
        if let Some(e) = &out.parts.evaluated {
            assert!(e.features.iter().all(|f| f.status.is_ok()), "{}: {:?}", d.name, e.features);
            for part in &e.components {
                if !part.settings.reference {
                    assert!(part.mesh.validate().watertight, "{} / {} is open", d.name, part.name);
                    assert_eq!(part.mesh.quality().degenerate_faces, 0, "{} / {} is degenerate", d.name, part.name);
                    let solid = csg::Solid { v: part.mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: part.mesh.faces.clone() };
                    assert_eq!(csg::self_crossings(&solid), 0, "{} / {} crosses itself", d.name, part.name);
                }
            }
        }
        out
    }

    #[test]
    fn every_new_starter_has_real_parts_and_matching_stones() {
        let lib = AlphaLibrary::builtin();
        assert_eq!(all().len(), 13);
        let mut seen = std::collections::HashSet::new();
        for t in all() {
            assert!(seen.insert(t.name), "duplicate template {}", t.name);
            let d = t.design();
            assert_eq!(d.name, t.name);
            let out = geometry(&d, &lib);
            let field = castability::judged_field_report(&d, &lib, &d.draft, 192, 128, Some(&out));
            let legacy = matches!(t.name, "Braided band" | "Shouldered cushion signet");
            assert_eq!(field.verdict, if legacy { Verdict::Marginal } else { Verdict::Castable }, "{}: {:?}", t.name, field.notes);
            if legacy {
                assert!(t.blurb.contains("marginal") && !dfm::findings_in(&d, &lib).is_empty(), "{} documents its inherited findings", t.name);
            } else {
                assert!(dfm::findings_in(&d, &lib).is_empty(), "{}: {:?}", t.name, dfm::findings_in(&d, &lib));
            }
            let stone_count = match t.name {
                "Cathedral solitaire" | "Bezel solitaire" | "Split-shank basket" => 1,
                "Halo" => 16,
                "Trilogy" | "Gypsy trio" => 3,
                "Toi et moi" => 2,
                "Half eternity" => 8,
                _ => 0,
            };
            let report = stones::report_built(&d, field.parting_z_mm, &out);
            assert_eq!(report.as_ref().map_or(0, |r| r.stone_count), stone_count, "{} count", t.name);
            if let Some(report) = report {
                assert_eq!(report.stone_count as usize, stones::all_stone_frames_built(&d, &out).len(), "{} preview/report disagree", t.name);
                assert_eq!(report.tight_pairs, 0, "{}: {:?}", t.name, report.crowding_note());
                for seat in &report.seats {
                    for warning in &seat.warnings {
                        assert!(t.name == "Half eternity" && warning.contains("is tight") && !warning.contains("will not fill"), "{} / {}: {warning}", t.name, seat.label);
                    }
                }
            }
            if d.draft.process == CastProcess::SandTwoPart && !legacy {
                let setup = manufacturing::Setup::from_design(&d);
                let inspected = manufacturing::inspect(&d, &lib, &setup, params()).unwrap();
                assert!(inspected.release.obstructions.is_empty(), "{}: {:?}", t.name, inspected.release.obstructions);
                assert_eq!(inspected.release.unresolved_rays, 0, "{}", t.name);
            }
        }
    }

    #[test]
    fn gallery_keeps_both_rails_through_its_side_faces_and_releases_in_delft() {
        let lib = AlphaLibrary::builtin();
        let d = settings::split_gallery();
        let params = BuildParams { theta_steps:384,profile_steps:144,..Default::default() };
        for pitch in [0.1,0.075] {
            let mut setup = manufacturing::Setup::from_design(&d);
            setup.sample_pitch_mm = pitch;
            let inspected = manufacturing::inspect(&d,&lib,&setup,params).unwrap();
            assert!(inspected.release.obstructions.is_empty());
            assert_eq!(inspected.release.unresolved_rays,0);
            assert_eq!(inspected.field.as_ref().unwrap().verdict,Verdict::Castable);
            let actual = inspected.prepared.mesh.scaled(1.0/inspected.prepared.scale);
            let wall = crate::cad::measure::thickness(&actual,0.8);
            assert!(wall.rays >= 380 && wall.unresolved == 0 && wall.below_limit == 0,"{wall:?}");
            let rows = settings::gallery_rail_sections(&actual,d.profile.width_mm);
            assert_eq!(rows.len(),11);
            for row in rows {
                assert_eq!(row.complete_sections,37,"{row:?}");
                assert_eq!(row.missing_sections,0,"{row:?}");
                assert!(row.inner_min_mm.is_some_and(|v|v>=0.8),"{row:?}");
                assert!(row.outer_min_mm.is_some_and(|v|v>=0.8),"{row:?}");
                eprintln!("gallery pitch={pitch}: {row:?}");
            }
        }
    }

    #[test]
    fn the_bezel_pattern_is_the_band_and_its_raised_locating_marks() {
        let lib = AlphaLibrary::builtin();
        let d = settings::bezel_solitaire();
        let pattern = mesh::try_build_pattern(&d, &lib, params()).unwrap();
        let mut plain = d.clone();
        plain.cad = None;
        let bare = mesh::try_build(&plain, &lib, params()).unwrap();
        let recipe = castability::pattern_parts(&d, &lib);
        assert!(!recipe.marks.is_empty());
        assert_eq!(recipe.design.layers.layers.len(), recipe.marks.len());
        assert_eq!(recipe.parts.len(), 2);
        plain.layers = recipe.design.layers.clone();
        let marked = mesh::try_build(&plain, &lib, params()).unwrap();
        assert!(pattern.mesh.vertices == marked.mesh.vertices, "only raised locating marks differ from the bare band");
        assert!(pattern.mesh.faces == marked.mesh.faces, "the sand pattern leaves both CAD parts out");
        assert!(pattern.report.volume_mm3 > bare.report.volume_mm3);
        assert!(pattern.report.volume_mm3 - bare.report.volume_mm3 < 0.5);
        let finished = geometry(&d, &lib);
        assert!(finished.report.volume_mm3 > bare.report.volume_mm3);
        assert_eq!(stones::report_built(&d, 0.0, &finished).unwrap().stone_count, 1);
    }

    #[test]
    fn every_stock_starter_opens_clean() {
        let lib = AlphaLibrary::builtin();
        let mut sand = Vec::new();
        for preset in crate::imported_base::PRESETS {
            let d = stock(preset).unwrap();
            let source = &d.imported_base.as_ref().unwrap().source;
            let master = if stock_sand_ready(preset) { format!("{} / drafted workshop master", preset.stock_name()) } else { preset.stock_name() };
            assert_eq!(source.name, master, "{}", preset.id);
            assert_eq!(d.name, stock_name(preset));
            assert_eq!(d.imported_base.as_ref().unwrap().sand_envelope, stock_sand_ready(preset));
            let out = geometry(&d, &lib);
            let report = castability::judged_field_report(&d, &lib, &d.draft, 192, 128, Some(&out));
            assert_eq!(report.verdict, Verdict::Castable, "{}: {:?}", preset.id, report.notes);
            assert!(dfm::findings_in(&d, &lib).is_empty(), "{}", preset.id);
            if stock_sand_ready(preset) {
                sand.push(preset.id);
                assert_eq!(d.draft.process, CastProcess::SandTwoPart);
                for pitch in [0.1, 0.075] {
                    let mut setup = manufacturing::Setup::from_design(&d);
                    setup.sample_pitch_mm = pitch;
                    let inspected = manufacturing::inspect(&d, &lib, &setup, params()).unwrap();
                    assert!(inspected.release.obstructions.is_empty(), "{}: {:?}", preset.id, inspected.release.obstructions);
                    assert_eq!(inspected.release.unresolved_rays, 0, "{}", preset.id);
                }
            } else {
                assert_eq!(d.draft.process, CastProcess::LostWax);
                assert!(!preset.sand_safe() || stock_process_note(preset).is_some(), "{} needs its sand-trial finding", preset.id);
            }
        }
        assert_eq!(sand, ["002", "006", "015", "017"]);
    }

    #[test]
    fn a_failed_sand_trial_fails_as_its_note_says() {
        let lib = AlphaLibrary::builtin();
        let mut failed = Vec::new();
        for preset in crate::imported_base::PRESETS.iter().filter(|p| p.sand_safe() && !stock_sand_ready(p)) {
            let note = stock_process_note(preset).unwrap_or_else(|| panic!("{} has no sand-trial note", preset.id));
            let d = stock_as(preset, true).unwrap();
            assert_eq!(d.draft.process, CastProcess::SandTwoPart);
            let finding = match mesh::try_build(&d, &lib, params()) {
                Err(e) => {
                    assert!(format!("{e:#}").contains("withdrawal support"), "{}: {e:#}", preset.id);
                    "withdrawal support".to_string()
                }
                Ok(out) => {
                    let report = castability::judged_field_report(&d, &lib, &d.draft, 192, 128, Some(&out));
                    eprintln!("sand trial {}: {:?} at {:.4}% undercut", preset.id, report.verdict, report.undercut_fraction() * 100.0);
                    assert_ne!(report.verdict, Verdict::Castable, "{} passes its sand trial", preset.id);
                    format!("{:?} verdict", report.verdict)
                }
            };
            assert!(note.contains(&finding), "{}: measured {finding}, note says {note:?}", preset.id);
            failed.push(preset.id);
        }
        assert_eq!(failed, ["001", "003", "005", "007", "012", "013", "016"]);
    }

    #[test]
    fn radial_head_symmetry_preserves_the_plan_classification() {
        for preset in crate::imported_base::PRESETS {
            let source = preset.load().unwrap();
            let floor = source.calibration.shoulder_end_mm;
            let mut edges = Vec::new();
            for f in &source.faces {
                for k in 0..3 {
                    let (mut a, mut b) = (source.vertices[f[k] as usize], source.vertices[f[(k + 1) % 3] as usize]);
                    if a[1] < floor && b[1] < floor { continue; }
                    if a[1] < floor {
                        let t = (floor - a[1]) / (b[1] - a[1]);
                        a = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t);
                    }
                    if b[1] < floor {
                        let t = (floor - b[1]) / (a[1] - b[1]);
                        b = std::array::from_fn(|k| b[k] + (a[k] - b[k]) * t);
                    }
                    edges.push(([a[0], a[2]], [b[0], b[2]]));
                }
            }
            let radii: Vec<f64> = (0..720).map(|k| {
                let (s, c) = (k as f64 * std::f64::consts::PI / 360.0).sin_cos();
                edges.iter().filter_map(|(a, b)| {
                    let delta = [b[0] - a[0], b[1] - a[1]];
                    let den = c * delta[1] - s * delta[0];
                    if den.abs() < 1e-10 { return None; }
                    let r = (a[0] * delta[1] - a[1] * delta[0]) / den;
                    let at = (a[0] * s - a[1] * c) / den;
                    (r >= 0.0 && (0.0..=1.0).contains(&at)).then_some(r)
                }).fold(0.0, f64::max)
            }).collect();
            let difference = (0..720).map(|k| (radii[k] - radii[(720-k)%720]).abs()).fold(0.0, f64::max);
            assert_eq!(preset.sand_safe(), difference <= 0.6, "{}: radial head asymmetry {difference:.4} mm", preset.id);
        }
    }

    #[test]
    fn retired_names_are_fixtures_without_replacing_current_starters() {
        assert_eq!(FIXTURE_NAMES.len(), 6);
        for name in FIXTURE_NAMES { assert!(fixture(name).is_some()); }
        assert!(fixture("Toi et moi").is_none());
        assert!(fixture("Split shank").is_none());
        for name in ["Heart signet", "Waved hexagon signet", "Cathedral solitaire stock", "Wishbone wave"] {
            assert!(!all().iter().any(|t| t.name == name));
        }
    }
}
