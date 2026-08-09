// A spread of finished designs, rendered and saved as projects.
//
// Every one is built only from what the app itself offers, so each is openable
// and editable rather than a picture of something the tool cannot make.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability;
use ringdesign_core::field::{
    BorderLayer, BorderProfile, Layer, LayerEntry, MilgrainLayer, SeatPadLayer, SignetOutline,
    Window, SIDE_FACE_MIN_DRAFT_DEG,
};
use ringdesign_core::mesh::{self, BuildParams, Mesh};
use ringdesign_core::profile::{ShankKind, TOP_DEG};
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{library, ProfileStyle, RingDesign};

#[path = "common/raster.rs"]
mod raster;

/// Rendered at this and box-filtered down, which is the whole of the
/// antialiasing: the rasterizer has none of its own and a ring is mostly edges.
const SS: usize = 3;
const W: usize = 900;
const H: usize = 900;

fn lib() -> AlphaLibrary {
    let mut lib = AlphaLibrary::builtin();
    for dir in library::alpha_dirs() {
        let _ = lib.load_dir(dir);
    }
    lib
}

fn shot(m: &Mesh, yaw: f64, pitch: f64) -> Vec<u8> {
    let big = raster::render(m, yaw, pitch, W * SS, H * SS);
    let mut out = vec![0u8; W * H * 3];
    for y in 0..H {
        for x in 0..W {
            for c in 0..3 {
                let mut sum = 0usize;
                for dy in 0..SS {
                    for dx in 0..SS {
                        sum += big[(((y * SS + dy) * W * SS) + x * SS + dx) * 3 + c] as usize;
                    }
                }
                out[(y * W + x) * 3 + c] = (sum / (SS * SS)) as u8;
            }
        }
    }
    out
}

fn save(dir: &str, name: &str, tag: &str, img: &[u8]) {
    image::save_buffer(
        format!("{dir}/{name}_{tag}.png"),
        img,
        W as u32,
        H as u32,
        image::ColorType::Rgb8,
    )
    .unwrap();
}

fn finish(dir: &str, name: &str, blurb: &str, d: &RingDesign, lib: &AlphaLibrary) {
    let built = mesh::build(
        d,
        lib,
        BuildParams { theta_steps: 1100, profile_steps: 300, ..Default::default() },
    );
    let rep = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
    println!(
        "{name:<22} {blurb}\n{:<22} {} {:.3}% undercut, worst {:+.1} deg, {} tris, watertight {}",
        "",
        rep.verdict.label(),
        rep.undercut_fraction() * 100.0,
        rep.worst_draft_deg,
        built.mesh.faces.len(),
        built.report.validation.watertight,
    );

    save(dir, name, "hero", &shot(&built.mesh, 0.55, 1.12));
    save(dir, name, "face", &shot(&built.mesh, 0.0, 1.571));
    let side = shot(&built.mesh, 1.571, 1.571);
    let mut turned = vec![0u8; side.len()];
    for y in 0..H {
        for x in 0..W {
            for k in 0..3 {
                turned[(x * H + (H - 1 - y)) * 3 + k] = side[(y * W + x) * 3 + k];
            }
        }
    }
    save(dir, name, "side", &turned);

    let designs = library::default_design_dir();
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design(designs.join(format!("{name}.ring.json")), d).unwrap();
}

/// A band with squared side faces, which is what makes the sides castable
/// ground for relief.
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
    d.shank.kind = ShankKind::Signet;
    d.shank.apply_signet(width);
    d.shank.head.outline = outline;
    d.shank.head.fit_length_to(width);
    d
}

fn tiling(d: &RingDesign, alpha: &str, height: f64) -> TilingLayer {
    let mut t = TilingLayer::default_for(alpha, &d.field_context());
    t.height_mm = height;
    t
}

