//! Bestiarium — Phoenix: a fire-opal breast, enfolding wings and a tapering ember spine.
//! Build unguarded, then run under an 8 GiB scope:
//! cargo build --release -p ringdesign-core --example bestiarium_phoenix
//! target/release/examples/bestiarium_phoenix OUT [--draft] [--verify] [--layout]
use anyhow::{Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    castability::{self, SandProcess, Verdict},
    csg, dfm,
    field::{Blend, Layer, LayerEntry, SeatPadLayer, SeatRunLayer, SeatStyle, Window, smoothstep},
    gem::{Gem, GemCut},
    library, manufacturing as mf, mesh, outline,
    profile::ShankKey,
    render::{self, Part},
    setting::{SolidKind, Stamp, StampTop},
    skin::{self, Atlas, Hide, Joints},
    stl,
};
use serde_json::json;
use std::{
    f64::consts::PI,
    path::{Path, PathBuf},
};

const RELIEF: f64 = 0.24;

fn params(draft: bool) -> BuildParams {
    BuildParams {
        theta_steps: if draft { 768 } else { 1536 },
        profile_steps: if draft { 320 } else { 448 },
        ..Default::default()
    }
}

fn band() -> RingDesign {
    let mut d = RingDesign {
        name: "Phoenix — reborn".into(),
        ..Default::default()
    };
    d.size = ringdesign_core::RingSize::from_diameter_mm(18.6);
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 6.6;
    d.profile.thickness_mm = 3.4;
    d.profile.crown_mm = 1.4;
    d.profile.comfort_fit_mm = 0.2;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = [
        (28., 1.12, 1.16, 1.),
        (62., 1.28, 1.36, 0.9),
        (90., 1.35, 1.45, 0.85),
        (118., 1.28, 1.36, 0.9),
        (152., 1.12, 1.16, 1.),
        (200., 1., 1., 1.),
        (240., 0.92, 0.95, 1.),
        (270., 0.88, 0.92, 1.),
        (300., 0.92, 0.95, 1.),
        (340., 1., 1., 1.),
    ]
    .into_iter()
    .map(
        |(theta_deg, width_scale, thickness_scale, crown_scale)| ShankKey {
            theta_deg,
            width_scale,
            thickness_scale,
            crown_scale,
        },
    )
    .collect();
    SandProcess::DelftClay.apply(&mut d.draft);
    d
}

fn opal() -> Gem {
    Gem {
        l_mm: 8.,
        preview_tint: Some([0.95, 0.18, 0.035]),
        ..Gem::cabochon(GemCut::Oval, 6.)
    }
}

fn sapphire() -> Gem {
    Gem {
        preview_tint: Some([0.96, 0.29, 0.045]),
        ..Gem::calibrated(GemCut::Round, 2.2)
    }
}

/// The contour plates are authored in measured hide millimetres. Their flame tips
/// lead down the shoulder; the drafted fall is part of the drawing, before clamping.
fn contour(along: f64, across: f64, pitch: f64) -> f64 {
    let scallop = 0.035 * (across.abs() * PI).sin().powi(2);
    let t = (along + 0.62 * across.abs() + scallop).rem_euclid(1.0);
    let fall = (0.55 / pitch).min(0.35);
    let low = 0.20;
    let crest = low + (1.0 - low) * (1.0 - fall).powf(0.9);
    let plate = if t <= 1.0 - fall {
        low + (1.0 - low) * t.powf(0.9)
    } else {
        crest + (low - crest) * smoothstep(1.0 - fall, 1.0, t)
    };
    (plate * (1.0 - 0.30 * across * across)).max(0.)
}

