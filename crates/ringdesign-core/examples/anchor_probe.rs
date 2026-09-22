//! M1 spike R5: the ring anchor seats a part at `inner_radius + profile.thickness + height`,
//! the bare reference crest. This measures, per template and angle, how far that radius sits
//! from the metal the build actually puts there — a ray from outside the ring in the z = 0 plane
//! against the built mesh — so a part's foot is known to float or bury before placement is
//! rebuilt on the surface.
//!
//!     cargo run --release --example anchor_probe
use ringdesign_core::{AlphaLibrary, BuildParams, interaction::picking::raycast, mesh, templates};

fn main() {
    let lib = AlphaLibrary::builtin();
    let params = BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() };
    println!("{:<30} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}   worst", "template (gap = surface − anchor, mm)", "0°", "45°", "90°", "135°", "180°", "270°");
    for t in templates::all() {
        let d = t.design();
        let Ok(built) = mesh::try_build(&d, &lib, params) else {
            println!("{:<30} does not build", t.name);
            continue;
        };
        let anchor = d.inner_radius_mm() + d.profile.thickness_mm;
        let mut row = String::new();
        let mut worst: f64 = 0.0;
        for theta in [0.0_f64, 45.0, 90.0, 135.0, 180.0, 270.0] {
            let a = theta.to_radians();
            let far = 40.0;
            let origin = [(far * a.cos()) as f32, (far * a.sin()) as f32, 0.0];
            let direction = [(-a.cos()) as f32, (-a.sin()) as f32, 0.0];
            match raycast(&built.mesh, origin, direction) {
                Some((_, p)) => {
                    let surface = (p[0] as f64).hypot(p[1] as f64);
                    let gap = surface - anchor;
                    worst = worst.max(gap.abs());
                    row.push_str(&format!(" {gap:>+7.2}"));
                }
                None => row.push_str(&format!(" {:>7}", "miss")),
            }
        }
        println!("{:<30}{row}   {worst:.2}", t.name);
    }
    println!();
    println!("A positive gap buries the part's foot in metal it stands on; a negative one floats it.");
}