/// Put a tiling on the band's side faces, and say so if there are none — a
/// layer that silently stays on the crest is the difference between relief that
/// releases and relief that locks in the sand.
fn onto_sides(t: &mut TilingLayer, d: &RingDesign) {
    let ctx = d.field_context();
    let ok = t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG);
    match ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG) {
        Some(f) if ok => println!(
            "     sides {:.2} / {:.2} mm, tiling at v {:.2} span {:.2}, mirrored {}",
            f.low_width(), f.high_width(), t.v_center_mm, t.v_span_mm, t.mirror_v
        ),
        _ => println!("     NO SIDE FACES — this band has no castable ground for relief"),
    }
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp".into());
    std::fs::create_dir_all(&dir).unwrap();
    let lib = lib();
    println!("{} alphas\n", lib.len());

    // --- 1. A heart signet, blank for the engraver. ---------------------------
    let d = signet(SignetOutline::Heart, 15.5, 1.6);
    finish(&dir, "01-heart-signet", "heart head, blank table, faired body", &d, &lib);

    // --- 2. Snake scales on a hexagon's side faces. ---------------------------
    // The sides face the pull, so relief there cannot undercut at any height —
    // which is why the scales can be this deep.
    let mut d = signet(SignetOutline::Hexagon, 14.0, 2.6);
    let ctx = d.field_context();
    let mut t = tiling(&d, "snake-3", 0.34);
    onto_sides(&mut t, &d);
    t.repeats_around = t.repeats_for_square_cells(&ctx);
    t.contrast = 1.25;
    d.layers.layers.push(LayerEntry::new("Scales", Layer::Tiling(t)));
    finish(&dir, "02-hexagon-snake", "hexagon head, snake scales on both sides", &d, &lib);

    // --- 3. A cushion signet with ornament on the shoulders only. -------------
    let mut d = signet(SignetOutline::Cushion, 14.5, 2.2);
    let ctx = d.field_context();
    let mut t = tiling(&d, "ornament-a-07", 0.30);
    onto_sides(&mut t, &d);
    t.repeats_around = 34;
    let mut e = LayerEntry::new("Shoulder ornament", Layer::Tiling(t));
    // Windowed off the head, so the table stays a surface a graver can cut.
    e.window = Window::except(TOP_DEG, 120.0);
    d.layers.layers.push(e);
    finish(&dir, "03-cushion-shoulders", "cushion head, ornament windowed off the table", &d, &lib);

    // --- 4. A marquise signet: the longest head the model makes. --------------
    let d = signet(SignetOutline::Marquise, 12.0, 2.0);
    finish(&dir, "04-marquise-signet", "marquise head, long swell", &d, &lib);

    // --- 5. Greek key on the side faces, rails and beads on the crest. ------
    // A binary mask like this raises vertical walls, so it goes where vertical
    // walls are parallel to the pull. The crest keeps domes only.
    let mut d = squared(7.5, 2.4);
    let ctx = d.field_context();
    let mut t = tiling(&d, "Greek Key", 0.28);
    onto_sides(&mut t, &d);
    t.repeats_around = 40;
    t.rows = 1;
    d.layers.layers.push(LayerEntry::new("Greek key", Layer::Tiling(t)));
    d.layers.layers.push(LayerEntry::new(
        "Rails",
        Layer::Border(BorderLayer {
            v_mm: ctx.band_v_len_mm * 0.5 - 1.5,
            width_mm: 0.7,
            height_mm: 0.22,
            profile: BorderProfile::Round,
            mirror: true,
            rope_twists: 0,
        }),
    ));
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
    finish(&dir, "05-greek-key", "greek key on the sides, rails and beads on top", &d, &lib);

    // --- 6. A rope band. -----------------------------------------------------
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::HalfRound);
    d.profile.width_mm = 5.0;
    d.profile.thickness_mm = 2.4;
    let ctx = d.field_context();
    d.layers.layers.push(LayerEntry::new(
        "Rope",
        Layer::Border(BorderLayer {
            v_mm: ctx.band_v_len_mm * 0.5,
            width_mm: 3.2,
            height_mm: 0.26,
            profile: BorderProfile::Rope,
            mirror: false,
            rope_twists: 42,
        }),
    ));
    d.layers.layers.push(LayerEntry::new(
        "Edge beads",
        Layer::Milgrain(MilgrainLayer {
            v_mm: ctx.band_v_len_mm * 0.5 - 2.0,
            bead_diameter_mm: 0.42,
            beads_around: 132,
            height_mm: 0.2,
            mirror: true,
        }),
    ));
    finish(&dir, "06-rope-band", "twisted rope with beaded edges", &d, &lib);

    // --- 7. A cathedral shank carrying a gem seat. ---------------------------
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::DShape);
    d.profile.width_mm = 4.0;
    d.profile.thickness_mm = 2.2;
    d.shank.kind = ShankKind::Cathedral;
    d.shank.amount = 0.8;
    let ctx = d.field_context();
    d.layers.layers.push(LayerEntry::new(
        "Seat",
        Layer::SeatPad(SeatPadLayer {
            theta_deg: TOP_DEG,
            v_mm: ctx.band_v_len_mm * 0.5,
            diameter_mm: 5.4,
            height_mm: 0.9,
            crown: 0.35,
            blend_mm: 2.2,
        }),
    ));
    d.layers.layers.push(LayerEntry::new(
        "Seat milgrain",
        Layer::Milgrain(MilgrainLayer {
            v_mm: ctx.band_v_len_mm * 0.5 - 1.4,
            bead_diameter_mm: 0.34,
            beads_around: 120,
            height_mm: 0.16,
            mirror: true,
        }),
    ));
    finish(&dir, "07-cathedral-seat", "cathedral shoulders, raised gem seat", &d, &lib);

    // --- 8. A hammered comfort band that will not spin. ----------------------
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::HalfRound);
    d.profile.width_mm = 6.5;
    d.profile.thickness_mm = 2.3;
    d.profile.comfort_fit_mm = 0.9;
    d.shank.kind = ShankKind::EuroFlat;
    d.shank.amount = 0.9;
    let mut t = tiling(&d, "Hammered", 0.11);
    t.repeats_around = 22;
    t.rows = 2;
    t.contrast = 0.7;
    d.layers.layers.push(LayerEntry::new("Hammered", Layer::Tiling(t)));
    finish(&dir, "08-hammered-euro", "hammered comfort fit on a euro flat shank", &d, &lib);

    // --- 9. Scale mail down both side faces of a tapered band. --------------
    let mut d = squared(6.5, 2.6);
    d.shank.kind = ShankKind::Tapered;
    d.shank.amount = 0.5;
    let ctx = d.field_context();
    let mut t = tiling(&d, "scale-22", 0.30);
    onto_sides(&mut t, &d);
    t.repeats_around = t.repeats_for_square_cells(&ctx);
    t.stagger = 0.5;
    d.layers.layers.push(LayerEntry::new("Scale mail", Layer::Tiling(t)));
    finish(&dir, "09-scale-mail", "scale mail down both sides of a tapered band", &d, &lib);

    // --- 10. An oval signet with a braided shank behind it. ------------------
    let mut d = signet(SignetOutline::Oval, 13.0, 2.6);
    let ctx = d.field_context();
    let mut t = tiling(&d, "Braid", 0.24);
    onto_sides(&mut t, &d);
    t.repeats_around = 36;
    let mut e = LayerEntry::new("Braid", Layer::Tiling(t));
    e.window = Window::except(TOP_DEG, 150.0);
    d.layers.layers.push(e);
    finish(&dir, "10-oval-braid", "oval head, braided shank behind it", &d, &lib);

    println!("\nwritten to {dir} and {}", library::default_design_dir().display());
}
