//! Basiliscus on native escutcheon stock, with palewise comb and a hackle-to-scale skin.
use anyhow::{Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, Mesh, ProfileStyle, RingDesign,
    castability::{self, CastProcess, Verdict},
    csg,
    field::{Blend, Layer, LayerEntry, SeatPadLayer, SeatStyle, Window, smoothstep},
    gem::{Gem, GemCut},
    imported_base::{ImportedBase, PRESETS, SurfaceChart},
    manufacturing as mf,
    render::{self, Part},
    reptile,
    setting::{self, SolidKind, Stamp, StampTop},
    skin::{self, Atlas, Hide, Sample},
};
use serde_json::{Value, json};
use std::{f64::consts::PI, path::Path, time::Instant};

const AW: usize = 2048;
const AH: usize = 768;
const HACKLE_HEIGHT: f64 = 0.75;
const FLANK_HEIGHT: f64 = 0.40;
const BELLY_HEIGHT: f64 = 0.30;

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * t * (10.0 + t * (6.0 * t - 15.0))
}

fn window(centre: f64, span: f64) -> Window {
    let mut w = Window::around(centre, span);
    w.fade_deg = 6.0;
    w
}

fn base() -> Result<RingDesign> {
    let mut d = RingDesign::default();
    let source = PRESETS.iter().find(|p| p.id == "020").unwrap().load()?;
    ImportedBase::attach(&mut d, source)?;
    d.name = "Basiliscus — king of serpents".into();
    d.imported_base.as_mut().unwrap().sand_envelope = false;
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = 16.5;
    d.shank.head.length_mm = 16.0;
    d.size = ringdesign_core::resize::size_from_bore(18.6).unwrap();
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart {
        profile: d.profile.clone(),
        bore_radius_mm: d.inner_radius_mm(),
    });
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = 0.8;
    d.draft.min_detail_mm = 0.15;
    d.draft.min_draft_deg = 0.0;
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    Ok(d)
}

fn setup(d: &RingDesign) -> mf::Setup {
    let mut setup = mf::Setup::from_design(d);
    setup.recipe.name = "Basiliscus / Silver 925 investment".into();
    setup.recipe.alloy = "Silver 925".into();
    setup.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925")
        .unwrap()
        .shrink_pct;
    setup.sample_pitch_mm = 0.1;
    setup.bench_notes = "Investment pattern with the flush marquise seat cast in place. Chase the barbs and keel lines at the bench, finish the bearing to the measured tsavorite, burnish the lip and polish the comb and reserved edges.".into();
    setup
}

fn pale_at(a: &Atlas, z: f64) -> (f64, f64) {
    let x = a.width / 4;
    let y = (1..a.height - 1)
        .min_by(|p, q| {
            (a.at(x, *p).p[2] - z)
                .abs()
                .total_cmp(&(a.at(x, *q).p[2] - z).abs())
        })
        .unwrap();
    (a.at(x, y).theta, a.at(x, y).v)
}

fn round_scales(row: f64, col: f64) -> f64 {
    let mut best: f64 = 0.0;
    let i0 = row.floor() as i64;
    for i in [i0 - 1, i0] {
        let stagger = if i.rem_euclid(2) == 0 { 0.0 } else { 0.5 };
        let centre = (col - stagger).round() + stagger;
        let t = (row - i as f64) / 1.55;
        if !(0.0..=1.0).contains(&t) {
            continue;
        }
        let dc = (col - centre).abs();
        let half = 0.56 * (1.0 - t.powf(2.4)).max(0.0).powf(0.6);
        if dc >= half {
            continue;
        }
        best = best.max(
            smoothstep(0.0, 0.22, half - dc) * smoothstep(0.0, 0.16, 1.0 - t) * (0.35 + 0.65 * t),
        );
    }
    best
}

