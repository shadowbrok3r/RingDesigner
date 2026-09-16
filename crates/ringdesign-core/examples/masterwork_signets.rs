//! Two richly ornamented, editable signets made with the app's own tools.
//! cargo run -p ringdesign-core --example masterwork_signets -- NEW_DIR [--draft] [--sand|--wax]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    alpha::{ProcRecipe, Procedural},
    castability::{CastProcess, SandProcess},
    curve::{CurveLayer, WireProfile},
    drawn::{DrawnAlpha, Stroke},
    field::{
        Blend, BorderLayer, BorderProfile, Decal, DecalLayer, FluteProfile, FlutesLayer,
        GroupLayer, Layer, LayerEntry, LayerStack, MilgrainLayer, OpenworkLayer, Remap,
        SeatPadLayer, SeatStyle, SideFacePick, SignetOutline, VGate, Window,
    },
    gem::{Gem, GemCut},
    manufacturing::{self as mf, Setup},
    render::{self, Part},
    svg::SvgAlpha,
    text::{TextAlpha, TextFont},
    tiling::{TilingLayer, WarpField},
};
use serde_json::json;
use std::{
    f64::consts::{PI, TAU},
    path::Path,
};

fn svg(body: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><filter id="soft"><feGaussianBlur stdDeviation="0.7"/></filter></defs><g filter="url(#soft)">{body}</g></svg>"#
    )
}
fn art(d: &mut RingDesign, name: &str, body: String) {
    d.svgs.push(SvgAlpha {
        name: name.into(),
        svg: body,
        invert: false,
    });
}
fn stroke(path: &str, width: f64) -> String {
    // Three nested shoulders produce a bevel in the actual height alpha.
    [(1.4,"#cccccc"),(1.0,"#666666"),(0.6,"#111111")].iter().map(|(k,c)|
        format!(r##"<path d="{path}" fill="none" stroke="{c}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"##,width*k)).collect()
}
fn sun() -> String {
    let mut s = String::new();
    for i in 0..12 {
        let a = i as f64 * TAU / 12.;
        let r = if i % 2 == 0 { 43. } else { 35. };
        s += &stroke(
            &format!(
                "M {} {} L {} {}",
                50. + 22. * a.cos(),
                50. + 22. * a.sin(),
                50. + r * a.cos(),
                50. + r * a.sin()
            ),
            4.8,
        );
    }
    s += r##"<defs><radialGradient id="disc"><stop stop-color="#000"/><stop offset="70%" stop-color="#444"/><stop offset="100%" stop-color="#fff" stop-opacity="0"/></radialGradient></defs><circle cx="50" cy="50" r="17" fill="url(#disc)"/>"##;
    svg(&s)
}
fn drafted_sun(lib: &mut AlphaLibrary) {
    let mut a = lib.get("Solar rays").unwrap().clone();
    a.name = "Solar rays with withdrawal stock".into();
    // Extend every isolated ray toward the parting line. The resulting
    // winged sun is monotone in the pull direction, with visible axial ribs.
    for x in 0..a.width {
        let mut h = 0_f32;
        for y in 0..a.height / 2 {
            let i = y * a.width + x;
            h = h.max(a.data[i]);
            a.data[i] = h;
        }
        h = 0.;
        for y in (a.height / 2..a.height).rev() {
            let i = y * a.width + x;
            h = h.max(a.data[i]);
            a.data[i] = h;
        }
        let mid = (a.height / 2) * a.width + x;
        let h = a.data[mid].max(a.data[mid - a.width]);
        a.data[mid] = h;
        a.data[mid - a.width] = h;
    }
    insert_portable(lib, a);
}
fn insert_portable(lib: &mut AlphaLibrary, a: ringdesign_core::Alpha) {
    // Use the same 16-bit samples as the saved document, before building.
    let a = ringdesign_core::Alpha::from_png16(a.name.clone(), &a.to_png16().unwrap()).unwrap();
    lib.insert(a);
}
fn palmette() -> String {
    let mut s = stroke("M50 85 C50 64 50 36 50 16", 3.8);
    for (y, spread) in [(72., 30.), (55., 34.), (38., 25.)] {
        for side in [-1., 1.] {
            s += &stroke(
                &format!(
                    "M50 {y} C{} {} {} {} {} {} C{} {} {} {} 50 {y}",
                    50. + side * 14.,
                    y + 2.,
                    50. + side * spread,
                    y - 2.,
                    50. + side * spread,
                    y - 17.,
                    50. + side * 18.,
                    y - 19.,
                    50. + side * 11.,
                    y - 5.
                ),
                4.5,
            );
        }
    }
    svg(&s)
}
fn lotus() -> String {
    svg(
        r##"<defs><linearGradient id="leaf"><stop stop-color="#999"/><stop offset=".35" stop-color="#222"/><stop offset=".6" stop-color="#111"/><stop offset="1" stop-color="#aaa"/></linearGradient></defs><g fill="url(#leaf)"><path d="M50 85 C34 62 39 37 50 10 C61 37 66 62 50 85 Z"/><path d="M39 84 C19 77 11 55 9 25 C27 38 33 61 39 84 Z"/><path d="M61 84 C81 77 89 55 91 25 C73 38 67 61 61 84 Z"/></g><path d="M21 90 Q50 99 79 90" fill="none" stroke="#444" stroke-width="11" stroke-linecap="round"/>"##,
    )
}
fn star_drawing() -> DrawnAlpha {
    let mut a = DrawnAlpha::new("Drawn evening star", 512, 512);
    for (from, to) in [([0.5, 0.10], [0.5, 0.90]), ([0.16, 0.5], [0.84, 0.5])] {
        let mut s = Stroke::new(0.08, 0.8, false);
        for i in 0..=40 {
            let t = i as f32 / 40.;
            s.push(
                from[0] + (to[0] - from[0]) * t,
                from[1] + (to[1] - from[1]) * t,
                (PI as f32 * t).sin().max(0.0).powf(0.45),
            );
        }
        a.strokes.push(s);
    }
    a
}
fn bloom() -> String {
    let mut s = String::new();
    for i in 0..8 {
        s += &format!(
            r#"<g transform="rotate({} 50 50)">{}{}{}</g>"#,
            i * 45,
            stroke("M50 46 C35 35 33 23 50 12 C67 23 65 35 50 46 Z", 2.0),
            stroke("M50 41 C46 31 48 23 50 19", 1.1),
            stroke("M48 31 L41 27 M49 36 L41 33", 0.9)
        );
    }
    s += &stroke(
        "M50 7 L53 10 L50 13 L47 10 Z M7 50 L10 47 L13 50 L10 53 Z M50 87 L53 90 L50 93 L47 90 Z M87 50 L90 47 L93 50 L90 53 Z",
        1.2,
    );
    svg(&s)
}
fn frame(beads: bool) -> String {
    let mut s = String::new();
    let r = 42.;
    let path = "M24 8 L76 8 L92 24 L92 76 L76 92 L24 92 L8 76 L8 24 Z";
    if !beads {
        s += &stroke(path, 2.2);
        s += &format!(
            r#"<g transform="translate(7 7) scale(.86)">{}</g>"#,
            stroke(path, 1.0)
        );
    } else {
        for i in 0..64 {
            let a = TAU * i as f64 / 64.;
            let norm = (a.cos().abs().powf(5.) + a.sin().abs().powf(5.)).powf(-0.2);
            s += &format!(
                r##"<circle cx="{}" cy="{}" r="1.0" fill="#444"/><circle cx="{}" cy="{}" r=".65" fill="#000"/>"##,
                50. + r * norm * a.cos(),
                50. + r * norm * a.sin(),
                50. + r * norm * a.cos(),
                50. + r * norm * a.sin()
            );
        }
    }
    svg(&s)
}
fn mask() -> String {
    // A paintable unrolled mask, strongest over the two shoulders.
    svg(
        r##"<defs><linearGradient id="g"><stop stop-color="#000"/><stop offset=".13" stop-color="#000"/><stop offset=".20" stop-color="#fff"/><stop offset=".30" stop-color="#fff"/><stop offset=".37" stop-color="#000"/><stop offset="1" stop-color="#000"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/>"##,
    )
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
            feather_mm: 0.15,
            invert: false,
        }),
    )
}
fn base(sand: bool) -> RingDesign {
    let mut d = RingDesign::default();
    d.name = if sand {
        "Solstice — solar palmette signet"
    } else {
        "Nocturne — night garden signet"
    }
    .into();
    d.size = ringdesign_core::resize::size_from_bore(18.2).unwrap();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = if sand { 12.6 } else { 13.8 };
    d.profile.thickness_mm = if sand { 3.2 } else { 2.8 };
    d.profile.crown_mm = 1.1;
    d.profile.flatten_sides();
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.15;
    d.profile.side_draft_deg = 4.;
    d.shank.apply_signet(d.profile.width_mm);
    d.shank.amount = 0.66;
    d.shank.head.outline = if sand {
        SignetOutline::Cushion
    } else {
        SignetOutline::Octagon
    };
    d.shank.head.length_mm = if sand { 14.2 } else { 15.5 };
    d.shank.head.rise_mm = 1.3;
    d.shank.head.dome = 1.0;
    d.shank.head.table_dome_mm = if sand { 1.2 } else { 0.12 };
    d.shank.head.rim_round_mm = 0.55;
    if !sand {
        d.shank.head.loft = 0.;
        d.shank.head.body_fair = 0.88;
    }
    d.shank.head.crest_round_mm = 0.6;
    d.shank.head.shoulder_deg = 48.;
    d.shank.head.swell_deg = Some(82.);
    d.shank.head.hollow_mm = if sand { 0. } else { 0.45 };
    let mut s = Setup::default();
    s.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    s.recipe.alloy = if sand { "Silver 925" } else { "Gold 14k" }.into();
    s.recipe.shrink_pct = ringdesign_core::metal::find(&s.recipe.alloy)
        .unwrap()
        .shrink_pct;
    s.recipe.process = if sand {
        CastProcess::SandTwoPart
    } else {
        CastProcess::LostWax
    };
    if !sand {
        s.recipe.sand = None;
        s.recipe.min_section_mm = 0.8;
        s.recipe.min_detail_mm = 0.18;
        s.recipe.min_draft_deg = 0.;
    }
    s.recipe.name = if sand {
        "Solstice / Delft clay / sterling"
    } else {
        "Nocturne / investment / 14k yellow"
    }
    .into();
    s.recipe.calibration_note =
        "Starting alloy shrink allowance; confirm with a measured shop trial.".into();
    s.sample_pitch_mm = 0.1;
    s.flask.width_mm = 70.;
    s.flask.length_mm = 70.;
    s.channels = vec![
        mf::Channel {
            kind: mf::ChannelKind::Gate,
            start: [0., 14., 0.],
            end: [0., 23., 0.],
            diameter_mm: 3.8,
        },
        mf::Channel {
            kind: mf::ChannelKind::Sprue,
            start: [0., 23., 0.],
            end: [0., 31., 0.],
            diameter_mm: 5.5,
        },
    ];
    s.bench_notes=if sand {
        "One-piece cast pattern; Z=0 parting, opposed Z withdrawal, preserve the bore island. Solar stock, lotus relief, ribs, vines and beading are included in the pattern. Only the fine satin microtexture is deferred to the bench. Remove gate and seam, polish raised ornament and burnish the solar disc; keep recessed ground satin or lightly oxidized. Review low-draft regions and run a physical pull trial. Gate layout is preliminary."
    } else {
        "One-piece investment-cast metal body. Fine botanical relief, gallery recesses, beadwork and proud setting stock require a sacrificial pattern. Stones shown are separate references, never cast metal. Drill/bur bearings and pavilion clearance to actual stones, preserve floor stock, set at the bench, polish rails and leaf edges while retaining the textured ground. Gate layout is preliminary."
    }.into();
    d.draft.process = s.recipe.process;
    d.draft.min_section_mm = s.recipe.min_section_mm;
    d.draft.min_detail_mm = s.recipe.min_detail_mm;
    d.draft.min_draft_deg = s.recipe.min_draft_deg;
    d.draft.sand = s.recipe.sand;
    d.manufacturing = Some(s);
    d
}

