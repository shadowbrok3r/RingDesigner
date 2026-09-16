//! Thalassa: a portable marine signet authored with the application's geometry tools.
//! cargo run --release -p ringdesign-core --example thalassa -- OUTPUT [--draft]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    alpha::{ProcRecipe, Procedural},
    castability::CastProcess,
    curve::{CurveLayer, WireProfile},
    field::{
        Blend, BorderLayer, BorderProfile, Decal, DecalLayer, FluteProfile, FlutesLayer,
        GroupLayer, Layer, LayerEntry, LayerStack, MilgrainLayer, OpenworkLayer, SeatPadLayer,
        SeatStyle, SideFacePick, SignetOutline, VGate, Window,
    },
    gem::{Gem, GemCut},
    interaction::{
        paint::{Brush, Gesture},
        picking::Hit,
    },
    manufacturing::{self as mf, Setup},
    svg::SvgAlpha,
    text::{TextAlpha, TextFont},
    tiling::{TilingLayer, WarpField},
};
use std::{f64::consts::TAU, path::Path};

fn svg(body: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><filter id="soft"><feGaussianBlur stdDeviation="0.55"/></filter></defs><g filter="url(#soft)">{body}</g></svg>"#
    )
}
fn line(path: &str, width: f64) -> String {
    [(1.45,"#bbb"),(1.0,"#555"),(0.55,"#111")].into_iter().map(|(k,c)|format!(r##"<path d="{path}" fill="none" stroke="{c}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"##,width*k)).collect()
}
fn art(d: &mut RingDesign, name: &str, body: String) {
    d.svgs.push(SvgAlpha {
        name: name.into(),
        svg: svg(&body),
        invert: false,
    });
}
fn shell() -> String {
    let mut s = line(
        "M17 68 C5 43 17 13 49 12 C82 10 96 37 84 64 Q76 76 61 81 L61 89 Q50 96 39 89 L39 81 Q24 77 17 68",
        2.6,
    );
    for i in 0..11 {
        let x = 16.0 + i as f64 * 6.8;
        let y = 16.0 + 0.016 * (x - 50.0).powi(2);
        s += &line(
            &format!(
                "M{} 84 C{} 60 {} 36 {x} {y}",
                45.0 + i as f64,
                35.0 + i as f64 * 3.0,
                x
            ),
            2.0,
        );
    }
    s += &line("M29 77 Q50 88 72 76 M35 86 Q50 93 65 85", 2.2);
    s
}
fn scroll() -> String {
    let mut s = line(
        "M11 85 C16 62 35 64 52 56 C83 43 82 12 60 13 C42 14 39 41 58 40 C72 39 65 24 57 29",
        4.1,
    );
    s += &line(
        "M21 88 C31 73 47 83 67 71 C86 61 92 43 85 29 M12 61 C20 37 32 40 40 27",
        2.0,
    );
    s
}
fn frame(pearls: bool) -> String {
    let mut s = String::new();
    if pearls {
        for i in 0..42 {
            let a = TAU * i as f64 / 42.0;
            s += &format!(
                r##"<circle cx="{}" cy="{}" r="1.35" fill="#222"/>"##,
                50.0 + 42.0 * a.cos(),
                50.0 + 36.0 * a.sin()
            );
        }
    } else {
        for (rx, ry) in [(46., 40.), (39., 33.)] {
            s += &line(
                &format!(
                    "M{} 50 A{rx} {ry} 0 1 0 {} 50 A{rx} {ry} 0 1 0 {} 50",
                    50.0 - rx,
                    50.0 + rx,
                    50.0 - rx
                ),
                2.0,
            );
        }
    }
    s
}
fn compass() -> String {
    let mut s = String::new();
    for i in 0..8 {
        let a = TAU * i as f64 / 8.0;
        let r = if i % 2 == 0 { 42.0 } else { 27.0 };
        s += &line(
            &format!(
                "M{} {} L{} {}",
                50.0 + 7.0 * a.cos(),
                50.0 + 7.0 * a.sin(),
                50.0 + r * a.cos(),
                50.0 + r * a.sin()
            ),
            if i % 2 == 0 { 5.0 } else { 3.0 },
        );
    }
    s
}
fn stamp(name: &str, alpha: &str, theta: f64, v: f64, size: f64, height: f64) -> LayerEntry {
    LayerEntry::new(
        name,
        Layer::Decals(DecalLayer {
            alpha: alpha.into(),
            decals: vec![Decal {
                theta_deg: theta,
                v_mm: v,
                size_mm: size,
                height_mm: height,
                ..Default::default()
            }],
            feather_mm: 0.18,
            invert: false,
        }),
    )
}

fn design(lib: &mut AlphaLibrary) -> RingDesign {
    let mut d = RingDesign::default();
    d.name = "Thalassa — tidal reliquary signet".into();
    d.size = ringdesign_core::resize::size_from_bore(18.3).unwrap();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = 14.2;
    d.profile.thickness_mm = 2.9;
    d.profile.crown_mm = 1.0;
    d.profile.flatten_sides();
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.2;
    d.profile.side_draft_deg = 4.0;
    d.shank.apply_signet(14.2);
    d.shank.amount = 0.66;
    d.shank.head.outline = SignetOutline::Oval;
    d.shank.head.length_mm = 17.2;
    d.shank.head.rise_mm = 1.4;
    d.shank.head.table_dome_mm = 0.15;
    d.shank.head.dome = 1.0;
    d.shank.head.rim_round_mm = 0.6;
    d.shank.head.loft = 0.0;
    d.shank.head.body_fair = 0.88;
    d.shank.head.crest_round_mm = 0.6;
    d.shank.head.shoulder_deg = 50.0;
    d.shank.head.swell_deg = Some(85.0);
    d.shank.head.hollow_mm = 0.35;
    let mut setup = Setup::default();
    setup.recipe.name = "Thalassa / investment / 14k gold".into();
    setup.recipe.alloy = "Gold 14k".into();
    setup.recipe.process = CastProcess::LostWax;
    setup.recipe.sand = None;
    setup.recipe.min_section_mm = 0.8;
    setup.recipe.min_detail_mm = 0.18;
    setup.recipe.min_draft_deg = 0.0;
    setup.recipe.shrink_pct = ringdesign_core::metal::find("Gold 14k").unwrap().shrink_pct;
    setup.sample_pitch_mm = 0.15;
    setup.bench_notes="Investment-cast body. Shell relief, scrolls, pearl borders, recessed gallery and raised seat stock remain editable. Set the seven separate stones after casting; finish bearings and pavilion clearance to the actual stones. Retain satin recesses and polish raised ribs. Pull animation is a geometric study of an expendable pattern, not a sand-casting claim.".into();
    d.draft.process = CastProcess::LostWax;
    d.draft.sand = None;
    d.draft.min_section_mm = 0.8;
    d.draft.min_detail_mm = 0.18;
    d.draft.min_draft_deg = 0.0;
    d.manufacturing = Some(setup);
    art(&mut d, "Thalassa shell", shell());
    art(&mut d, "Tidal scroll", scroll());
    art(&mut d, "Oval tide frame", frame(false));
    art(&mut d, "Seed pearl oval", frame(true));
    art(&mut d, "Mariner compass", compass());
    d.recipes.push(ProcRecipe {
        name: "Current weave".into(),
        kind: Procedural::GuillocheWeave,
        repeats: 1,
        gamma: 1.1,
        ..Default::default()
    });
    d.recipes.push(ProcRecipe {
        name: "Sea mist satin".into(),
        kind: Procedural::Hammered,
        repeats: 1,
        gamma: 1.25,
        ..Default::default()
    });
    d.texts.push(TextAlpha {
        name: "Thalassa hallmark".into(),
        text: "THALASSA".into(),
        font: TextFont::Serif,
        tracking: 0.12,
    });
    d.bake_all(lib);
    let ctx = d.field_context();
    let (lo, hi) = ctx.side_faces_std().unwrap().low.unwrap();
    let side = (lo + hi) * 0.5;
    let span = hi - lo;
    let mut satin = TilingLayer::default_for("Sea mist satin", &ctx);
    satin.fit_to_side_faces(&ctx, 80.0);
    satin.height_mm = 0.045;
    satin.repeats_around = 14;
    let mut e = LayerEntry::new("Sea mist cheek ground", Layer::Tiling(satin));
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    let mut weave = TilingLayer::default_for("Current weave", &ctx);
    weave.fit_to_side_faces(&ctx, 80.0);
    weave.height_mm = 0.16;
    weave.repeats_around = 18;
    weave.shear = 0.22;
    weave.warp = Some(WarpField {
        points: (0..12)
            .map(|i| {
                let u = i as f64 / 12.0;
                [u, side + 0.13 * (TAU * u * 3.0).sin()]
            })
            .collect(),
        strength: 0.7,
        falloff_mm: span,
    });
    let mut e = LayerEntry::new("Warped tidal guilloche", Layer::Tiling(weave));
    e.window = Window::except(90.0, 80.0);
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    for (name, v) in [
        ("Inner rope rail", lo + span * 0.13),
        ("Outer polished rail", hi - span * 0.13),
    ] {
        let e = LayerEntry::new(
            name,
            Layer::Border(BorderLayer {
                v_mm: v,
                width_mm: 0.28,
                height_mm: 0.18,
                profile: BorderProfile::Round,
                mirror: true,
                ..Default::default()
            }),
        );
        d.layers.layers.push(e);
    }
    let mut e = LayerEntry::new(
        "Twin seed-pearl cheek borders",
        Layer::Milgrain(MilgrainLayer {
            v_mm: lo + span * 0.34,
            bead_diameter_mm: 0.4,
            beads_around: 112,
            height_mm: 0.18,
            mirror: true,
        }),
    );
    e.window = Window::except(90.0, 75.0);
    d.layers.layers.push(e);
    let mut face = LayerStack::default();
    face.layers.push(stamp(
        "Double oval tide frame",
        "Oval tide frame",
        90.0,
        ctx.crest_v_mm,
        12.8,
        0.24,
    ));
    face.layers.push(stamp(
        "Pearled cartouche",
        "Seed pearl oval",
        90.0,
        ctx.crest_v_mm,
        12.2,
        0.20,
    ));
    face.layers.push(stamp(
        "Radiating shell ribs",
        "Thalassa shell",
        90.0,
        ctx.crest_v_mm,
        9.2,
        0.28,
    ));
    d.layers.layers.push(LayerEntry::new(
        "Shell cartouche / three editable ornaments",
        Layer::Group(GroupLayer {
            stack: face,
            recipe: None,
        }),
    ));
    for (theta, flip) in [(43.0, false), (137.0, true)] {
        let mut e = stamp(
            "Sculpted tidal shoulder scroll",
            "Tidal scroll",
            theta,
            ctx.crest_v_mm,
            6.0,
            0.3,
        );
        if let Layer::Decals(l) = &mut e.layer {
            l.decals[0].flip = flip;
            l.decals[0].rotation_deg = if flip { 80.0 } else { -80.0 };
        }
        d.layers.layers.push(e);
    }
    for (name, v) in [
        ("Lower flowing rail", ctx.crest_v_mm - 3.3),
        ("Upper flowing rail", ctx.crest_v_mm + 3.3),
    ] {
        let points = (0..13)
            .map(|i| {
                let x = i as f64 / 12.0;
                [x, v + 0.45 * (TAU * x * 3.0).sin()]
            })
            .collect();
        let mut e = LayerEntry::new(
            name,
            Layer::Curve(CurveLayer {
                points,
                repeats_around: 1,
                closed: true,
                width_mm: 0.28,
                height_mm: 0.17,
                profile: WireProfile::Round,
                taper: 0.0,
                mirror_v: false,
            }),
        );
        e.window = Window::except(90.0, 80.0);
        e.window.fade_deg = 12.0;
        d.layers.layers.push(e);
    }
    for (name, theta, v, cut, width, style) in [
        (
            "Central oval sea-glass bezel",
            90.0,
            ctx.crest_v_mm,
            GemCut::Oval,
            3.4,
            SeatStyle::Bezel,
        ),
        (
            "East pearl setting",
            64.0,
            ctx.crest_v_mm,
            GemCut::Round,
            1.3,
            SeatStyle::Bezel,
        ),
        (
            "West pearl setting",
            116.0,
            ctx.crest_v_mm,
            GemCut::Round,
            1.3,
            SeatStyle::Bezel,
        ),
        (
            "North pearl setting",
            90.0,
            ctx.crest_v_mm + 4.5,
            GemCut::Round,
            1.25,
            SeatStyle::GypsyMound,
        ),
        (
            "South pearl setting",
            90.0,
            ctx.crest_v_mm - 4.5,
            GemCut::Round,
            1.25,
            SeatStyle::GypsyMound,
        ),
        (
            "Port shoulder stone",
            22.0,
            ctx.crest_v_mm,
            GemCut::Round,
            1.4,
            SeatStyle::GypsyMound,
        ),
        (
            "Starboard shoulder stone",
            158.0,
            ctx.crest_v_mm,
            GemCut::Round,
            1.4,
            SeatStyle::GypsyMound,
        ),
    ] {
        let mut seat = SeatPadLayer {
            theta_deg: theta,
            v_mm: v,
            style,
            bezel_wall_mm: 0.34,
            recess_mm: 0.38,
            blend_mm: 0.3,
            metal_true: true,
            ..Default::default()
        };
        seat.fit_stone(Gem::calibrated(cut, width));
        d.layers
            .layers
            .push(LayerEntry::new(name, Layer::SeatPad(seat)));
    }
    let mut points = stamp(
        "Four compass glints",
        "Mariner compass",
        90.0,
        ctx.crest_v_mm,
        1.1,
        0.14,
    );
    if let Layer::Decals(l) = &mut points.layer {
        l.decals.clear();
        for theta in [77.0, 103.0] {
            for v in [ctx.crest_v_mm - 2.9, ctx.crest_v_mm + 2.9] {
                l.decals.push(Decal {
                    theta_deg: theta,
                    v_mm: v,
                    size_mm: 1.1,
                    height_mm: 0.14,
                    ..Default::default()
                });
            }
        }
    }
    d.layers.layers.push(points);
    let mut flutes = LayerEntry::new(
        "Axial shell reeds on lower shank",
        Layer::Flutes(FlutesLayer {
            count: 34,
            profile: FluteProfile::Round,
            width_mm: 0.5,
            height_mm: 0.12,
            lean: 0.05,
            along: false,
        }),
    );
    flutes.window = Window::except(90.0, 145.0);
    flutes.window.fade_deg = 12.0;
    d.layers.layers.push(flutes);
    let mut recess = TilingLayer::default_for("Tidal scroll", &ctx);
    recess.fit_to_side_faces(&ctx, 80.0);
    recess.height_mm = 1.0;
    recess.repeats_around = 20;
    recess.edge_mm = 0.18;
    let mut e = LayerEntry::new(
        "Recessed wave gallery / retained floor",
        Layer::Openwork(OpenworkLayer {
            tiling: recess,
            depth_mm: 0.7,
            keep_mm: 1.25,
        }),
    );
    e.window = Window::except(90.0, 120.0);
    e.window.fade_deg = 12.0;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    e.blend = Blend::Add;
    d.layers.layers.push(e);
    let mut weave = TilingLayer::default_for("Current weave", &ctx);
    weave.repeats_around = 20;
    weave.rows = 2;
    weave.v_span_mm = 8.4;
    weave.height_mm = 0.07;
    weave.shear = 0.4;
    let mut e = LayerEntry::new("Engraved current ground", Layer::Tiling(weave));
    e.blend = Blend::Subtract;
    e.window = Window::except(90.0, 105.0);
    e.window.fade_deg = 15.0;
    d.layers.layers.push(e);
    let brush = Brush {
        diameter_mm: 0.32,
        depth_mm: 0.14,
        engrave: true,
        ..Default::default()
    };
    let mut gesture = Gesture::default();
    for centre in [195.0, 225.0] {
        for i in 0..20 {
            let theta = centre - 7.0 + i as f64 * 0.7;
            let v = ctx.crest_v_mm + 0.7 * (i as f64 / 19.0 * std::f64::consts::PI).sin();
            let hit = Hit {
                ray: ([0.0; 3], [0.0; 3]),
                world: [0.0; 3],
                face: 0,
                theta_deg: theta,
                v_mm: v,
                radial_wall_mm: 2.5,
                relief_mm: 0.0,
            };
            gesture.push(&d, &brush, &hit, 0.9, [0.0; 2]);
        }
        gesture.miss();
    }
    gesture.commit(&mut d, &brush);
    let mut hallmark = stamp(
        "Palm hallmark / bench engraving",
        "Thalassa hallmark",
        270.0,
        ctx.crest_v_mm,
        9.3,
        0.12,
    );
    hallmark.blend = Blend::Subtract;
    hallmark.bench_only = true;
    d.layers.layers.push(hallmark);
    d.bake_all(lib);
    d.embed_alphas(lib);
    d
}
fn census(d: &RingDesign) -> serde_json::Value {
    fn count(s: &LayerStack) -> usize {
        s.layers
            .iter()
            .map(|e| {
                1 + match &e.layer {
                    Layer::Group(g) => count(&g.stack),
                    _ => 0,
                }
            })
            .sum()
    }
    serde_json::json!({"name":d.name,"top_level_layers":d.layers.layers.len(),"total_layer_entries":count(&d.layers),"stones":ringdesign_core::setstone::set_stones(d).len(),"svg_sources":d.svgs.len(),"procedural_sources":d.recipes.len(),"drawings":d.drawn.len(),"text_sources":d.texts.len()})
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let out = Path::new(
        args.get(1)
            .map(String::as_str)
            .unwrap_or("showcase/thalassa"),
    );
    std::fs::create_dir_all(out)?;
    let draft = args.iter().any(|a| a == "--draft");
    let mut lib = AlphaLibrary::builtin();
    let mut d = if let Some(index) = args.iter().position(|a| a == "--source") {
        let path = args
            .get(index + 1)
            .ok_or_else(|| anyhow::anyhow!("--source needs a ring JSON path"))?;
        let d = ringdesign_core::library::load_design(path)?;
        d.unpack_embedded(&mut lib);
        d.bake_all(&mut lib);
        d
    } else {
        design(&mut lib)
    };
    let params = BuildParams {
        theta_steps: if draft { 480 } else { 960 },
        profile_steps: if draft { 192 } else { 384 },
        ..Default::default()
    };
    d.build = params;
    ringdesign_core::library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let build = ringdesign_core::mesh::try_build(&d, &lib, params)?;
    ensure!(
        build.mesh.validate().watertight,
        "Non-watertight nominal ring"
    );
    ringdesign_core::stl::write_stl(out.join("nominal-ring.stl"), &build.mesh, &d.name)?;
    if let Some(r) = ringdesign_core::stones::report(&d, d.draft.parting_z_mm) {
        let closest=r.closest.as_ref().map(|p|serde_json::json!({"a":p.a,"b":p.b,"girdle_gap_mm":p.gap_mm,"depth_gap_mm":p.gap_deep_mm}));
        let checks: Vec<_> = r.seats.iter().map(|s| format!("{s:?}")).collect();
        std::fs::write(
            out.join("stones.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"stone_count":r.stone_count,"total_carats":r.total_carats,"tight_pairs":r.tight_pairs,"closest":closest,"seat_checks":checks}),
            )?,
        )?;
    }
    let inspection = mf::inspect(&d, &lib, d.manufacturing.as_ref().unwrap(), params)?;
    std::fs::write(
        out.join("inspection.json"),
        serde_json::to_vec_pretty(&mf::package::report(
            &d,
            d.manufacturing.as_ref().unwrap(),
            &inspection,
            false,
        ))?,
    )?;
    let nocturne = ringdesign_core::library::load_design(
        "showcase/masterwork-signets/nocturne/design.ring.json",
    )?;
    let counts = serde_json::json!({"thalassa":census(&d),"nocturne":census(&nocturne)});
    std::fs::write(out.join("census.json"), serde_json::to_vec_pretty(&counts)?)?;
    println!("{}", serde_json::to_string_pretty(&counts)?);
    let gems = ringdesign_core::gems::preview_mesh(&d, &lib);
    let mut parts = vec![ringdesign_core::render::Part::metal(
        &build.mesh,
        [0.86, 0.74, 0.49],
    )];
    if let Some(g) = &gems {
        parts.push(ringdesign_core::render::Part::stone(g));
    }
    ringdesign_core::render::write_png_parts(
        &out.join("hero.png"),
        &parts,
        0.48,
        1.1,
        if draft { 900 } else { 1400 },
    )?;
    ringdesign_core::render::write_png_parts(
        &out.join("face.png"),
        &parts,
        0.0,
        std::f64::consts::FRAC_PI_2,
        if draft { 900 } else { 1400 },
    )?;
    println!(
        "Thalassa: {} triangles; {} sampled pull obstructions (investment pattern)",
        build.mesh.faces.len(),
        inspection.release.obstructions.len()
    );
    Ok(())
}