/// Lanceolate vanes overlap away from the pale and become short keeled scales.
fn hackle_scale(along: f64, across: f64, morph: f64) -> (f64, f64) {
    let pitch = (1.5 + 0.8 * smooth(0.0, 8.0, along)) * (1.0 - morph) + 1.1 * morph;
    let aspect = 3.2 * (1.0 - morph) + 1.2 * morph;
    let length = pitch * aspect;
    let step = length * 0.70;
    let fan = 1.0 + 0.065 * along * (1.0 - morph);
    let cross = across / fan;
    let row = (cross / pitch).round() as i64;
    let mut height: f64 = 0.0;
    let mut barbs: f64 = 0.0;
    for j in row - 1..=row + 1 {
        let y = cross - j as f64 * pitch;
        let offset = j.rem_euclid(2) as f64 * 0.5 * step;
        let i0 = ((along - offset) / step).floor() as i64;
        for i in i0 - 1..=i0 {
            let t = (along - offset - i as f64 * step) / length;
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let half = 0.125 + (0.53 * pitch - 0.125) * (PI * t).sin().max(0.0).powf(0.68);
            let edge = 1.0 - smooth((half - 0.23).max(0.0), half, y.abs());
            let tip = 1.0 - smooth(0.90, 1.0, t);
            let root = smooth(0.0, 0.13, t);
            let vane = 0.44 + 0.33 * smooth(0.0, 0.86, t) - 0.12 * (y / half.max(0.1)).powi(2);
            let rachis = 0.11 * (1.0 - smooth(0.04, 0.19, y.abs())) * smooth(0.05, 0.25, t);
            let h = (vane + rachis) * edge * tip * root;
            if h > height {
                height = h;
                let line = ((t * length - 0.58 * y.abs()) / 0.80).rem_euclid(1.0);
                barbs = (1.0 - smooth(0.04, 0.17, (line - 0.5).abs()))
                    * smooth(0.10, 0.25, y.abs())
                    * edge
                    * tip
                    * root;
            }
        }
    }
    (height, barbs)
}

fn skin_masks(a: &Atlas, hide: &Hide, s: &Sample) -> (f64, f64, f64) {
    let h = hide.at(s);
    let over_bore = smooth(a.bore + 1.15, a.bore + 1.65, s.p[0].hypot(s.p[1]));
    let edge = h.rim - h.across.abs();
    let crown = smooth(-0.25, 0.55, edge) * (1.0 - smooth(0.52, 0.80, s.n[2].abs()));
    let crease = smooth(0.40, 1.00, edge.abs());
    let neck = smooth(38.0, 46.0, s.theta)
        * (1.0 - smooth(64.0, 72.0, s.theta))
        * (1.0 - smooth(0.30, 0.75, (s.p[2] + 3.05).abs()));
    (over_bore * crease * (1.0 - neck), crown, h.along.abs())
}

fn portable(
    d: &mut RingDesign,
    lib: &mut AlphaLibrary,
    alpha: Alpha,
    height: f64,
    win: Window,
    bench: bool,
) -> Result<()> {
    let name = alpha.name.clone();
    lib.insert(Alpha::from_png16(&name, &alpha.to_png16()?)?);
    let mut layer = skin::hide_layer(d, &name, height, win);
    layer.bench_only = bench;
    if bench {
        layer.blend = Blend::Subtract;
    }
    d.layers.layers.push(layer);
    Ok(())
}

