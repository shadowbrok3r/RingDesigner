//! M1 spike for M2's headline: a kernel part, tessellated and traced, joins or cuts the built
//! height-field band through `csg.rs` — the operation the analytic kernel refuses on any band.
//! Per template: drop the part onto the surface at the top of the ring (spike R5's rule), union
//! a bezel cylinder, subtract a pilot, and report time, closure and volume.
//!
//!     cargo run --release --example join_probe
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    cad::{self, Component, Document, Feature, Operation},
    csg::{self, Frame, Op, Solid},
    interaction::picking::raycast,
    mesh, templates,
};
use std::time::Instant;

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { std::array::from_fn(|k| a[k] - b[k]) }
fn scale(a: [f64; 3], s: f64) -> [f64; 3] { a.map(|v| v * s) }
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn norm(a: [f64; 3]) -> f64 { a.iter().map(|v| v * v).sum::<f64>().sqrt() }

/// A kernel part as an f64 csg solid, from the traced tessellation.
fn part(op: Operation) -> Solid {
    let mut d = RingDesign::default();
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "part".into(), enabled: true, operation: op, component: Component::default() }).unwrap();
    d.cad = Some(doc);
    let e = cad::evaluate(&d, &AlphaLibrary::builtin(), BuildParams::default()).expect("part evaluates");
    let c = e.components.into_iter().next().unwrap();
    Solid { v: c.trace.positions.clone(), f: c.mesh.faces.clone() }
}

fn report(label: &str, started: Instant, result: Result<Solid, csg::Snag>, before: f64) -> Option<Solid> {
    let ms = started.elapsed().as_secs_f64() * 1e3;
    match result {
        Ok(mut s) => {
            let cleaned = csg::clean(&mut s, 2e-5);
            let (open, repeated) = s.open_edges();
            let dv = s.volume() - before;
            println!("    {label:<22} {ms:>8.1} ms  {} faces  open {open} repeated {repeated}  cleaned {cleaned}  Δvolume {dv:+.2} mm³", s.f.len());
            Some(s)
        }
        Err(e) => {
            println!("    {label:<22} {ms:>8.1} ms  FAILED {e}");
            None
        }
    }
}

fn main() {
    let lib = AlphaLibrary::builtin();
    let bezel = part(Operation::Cylinder { radius_mm: 3.0, height_mm: 2.5 });
    let pilot = part(Operation::Cylinder { radius_mm: 1.2, height_mm: 12.0 });
    for (label, params) in [("preview 256×128", BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() }), ("export 1024×384", BuildParams { theta_steps: 1024, profile_steps: 384, ..BuildParams::default() })] {
        println!("== {label}");
        for t in templates::all().iter().filter(|t| ["Court band", "Heart signet", "Braided band", "Cathedral solitaire stock"].contains(&t.name)) {
            let d = t.design();
            let Ok(built) = mesh::try_build(&d, &lib, params) else { continue };
            let band = Solid {
                v: built.mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(),
                f: built.mesh.faces.clone(),
            };
            let (open, repeated) = band.open_edges();
            println!("  {:<28} {} faces, open {open} repeated {repeated}", t.name, band.f.len());
            // Drop onto the surface at the top of the ring: a radial ray in the finger's plane.
            let theta = 90.0_f64.to_radians();
            let Some((face, hit)) = raycast(&built.mesh, [(40.0 * theta.cos()) as f32, (40.0 * theta.sin()) as f32, 0.0], [(-theta.cos()) as f32, (-theta.sin()) as f32, 0.0]) else {
                println!("    no surface at the top");
                continue;
            };
            let (a, b, c) = built.mesh.triangle(&built.mesh.faces[face]).unwrap();
            let n = cross(sub(b, a), sub(c, a));
            let normal = scale(n, 1.0 / norm(n).max(1e-12));
            // The foot sinks 0.05 mm so the contact is never coplanar with the band.
            let origin = sub(hit.map(f64::from), scale(normal, 0.05));
            let frame = Frame::from_normal(origin, normal, [0.0, 0.0, 1.0]);
            // A kernel cylinder stands on z = −h/2; lift it so its base is the frame's origin.
            let seated = |s: &Solid, half: f64| s.translated([0.0, 0.0, half]).placed(&frame);
            let before = band.volume();
            let joined = report("union bezel", Instant::now(), csg::combine(&band, &seated(&bezel, 1.25), Op::Union), before);
            if let Some(joined) = joined {
                let before = joined.volume();
                report("subtract pilot", Instant::now(), csg::combine(&joined, &seated(&pilot, -6.0), Op::Subtract), before);
            }
        }
    }
}
