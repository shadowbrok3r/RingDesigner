//! Reptilia: three bands and two native signets, with portable sculpted skins.
//! cargo run -p ringdesign-core --release --example reptilia -- NEW_DIR [--draft] [SLUG]
use anyhow::{Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, Blend, BuildParams, Layer, LayerEntry, ProfileStyle, RingDesign,
    alpha::Procedural,
    field::{SeatPadLayer, SeatStyle, smoothstep},
    gem::{Gem, GemCut},
    imported_base::{ImportedBase, PRESETS, SurfaceChart},
    library, manufacturing as mf, mesh, render, reptile, stl, tiling::TilingLayer,
};
use std::{f64::consts::{PI, TAU}, path::Path};

#[path = "common/sand_stock.rs"]
mod sand_stock;

const W: usize = 2048;
const H: usize = 768;
const RELIEF: f64 = 0.52;
const ALL: [&str; 5] = ["ecdysis", "tessera", "lorica", "ophidian", "varanus"];

#[derive(Clone, Copy, Default)]
struct Sample {
    p: [f64; 3],
    nr: f64,
    along: f64,
    across: f64,
    half: f64,
}

struct Skin {
    samples: Vec<Sample>,
    circumference: f64,
    crest: Vec<usize>,
    crest_v: f64,
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

impl Skin {
    fn new(d: &RingDesign) -> Result<Self> {
        let ctx = d.field_context();
        let mut samples = vec![Sample::default(); W * H];
        let section = d.reference_loop();
        let profile: Vec<_> = section.pts.iter().filter(|p| p.surface).collect();
        for x in 0..W {
            let theta = TAU * x as f64 / W as f64;
            for y in 0..H {
                let f = y as f64 / H as f64;
                let (p, nr) = if let Some(s) = &ctx.imported_surface {
                    let frame = s.frame(theta.to_degrees(), f);
                    (frame.point, frame.normal[0] * theta.cos() + frame.normal[1] * theta.sin())
                } else {
                    let v = f * section.surface_len_mm;
                    let j = profile.partition_point(|p| p.v_mm < v).clamp(1, profile.len() - 1);
                    let (a, b) = (profile[j - 1], profile[j]);
                    let t = ((v - a.v_mm) / (b.v_mm - a.v_mm).max(1e-9)).clamp(0.0, 1.0);
                    let r = a.r + (b.r - a.r) * t;
                    ([r * theta.cos(), r * theta.sin(), a.z + (b.z - a.z) * t],
                     a.nr + (b.nr - a.nr) * t)
                };
                samples[y * W + x] = Sample { p, nr, ..Default::default() };
            }
        }
        let crest: Vec<usize> = (0..W).map(|x| {
            (1..H - 1).min_by(|a, b| samples[a * W + x].p[2].abs().total_cmp(&samples[b * W + x].p[2].abs())).unwrap()
        }).collect();
        let mut arc = vec![0.0; W];
        for x in 1..W {
            arc[x] = arc[x - 1] + distance(samples[crest[x - 1] * W + x - 1].p, samples[crest[x] * W + x].p);
        }
        let circumference = arc[W - 1] + distance(samples[crest[W - 1] * W + W - 1].p, samples[crest[0] * W].p);
        let start = arc[W / 4];
        for x in 0..W {
            let half = (0..H).map(|y| samples[y * W + x].p[2].abs()).fold(0.0, f64::max);
            let c = crest[x];
            let mut cross = vec![0.0; H];
            for y in c + 1..H {
                cross[y] = cross[y - 1] + distance(samples[y * W + x].p, samples[(y - 1) * W + x].p);
            }
            for y in (0..c).rev() {
                cross[y] = cross[y + 1] - distance(samples[y * W + x].p, samples[(y + 1) * W + x].p);
            }
            for y in 0..H {
                let s = &mut samples[y * W + x];
                s.along = (arc[x] - start + circumference * 0.5).rem_euclid(circumference) - circumference * 0.5;
                s.across = cross[y];
                s.half = half;
            }
        }
        // Exact z=0 placement for the gem, independent of atlas resolution.
        let crest_v = if let Some(s) = &ctx.imported_surface {
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..40 {
                let f = (lo + hi) * 0.5;
                if s.point(90.0, f)[2] < 0.0 { lo = f; } else { hi = f; }
            }
            (lo + hi) * 0.5 * ctx.band_v_len_mm
        } else { ctx.crest_v_mm };
        Ok(Self { samples, circumference, crest, crest_v })
    }