fn author(params: BuildParams) -> Result<(RingDesign, AlphaLibrary, Value)> {
    let mut d = base()?;
    d.build = params;
    let a = Atlas::of(&d, AW, AH)?;
    let hide = Hide::of(&a);
    let x = AW / 4;
    let table: Vec<f64> = (1..AH - 1)
        .filter_map(|y| {
            let s = a.at(x, y);
            (s.p[1] >= a.top - 0.3 && s.n[1] > 0.75).then_some(s.p[2])
        })
        .collect();
    ensure!(!table.is_empty(), "Escutcheon has no measured table");
    let fess = table.iter().sum::<f64>() / table.len() as f64;
    let low = table.iter().copied().fold(f64::MAX, f64::min);
    let high = table.iter().copied().fold(f64::MIN, f64::max);
    println!(
        "pale table {low:.3}..{high:.3} mm, fess {fess:.3}, top {:.3}",
        a.top
    );
    let l70 = hide.along[(AW as f64 * 160.0 / 360.0).round() as usize].abs();
    let l110 = hide.along[(AW as f64 * 200.0 / 360.0).round() as usize].abs();
    let mut lib = AlphaLibrary::builtin();
    let feather = |s: &Sample| {
        let h = hide.at(s);
        let (_, _, along) = skin_masks(&a, &hide, s);
        let m = smooth(l70, l110, along);
        let cross = h.across - fess * (1.0 - smooth(7.0, 16.0, along));
        hackle_scale(along, cross, m)
    };
    let seat_mask = |s: &Sample| {
        let z = s.p[2] - fess;
        let q = ((s.p[0].abs() / 2.6).powf(1.5) + (z.abs() / 4.6).powf(1.5)).powf(1.0 / 1.5);
        let disc = 1.0 - smooth(1.0, 1.10, q);
        let strip = 1.0 - smooth(0.7, 0.9, s.p[0].abs());
        disc.max(strip) * smooth(a.top - 0.55, a.top - 0.3, s.p[1])
    };
    let alpha = a.paint("Hackles into scales", |s| {
        let (bore, crown, _) = skin_masks(&a, &hide, s);
        let mask = seat_mask(s);
        (feather(s).0 * (1.0 - mask) + 0.35 * mask) * crown * bore
    });
    portable(
        &mut d,
        &mut lib,
        alpha,
        HACKLE_HEIGHT,
        window(90.0, 240.0),
        false,
    )?;
    let alpha = a.paint("Serpent flanks", |s| {
        let h = hide.at(s);
        let (bore, crown, along) = skin_masks(&a, &hide, s);
        let m = smooth(l70, l110 + 5.0, along);
        let rounds = round_scales(along / 2.1, h.across / 2.8);
        let shields = reptile::shields(along / 2.5, h.across / 2.8);
        (rounds * (1.0 - m) + shields * m) * (1.0 - crown) * bore
    });
    portable(
        &mut d,
        &mut lib,
        alpha,
        FLANK_HEIGHT,
        window(180.0, 360.0),
        false,
    )?;
    let reach = hide.reach();
    let alpha = a.paint("Ventral scutes", |s| {
        let h = hide.at(s);
        let (bore, crown, _) = skin_masks(&a, &hide, s);
        reptile::ventral((reach - h.along.abs()) / 2.8, h.across / 2.0) * crown * bore
    });
    portable(
        &mut d,
        &mut lib,
        alpha,
        BELLY_HEIGHT,
        window(270.0, 110.0),
        false,
    )?;
    let alpha = a.paint("Graver's barbs and keels", |s| {
        let (bore, crown, along) = skin_masks(&a, &hide, s);
        feather(s).1 * crown * bore * (1.0 - seat_mask(s)) * (1.0 - smooth(l110, l110 + 5.0, along))
    });
    portable(&mut d, &mut lib, alpha, 0.065, window(90.0, 240.0), true)?;
    let gem = Gem {
        l_mm: 8.0,
        preview_tint: Some([0.025, 0.30, 0.085]),
        ..Gem::calibrated(GemCut::Marquise, 4.0)
    };
    let (theta_deg, v_mm) = pale_at(&a, fess);
    let mut seat = SeatPadLayer {
        theta_deg,
        v_mm,
        style: SeatStyle::Boss,
        crown: 0.15,
        blend_mm: 0.45,
        metal_true: true,
        solid: SolidKind::Flush,
        through: true,
        rot_deg: 90.0,
        ..Default::default()
    };
    seat.fit_stone(gem);
    seat.height_mm = 0.60;
    let mut e = LayerEntry::new("Tsavorite, flush", Layer::SeatPad(seat));
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    let mut positions = Vec::new();
    for side in [-1.0, 1.0] {
        for k in 0..2 {
            let z = fess + side * (5.25 + k as f64 * 1.45);
            if z - 0.65 < low + 0.6 || z + 0.65 > high - 0.6 {
                continue;
            }
            let (theta_deg, v_mm) = pale_at(&a, z);
            let stamp = Stamp {
                name: format!("Comb lobe, {}", positions.len() + 1),
                theta_deg,
                v_mm,
                rot_deg: 90.0,
                outline: ringdesign_core::outline::comb_lobe(1.3, 1.1, 0.3),
                height_mm: if k == 0 { 0.50 } else { 0.38 },
                sink_mm: 0.3,
                draft_deg: 2.0,
                cut: false,
                bench: false,
                along_pull: false,
                tier: 0,
                top: StampTop::Dome { crown_mm: 0.12 },
            };
            positions.push(z);
            d.stamps.push(stamp);
        }
    }
    ensure!(
        positions.len() >= 2,
        "Measured pale has no room for the inner comb pair"
    );
    Ok((
        d,
        lib,
        json!({"fess_z_mm":fess,"table_low_z_mm":low,"table_high_z_mm":high,"comb_z_mm":positions,"morph_start_along_mm":l70,"morph_end_along_mm":l110,"stock":"020 native unmirrored","cast_tip_min_mm":0.25,"comb_width_mm":1.1}),
    ))
}

