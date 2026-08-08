// Does a narrow shank swelling into a signet head still pull from sand?
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability;
use ringdesign_core::field::{Layer, LayerEntry, SignetLayer, SignetOutline};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::ShankKind;
use ringdesign_core::{ProfileStyle, RingDesign};

#[path = "common/raster.rs"]
mod raster;

const W: usize = 780;
const H: usize = 780;

fn params() -> BuildParams {
    BuildParams { theta_steps: 900, profile_steps: 256, ..Default::default() }
}

/// A classic signet: round shank, broad head, flat table.
fn signet(outline: SignetOutline, head_w: f64, amount: f64, table: f64) -> RingDesign {
    let mut d = RingDesign::default();
    // A flat crest is what lets the table land as a true plane; the taper's own
    // crown scaling rounds the shank back off toward a wire.
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = head_w;
    d.profile.thickness_mm = 2.8;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Signet;
    d.shank.amount = amount;
    // The head silhouette and the table read as one shape when they share a
    // fullness: an oval table on an oval head.
    d.shank.head_shape_a = outline.exponent();

    let ctx = d.field_context();
    let mut s = SignetLayer::fitted_to(&ctx);
    s.outline = outline;
    s.height_mm = table;
    // A crisp facet rather than a soft swelling: most of the face dead flat,
    // then a short shoulder.
    s.top_flat = 0.86;
    s.shoulder_mm = 0.8;
    d.layers.layers.push(LayerEntry::new("table", Layer::Signet(s)));
    d
}

fn run(name: &str, d: &RingDesign, lib: &AlphaLibrary, out: &str, views: &[(&str, f64, f64)]) {
    let built = mesh::build(d, lib, params());
    let rep = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
    let (min, max) = built.mesh.bounds().unwrap();
    println!(
        "{name:<26} {:<18} {:>6.3}% undercut  worst {:>+7.2} deg  band z {:.2} mm  watertight {}",
        rep.verdict.label(),
        rep.undercut_fraction() * 100.0,
        rep.worst_draft_deg,
        (max.2 - min.2) as f64,
        built.report.validation.watertight,
    );
    for (view, yaw, pitch) in views {
        let img = raster::render(&built.mesh, *yaw, *pitch, W, H);
        image::save_buffer(
            format!("{out}/{name}_{view}.png"),
            &img,
            W as u32,
            H as u32,
            image::ColorType::Rgb8,
        )
        .unwrap();
    }
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp".into());
    let lib = AlphaLibrary::builtin();

    // How hard the taper can be pushed before the swell locks in the sand.
    println!("taper strength, 12 mm head:");
    for amount in [0.0, 0.3, 0.5, 0.7, 0.85, 1.0] {
        let d = signet(SignetOutline::Oval, 12.0, amount, 0.5);
        let frac = d.shank.signet_width_frac(270.0);
        let built = mesh::build(&d, &lib, params());
        let rep = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
        println!(
            "  amount {amount:.2}  shank {:.2} mm ({:.0}% of head)  {:<18} {:>6.3}% undercut  worst {:>+7.2} deg",
            12.0 * frac,
            frac * 100.0,
            rep.verdict.label(),
            rep.undercut_fraction() * 100.0,
            rep.worst_draft_deg
        );
    }

    println!("\ntable size (fraction of the room across the head):");
    for frac in [0.55, 0.70, 0.82, 0.92] {
        let mut d = signet(SignetOutline::Oval, 12.0, 0.85, 0.5);
        d.shank.head_span_deg = 100.0;
        let ctx = d.field_context();
        let room = SignetLayer::room_across(&ctx);
        if let Some(Layer::Signet(t)) = d.layers.layers.last_mut().map(|e| &mut e.layer) {
            t.width_mm = room * frac;
            t.length_mm = t.width_mm * 1.55;
        }
        let over = match d.layers.layers.last().map(|e| &e.layer) {
            Some(Layer::Signet(t)) => t.overhangs(&ctx),
            _ => false,
        };
        let built = mesh::build(&d, &lib, params());
        let rep = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
        let name = format!("tbl_{:.0}", frac * 100.0);
        println!(
            "  {name:<8} table {:.1} mm of {room:.1} mm room  overhangs {over:<5}  {:<18} {:>6.3}% undercut  worst {:>+6.2} deg",
            room * frac,
            rep.verdict.label(),
            rep.undercut_fraction() * 100.0,
            rep.worst_draft_deg
        );
        for (view, yaw, pitch) in [("hero", 0.5, 1.15), ("face", 0.0, 1.571)] {
            let img = raster::render(&built.mesh, yaw, pitch, W, H);
            image::save_buffer(
                format!("{out}/{name}_{view}.png"),
                &img, W as u32, H as u32, image::ColorType::Rgb8,
            ).unwrap();
        }
    }

    println!("\nhead arc:");
    for span in [90.0, 110.0, 130.0, 156.0] {
        let mut d = signet(SignetOutline::Oval, 12.0, 0.85, 0.5);
        d.shank.head_span_deg = span;
        let built = mesh::build(&d, &lib, params());
        let rep = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
        let name = format!("span_{span:.0}");
        println!(
            "  {name:<10} {:<18} {:>6.3}% undercut  worst {:>+6.2} deg",
            rep.verdict.label(),
            rep.undercut_fraction() * 100.0,
            rep.worst_draft_deg
        );
        for (view, yaw, pitch) in [("hero", 0.5, 1.15), ("face", 0.0, 1.571)] {
            let img = raster::render(&built.mesh, yaw, pitch, W, H);
            image::save_buffer(
                format!("{out}/{name}_{view}.png"),
                &img, W as u32, H as u32, image::ColorType::Rgb8,
            ).unwrap();
        }
    }

    println!("\nheads:");
    // The head faces +Y, so a pitch near a right angle looks straight at it.
    let views: &[(&str, f64, f64)] =
        &[("hero", 0.5, 1.15), ("face", 0.0, 1.571), ("profile", 1.571, 1.571)];
    run("sig_oval", &signet(SignetOutline::Oval, 12.0, 0.85, 0.5), &lib, &out, views);
    run("sig_cushion", &signet(SignetOutline::Cushion, 12.5, 0.85, 0.5), &lib, &out, views);
    run("sig_round", &signet(SignetOutline::Round, 11.5, 0.85, 0.5), &lib, &out, views);
    run("sig_rectangle", &signet(SignetOutline::Rectangle, 11.0, 0.85, 0.45), &lib, &out, views);
    run("no_taper_control", &signet(SignetOutline::Oval, 12.0, 0.0, 0.5), &lib, &out, views);
}