fn decorate(sand: bool, lib: &mut AlphaLibrary) -> RingDesign {
    let mut d = base(sand);
    art(&mut d, "Solar rays", sun());
    art(&mut d, "Palmette", if sand { lotus() } else { palmette() });
    art(&mut d, "Shoulder mask", mask());
    art(
        &mut d,
        "Sunseed tile",
        svg(
            r##"<defs><radialGradient id="seed"><stop stop-color="#000"/><stop offset=".45" stop-color="#444"/><stop offset="1" stop-color="#fff"/></radialGradient></defs><circle cx="50" cy="50" r="40" fill="url(#seed)"/>"##,
        ),
    );
    if !sand {
        art(&mut d, "Night bloom", bloom());
        art(&mut d, "Seal frame", frame(false));
        art(&mut d, "Seal pearls", frame(true));
        d.drawn.push(star_drawing());
        d.texts.push(TextAlpha {
            name: "Nocturne signature".into(),
            text: "NOCTURNE".into(),
            font: TextFont::Serif,
            tracking: 0.12,
        });
    }
    d.recipes.push(ProcRecipe {
        name: "Satin ground".into(),
        kind: Procedural::Hammered,
        repeats: 1,
        gamma: 1.2,
        ..Default::default()
    });
    d.recipes.push(ProcRecipe {
        name: "Engine turned waves".into(),
        kind: if sand {
            Procedural::Scales
        } else {
            Procedural::GuillocheWeave
        },
        repeats: 1,
        ..Default::default()
    });
    d.bake_all(lib);
    let ctx = d.field_context();
    let faces = ctx.side_faces_std().unwrap();
    let (lo, hi) = faces.low.unwrap();
    let side = (lo + hi) * 0.5;
    let span = hi - lo;
    println!(
        "{} chart: span {:.3}, crest {:.3}, side {:.3}..{:.3}, head stretch {:.3}",
        d.name,
        ctx.band_v_len_mm,
        ctx.crest_v_mm,
        lo,
        hi,
        ctx.station_stretch(90.)
    );

    let mut ground = TilingLayer::default_for("Satin ground", &ctx);
    ground.fit_to_side_faces(&ctx, 80.);
    ground.height_mm = if sand { 0.035 } else { 0.06 };
    ground.repeats_around = 12;
    let mut e = LayerEntry::new("Fine satin ground / bench finish", Layer::Tiling(ground));
    e.bench_only = sand;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);

    let mut guilloche = TilingLayer::default_for(
        if sand {
            "Sunseed tile"
        } else {
            "Engine turned waves"
        },
        &ctx,
    );
    guilloche.fit_to_side_faces(&ctx, 80.);
    guilloche.height_mm = if sand { 0.17 } else { 0.23 };
    guilloche.repeats_around = if sand { 20 } else { 16 };
    guilloche.edge_mm = if sand { 0.22 } else { 0.10 };
    guilloche.warp = Some(WarpField {
        points: (0..12)
            .map(|i| {
                let u = i as f64 / 12.;
                [u, side + 0.12 * (TAU * u * 2.).sin()]
            })
            .collect(),
        strength: 0.6,
        falloff_mm: span,
    });
    let mut e = LayerEntry::new(
        if sand {
            "Warped sunseed tessellation / shoulder mask"
        } else {
            "Warped guilloche / shoulder mask"
        },
        Layer::Tiling(guilloche),
    );
    e.mask = Some("Shoulder mask".into());
    e.window = Window::except(270., 80.);
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    e.remap = Remap::cushion(if sand { 0.17 } else { 0.23 });
    d.layers.layers.push(e);

    for (n, v) in [("Inner", lo + span * 0.16), ("Outer", hi - span * 0.15)] {
        let mut e = LayerEntry::new(
            format!("{n} polished cheek rail"),
            Layer::Border(BorderLayer {
                v_mm: v,
                width_mm: if sand { 0.36 } else { 0.25 },
                height_mm: 0.16,
                profile: BorderProfile::Round,
                mirror: true,
                ..Default::default()
            }),
        );
        e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
        d.layers.layers.push(e);
    }
    let mut e = LayerEntry::new(
        "Pearl edging",
        Layer::Milgrain(MilgrainLayer {
            v_mm: lo + span * 0.33,
            bead_diameter_mm: if sand { 0.56 } else { 0.38 },
            beads_around: if sand { 76 } else { 108 },
            height_mm: if sand { 0.17 } else { 0.18 },
            mirror: true,
        }),
    );
    e.window = Window::except(90., 80.);
    e.window.fade_deg = 14.;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);

    let cheek = if sand { 2.05 } else { 1.2 };
    let mut leaves = stamp(
        "Paired palmette cheek seals",
        "Palmette",
        90.,
        cheek,
        7.5,
        if sand { 0.36 } else { 0.28 },
    );
    // Compress the vector artwork to compensate the narrow side-face chart.
    let leaf_name = "Palmette cheeks";
    let compressed = d
        .svgs
        .iter()
        .find(|a| a.name == "Palmette")
        .unwrap()
        .svg
        .replace(
            "viewBox=\"0 0 100 100\"",
            &format!(
                "viewBox=\"0 0 100 100\" width=\"100\" height=\"{}\" preserveAspectRatio=\"none\"",
                if sand { 39 } else { 28 }
            ),
        );
    art(&mut d, leaf_name, compressed);
    if let Layer::Decals(l) = &mut leaves.layer {
        l.alpha = leaf_name.into();
        l.decals.push(Decal {
            v_mm: ctx.band_v_len_mm - cheek,
            flip: true,
            ..l.decals[0]
        });
    }
    d.layers.layers.push(leaves);

    let mut wire = CurveLayer::preset_vine(&ctx);
    wire.retarget_v(side, span * 0.20);
    wire.width_mm = if sand { 0.48 } else { 0.30 };
    wire.height_mm = if sand { 0.18 } else { 0.24 };
    wire.profile = WireProfile::Round;
    wire.repeats_around = 8;
    wire.mirror_v = true;
    let mut e = LayerEntry::new("Woven vine shoulder wires", Layer::Curve(wire));
    e.window = Window::except(90., 105.);
    e.window.fade_deg = 15.;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.10;
    d.layers.layers.push(e);
    let mut e = LayerEntry::new(
        "Axial palm reeds",
        Layer::Flutes(FlutesLayer {
            count: 38,
            profile: FluteProfile::Round,
            width_mm: 0.7,
            height_mm: 0.15,
            lean: 0.,
            along: false,
        }),
    );
    e.window = Window::around(270., 100.);
    e.window.fade_deg = 15.;
    d.layers.layers.push(e);

    if sand {
        drafted_sun(lib);
        d.layers.layers.push(stamp(
            "Winged sun / axial withdrawal stock",
            "Solar rays with withdrawal stock",
            90.,
            ctx.crest_v_mm,
            9.4,
            0.24,
        ));
        d.layers.layers.push(LayerEntry::new(
            "Burnished solar disc",
            Layer::SeatPad(SeatPadLayer {
                theta_deg: 90.,
                v_mm: ctx.crest_v_mm,
                diameter_mm: 3.3,
                height_mm: 0.64,
                style: SeatStyle::GypsyMound,
                blend_mm: 0.45,
                metal_true: true,
                ..Default::default()
            }),
        ));
        let mut e = LayerEntry::new(
            "Broad axial shoulder fans",
            Layer::Flutes(FlutesLayer {
                count: 30,
                profile: FluteProfile::Round,
                width_mm: 1.25,
                height_mm: 0.22,
                lean: 0.,
                along: false,
            }),
        );
        e.window = Window::except(90., 72.);
        e.window.fade_deg = 12.;
        d.layers.layers.push(e);
    } else {
        let mut seal = LayerStack::default();
        seal.layers.push(stamp(
            "Octagonal double frame",
            "Seal frame",
            90.,
            ctx.crest_v_mm,
            9.8,
            0.20,
        ));
        seal.layers.push(stamp(
            "Pearled seal perimeter",
            "Seal pearls",
            90.,
            ctx.crest_v_mm,
            8.8,
            0.18,
        ));
        seal.layers.push(stamp(
            "Eight-petal night flower",
            "Night bloom",
            90.,
            ctx.crest_v_mm,
            7.8,
            0.24,
        ));
        d.layers.layers.push(LayerEntry::new(
            "Botanical seal / editable group",
            Layer::Group(GroupLayer {
                stack: seal,
                recipe: None,
            }),
        ));
        let mut tex = TilingLayer::default_for("Engine turned waves", &ctx);
        tex.repeats_around = 18;
        tex.v_span_mm = 6.4;
        tex.height_mm = 0.035;
        tex.rows = 2;
        tex.feather_mm = 0.4;
        let mut e = LayerEntry::new("Fine guilloche seal ground", Layer::Tiling(tex));
        e.window = Window::around(90., 38.);
        e.window.fade_deg = 6.;
        e.blend = Blend::Subtract;
        d.layers.layers.push(e);
        for (theta, flip) in [(43., false), (137., true)] {
            let mut leaf = stamp(
                "Sculpted acanthus shoulder",
                "Palmette",
                theta,
                ctx.crest_v_mm,
                5.8,
                0.32,
            );
            if let Layer::Decals(l) = &mut leaf.layer {
                l.decals[0].flip = flip;
                l.decals[0].rotation_deg = if flip { 90. } else { -90. };
            }
            d.layers.layers.push(leaf);
        }
        let mut cavities = TilingLayer::default_for("Palmette", &ctx);
        cavities.fit_to_side_faces(&ctx, 80.);
        cavities.height_mm = 1.;
        cavities.repeats_around = 18;
        cavities.edge_mm = 0.18;
        let mut e = LayerEntry::new(
            "Recessed botanical gallery / retained floor",
            Layer::Openwork(OpenworkLayer {
                tiling: cavities,
                depth_mm: 0.9,
                keep_mm: 1.15,
            }),
        );
        e.window = Window::except(90., 100.);
        e.window.fade_deg = 12.;
        e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
        e.blend = Blend::Add;
        d.layers.layers.push(e);
        let mut centre = SeatPadLayer {
            theta_deg: 90.,
            v_mm: ctx.crest_v_mm,
            style: SeatStyle::Bezel,
            bezel_wall_mm: 0.32,
            recess_mm: 0.48,
            blend_mm: 0.28,
            metal_true: true,
            ..Default::default()
        };
        centre.fit_stone(Gem::calibrated(GemCut::Round, 2.3));
        d.layers.layers.push(LayerEntry::new(
            "Emerald dew / central bezel stock",
            Layer::SeatPad(centre),
        ));
        for theta in [25., 155.] {
            let mut seat = SeatPadLayer {
                theta_deg: theta,
                v_mm: ctx.crest_v_mm,
                style: SeatStyle::GypsyMound,
                height_mm: 0.50,
                blend_mm: 0.38,
                dimple_mm: 0.25,
                metal_true: true,
                ..Default::default()
            };
            seat.fit_stone(Gem::calibrated(GemCut::Round, 1.5));
            d.layers.layers.push(LayerEntry::new(
                format!("Shoulder emerald / {theta} degrees"),
                Layer::SeatPad(seat),
            ));
        }
        let mut body = TilingLayer::default_for("Engine turned waves", &ctx);
        body.repeats_around = 18;
        body.rows = 2;
        body.v_span_mm = 8.5;
        body.height_mm = 0.065;
        body.feather_mm = 0.65;
        body.shear = 0.3;
        let mut e = LayerEntry::new("Engraved guilloche shoulder ground", Layer::Tiling(body));
        e.window = Window::except(90., 78.);
        e.window.fade_deg = 12.;
        e.blend = Blend::Subtract;
        d.layers.layers.push(e);
        let mut stars = stamp(
            "Four drawn evening stars",
            "Drawn evening star",
            90.,
            ctx.crest_v_mm,
            1.,
            0.16,
        );
        if let Layer::Decals(l) = &mut stars.layer {
            l.decals.clear();
            for theta in [76., 104.] {
                for v in [ctx.crest_v_mm - 2.9, ctx.crest_v_mm + 2.9] {
                    l.decals.push(Decal {
                        theta_deg: theta,
                        v_mm: v,
                        size_mm: 0.95,
                        height_mm: 0.15,
                        ..Default::default()
                    });
                }
            }
        }
        d.layers.layers.push(stars);
        // Carving must follow the raised stock: a later Max layer would
        // fill the negative field back to zero outside its own footprint.
        let mut cuts = Vec::new();
        d.layers.layers.retain(|e| {
            let cut = matches!(e.layer, Layer::Openwork(_)) || e.blend == Blend::Subtract;
            if cut {
                cuts.push(e.clone());
            }
            !cut
        });
        // An editable alpha protects the raised leafwork, frame and seats
        // from background engraving, like a painted resist mask.
        d.bake_all(lib);
        let ornamental: Vec<_> = d
            .layers
            .layers
            .iter()
            .filter(|e| {
                matches!(
                    e.layer,
                    Layer::Group(_) | Layer::Decals(_) | Layer::SeatPad(_)
                )
            })
            .collect();
        let (w, h) = (1024, 384);
        let mut data = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let uv = ringdesign_core::field::Uv {
                    u: x as f64 / (w - 1) as f64 * ctx.circumference_mm,
                    v: y as f64 / (h - 1) as f64 * ctx.band_v_len_mm,
                };
                let relief = ornamental
                    .iter()
                    .map(|e| e.layer.height(uv, &ctx, lib))
                    .fold(0_f64, f64::max);
                data.push((1. - ringdesign_core::field::smoothstep(0.015, 0.065, relief)) as f32);
            }
        }
        insert_portable(
            lib,
            ringdesign_core::Alpha::new("Engraving resist", w, h, data),
        );
        for e in &mut cuts {
            if e.blend == Blend::Subtract {
                e.mask = Some("Engraving resist".into());
            }
        }
        d.layers.layers.extend(cuts);
        let mut signature = stamp(
            "Palm inscription / bench engraving",
            "Nocturne signature",
            270.,
            ctx.crest_v_mm,
            9.5,
            0.12,
        );
        signature.blend = Blend::Subtract;
        signature.bench_only = true;
        d.layers.layers.push(signature);
    }
    d.bake_all(lib);
    d.embed_alphas(lib);
    d
}