fn solid(mesh: &Mesh) -> csg::Solid {
    csg::Solid {
        v: mesh
            .vertices
            .iter()
            .map(|p| [p.0 as f64, p.1 as f64, p.2 as f64])
            .collect(),
        f: mesh.faces.clone(),
    }
}

fn pair_png(path: &Path, left: &[Part<'_>], right: &[Part<'_>], edge: usize) -> Result<()> {
    let a = render::render_parts_ss(left, 0.55, 1.0, edge, edge, 2);
    let b = render::render_parts_ss(right, 0.55, 1.0, edge, edge, 2);
    let mut image = Vec::with_capacity(edge * edge * 6);
    for y in 0..edge {
        image.extend_from_slice(&a[y * edge * 3..(y + 1) * edge * 3]);
        image.extend_from_slice(&b[y * edge * 3..(y + 1) * edge * 3]);
    }
    image::save_buffer(
        path,
        &image,
        (2 * edge) as u32,
        edge as u32,
        image::ColorType::Rgb8,
    )?;
    Ok(())
}

fn write(out: &Path, draft: bool, verify: bool) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let (theta_steps, profile_steps) = if draft { (768, 320) } else { (1536, 448) };
    let params = BuildParams {
        theta_steps,
        profile_steps,
        refine: None,
        ..Default::default()
    };
    let start = Instant::now();
    let (d, lib, composition) = author(params)?;
    let built = ringdesign_core::mesh::try_build(&d, &lib, params)?;
    println!(
        "built {} faces in {:.1} s; {:?}",
        built.mesh.faces.len(),
        start.elapsed().as_secs_f64(),
        built.solids.notes
    );
    let gems = ringdesign_core::gems::built_meshes(&d, &lib, &built);
    let mut parts = vec![Part::metal(&built.mesh, render::GOLD)];
    parts.extend(gems.iter().map(|(m, tint)| Part::tinted_stone(m, *tint)));
    let edge = if draft { 1100 } else { 1600 };
    for (name, yaw, pitch) in [
        ("hero", 0.55, 1.0),
        ("face", 0.0, PI * 0.5),
        ("palm", PI, 1.05),
        ("side", 0.0, 0.0),
        ("shoulder", 1.05, 0.8),
        ("reverse", PI, 0.45),
    ] {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    render::write_png_parts(out.join("face-300.png"), &parts, 0.0, PI * 0.5, 300)?;
    render::write_png_parts(out.join("stones.png"), &parts, 0.1, 1.25, edge)?;
    let bare = ringdesign_core::mesh::try_build(&base()?, &lib, params)?;
    pair_png(
        &out.join("bare-vs-finished.png"),
        &[Part::metal(&bare.mesh, render::GOLD)],
        &parts,
        if draft { 700 } else { 1100 },
    )?;
    let cross = csg::self_crossings(&solid(&built.mesh));
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let dfm = ringdesign_core::dfm::findings_in(&d, &lib);
    let stone_report = ringdesign_core::stones::report_built(&d, 0.0, &built);
    let stone_count = stone_report.as_ref().map_or(0, |r| r.stone_count as usize);
    let warnings: Vec<_> = stone_report
        .iter()
        .flat_map(|r| r.seats.iter().flat_map(|s| s.warnings.iter()))
        .cloned()
        .collect();
    let inspection = mf::inspect(&d, &lib, &setup(&d), params)?;
    let pattern_cross = csg::self_crossings(&solid(&inspection.prepared.mesh));
    let pattern_validation = inspection.prepared.mesh.validate();
    let pattern_quality = inspection.prepared.mesh.quality();
    let unstruck = ringdesign_core::mesh::try_build(&setting::without_solids(&d), &lib, params)?;
    let unstruck = solid(&unstruck.mesh);
    let ctx = d.field_context();
    let mut made_gates = Vec::new();
    for stamp in &d.stamps {
        ensure!(stamp.tier == 0, "Comb inspection expects tier zero");
        let part = stamp
            .solid(&stamp.frame(&d, &ctx), &unstruck)
            .map_err(anyhow::Error::msg)?;
        let check = part.check(true);
        made_gates.push(json!({"name":stamp.name,"self_crossings":check.self_crossings,"zero_area_faces":check.zero_area_faces,"open_edges":check.open_edges,"repeated_edges":check.repeated_edges}));
    }
    for (stone, frame) in ringdesign_core::stones::stone_frames(&d) {
        let stand = stone.stand_off_mm();
        let bare = [
            frame.girdle[0] - frame.normal[0] * stand,
            frame.girdle[1] - frame.normal[1] * stand,
        ];
        let r = bare[0].hypot(bare[1]);
        let outward = (frame.normal[0] * bare[0] + frame.normal[1] * bare[1]) / r;
        let through = (stone.seat.through && outward > 0.75)
            .then(|| stand + (r - d.inner_radius_mm()).max(0.0) / outward + 0.6);
        let fit = setting::Fit {
            surface_z: stone.seat.height_mm - stand,
            through_mm: through,
            prongs: 0,
        };
        let parts =
            setting::parts(stone.gem, stone.seat.solid, fit).map_err(|e| anyhow::anyhow!("{e}"))?;
        for part in parts.add.iter().chain(&parts.cut) {
            let check = part.check(true);
            made_gates.push(json!({"name":stone.label,"self_crossings":check.self_crossings,"zero_area_faces":check.zero_area_faces,"open_edges":check.open_edges,"repeated_edges":check.repeated_edges}));
        }
    }
    ringdesign_core::library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let mut cold = Value::Null;
    if verify {
        let t = Instant::now();
        let saved = ringdesign_core::library::load_design(out.join("design.ring.json"))?;
        let load_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let library = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let bake_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let rebuilt = ringdesign_core::mesh::try_build(&saved, &library, params)?;
        let build_ms = t.elapsed().as_secs_f64() * 1000.0;
        let same = rebuilt.mesh.vertices == built.mesh.vertices
            && rebuilt.mesh.faces == built.mesh.faces
            && rebuilt.mesh.normals == built.mesh.normals;
        cold = json!({"identical_vertices_faces_normals":same,"read_ms":load_ms,"bake_ms":bake_ms,"build_ms":build_ms});
        ensure!(same, "Cold design geometry changed");
    }
    let gates = [
        (
            "finished mesh watertight with zero degenerates",
            built.report.validation.watertight && built.report.quality.degenerate_faces == 0,
        ),
        ("finished mesh has zero self crossings", cross == 0),
        (
            "investment pattern watertight with zero degenerates and crossings",
            pattern_validation.watertight
                && pattern_quality.degenerate_faces == 0
                && pattern_cross == 0,
        ),
        (
            "all solids and stamps resolved",
            built.solids.notes.is_empty()
                && built.parts.notes.is_empty()
                && built.solids.stamped == d.stamps.len()
                && built.solids.resolved == 1,
        ),
        (
            "every made solid closed without crossings or degenerates",
            made_gates.iter().all(|g| {
                g["self_crossings"] == 0
                    && g["zero_area_faces"] == 0
                    && g["open_edges"] == 0
                    && g["repeated_edges"] == 0
            }),
        ),
        (
            "lost wax verdict and 0.8 mm fill",
            field.process == CastProcess::LostWax
                && field.verdict == Verdict::Castable
                && field.thinnest_wall_mm >= 0.8,
        ),
        ("zero DFM findings", dfm.is_empty()),
        (
            "one stone in report and preview, no warnings or crowding",
            stone_count == 1
                && gems.len() == 1
                && warnings.is_empty()
                && stone_report.as_ref().is_some_and(|r| r.tight_pairs == 0),
        ),
    ];
    let report = json!({"name":d.name,"build":{"theta_steps":theta_steps,"profile_steps":profile_steps,"faces":built.mesh.faces.len(),"build_ms":built.report.build_ms},"composition":composition,"geometry":{"validation":built.report.validation,"quality":built.report.quality,"self_crossings":cross,"volume_mm3":built.report.volume_mm3},"pattern":{"validation":pattern_validation,"quality":pattern_quality,"self_crossings":pattern_cross},"solids":{"resolved":built.solids.resolved,"stamped":built.solids.stamped,"notes":built.solids.notes,"parts_notes":built.parts.notes},"made_parts":made_gates,"field":field,"dfm":dfm.iter().map(|f| json!({"label":f.label,"message":f.message})).collect::<Vec<_>>(),"stones":{"reported":stone_count,"previewed":gems.len(),"warnings":warnings},"process":{"name":"LostWax","release_gates":"Not applicable to native upright investment pattern","draft_clamp":"Not applied to lost wax","minimum_section_mm":0.8},"gates":gates.iter().map(|(name, pass)|json!({"gate":name,"pass":pass})).collect::<Vec<_>>(),"cold_reload":cold});
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    std::fs::write(
        out.join("mesh.json"),
        serde_json::to_vec_pretty(&built.report)?,
    )?;
    if verify {
        std::fs::write(
            out.join("verification.json"),
            serde_json::to_vec_pretty(
                &json!({"cold_design_reload":cold,"design_bytes":std::fs::metadata(out.join("design.ring.json"))?.len(),"source_kinds":["native stock","four painted Atlas PNG16 masks","seat","comb stamps"],"template_gate":"pending cold graph lift"}),
            )?,
        )?;
    }
    ringdesign_core::stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
    ringdesign_core::stl::write_stl(
        out.join("casting-pattern.stl"),
        &inspection.prepared.mesh,
        "Basiliscus / investment pattern",
    )?;
    for (mesh, _) in &gems {
        ringdesign_core::stl::write_stl(
            out.join("reference-tsavorite.stl"),
            mesh,
            "Tsavorite 8 x 4 mm",
        )?;
    }
    std::fs::create_dir_all(out.join("artwork"))?;
    for name in d.layers.referenced_alphas() {
        if let Some(alpha) = lib.get(name) {
            std::fs::write(
                out.join("artwork")
                    .join(format!("{}.png", name.replace([' ', '\''], "-"))),
                alpha.to_png16()?,
            )?;
        }
    }
    println!(
        "crossings {cross}, pattern crossings {pattern_cross}, wall {:.3} mm, DFM {}, stones {stone_count}/{}",
        field.thinnest_wall_mm,
        dfm.len(),
        gems.len()
    );
    for (name, pass) in &gates {
        println!("{}: {name}", if *pass { "pass" } else { "FAIL" });
    }
    let failed: Vec<_> = gates.iter().filter(|g| !g.1).map(|g| g.0).collect();
    ensure!(failed.is_empty(), "Failed gates: {failed:?}");
    Ok(())
}