    /// Minimal additional stock that supports the final sculpture for a ±Z pull.
    /// Only the positive difference is deferred to subtractive bench chasing.
    fn supported(&self, finished: &[f32]) -> Vec<f32> {
        let mut cast = finished.to_vec();
        for x in 0..W {
            let c = self.crest[x];
            for side in [0, 1] {
                let range: Box<dyn Iterator<Item = usize>> = if side == 0 {
                    Box::new(1..=c)
                } else {
                    Box::new((c..H - 1).rev())
                };
                let mut radius = 0.0f64;
                let mut previous_z: Option<f64> = None;
                for y in range {
                    let at = y * W + x;
                    let s = self.samples[at];
                    let r = s.p[0].hypot(s.p[1]);
                    let desired = r + cast[at] as f64 * RELIEF * s.nr;
                    // A little positive draft avoids long zero-slope terraces
                    // turning into isolated traps between mesh sample columns.
                    let rise = previous_z.map_or(0.0, |z| (s.p[2] - z).abs() * 0.10);
                    radius = (radius + rise).max(desired);
                    previous_z = Some(s.p[2]);
                    if s.nr > 0.12 {
                        cast[at] = cast[at].max(((radius - r) / (RELIEF * s.nr)) as f32).clamp(0.0, 1.0);
                    }
                }
            }
            // The profile sampler's normalized arc can move a narrow painted
            // crest by a fraction of a texel at another mesh resolution. A
            // short constant-height crown keeps its maximum on the geometric
            // parting line; the bench map restores the sculpted centre detail.
            let crown = (0..H)
                .filter(|&y| self.samples[y * W + x].p[2].abs() < 0.52)
                .map(|y| cast[y * W + x])
                .fold(0.0f32, f32::max);
            for y in 0..H {
                let z = self.samples[y * W + x].p[2].abs();
                let stock = crown * (1.0 - smoothstep(0.32, 0.60, z)) as f32;
                cast[y * W + x] = cast[y * W + x].max(stock);
            }
        }
        cast
    }
}

fn base(slug: &str) -> Result<RingDesign> {
    let mut d = RingDesign::default();
    d.size = ringdesign_core::resize::size_from_bore(18.6)?;
    d.name = match slug {
        "ecdysis" => "Ecdysis — ventral scales",
        "tessera" => "Tessera — shield mosaic",
        "lorica" => "Lorica — crocodile armour",
        "ophidian" => "Ophidian — amethyst serpent",
        "varanus" => "Varanus — sovereign scales",
        _ => anyhow::bail!("Unknown reptile design {slug}"),
    }.into();
    if matches!(slug, "ophidian" | "varanus") {
        let id = if slug == "ophidian" { "013" } else { "017" };
        let source = PRESETS.iter().find(|p| p.id == id).unwrap().load()?;
        ImportedBase::attach(&mut d, sand_stock::sand_stock(source)?)?;
        d.size = ringdesign_core::resize::size_from_bore(18.6)?;
        d.profile.width_mm = if slug == "ophidian" { 12.0 } else { 13.0 };
        d.shank.head.length_mm = if slug == "ophidian" { 11.8 } else { 17.0 };
        let b = d.imported_base.as_mut().unwrap();
        b.sand_envelope = true;
        b.chart = Some(SurfaceChart { profile: d.profile.clone(), bore_radius_mm: 9.3 });
    } else {
        d.profile.apply_style(ProfileStyle::LowDome);
        d.profile.width_mm = match slug { "ecdysis" => 8.5, "tessera" => 9.0, _ => 8.0 };
        d.profile.thickness_mm = 2.0;
        d.profile.comfort_fit_mm = 0.20;
        d.profile.edge_round_mm = 0.22;
    }
    d.build = BuildParams { theta_steps: 1536, profile_steps: 480, ..Default::default() };
    let mut setup = mf::Setup::default();
    setup.recipe.name = format!("{} / sterling silver / cast and chased", d.name);
    setup.recipe.calibration_note = "Cast supported sculpture in Delft clay; chase only the explicitly deferred joints and scale facets. Oxidize recesses and polish scale crowns. Shrink allowance requires a shop trial.".into();
    setup.auto_parting = false;
    setup.parting_mm = 0.0;
    setup.sample_pitch_mm = 0.10;
    d.manufacturing = Some(setup);
    Ok(d)
}

fn composition(slug: &str, s: Sample, circumference: f64) -> f64 {
    let w = (s.p[2] / s.half.max(0.5)).abs();
    let z = s.across;
    let edge = 1.0 - smoothstep(s.half - 0.50, s.half - 0.18, s.p[2].abs());
    let outer = smoothstep(0.20, 0.65, s.nr);
    let pitch = circumference / match slug { "ecdysis" => 48.0, "tessera" => 36.0, "lorica" => 24.0, "ophidian" => 52.0, _ => 32.0 };
    let u = s.along / pitch;
    let small = reptile::snake(u * 1.5 + 0.17 * w, z / 0.70);
    let relief = match slug {
        "ecdysis" => {
            let ventral = reptile::ventral(u, z / (s.half * 0.56));
            let t = smoothstep(0.46, 0.64, w);
            ventral * (1.0 - t) + small * t * 0.82
        }
        "tessera" => {
            // Six broad chevron transitions connect fields of shield scales.
            // Their boundaries lean with the same direction as the shield rows.
            let panel = ((s.along / circumference * 6.0 + 0.10 * w + 0.5).rem_euclid(1.0) - 0.5).abs();
            let t = smoothstep(0.18, 0.28, panel);
            let shield = reptile::shields(u, z / 1.05);
            let plate = reptile::ventral(u * 0.5 + 0.44 * w, w);
            (shield * (1.0 - t) + plate * t) * (1.0 - 0.10 * w)
        }
        "lorica" => {
            let col = 0.5 + z / 2.15 + 0.18 * (z / s.half).powi(3);
            let plates = reptile::crocodile(u, col);
            let t = smoothstep(0.61, 0.79, w);
            plates * (1.0 - t) + small * 0.74 * t
        }
        "ophidian" => {
            // Each flank carries smaller scales as the shoulders narrow.
            let across = z / 0.84 + 0.18 * (z / s.half).powi(3);
            let scales = reptile::snake(u + 0.12 * (s.along / circumference * TAU).sin() * w, across);
            let belly = reptile::ventral(u, w);
            let palm = smoothstep(circumference * 0.34, circumference * 0.44, s.along.abs());
            let mut h = scales * (1.0 - palm) + belly * palm;
            // Two polished concentric oval borders grow out of the scale field.
            let q = ((s.p[0] / 4.15).powi(2) + (s.p[2] / 3.1).powi(2)).sqrt();
            let face = 1.0 - smoothstep(5.0, 6.4, s.along.abs());
            let oval = (1.0 - smoothstep(0.020, 0.065, (q - 1.15).abs())) * face;
            let reserve = (1.0 - smoothstep(1.02, 1.11, q)) * face;
            h = h * (1.0 - reserve) + 0.64 * reserve;
            h.max(oval * 0.96)
        }
        _ => {
            // A large central row of osteoderms becomes fine shield scales at
            // the shoulders and a broad ventral row at the palm.
            let col = 0.5 + z / 2.75 + 0.30 * (z / s.half).powi(3);
            let plates = reptile::crocodile(u + 0.08 * w.powi(2), col);
            let flank = reptile::shields(u * 1.5, z / 0.85);
            let t = smoothstep(0.49, 0.70, w);
            let h = plates * (1.0 - t) + flank * 0.79 * t;
            let palm = smoothstep(circumference * 0.35, circumference * 0.44, s.along.abs());
            h * (1.0 - palm) + reptile::ventral(u * 1.5, w) * palm
        }
    };
    if matches!(slug, "ophidian" | "varanus") {
        let cheek = reptile::snake(u * 1.5, z / 0.70) * 0.30;
        let body = smoothstep(0.25, 0.70, s.nr);
        let bore_reserve = smoothstep(9.3 + 0.45, 9.3 + 0.95, s.p[0].hypot(s.p[1]));
        ((relief * body + cheek * (1.0 - body)) * bore_reserve).clamp(0.0, 1.0)
    } else {
        (relief * edge * outer).clamp(0.0, 1.0)
    }
}

fn atlas_layer(d: &RingDesign, name: &str, bench: bool) -> LayerEntry {
    let ctx = d.field_context();
    let mut tile = TilingLayer::default_for(name, &ctx);
    tile.repeats_around = 1;
    tile.rows = 1;
    tile.v_center_mm = ctx.band_v_len_mm * 0.5;
    tile.v_span_mm = ctx.band_v_len_mm;
    tile.height_mm = RELIEF;
    tile.feather_mm = 0.0;
    let mut e = LayerEntry::new(name, Layer::Tiling(tile));
    e.bench_only = bench;
    e.blend = if bench { Blend::Subtract } else { Blend::Add };
    e
}

fn author(slug: &str) -> Result<(RingDesign, AlphaLibrary)> {
    let mut d = base(slug)?;
    let skin = Skin::new(&d)?;
    let desired: Vec<f32> = skin.samples.iter().map(|&s| composition(slug, s, skin.circumference) as f32).collect();
    let cast = skin.supported(&desired);
    let chasing: Vec<f32> = cast.iter().zip(&desired).map(|(a, b)| (a - b).max(0.0)).collect();
    let mut lib = AlphaLibrary::default();
    for (name, values, bench) in [
        (format!("{slug} / cast scale sculpture"), cast, false),
        (format!("{slug} / chased scale joints"), chasing, true),
    ] {
        let a = Alpha::new(&name, W, H, values);
        lib.insert(Alpha::from_png16(name.clone(), &a.to_png16()?)?);
        d.layers.layers.push(atlas_layer(&d, &name, bench));
    }
    if slug == "ophidian" {
        let mut seat = SeatPadLayer {
            theta_deg: 90.0, v_mm: skin.crest_v,
            style: SeatStyle::Boss, metal_true: true,
            solid: ringdesign_core::setting::SolidKind::Flush,
            through: true, blend_mm: 0.55, crown: 0.10,
            ..Default::default()
        };
        let mut gem = Gem::calibrated(GemCut::Oval, 5.0);
        gem.l_mm = 7.0;
        gem.preview_tint = Some([0.27, 0.045, 0.46]);
        seat.fit_stone(gem);
        seat.height_mm = 0.7;
        let mut entry = LayerEntry::new("Amethyst / 7 × 5 mm oval / flush setting", Layer::SeatPad(seat));
        entry.blend = Blend::Max;
        d.layers.layers.push(entry);
    }
    d.bake_all(&mut lib);
    d.embed_alphas(&lib);
    Ok((d, lib))
}

fn write(out: &Path, slug: &str, draft: bool) -> Result<()> {
    std::fs::create_dir(out)?;
    println!("Authoring {slug}");
    let (mut d, lib) = author(slug)?;
    if draft { d.build = BuildParams { theta_steps: 768, profile_steps: 320, ..d.build }; }
    let built = mesh::try_build(&d, &lib, d.build)?;
    ensure!(built.report.validation.watertight, "{slug} is not watertight");
    ensure!(built.report.quality.degenerate_faces == 0, "{slug} has degenerate triangles");
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let saved = library::load_design(out.join("design.ring.json"))?;
    let cold = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
    let rebuilt = mesh::try_build(&saved, &cold, saved.build)?;
    ensure!(rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces, "{slug}: cold load changed geometry");
    let gem = ringdesign_core::gems::preview_mesh(&d, &lib);
    let mut metal = render::Part::metal(&built.mesh, [0.82, 0.84, 0.86]);
    metal.roughness = 0.25;
    let mut parts = vec![metal];
    if let Some(g) = &gem {
        let mut p = render::Part::stone(g);
        p.tint = [0.27, 0.045, 0.46];
        parts.push(p);
        stl::write_stl(out.join("reference-amethyst.stl"), g, "Reference amethyst / do not cast")?;
    }
    for (view, yaw, pitch) in [("hero", 0.48, 0.95), ("face", 0.0, PI * 0.5), ("palm", PI, 1.10)] {
        render::write_png_parts(out.join(format!("{view}.png")), &parts, yaw, pitch, if draft { 1000 } else { 1400 })?;
    }
    stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
    let inspection = mf::inspect(&d, &lib, d.manufacturing.as_ref().unwrap(), d.build)?;
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&mf::package::report(&d, d.manufacturing.as_ref().unwrap(), &inspection, false))?)?;
    std::fs::write(out.join("mesh.json"), serde_json::to_vec_pretty(&built.report)?)?;
    stl::write_stl(out.join("casting-pattern.stl"), &inspection.prepared.mesh, &format!("{} / shrink compensated sand pattern", d.name))?;
    render::write_png(out.join("casting-pattern.png"), &inspection.prepared.mesh, 0.48, 0.95, 1000, [0.82, 0.84, 0.86])?;
    println!("{slug}: {} triangles; release {:?}; {} obstructions / {} unresolved; {}", built.mesh.faces.len(), inspection.release.status, inspection.release.obstructions.len(), inspection.release.unresolved_rays, inspection.details.join("; "));
    let art = out.join("artwork");
    std::fs::create_dir(&art)?;
    for name in d.layers.referenced_alphas() {
        if let Some(a) = lib.get(name) {
            std::fs::write(art.join(format!("{}.png", name.replace([' ', '/'], "-"))), a.to_png16()?)?;
        }
    }
    if !draft {
        ensure!(inspection.release.obstructions.is_empty() && inspection.release.unresolved_rays == 0, "{slug}: pattern needs release correction");
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ensure!(!args.is_empty(), "reptilia NEW_DIR [--draft] [ecdysis|tessera|lorica|ophidian|varanus]");
    let root = Path::new(&args[0]);
    if args.iter().any(|s| s == "--verify") {
        for slug in ALL {
            let dir = root.join(slug);
            let d = library::load_design(dir.join("design.ring.json"))?;
            let mut setup = d.manufacturing.clone().unwrap();
            let pattern = mf::prepare(&d, &AlphaLibrary::default(), &setup, d.build)?;
            setup.sample_pitch_mm = 0.075;
            let release = mf::release::analyze(&pattern.mesh, &setup)?;
            std::fs::write(dir.join("release-fine.json"), serde_json::to_vec_pretty(&release)?)?;
            println!("{slug}: {:?} mm release grid, {} obstructions / {} unresolved", release.cell_mm, release.obstructions.len(), release.unresolved_rays);
            ensure!(release.obstructions.is_empty() && release.unresolved_rays == 0, "{slug}: fine release check failed");
        }
        return Ok(());
    }
    std::fs::create_dir_all(root)?;
    let only = args.iter().find(|s| ALL.contains(&s.as_str()));
    for slug in ALL {
        if only.is_some_and(|s| s != slug) { continue; }
        write(&root.join(slug), slug, args.iter().any(|s| s == "--draft"))?;
    }
    let art = root.join("tiles");
    std::fs::create_dir_all(&art)?;
    for p in [Procedural::SnakeKeels, Procedural::VentralScutes, Procedural::CrocodileScutes, Procedural::ReptileShields] {
        std::fs::write(art.join(format!("{}.png", p.label().replace(' ', "-"))), p.generate(512).to_png16()?)?;
    }
    Ok(())
}