fn write(
    dir: &Path,
    mut d: RingDesign,
    lib: &AlphaLibrary,
    draft: bool,
) -> Result<serde_json::Value> {
    std::fs::create_dir(dir)?;
    let params = BuildParams {
        theta_steps: if draft { 960 } else { 1920 },
        profile_steps: if draft { 384 } else { 640 },
        refine: None,
        ..Default::default()
    };
    d.build = params;
    let setup = d.manufacturing.clone().unwrap();
    let mesh = ringdesign_core::mesh::try_build(&d, lib, params)?.mesh;
    let mut bare = d.clone();
    bare.layers.layers.clear();
    let bare_mesh = ringdesign_core::mesh::try_build(&bare, lib, params)?.mesh;
    let inspection = mf::inspect(&d, lib, &setup, params)?;
    println!(
        "{}: {:?}, {} obstructions, {} unresolved, wall {:?}",
        d.name,
        inspection.release.status,
        inspection.release.obstructions.len(),
        inspection.release.unresolved_rays,
        inspection.field.as_ref().map(|f| f.thinnest_wall_mm)
    );
    let tint = if setup.recipe.process == CastProcess::SandTwoPart {
        [0.79, 0.80, 0.81]
    } else {
        [0.86, 0.70, 0.42]
    };
    let gems = ringdesign_core::gems::preview_mesh(&d, lib);
    let mut parts = vec![Part::metal(&mesh, tint)];
    if let Some(g) = gems.as_ref() {
        let mut p = Part::stone(g);
        p.tint = [0.07, 0.50, 0.32];
        parts.push(p);
    }
    let edge = if draft { 960 } else { 1400 };
    render::write_png(
        &dir.join("structure.png"),
        &bare_mesh,
        0.48,
        1.0,
        edge,
        tint,
    )?;
    render::write_png(
        &dir.join("pattern.png"),
        &inspection.prepared.mesh,
        0.48,
        1.0,
        edge,
        tint,
    )?;
    for (name, yaw, pitch) in [
        ("hero", 0.48, 1.0),
        ("seal", 0., PI / 2.),
        ("cheek", 0., 0.24),
        ("reverse", PI, 0.8),
        ("palm", PI, PI / 2.),
    ] {
        render::write_png_parts(&dir.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    let report = mf::package::report(&d, &setup, &inspection, false);
    std::fs::write(dir.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    ringdesign_core::library::save_design_embedded(dir.join("design.ring.json"), &d, lib)?;
    std::fs::write(
        dir.join("layers.json"),
        serde_json::to_vec_pretty(&d.layers)?,
    )?;
    if !draft {
        ensure!(mesh.validate().watertight, "Invalid nominal mesh");
        if setup.recipe.process == CastProcess::SandTwoPart {
            ensure!(
                inspection.release.obstructions.is_empty()
                    && inspection.release.unresolved_rays == 0,
                "Sand withdrawal blocked"
            );
            let mut fine = setup.clone();
            fine.sample_pitch_mm = 0.075;
            let r = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
            ensure!(
                r.obstructions.is_empty() && r.unresolved_rays == 0,
                "Fine sand withdrawal blocked"
            );
            std::fs::write(
                dir.join("release-fine.json"),
                serde_json::to_vec_pretty(&r)?,
            )?;
        }
        mf::package::export(&dir.join("pattern-package"), &d, lib, &setup, params, false)?;
        ringdesign_core::stl::write_stl(dir.join("nominal.stl"), &mesh, &d.name)?;
        ringdesign_core::threemf::write_3mf(
            dir.join("nominal.3mf"),
            &mesh,
            &d.name,
            &d.size.display(),
        )?;
        let artwork = dir.join("artwork");
        std::fs::create_dir(&artwork)?;
        for a in &d.svgs {
            std::fs::write(
                artwork.join(format!("{}.svg", a.name.replace(' ', "-"))),
                &a.svg,
            )?;
        }
        for name in d.layers.referenced_alphas() {
            if let Some(a) = lib.get(&name) {
                std::fs::write(
                    artwork.join(format!("{}.png", name.replace(' ', "-"))),
                    a.to_png16()?,
                )?;
            }
        }
        // Fresh, empty libraries prove both the project and package sources
        // carry every authored alpha; compare actual triangles and samples.
        let mut verified = Vec::new();
        for source in [
            dir.join("design.ring.json"),
            dir.join("pattern-package/design.ring.json"),
        ] {
            let saved = ringdesign_core::library::load_design(&source)?;
            let empty = AlphaLibrary::default();
            let fresh = mf::source_library(&saved, &empty);
            for name in saved.layers.referenced_alphas() {
                ensure!(fresh.get(&name).is_some(), "Missing alpha {name}");
            }
            let rebuilt = ringdesign_core::mesh::try_build(&saved, &fresh, params)?.mesh;
            ensure!(
                rebuilt.vertices == mesh.vertices && rebuilt.faces == mesh.faces,
                "Nominal source round trip changed geometry"
            );
            let prepared = mf::prepare(&saved, &fresh, &setup, params)?;
            ensure!(
                prepared.mesh.vertices == inspection.prepared.mesh.vertices
                    && prepared.mesh.faces == inspection.prepared.mesh.faces,
                "Pattern source round trip changed geometry"
            );
            verified.push(json!({"source":source.file_name().unwrap().to_string_lossy(),"parent":source.parent().unwrap().file_name().unwrap().to_string_lossy(),"nominal_identical":true,"pattern_identical":true,"empty_library":true}));
        }
        std::fs::write(
            dir.join("verification.json"),
            serde_json::to_vec_pretty(&verified)?,
        )?;
        let mut sand = setup.clone();
        sand.recipe.process = CastProcess::SandTwoPart;
        std::fs::write(
            dir.join("sand-orientations.json"),
            serde_json::to_vec_pretty(&mf::release::compare_orientations(
                &inspection.prepared.mesh,
                &sand,
            )?)?,
        )?;
        let view_params = BuildParams {
            theta_steps: 480,
            profile_steps: 240,
            refine: None,
            ..Default::default()
        };
        let light_mesh = ringdesign_core::mesh::try_build(&d, lib, view_params)?.mesh;
        std::fs::write(
            dir.join("wall-screen.json"),
            serde_json::to_vec_pretty(&ringdesign_core::cad::measure::thickness(
                &light_mesh,
                setup.recipe.min_section_mm,
            ))?,
        )?;
        ringdesign_core::gltf::write_glb(
            dir.join("preview-metal.glb"),
            &light_mesh,
            &d.name,
            tint,
        )?;
        let mut encoder =
            image::codecs::gif::GifEncoder::new(std::fs::File::create(dir.join("turntable.gif"))?);
        encoder.set_repeat(image::codecs::gif::Repeat::Infinite)?;
        let mut moving = vec![Part::metal(&light_mesh, tint)];
        if let Some(g) = gems.as_ref() {
            let mut p = Part::stone(g);
            p.tint = [0.07, 0.50, 0.32];
            moving.push(p);
        }
        for i in 0..32 {
            let rgb = render::render_parts_ss(&moving, i as f64 / 32. * TAU, 1.05, 520, 520, 2);
            let pixels = image::RgbImage::from_raw(520, 520, rgb).unwrap();
            let rgba = image::DynamicImage::ImageRgb8(pixels).into_rgba8();
            encoder.encode_frame(image::Frame::from_parts(
                rgba,
                0,
                0,
                image::Delay::from_numer_denom_ms(110, 1),
            ))?;
        }
        if let Some(stones) = ringdesign_core::stones::report(&d, 0.).filter(|s| s.stone_count > 0)
        {
            ringdesign_core::stonemap::write_stone_map_svg(
                dir.join("setting-map.svg"),
                &d,
                Some(&stones),
            )?;
            let seats:Vec<_>=stones.seats.iter().map(|s|json!({"label":s.label,"count":s.count,"gem":s.gem,"available_depth_mm":s.depth_available_mm,"edge_clearance_mm":s.edge_clearance_mm,"warnings":s.warnings})).collect();
            std::fs::write(
                dir.join("stones.json"),
                serde_json::to_vec_pretty(
                    &json!({"count":stones.stone_count,"diamond_equivalent_carats":stones.total_carats,"tight_pairs":stones.tight_pairs,"seats":seats,"note":"Green colour is the intended emerald finish study; carats use the app's diamond-density estimate. Choose actual stones before cutting bearings."}),
                )?,
            )?;
        }
    }
    let grams = mesh.volume_mm3()
        * ringdesign_core::metal::find(&setup.recipe.alloy)
            .unwrap()
            .density
        / 1000.;
    Ok(
        json!({"name":d.name,"layers":d.layers.layers.len(),"grams":grams,"volume_mm3":mesh.volume_mm3(),"cast_grams":inspection.ring_grams,"mesh":mesh.validate(),"process":setup.recipe.process,"release":inspection.release.status,"obstructions":inspection.release.obstructions.len(),"radial_wall_mm":inspection.field.as_ref().map(|f|f.thinnest_wall_mm),"bore_mm":18.2,"alloy":setup.recipe.alloy,"bench":setup.bench_notes,"detail_findings":inspection.details,"bench_layers":inspection.prepared.bench_layers}),
    )
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let dir = Path::new(
        args.get(1)
            .map(String::as_str)
            .unwrap_or("/tmp/masterwork-signets"),
    );
    if args.iter().any(|a| a == "--verify") {
        for slug in ["solstice", "nocturne"] {
            let folder = dir.join(slug);
            for path in [
                folder.join("design.ring.json"),
                folder.join("pattern-package/design.ring.json"),
            ] {
                let d = ringdesign_core::library::load_design(&path)?;
                let empty = AlphaLibrary::default();
                let lib = mf::source_library(&d, &empty);
                let mesh = ringdesign_core::mesh::try_build(&d, &lib, d.build)?.mesh;
                let bytes = ringdesign_core::stl::to_stl_binary(&mesh, "nominal");
                ensure!(
                    bytes[80..] == std::fs::read(folder.join("nominal.stl"))?[80..],
                    "Nominal identity changed"
                );
                let i = mf::inspect(&d, &lib, d.manufacturing.as_ref().unwrap(), d.build)?;
                let bytes = ringdesign_core::stl::to_stl_binary(&i.prepared.mesh, "pattern");
                ensure!(
                    bytes[80..] == std::fs::read(folder.join("pattern-package/pattern.stl"))?[80..],
                    "Pattern identity changed"
                );
            }
            println!(
                "{slug}: both source files rebuild identical nominal and pattern STL payloads"
            );
        }
        return Ok(());
    }
    ensure!(!dir.exists(), "Output already exists: {}", dir.display());
    std::fs::create_dir_all(dir)?;
    let mut lib = AlphaLibrary::builtin();
    let draft = args.iter().any(|a| a == "--draft");
    let mut records = Vec::new();
    for (sand, slug) in [(true, "solstice"), (false, "nocturne")] {
        if (sand && args.iter().any(|a| a == "--wax"))
            || (!sand && args.iter().any(|a| a == "--sand"))
        {
            continue;
        }
        let d = decorate(sand, &mut lib);
        let mut row = write(&dir.join(slug), d, &lib, draft)?;
        row["slug"] = slug.into();
        records.push(row);
        std::fs::write(
            dir.join("collection.json"),
            serde_json::to_vec_pretty(&records)?,
        )?;
    }
    std::fs::write(
        dir.join("collection.json"),
        serde_json::to_vec_pretty(&records)?,
    )?;
    Ok(())
}