fn probe(out: &Path) -> Result<()> {
    let d = ringdesign_core::library::load_design(out.join("design.ring.json"))?;
    let lib = mf::source_library(&d, &AlphaLibrary::default()).into_owned();
    let atlas = Atlas::of(&d, AW, AH)?;
    let hide = Hide::of(&atlas);
    let mut part = setting::without_solids(&d);
    part.layers.layers.clear();
    let params = BuildParams {
        theta_steps: 768,
        profile_steps: 320,
        refine: None,
        ..Default::default()
    };
    let mut rows = Vec::new();
    for step in 0..=d.layers.layers.len() {
        if step > 0 {
            part.layers.layers.push(d.layers.layers[step - 1].clone());
        }
        let layer = part.layers.layers.last_mut();
        if let Some(e) = layer {
            if let Layer::SeatPad(s) = &mut e.layer {
                s.solid = SolidKind::None;
            }
        }
        let built = ringdesign_core::mesh::try_build(&part, &lib, params)?;
        let s = solid(&built.mesh);
        let crossings = csg::self_crossings(&s);
        let name = if step == 0 {
            "Bare stock".to_string()
        } else {
            d.layers.layers[step - 1].name.clone()
        };
        println!("{name}: {crossings}");
        let mut angular = Vec::new();
        let mut localized = Vec::new();
        if crossings > 0 {
            let mut bins = vec![Vec::new(); 180];
            for f in &s.f {
                let p = s.v[f[0] as usize];
                let i = (p[1].atan2(p[0]).to_degrees().rem_euclid(360.0) / 2.0) as usize;
                bins[i.min(179)].push(*f);
            }
            let mut local = csg::Solid {
                v: s.v,
                f: Vec::new(),
            };
            for b in 0..180 {
                local.f = [
                    bins[(b + 179) % 180].as_slice(),
                    bins[b].as_slice(),
                    bins[(b + 1) % 180].as_slice(),
                ]
                .concat();
                let n = csg::self_crossings(&local);
                if n > 0 {
                    angular.push([b * 2 + 1, n]);
                    if step == 1 || step == 2 {
                        let all = local.f.clone();
                        for zbin in -20..20 {
                            let z = zbin as f64 * 0.5;
                            local.f = all
                                .iter()
                                .copied()
                                .filter(|f| {
                                    f.iter().any(|i| (local.v[*i as usize][2] - z).abs() < 0.5)
                                })
                                .collect();
                            let count = csg::self_crossings(&local);
                            if count > 0 {
                                localized.push(
                                    json!({"theta_deg":b * 2 + 1,"z_mm":z,"crossings":count}),
                                );
                            }
                        }
                    }
                }
            }
        }
        let mut sites = Vec::new();
        if step == 1 || step == 2 {
            for point in crossing_sites(&built.mesh) {
                let theta = point[1].atan2(point[0]).to_degrees().rem_euclid(360.0);
                let x = (theta / 360.0 * AW as f64).round() as usize % AW;
                let sample = (0..AH)
                    .map(|y| atlas.at(x, y))
                    .min_by(|a, b| {
                        let dist = |s: &Sample| {
                            s.p.iter()
                                .zip(point)
                                .map(|(v, w)| (v - w).powi(2))
                                .sum::<f64>()
                        };
                        dist(a).total_cmp(&dist(b))
                    })
                    .unwrap();
                let hp = hide.at(sample);
                sites.push(json!({"point":point,"theta_deg":theta,"bare_point":sample.p,"normal":sample.n,"v_mm":sample.v,"rim_mm":hp.rim,"across_mm":hp.across,"edge_distance_mm":hp.rim-hp.across.abs()}));
            }
        }
        rows.push(json!({"layer":name,"crossings":crossings,"angular":angular,"localized":localized,"sites":sites}));
    }
    std::fs::write(
        out.join("crossing-probe.json"),
        serde_json::to_vec_pretty(&rows)?,
    )?;
    Ok(())
}