fn plumage(d: &mut RingDesign, lib: &mut AlphaLibrary, art: &Path) -> Result<f64> {
    let a = Atlas::of(d, 2048, 768)?;
    let h = Hide::of(&a);
    let joints = Joints::eccentric(0., h.reach(), 4.4, 3.4);
    let mut flame = a.paint("Phoenix flame plumage", |s| {
        let p = h.at(s);
        let Some((_, t, pitch)) = joints.at(p.along.abs()) else {
            return 0.;
        };
        let across = p.across / p.rim.max(0.1);
        let crown = 1.0 - smoothstep(0.82, 1.0, across.abs());
        let seat_strip = smoothstep(0.90, 2.0, p.across.abs());
        let breast = smoothstep(4.5, 5.1, p.along.abs());
        contour(t, across, pitch) * crown * seat_strip * breast
    });
    let clamp = skin::draft_clamp(&a, &mut flame, RELIEF)?;
    println!(
        "  plumage clamp {:.6} mm / {} texels",
        clamp.worst_mm, clamp.texels_cut
    );
    std::fs::write(art.join("flame-plumage.png"), flame.to_png16()?)?;
    lib.insert(Alpha::from_png16(flame.name.clone(), &flame.to_png16()?)?);
    let mut entry = skin::hide_layer(
        d,
        "Phoenix flame plumage",
        RELIEF,
        Window::around(90., 360.),
    );
    entry.name = "Flame plumage".into();
    d.layers.layers.push(entry);
    let barbs = a.paint("Phoenix feather barbs", |s| {
        let p = h.at(s);
        let across = p.across / p.rim.max(0.1);
        let vein = (p.along.abs() * 4.8 + p.across.abs() * 5.6).sin().abs();
        (1. - smoothstep(0.16, 0.40, vein))
            * smoothstep(1.3, 1.6, p.across.abs())
            * (1. - smoothstep(0.72, 0.91, across.abs()))
            * smoothstep(4.5, 5.1, p.along.abs())
    });
    std::fs::write(art.join("barbs.png"), barbs.to_png16()?)?;
    lib.insert(Alpha::from_png16(barbs.name.clone(), &barbs.to_png16()?)?);
    let mut e = skin::hide_layer(d, "Phoenix feather barbs", 0.045, Window::around(90., 360.));
    e.name = "Hand-cut vane barbs".into();
    e.blend = Blend::Subtract;
    e.bench_only = true;
    d.layers.layers.push(e);
    Ok(clamp.worst_mm)
}

/// A swept quill with its tip following the annular cheek. These are actual
/// silhouettes, with drafted walls and a pitched vane, never rectangular decals.
fn feather(
    d: &RingDesign,
    a: &Atlas,
    name: String,
    theta: f64,
    high: bool,
    fraction: f64,
    length: f64,
    width: f64,
    tipward: f64,
    height: f64,
) -> Result<Stamp> {
    let ctx = d.field_context();
    let sides = ctx
        .side_faces_at(theta)
        .ok_or_else(|| anyhow::anyhow!("No side faces at {theta}"))?;
    let (v0, v1) = if high { sides.high } else { sides.low }
        .ok_or_else(|| anyhow::anyhow!("No cheek at {theta}"))?;
    let middle = 0.5 + (fraction - 0.5) * smoothstep(0., 0.92, 0.5);
    let v = v0 + (v1 - v0) * middle;
    let mut stamp = Stamp {
        name,
        theta_deg: theta,
        v_mm: v,
        rot_deg: 0.,
        outline: outline::quill(length, width, (width * 0.32).min(0.3)),
        height_mm: 0.10,
        sink_mm: 0.16,
        draft_deg: 20.,
        cut: false,
        bench: false,
        along_pull: true,
        tier: 0,
        top: StampTop::Dome {
            crown_mm: 0.22 + 0.20 * height,
        },
    };
    let frame = stamp.frame(d, &ctx);
    let radius = frame.origin[0].hypot(frame.origin[1]);
    // Follow the actual keyed cheek, not a tangent-plane rectangle. This keeps
    // every point on metal as the shoulder narrows and avoids a floating tip.
    for p in &mut stamp.outline {
        let at = (theta + (p[0] * tipward / radius).to_degrees()).rem_euclid(360.);
        let sides = ctx.side_faces_at(at).unwrap();
        let (lo, hi) = if high { sides.high } else { sides.low }.unwrap();
        let stretch = ctx.station_stretch(at);
        let t = (p[0] / length + 0.5).clamp(0., 1.);
        let fan = 0.5 + (fraction - 0.5) * smoothstep(0., 0.92, t);
        let av = lo + (hi - lo) * fan + p[1] / stretch;
        let world = a.point(at, av);
        let delta: [f64; 3] = std::array::from_fn(|k| world[k] - frame.origin[k]);
        p[0] = (0..3).map(|k| delta[k] * frame.x[k]).sum();
        p[1] = (0..3).map(|k| delta[k] * frame.y[k]).sum();
    }
    let area: f64 = (0..stamp.outline.len())
        .map(|i| {
            let p = stamp.outline[i];
            let q = stamp.outline[(i + 1) % stamp.outline.len()];
            p[0] * q[1] - p[1] * q[0]
        })
        .sum();
    if area < 0. {
        stamp.outline.reverse();
    }
    Ok(stamp)
}

