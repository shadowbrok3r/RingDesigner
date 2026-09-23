//! Junction fillets on a torus shank by post radius, foot depth and fillet radius: `cargo run -p ringdesign-occt --features kernel-occt --example junction_probe`.
use ringdesign_occt::kernel::run;
use ringdesign_occt::protocol::{Cylinder, Frame, Request, Response, Tolerance, Torus};

fn main() {
    let (major, minor) = (9.85, 1.2);
    for post in [0.6, 0.8, 1.0] {
        for sink in [0.25, 0.5, 1.0] {
            let height = 2.5;
            let origin = [0.0, major + minor + height / 2.0 - sink, 0.0];
            let cylinder = Cylinder { radius_mm: post, height_mm: height, frame: Frame { origin, x: [0.0, 0.0, -1.0], y: [-1.0, 0.0, 0.0], z: [0.0, 1.0, 0.0] } };
            let mut row = format!("post r {post:.1}, foot {sink:.2} into the tube:");
            for radius in [0.1, 0.2, 0.3, 0.4, 0.6] {
                let request = Request::Junction { torus: Torus { major_mm: major, minor_mm: minor }, cylinder, radius_mm: radius, tolerance: Tolerance::EXPORT };
                row.push_str(&match run(&request) {
                    Response::Done { solids, kernel_ms, notes } => format!(" r {radius}: {} tris {kernel_ms:.0} ms [{}];", solids[0].mesh.triangles, notes[0].split(':').next().unwrap_or("")),
                    Response::Refused { message } => format!(" r {radius}: refused ({});", message.chars().take(60).collect::<String>()),
                });
            }
            println!("{row}");
        }
    }
}