fn crossing_sites(mesh: &Mesh) -> Vec<[f64; 3]> {
    use std::collections::HashMap;
    let s = solid(mesh);
    let side = |a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]| {
        let xyz = |p: [f64; 3]| robust::Coord3D {
            x: p[0],
            y: p[1],
            z: p[2],
        };
        robust::orient3d(xyz(a), xyz(b), xyz(c), xyz(d))
    };
    let pierce = |p: [f64; 3], q: [f64; 3], tri: [[f64; 3]; 3]| {
        let [a, b, c] = tri;
        let sp = side(a, b, c, p);
        let sq = side(a, b, c, q);
        if sp == 0.0 || sq == 0.0 || (sp > 0.0) == (sq > 0.0) {
            return None;
        }
        let signs = [side(p, q, a, b), side(p, q, b, c), side(p, q, c, a)];
        if signs.iter().all(|v| *v > 0.0) || signs.iter().all(|v| *v < 0.0) {
            let t = sp / (sp - sq);
            Some(std::array::from_fn(|k| p[k] + t * (q[k] - p[k])))
        } else {
            None
        }
    };
    let boxes: Vec<([i32; 3], [i32; 3])> =
        s.f.iter()
            .map(|f| {
                let lo = std::array::from_fn(|k| {
                    f.iter()
                        .map(|i| s.v[*i as usize][k])
                        .fold(f64::MAX, f64::min)
                });
                let hi = std::array::from_fn(|k| {
                    f.iter()
                        .map(|i| s.v[*i as usize][k])
                        .fold(f64::MIN, f64::max)
                });
                (
                    lo.map(|v| (v / 0.3).floor() as i32),
                    hi.map(|v| (v / 0.3).floor() as i32),
                )
            })
            .collect();
    let mut cells: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
    for (i, (lo, hi)) in boxes.iter().enumerate() {
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    cells.entry([x, y, z]).or_default().push(i);
                }
            }
        }
    }
    let mut stamp = vec![usize::MAX; s.f.len()];
    let mut points = Vec::new();
    for (i, (lo, hi)) in boxes.iter().enumerate() {
        let a = s.f[i].map(|k| s.v[k as usize]);
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    for &j in &cells[&[x, y, z]] {
                        if j <= i || stamp[j] == i {
                            continue;
                        }
                        stamp[j] = i;
                        if s.f[i].iter().any(|v| s.f[j].contains(v)) {
                            continue;
                        }
                        let b = s.f[j].map(|k| s.v[k as usize]);
                        let point = (0..3)
                            .find_map(|k| pierce(a[k], a[(k + 1) % 3], b))
                            .or_else(|| (0..3).find_map(|k| pierce(b[k], b[(k + 1) % 3], a)));
                        if let Some(p) = point {
                            points.push(p);
                        }
                    }
                }
            }
        }
    }
    points
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let out = args
        .iter()
        .find(|s| !s.starts_with("--"))
        .map_or("showcase/bestiarium/basiliscus", String::as_str);
    if args.iter().any(|a| a == "--probe-saved") {
        return probe(Path::new(out));
    }
    if args.iter().any(|a| a == "--relief-sweep") {
        let d = ringdesign_core::library::load_design(Path::new(out).join("design.ring.json"))?;
        let lib = mf::source_library(&d, &AlphaLibrary::default()).into_owned();
        let mut rows = Vec::new();
        for amplitude in [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.75, 1.0] {
            let mut part = setting::without_solids(&d);
            part.layers
                .layers
                .retain(|l| matches!(l.layer, Layer::Tiling(_)));
            for layer in &mut part.layers.layers {
                if let Layer::Tiling(t) = &mut layer.layer {
                    t.height_mm *= amplitude;
                }
            }
            let p = BuildParams {
                theta_steps: 768,
                profile_steps: 320,
                refine: None,
                ..Default::default()
            };
            let built = ringdesign_core::mesh::try_build(&part, &lib, p)?;
            let crossings = csg::self_crossings(&solid(&built.mesh));
            println!("amplitude {amplitude}: {crossings} crossings");
            rows.push(json!({"amplitude":amplitude,"crossings":crossings}));
        }
        std::fs::write(
            Path::new(out).join("relief-sweep.json"),
            serde_json::to_vec_pretty(&rows)?,
        )?;
        return Ok(());
    }
    write(
        Path::new(out),
        args.iter().any(|a| a == "--draft"),
        args.iter().any(|a| a == "--verify"),
    )
}
