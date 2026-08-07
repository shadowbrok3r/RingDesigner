// Software z-buffer rasterizer: renders a built ring to a PNG for eyeballing.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::field::{BorderLayer, Layer, LayerEntry, MilgrainLayer, SeatPadLayer};
use ringdesign_core::mesh::{BuildParams, Mesh};
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{ProfileStyle, RingDesign, mesh};

const W: usize = 900;
const H: usize = 900;

fn render(m: &Mesh, yaw: f64, pitch: f64) -> Vec<u8> {
    render_classed(m, yaw, pitch, None)
}

fn render_classed(m: &Mesh, yaw: f64, pitch: f64, classes: Option<&[ringdesign_core::FaceClass]>) -> Vec<u8> {
    let (min, max) = m.bounds().unwrap();
    let c = [
        (min.0 + max.0) as f64 * 0.5,
        (min.1 + max.1) as f64 * 0.5,
        (min.2 + max.2) as f64 * 0.5,
    ];
    let ext = ((max.0 - min.0).max(max.1 - min.1).max(max.2 - min.2)) as f64;
    let scale = W as f64 / (ext * 1.25);

    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let xf = |p: [f64; 3]| -> [f64; 3] {
        let (x, y, z) = (p[0] - c[0], p[1] - c[1], p[2] - c[2]);
        let (x1, y1) = (x * cy - y * sy, x * sy + y * cy);
        let (y2, z2) = (y1 * cp - z * sp, y1 * sp + z * cp);
        [x1, y2, z2]
    };

    let mut depth = vec![f64::NEG_INFINITY; W * H];
    let mut img = vec![18u8; W * H * 3];
    let light = {
        let l: [f64; 3] = [-0.35, -0.5, 0.79];
        let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        [l[0] / n, l[1] / n, l[2] / n]
    };

    for (fi, f) in m.faces.iter().enumerate() {
        let Some((a, b, cc)) = m.triangle(f) else { continue };
        let (ta, tb, tc) = (xf(a), xf(b), xf(cc));
        let n = {
            let e1 = [tb[0] - ta[0], tb[1] - ta[1], tb[2] - ta[2]];
            let e2 = [tc[0] - ta[0], tc[1] - ta[1], tc[2] - ta[2]];
            let v = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if l < 1e-12 { continue } else { [v[0] / l, v[1] / l, v[2] / l] }
        };
        if n[2] <= 0.0 {
            continue;
        }
        let px = |p: [f64; 3]| (p[0] * scale + W as f64 * 0.5, -p[1] * scale + H as f64 * 0.5);
        let (p0, p1, p2) = (px(ta), px(tb), px(tc));
        let minx = p0.0.min(p1.0).min(p2.0).floor().max(0.0) as usize;
        let maxx = (p0.0.max(p1.0).max(p2.0).ceil() as usize).min(W - 1);
        let miny = p0.1.min(p1.1).min(p2.1).floor().max(0.0) as usize;
        let maxy = (p0.1.max(p1.1).max(p2.1).ceil() as usize).min(H - 1);
        let area = (p1.0 - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (p1.1 - p0.1);
        if area.abs() < 1e-9 {
            continue;
        }
        let diff = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0);
        let spec = diff.powf(28.0);
        let shade = 0.16 + 0.72 * diff;
        let base = match classes {
            Some(cs) => cs.get(fi).map(|c| c.rgb()).unwrap_or([0.87, 0.71, 0.43]),
            None => [0.87, 0.71, 0.43],
        };
        let lit = if classes.is_some() { 0.35 + 0.65 * diff } else { shade };
        let hl = if classes.is_some() { 0.0 } else { spec * 0.85 };
        let col = [
            ((base[0] as f64 * lit + hl) * 255.0).min(255.0) as u8,
            ((base[1] as f64 * lit + hl) * 255.0).min(255.0) as u8,
            ((base[2] as f64 * lit + hl) * 255.0).min(255.0) as u8,
        ];
        for y in miny..=maxy {
            for x in minx..=maxx {
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                let w0 = ((p1.0 - p0.0) * (fy - p0.1) - (fx - p0.0) * (p1.1 - p0.1)) / area;
                let w1 = ((fx - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (fy - p0.1)) / area;
                if w0 < 0.0 || w1 < 0.0 || w0 + w1 > 1.0 {
                    continue;
                }
                let z = ta[2] + w1 * (tb[2] - ta[2]) + w0 * (tc[2] - ta[2]);
                let i = y * W + x;
                if z > depth[i] {
                    depth[i] = z;
                    img[i * 3] = col[0];
                    img[i * 3 + 1] = col[1];
                    img[i * 3 + 2] = col[2];
                }
            }
        }
    }
    img
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp".into());
    let lib = AlphaLibrary::builtin();

    let variants: Vec<(&str, Box<dyn Fn(&mut RingDesign, &AlphaLibrary)>)> = vec![
        ("1_rope_tiled", Box::new(|d: &mut RingDesign, _l: &AlphaLibrary| {
            d.profile.width_mm = 7.0;
            d.profile.thickness_mm = 2.4;
            d.profile.apply_style(ProfileStyle::HalfRound);
            let ctx = d.field_context();
            let mut t = TilingLayer::default_for("Rope", &ctx);
            t.repeats_around = 30;
            t.height_mm = 0.45;
            t.v_span_mm = ctx.band_v_len_mm * 0.55;
            d.layers.layers.push(LayerEntry::new("rope", Layer::Tiling(t)));
            d.layers.layers.push(LayerEntry::new("rails", Layer::Border(BorderLayer::default())));
        })),
        ("2_scales_seat", Box::new(|d: &mut RingDesign, _l: &AlphaLibrary| {
            d.profile.width_mm = 8.0;
            d.profile.thickness_mm = 2.6;
            d.profile.apply_style(ProfileStyle::HighDome);
            let ctx = d.field_context();
            let mut t = TilingLayer::default_for("Scales", &ctx);
            t.repeats_around = 26;
            t.rows = 3;
            t.height_mm = 0.4;
            t.v_span_mm = ctx.band_v_len_mm * 0.8;
            d.layers.layers.push(LayerEntry::new("scales", Layer::Tiling(t)));
            d.layers.layers.push(LayerEntry::new(
                "seat",
                Layer::SeatPad(SeatPadLayer { diameter_mm: 6.0, height_mm: 1.5, v_mm: ctx.crest_v_mm, ..Default::default() }),
            ));
        })),
        ("3_greekkey_milgrain", Box::new(|d: &mut RingDesign, _l: &AlphaLibrary| {
            d.profile.width_mm = 6.5;
            d.profile.thickness_mm = 2.2;
            d.profile.apply_style(ProfileStyle::DShape);
            let ctx = d.field_context();
            let mut t = TilingLayer::default_for("Greek Key", &ctx);
            t.repeats_around = 20;
            t.height_mm = 0.35;
            t.v_span_mm = ctx.band_v_len_mm * 0.45;
            d.layers.layers.push(LayerEntry::new("key", Layer::Tiling(t)));
            d.layers.layers.push(LayerEntry::new("milgrain", Layer::Milgrain(MilgrainLayer::default())));
        })),
        ("4_braid_cathedral", Box::new(|d: &mut RingDesign, _l: &AlphaLibrary| {
            d.profile.width_mm = 6.0;
            d.profile.thickness_mm = 2.2;
            d.profile.apply_style(ProfileStyle::HalfRound);
            d.shank.kind = ringdesign_core::ShankKind::Cathedral;
            d.shank.amount = 0.8;
            let ctx = d.field_context();
            let mut t = TilingLayer::default_for("Braid", &ctx);
            t.repeats_around = 36;
            t.height_mm = 0.4;
            t.v_span_mm = ctx.band_v_len_mm * 0.6;
            d.layers.layers.push(LayerEntry::new("braid", Layer::Tiling(t)));
        })),
    ];

    for (name, setup) in &variants {
        let mut d = RingDesign::default();
        d.size = ringdesign_core::RingSize(8.0);
        setup(&mut d, &lib);
        let built = mesh::build(&d, &lib, BuildParams { theta_steps: 1024, profile_steps: 320, ..Default::default() });
        println!(
            "{name}: {} tris, watertight={}, {:.1} mm3, {} ms",
            built.report.validation.triangle_count,
            built.report.validation.watertight,
            built.report.volume_mm3,
            built.report.build_ms
        );
        let img = render(&built.mesh, 0.0, 1.05);
        image::save_buffer(format!("{out}/{name}.png"), &img, W as u32, H as u32, image::ColorType::Rgb8).unwrap();
    }

    // Draft-coloured pair: relief on the crest vs relief on the side faces.
    for (name, v_frac, span) in [("5_draft_crest", 0.50, 0.45), ("6_draft_sides", 0.16, 0.22)] {
        let mut d = RingDesign::default();
        d.profile.width_mm = 7.0;
        d.profile.thickness_mm = 2.4;
        d.profile.apply_style(ProfileStyle::HalfRound);
        let c = d.field_context();
        let mut t = TilingLayer::default_for("Hammered", &c);
        t.repeats_around = 16;
        t.height_mm = 0.25;
        t.v_center_mm = c.band_v_len_mm * v_frac;
        t.v_span_mm = c.band_v_len_mm * span;
        d.layers.layers.push(LayerEntry::new("hammered", Layer::Tiling(t)));
        if name.ends_with("sides") {
            let mut t2 = TilingLayer::default_for("Hammered", &c);
            t2.repeats_around = 16;
            t2.height_mm = 0.25;
            t2.v_center_mm = c.band_v_len_mm * (1.0 - span * 0.72);
            t2.v_span_mm = c.band_v_len_mm * span;
            d.layers.layers.push(LayerEntry::new("hammered2", Layer::Tiling(t2)));
        }
        let built = mesh::build(&d, &lib, BuildParams { theta_steps: 768, profile_steps: 256, ..Default::default() });
        let rep = ringdesign_core::castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
        println!(
            "{name}: {} undercut {:.2}% area, worst {:+.1} deg",
            rep.verdict.label(), rep.undercut_fraction() * 100.0, rep.worst_draft_deg
        );
        let img = render_classed(&built.mesh, 0.0, 1.05, Some(&rep.classes));
        image::save_buffer(format!("{out}/{name}.png"), &img, W as u32, H as u32, image::ColorType::Rgb8).unwrap();
    }

    // Signet, flange, and a mirror-tiled fragment of a real ornament.
    {
        let mut d = RingDesign::default();
        d.profile.width_mm = 7.0;
        d.profile.thickness_mm = 2.6;
        d.profile.apply_style(ProfileStyle::HalfRound);
        d.shank.kind = ringdesign_core::ShankKind::Cathedral;
        d.shank.amount = 0.9;
        let c = d.field_context();
        d.layers.layers.push(LayerEntry::new(
            "signet",
            Layer::Signet(ringdesign_core::field::SignetLayer::fitted_to(&c)),
        ));
        let built = mesh::build(&d, &lib, BuildParams { theta_steps: 1024, profile_steps: 320, ..Default::default() });
        println!("7_signet: watertight={} {:.0} mm3", built.report.validation.watertight, built.report.volume_mm3);
        image::save_buffer(format!("{out}/7_signet.png"), &render(&built.mesh, 0.35, 0.75), W as u32, H as u32, image::ColorType::Rgb8).unwrap();
    }
    {
        let mut d = RingDesign::default();
        d.profile.width_mm = 6.0;
        d.profile.thickness_mm = 2.2;
        d.profile.apply_style(ProfileStyle::HalfRound);
        d.profile.flange = ringdesign_core::profile::Flange {
            enabled: true, v_pos: 0.5, extent_mm: 1.4, thickness_mm: 0.9, edge_round_mm: 0.15,
        };
        let built = mesh::build(&d, &lib, BuildParams { theta_steps: 1024, profile_steps: 320, ..Default::default() });
        println!("8_flange: watertight={} {:.0} mm3", built.report.validation.watertight, built.report.volume_mm3);
        image::save_buffer(format!("{out}/8_flange.png"), &render(&built.mesh, 0.0, 0.5), W as u32, H as u32, image::ColorType::Rgb8).unwrap();
    }
    {
        let mut lib2 = lib.clone();
        let mut d = RingDesign::default();
        d.profile.width_mm = 7.0;
        d.profile.thickness_mm = 2.4;
        d.profile.apply_style(ProfileStyle::HalfRound);
        // Harvest a fragment of a real ornament and mirror it into a tile.
        let name = if lib2.get("ornament-b-25").is_some() { "ornament-b-25" } else { "Floral" };
        let src = lib2.get(name).unwrap().clone();
        let (before_h, _) = src.seam_error();
        let clip = src
            .crop(ringdesign_core::alpha::CropRect { x0: 0.30, y0: 0.28, x1: 0.70, y1: 0.72 })
            .auto_trim(0.04, 0.02)
            .mirror_tile(ringdesign_core::alpha::Axis::Horizontal)
            .edge_fade(0.10, ringdesign_core::alpha::Axis::Vertical)
            .renamed("harvested");
        let (after_h, _) = clip.seam_error();
        println!("9_clip: source {name} seam {before_h:.4} -> harvested tile seam {after_h:.4}");
        lib2.insert(clip);
        let c = d.field_context();
        let mut t = TilingLayer::default_for("harvested", &c);
        t.repeats_around = 14;
        t.height_mm = 0.30;
        t.v_span_mm = c.band_v_len_mm * 0.55;
        d.layers.layers.push(LayerEntry::new("harvested", Layer::Tiling(t)));
        let built = mesh::build(&d, &lib2, BuildParams { theta_steps: 1024, profile_steps: 320, ..Default::default() });
        println!("9_clip: watertight={}", built.report.validation.watertight);
        image::save_buffer(format!("{out}/9_clip.png"), &render(&built.mesh, 0.0, 1.05), W as u32, H as u32, image::ColorType::Rgb8).unwrap();
    }
}
