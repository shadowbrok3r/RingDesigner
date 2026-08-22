// Does a narrow shank swelling into a signet head still pull from sand?
//
// The head here is the band itself, not a pad standing on it: the outline is
// the band's own plan silhouette and the table is its crest, so what this
// measures is whether the base geometry releases before any layer is added.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability;
use ringdesign_core::field::SignetOutline;
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

/// A classic signet: round shank, broad flat-topped head, blank table.
fn signet(outline: SignetOutline, head_w: f64, amount: f64, rise: f64) -> RingDesign {
    let mut d = RingDesign::default();
    // Squared sides give the head flanks that face the pull, which is where a
    // signet's ornament goes. The head flattens its own crest whatever the
    // profile, and the taper rounds the shank back off toward a wire.
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = head_w;
    d.profile.thickness_mm = 2.8;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Signet;
    d.shank.amount = amount;
    d.shank.head.outline = outline;
    d.shank.head.rise_mm = rise;
    d.shank.head.fit_length_to(head_w);
    d
}

fn report(name: &str, d: &RingDesign, lib: &AlphaLibrary) {
    let built = mesh::build(d, lib, params());
    let rep = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
    let (min, max) = built.mesh.bounds().unwrap();
    println!(
        "  {name:<14} {:<18} {:>6.3}% undercut  worst {:>+7.2} deg  head {:.2} mm tall  \
         watertight {}",
        rep.verdict.label(),
        rep.undercut_fraction() * 100.0,
        rep.worst_draft_deg,
        (max.1 - min.1) as f64 * 0.5,
        built.report.validation.watertight,
    );
}

fn run(name: &str, d: &RingDesign, lib: &AlphaLibrary, out: &str, views: &[(&str, f64, f64)]) {
    report(name, d, lib);
    let built = mesh::build(d, lib, params());
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
    let inner_r = RingDesign::default().inner_radius_mm();

    // How hard the taper can be pushed before the swell locks in the sand.
    println!("taper strength, 12 mm head:");
    for amount in [0.0, 0.3, 0.5, 0.7, 0.85, 1.0] {
        let d = signet(SignetOutline::Oval, 12.0, amount, 0.8);
        let crest_r = d.reference_loop().crest_radius_mm;
        let frac = d.shank.signet_width_frac(270.0, inner_r, crest_r, &d.profile);
        report(&format!("amount {amount:.2}"), &d, &lib);
        println!("                 shank {:.2} mm ({:.0}% of head)", 12.0 * frac, frac * 100.0);
    }

    // How far the table can stand off the band before the shoulder walls up.
    println!("\ntable rise:");
    for rise in [0.0, 0.4, 0.8, 1.4, 2.2] {
        let d = signet(SignetOutline::Oval, 12.0, 0.85, rise);
        report(&format!("rise {rise:.1} mm"), &d, &lib);
    }

    // A longer face means a plane held further round, which is what makes the
    // corners of the table stand off the band.
    println!("\nface length:");
    for len in [8.0, 12.0, 16.0, 20.0] {
        let mut d = signet(SignetOutline::Oval, 12.0, 0.85, 0.8);
        d.shank.head.length_mm = len;
        report(&format!("length {len:.0} mm"), &d, &lib);
    }

    println!("\nshoulder arc:");
    for sh in [10.0, 18.0, 26.0, 40.0] {
        let mut d = signet(SignetOutline::Oval, 12.0, 0.85, 0.8);
        d.shank.head.shoulder_deg = sh;
        report(&format!("shoulder {sh:.0}"), &d, &lib);
    }

    println!("\nheads:");
    // The head faces +Y, so a pitch near a right angle looks straight at it.
    let views: &[(&str, f64, f64)] =
        &[("hero", 0.5, 1.15), ("face", 0.0, 1.571), ("profile", 1.571, 1.571)];
    for o in SignetOutline::ALL {
        let name = format!("out_{}", o.label().to_lowercase());
        run(&name, &signet(*o, 12.0, 0.85, 0.8), &lib, &out, views);
    }
}