fn wings(d: &mut RingDesign, art: &Path) -> Result<()> {
    let atlas = Atlas::of(d, 2048, 768)?;
    for high in [false, true] {
        let side = if high { "high" } else { "low" };
        for (tier, theta, len) in [
            ("coverts", 65., 7.5),
            ("secondaries", 45., 9.8),
            ("primaries", 19., 12.5),
        ] {
            let theta = if high {
                (180.0_f64 - theta).rem_euclid(360.)
            } else {
                theta
            };
            let mut svg = String::from(
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-6 -3 12 6"><g fill="#d9b55c" stroke="#644411" stroke-width=".06">"##,
            );
            for (i, fraction) in [0.22, 0.50, 0.78].into_iter().enumerate() {
                let f = feather(
                    d,
                    &atlas,
                    format!("Wing {side}, {tier} {}", i + 1),
                    theta,
                    high,
                    fraction,
                    len - i as f64 * 0.6,
                    1.15,
                    if high { 1. } else { -1. },
                    0.25 + i as f64 * 0.10,
                )?;
                svg += &format!(
                    "<polygon points=\"{}\" transform=\"translate(0 {})\"/>",
                    f.outline
                        .iter()
                        .map(|p| format!("{:.4},{:.4}", p[0], p[1]))
                        .collect::<Vec<_>>()
                        .join(" "),
                    i as f64 - 1.
                );
                d.stamps.push(f);
                let mut vein = feather(
                    d,
                    &atlas,
                    format!("Cut rachis {side}, {tier} {}", i + 1),
                    theta,
                    high,
                    fraction,
                    (len - i as f64 * 0.6) * 0.73,
                    0.18,
                    if high { 1. } else { -1. },
                    0.08,
                )?;
                vein.cut = true;
                vein.bench = true;
                vein.tier = 1;
                vein.sink_mm = 0.09;
                vein.top = StampTop::Flat;
                if i == 1 {
                    d.stamps.push(vein);
                }
            }
            svg += "</g></svg>";
            std::fs::write(art.join(format!("wing-{side}-{tier}.svg")), svg)?;
        }
        for (i, (theta, len, fraction)) in [(310., 16., 0.42), (279., 12., 0.60)]
            .into_iter()
            .enumerate()
        {
            let theta = if high {
                (180.0_f64 - theta).rem_euclid(360.)
            } else {
                theta
            };
            let f = feather(
                d,
                &atlas,
                format!("Tail {side}, streamer {}", i + 1),
                theta,
                high,
                fraction,
                len,
                1.0,
                if high { 1. } else { -1. },
                0.30,
            )?;
            d.stamps.push(f);
        }
        // A crested raptor profile at the breast, flowing straight into its wing.
        // Both cheek reliefs describe the same head round the central fire opal.
        let theta = 90.;
        let ctx = d.field_context();
        let sides = ctx.side_faces_at(theta).unwrap();
        let (lo, hi) = if high { sides.high } else { sides.low }.unwrap();
        let mut head = Stamp {
            name: format!("Phoenix head, {side}"),
            theta_deg: theta,
            v_mm: (lo + hi) * 0.5,
            rot_deg: if high { 180. } else { 0. },
            outline: bird_outline(),
            height_mm: 0.10,
            sink_mm: 0.12,
            draft_deg: 3.,
            cut: false,
            bench: false,
            along_pull: true,
            tier: 0,
            top: StampTop::Dome { crown_mm: 0.28 },
        };
        // Head is short enough that the cheek's own projection keeps its entire
        // silhouette on the breast; the beak is blunt enough for Delft clay.
        let area: f64 = (0..head.outline.len())
            .map(|i| {
                let p = head.outline[i];
                let q = head.outline[(i + 1) % head.outline.len()];
                p[0] * q[1] - p[1] * q[0]
            })
            .sum();
        if area < 0. {
            head.outline.reverse();
        }
        let mut eye = head.clone();
        eye.name = format!("Engraved eye, {side}");
        eye.cut = true;
        eye.bench = true;
        eye.tier = 1;
        eye.height_mm = 0.10;
        eye.sink_mm = 0.18;
        eye.top = StampTop::Flat;
        eye.outline = (0..16)
            .map(|i| {
                let t = i as f64 * 2. * PI / 16.;
                [0.56 + 0.19 * t.cos(), 0.39 + 0.15 * t.sin()]
            })
            .collect();
        d.stamps.push(head);
        d.stamps.push(eye);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_trails_keep_fifteen_stones_and_delft_stock() {
        let mut d = band();
        seats(&mut d);
        let r = ringdesign_core::stones::report(&d, 0.).unwrap();
        assert_eq!(r.stone_count, 15);
        assert_eq!(
            r.seats.iter().map(|s| s.count).collect::<Vec<_>>(),
            vec![1, 7, 7]
        );
        assert_eq!(opal().l_mm, 8.);
        assert_eq!(opal().w_mm, 6.);
        assert_eq!(r.tight_pairs, 0);
        assert!(r.closest.unwrap().worst_mm() > 1.2);
        assert!(
            r.seats
                .iter()
                .filter_map(|s| s.bridge_mm)
                .all(|b| b >= d.draft.min_section_mm)
        );
    }
}

fn bird_outline() -> Vec<[f64; 2]> {
    let mut p = vec![[-2.8, -0.8]];
    for (a, b, c) in [
        ([-1.7, -0.85], [-0.8, -0.17], [-0.10, -0.15]),
        ([0.70, -0.05], [1.30, -0.40], [1.65, -0.68]),
        ([1.96, -0.58], [1.98, -0.25], [1.60, 0.06]),
        ([1.42, 0.24], [1.20, 0.32], [1.02, 0.48]),
        ([0.89, 1.12], [0.16, 1.28], [-0.62, 0.88]),
        ([-1.30, 1.44], [-2.50, 1.37], [-3.25, 1.13]),
        ([-2.72, 1.00], [-2.15, 0.84], [-1.84, 0.66]),
        ([-2.41, 0.85], [-3.22, 0.74], [-3.54, 0.42]),
        ([-3.04, 0.44], [-2.56, 0.20], [-2.32, 0.06]),
        ([-2.51, -0.27], [-2.72, -0.60], [-2.8, -0.8]),
    ] {
        let s = *p.last().unwrap();
        for i in 1..=4 {
            let t = i as f64 / 4.;
            let u = 1. - t;
            p.push(std::array::from_fn(|k| {
                u * u * u * s[k] + 3. * u * u * t * a[k] + 3. * u * t * t * b[k] + t * t * t * c[k]
            }));
        }
    }
    p.pop();
    p
}

fn seats(d: &mut RingDesign) {
    let ctx = d.field_context();
    let mut seat = SeatPadLayer {
        theta_deg: 90.,
        v_mm: ringdesign_core::setting::crest_v(d, 90.).unwrap(),
        style: SeatStyle::Boss,
        crown: 1.,
        blend_mm: 0.55,
        metal_true: true,
        solid: SolidKind::Bezel,
        mark_mm: 0.8,
        ..Default::default()
    };
    seat.fit_stone(opal());
    seat.height_mm = 0.7;
    d.layers.layers.push(LayerEntry::new(
        "Fire opal, made collet",
        Layer::SeatPad(seat),
    ));
    for (name, theta, span) in [
        ("Fire trail, east", 6., 114.),
        ("Fire trail, west", 180., 118.),
    ] {
        let mut run = SeatRunLayer {
            gem: sapphire(),
            taper: 0.5,
            taper_theta_deg: 90.,
            bridge_mm: 0.90,
            seat: SeatPadLayer {
                style: SeatStyle::GypsyMound,
                height_mm: 0.5,
                crown: 1.,
                blend_mm: 0.5,
                solid: SolidKind::Bead,
                v_mm: ctx.crest_v_mm,
                mark_mm: 0.6,
                ..Default::default()
            },
            ..Default::default()
        };
        run.solve_spacing(&ctx);
        let mut e = LayerEntry::new(name, Layer::SeatRun(run));
        e.window = Window::around(theta, span);
        e.window.fade_deg = 6.;
        d.layers.layers.push(e);
    }
}

fn author(art: &Path) -> Result<(RingDesign, AlphaLibrary, f64)> {
    std::fs::create_dir_all(art)?;
    let mut d = band();
    let mut lib = AlphaLibrary::default();
    let clamp = plumage(&mut d, &mut lib, art)?;
    wings(&mut d, art)?;
    seats(&mut d);
    Ok((d, lib, clamp))
}

fn solid(mesh: &mesh::Mesh) -> csg::Solid {
    csg::Solid {
        v: mesh
            .vertices
            .iter()
            .map(|p| [p.0 as f64, p.1 as f64, p.2 as f64])
            .collect(),
        f: mesh.faces.clone(),
    }
}

fn checked_part(name: &str, part: &csg::Solid) -> serde_json::Value {
    let c = part.check(true);
    json!({"name":name,"vertices":part.v.len(),"faces":part.f.len(),"open_edges":c.open_edges,
        "repeated_edges":c.repeated_edges,"degenerate_faces":c.zero_area_faces,"self_crossings":c.self_crossings,
        "volume_mm3":c.volume})
}

/// Inspect actual projected solids, rather than inferring their quality from
/// the final boolean. The crest settings cannot reach these cheek footprints;
/// that separation is measured by a second projection with settings removed.
fn made_parts(d: &RingDesign, lib: &AlphaLibrary, p: BuildParams) -> Result<serde_json::Value> {
    let ctx = d.field_context();
    let mut parts = Vec::new();
    let mut projection = Vec::new();
    let tiers: std::collections::BTreeSet<_> = d.stamps.iter().map(|s| s.tier).collect();
    for tier in tiers {
        let mut prior = d.clone();
        prior.stamps.retain(|s| s.tier < tier);
        let staged = mesh::try_build(&prior, lib, p)?;
        ensure!(
            staged.solids.notes.is_empty(),
            "Prior tier {tier} did not resolve"
        );
        let mut naked = ringdesign_core::setting::without_solids(&prior);
        naked.stamps = prior.stamps.clone();
        let no_settings = mesh::try_build(&naked, lib, p)?;
        ensure!(
            no_settings.solids.notes.is_empty(),
            "Prior tier {tier} without settings did not resolve"
        );
        let on = solid(&staged.mesh);
        let without = solid(&no_settings.mesh);
        for s in d.stamps.iter().filter(|s| s.tier == tier) {
            let frame = s.frame(d, &ctx);
            let part = s.solid(&frame, &on).map_err(anyhow::Error::msg)?;
            let alternate = s.solid(&frame, &without).map_err(anyhow::Error::msg)?;
            let same_topology = part.f == alternate.f && part.v.len() == alternate.v.len();
            let max_delta = if same_topology {
                part.v
                    .iter()
                    .zip(&alternate.v)
                    .flat_map(|(a, b)| a.iter().zip(b).map(|(a, b)| (a - b).abs()))
                    .fold(0_f64, f64::max)
            } else {
                f64::INFINITY
            };
            projection.push(
                json!({"name":s.name,"tier":tier,"same_topology":same_topology,
                "identical_coordinates":part.v==alternate.v,"max_coordinate_delta_mm":max_delta}),
            );
            parts.push(checked_part(&s.name, &part));
        }
    }
    for (stone, frame) in ringdesign_core::stones::stone_frames(d) {
        let stand = stone.stand_off_mm();
        let uv = ringdesign_core::field::Uv {
            u: ctx.u_of_theta(stone.theta_deg),
            v: stone.v_mm,
        };
        let relief = d.layers.height(uv, &ctx, lib);
        let bare = [
            frame.girdle[0] - frame.normal[0] * stand,
            frame.girdle[1] - frame.normal[1] * stand,
        ];
        let r = bare[0].hypot(bare[1]);
        let outward = (frame.normal[0] * bare[0] + frame.normal[1] * bare[1]) / r.max(1e-9);
        let through = (stone.seat.through
            && outward > 0.75
            && stone.gem.form == ringdesign_core::gem::GemForm::Faceted)
            .then(|| stand + (r - d.inner_radius_mm()).max(0.) / outward + 0.6);
        let fit = ringdesign_core::setting::Fit {
            surface_z: relief - stand,
            through_mm: through,
            prongs: stone.seat.prongs,
        };
        let made = ringdesign_core::setting::parts(stone.gem, stone.seat.solid, fit)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        for (phase, solids) in [("head", &made.add), ("cutter", &made.cut)] {
            for (i, solid) in solids.iter().enumerate() {
                parts.push(checked_part(&format!("{} {phase} {i}", stone.label), solid));
            }
        }
        for (i, (centre, radius)) in made.beads.iter().enumerate() {
            parts.push(checked_part(
                &format!("{} bead {i}", stone.label),
                &ringdesign_core::setting::ball(*centre, *radius, 8),
            ));
        }
    }
    Ok(json!({"parts":parts,"stamp_projection_independence":projection}))
}

fn stone_json(d: &RingDesign) -> serde_json::Value {
    let report = ringdesign_core::stones::report(d, 0.).unwrap();
    json!({"count":report.stone_count,"carats":report.total_carats,"tight_pairs":report.tight_pairs,
        "closest":report.closest.as_ref().map(|p|json!({"girdle_mm":p.gap_mm,"depth_mm":p.gap_deep_mm})),
        "crowding":report.crowding.iter().map(|p|json!({"a":p.a,"b":p.b,"girdle_mm":p.gap_mm,"depth_mm":p.gap_deep_mm})).collect::<Vec<_>>(),
        "warnings":report.seats.iter().flat_map(|s|s.warnings.iter().map(|w|format!("{}: {w}",s.label))).collect::<Vec<_>>(),
        "stations":ringdesign_core::stones::stone_frames(d).iter().map(|(s,f)|json!({"name":s.label,"theta":s.theta_deg,"gem":s.gem,"girdle":f.girdle})).collect::<Vec<_>>()})
}

/// Weld only the preview stones, preserving their exact vertex positions. The
/// welded cabochon gets area-weighted smooth normals; the sapphires still shade
/// by facets. The component census counts the meshes actually sent to rendering.
fn preview_stones(
    d: &RingDesign,
    lib: &AlphaLibrary,
    b: &mesh::BuildResult,
) -> (Vec<(mesh::Mesh, [f32; 3])>, usize) {
    let mut count = 0;
    let groups = ringdesign_core::gems::built_meshes(d, lib, b)
        .into_iter()
        .map(|(m, tint)| {
            let mut out = mesh::Mesh::default();
            let mut index = std::collections::HashMap::new();
            for f in m.faces {
                let face = f.map(|i| {
                    let p = m.vertices[i as usize];
                    let key = [p.0, p.1, p.2].map(|x| (x * 1e5).round() as i64);
                    *index.entry(key).or_insert_with(|| {
                        out.vertices.push(p);
                        (out.vertices.len() - 1) as u32
                    })
                });
                out.faces.push(face);
            }
            let mut parent: Vec<usize> = (0..out.vertices.len()).collect();
            fn root(p: &mut [usize], mut i: usize) -> usize {
                while p[i] != i {
                    p[i] = p[p[i]];
                    i = p[i];
                }
                i
            }
            for f in &out.faces {
                for j in [1, 2] {
                    let a = root(&mut parent, f[0] as usize);
                    let z = root(&mut parent, f[j] as usize);
                    parent[z] = a;
                }
            }
            let mut connected = std::collections::BTreeSet::new();
            for i in 0..parent.len() {
                connected.insert(root(&mut parent, i));
            }
            count += connected.len();
            out.normals = vec![mesh::Vec3(0., 0., 0.); out.vertices.len()];
            for f in &out.faces {
                let [a, b, c] = f.map(|i| out.vertices[i as usize]);
                let u = [b.0 - a.0, b.1 - a.1, b.2 - a.2];
                let v = [c.0 - a.0, c.1 - a.1, c.2 - a.2];
                let n = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                for i in f {
                    let q = &mut out.normals[*i as usize];
                    q.0 += n[0];
                    q.1 += n[1];
                    q.2 += n[2];
                }
            }
            for n in &mut out.normals {
                let len = (n.0 * n.0 + n.1 * n.1 + n.2 * n.2).sqrt().max(1e-12);
                n.0 /= len;
                n.1 /= len;
                n.2 /= len;
            }
            (out, tint)
        })
        .collect();
    (groups, count)
}

fn crop(m: &mesh::Mesh, centre: [f64; 3], radius: f64) -> mesh::Mesh {
    let mut out = m.clone();
    out.faces.retain(|f| {
        f.iter().any(|i| {
            let p = m.vertices[*i as usize];
            (p.0 as f64 - centre[0]).powi(2)
                + (p.1 as f64 - centre[1]).powi(2)
                + (p.2 as f64 - centre[2]).powi(2)
                < radius * radius
        })
    });
    let used: std::collections::BTreeSet<u32> = out.faces.iter().flatten().copied().collect();
    let mut remap = vec![0u32; out.vertices.len()];
    out.vertices = used
        .iter()
        .enumerate()
        .map(|(i, j)| {
            remap[*j as usize] = i as u32;
            m.vertices[*j as usize]
        })
        .collect();
    out.normals = used.iter().map(|j| m.normals[*j as usize]).collect();
    for f in &mut out.faces {
        for i in f {
            *i = remap[*i as usize];
        }
    }
    out
}

fn renders(
    out: &Path,
    d: &RingDesign,
    lib: &AlphaLibrary,
    b: &mesh::BuildResult,
    edge: usize,
) -> Result<usize> {
    let (gems, count) = preview_stones(d, lib, b);
    let parts_of = || {
        gems.iter().map(|(m, t)| {
            let mut p = Part::tinted_stone(m, *t);
            p.smooth = *t == opal().preview_tint.unwrap();
            p
        })
    };
    // Match the collection's smooth-shaded studio export. The original mesh,
    // including its corner normals, remains authoritative and is verified cold.
    let mut display = b.mesh.clone();
    display.corner_normals.clear();
    let mut parts = vec![Part::metal(&display, render::GOLD)];
    parts.extend(parts_of());
    for (m, t) in &gems {
        let name = if *t == opal().preview_tint.unwrap() {
            "reference-fire-opal.stl"
        } else {
            "reference-orange-sapphires.stl"
        };
        stl::write_stl(out.join(name), m, "Phoenix preview stones")?;
    }
    for (name, yaw, pitch) in [
        ("hero", 0.38, 0.60),
        ("face", 0., PI * 0.5),
        ("palm", PI, PI * 0.5),
        ("side", 0., 0.),
        ("shoulder", -0.8, 0.45),
        ("reverse", PI, 0.2),
    ] {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    let f = &ringdesign_core::stones::stone_frames(d)[0].1;
    let close = crop(&display, f.girdle, 7.5);
    let mut detail = vec![
        Part::metal(&close, render::GOLD),
        Part::metal(&display, render::GOLD),
    ];
    detail.extend(parts_of());
    render::write_png_parts(out.join("stones.png"), &detail, 0.3, 1.18, edge)?;
    let bare = mesh::try_build(&band(), lib, params(true))?;
    let left = render::render_parts_ss(
        &[Part::metal(&bare.mesh, render::GOLD)],
        0.38,
        0.60,
        edge,
        edge,
        3,
    );
    let right = render::render_parts_ss(&parts, 0.38, 0.60, edge, edge, 3);
    let mut pair = Vec::with_capacity(edge * edge * 6);
    for y in 0..edge {
        pair.extend_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        pair.extend_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    image::save_buffer(
        out.join("bare-vs-finished.png"),
        &pair,
        (edge * 2) as u32,
        edge as u32,
        image::ColorType::Rgb8,
    )?;
    Ok(count)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("showcase/bestiarium/phoenix"));
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    std::fs::create_dir_all(&out)?;
    let art = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/bestiarium/art/phoenix");
    let (d, lib, clamp) = author(&art)?;
    let stones = stone_json(&d);
    println!("  stones {stones}");
    std::fs::write(out.join("layout.json"), serde_json::to_vec_pretty(&stones)?)?;
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    if args.iter().any(|a| a == "--layout") {
        return Ok(());
    }
    let p = params(draft);
    println!("  building {} × {}", p.theta_steps, p.profile_steps);
    let built = mesh::try_build(&d, &lib, p)?;
    println!(
        "  built {} triangles in {} ms; notes {:?}",
        built.mesh.faces.len(),
        built.report.build_ms,
        built.solids.notes
    );
    stl::write_stl(
        out.join("finished-metal.stl"),
        &built.mesh,
        "Phoenix finished metal",
    )?;
    let preview_count = renders(&out, &d, &lib, &built, if draft { 1000 } else { 1600 })?;
    println!("  rendered draft views");
    let mut cold_equal = false;
    if verify {
        let restored = library::load_design(out.join("design.ring.json"))?;
        let cold = mf::source_library(&restored, &AlphaLibrary::default()).into_owned();
        let again = mesh::try_build(&restored, &cold, p)?;
        cold_equal = built.mesh.vertices == again.mesh.vertices
            && built.mesh.faces == again.mesh.faces
            && built.mesh.normals == again.mesh.normals;
        ensure!(cold_equal, "Cold reopen changed the mesh");
    }
    let cross = csg::self_crossings(&solid(&built.mesh));
    let made = made_parts(&d, &lib, p)?;
    let field = castability::judged_field_report(&d, &lib, &d.draft, 256, 128, Some(&built));
    let findings = dfm::findings_in(&d, &lib);
    let setup = mf::Setup {
        recipe: mf::Recipe::sand(SandProcess::DelftClay),
        sample_pitch_mm: 0.1,
        ..Default::default()
    };
    let inspection = mf::inspect(&d, &lib, &setup, p)?;
    stl::write_stl(
        out.join("pattern.stl"),
        &inspection.prepared.mesh,
        "Phoenix Delft pattern",
    )?;
    let mut fine = setup.clone();
    fine.sample_pitch_mm = 0.075;
    let release_fine = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
    let report = json!({"design":"Phoenix","draft":draft,"build":built.report,"plumage_clamp_mm":clamp,
        "cold_equal":cold_equal,"mesh_self_crossings":cross,"made_parts":made,"solids_notes":built.solids.notes,
        "solids":{"seats":built.solids.resolved,"stamps":built.solids.stamped},"field":field,
        "dfm":findings.iter().map(|f|format!("{}: {}",f.label,f.message)).collect::<Vec<_>>(),
        "stones":stones,"preview_stone_components":preview_count,"release_0_1":inspection.release,"release_0_075":release_fine});
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    println!(
        "  field {:?}, DFM {}, crossings {}, releases {}/{}",
        field.verdict,
        findings.len(),
        cross,
        inspection.release.obstructions.len(),
        release_fine.obstructions.len()
    );
    ensure!(
        built.report.validation.watertight && built.report.quality.degenerate_faces == 0,
        "Mesh validation failed"
    );
    ensure!(
        built.solids.notes.is_empty() && cross == 0,
        "Boolean or self-crossing failure"
    );
    ensure!(
        made["parts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["open_edges"] == 0
                && p["repeated_edges"] == 0
                && p["degenerate_faces"] == 0
                && p["self_crossings"] == 0),
        "A raw made part fails its geometry gate"
    );
    ensure!(
        made["stamp_projection_independence"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["same_topology"] == true
                && p["max_coordinate_delta_mm"]
                    .as_f64()
                    .is_some_and(|d| d < 1e-5)),
        "A cheek stamp projection depends on settings phase"
    );
    ensure!(clamp <= 0.05, "Plumage clamp exceeds 0.05 mm");
    ensure!(
        field.verdict == Verdict::Castable && findings.is_empty(),
        "Casting field or DFM failed"
    );
    ensure!(
        inspection.release.obstructions.is_empty()
            && inspection.release.unresolved_rays == 0
            && release_fine.obstructions.is_empty()
            && release_fine.unresolved_rays == 0,
        "Delft release failed"
    );
    ensure!(stones["count"] == 15, "Expected 15 visible stones");
    ensure!(
        preview_count == 15,
        "Expected 15 actual preview stone components"
    );
    Ok(())
}
