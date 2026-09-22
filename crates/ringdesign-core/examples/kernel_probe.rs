//! M1 spike R2: how long each kernel operation takes as its operands grow, so the
//! faceted-operand gate is a measured number. Every case runs in a child thread
//! under a deadline; a case that outruns it is reported as such and left behind.
//!
//!     cargo run --release --example kernel_probe [-- deadline_secs]
use cadkernel::brep::{self, make, Body, Operation};
use std::time::{Duration, Instant};

fn faceted_torus(major: f64, minor: f64, nt: usize, np: usize) -> Body {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for i in 0..nt {
        let a = std::f64::consts::TAU * i as f64 / nt as f64;
        for j in 0..np {
            let p = std::f64::consts::TAU * j as f64 / np as f64;
            let r = major + minor * p.cos();
            vertices.push([r * a.cos(), r * a.sin(), minor * p.sin()]);
        }
    }
    for i in 0..nt {
        for j in 0..np {
            let (ni, nj) = ((i + 1) % nt, (j + 1) % np);
            faces.push(vec![i * np + j, ni * np + j, ni * np + nj]);
            faces.push(vec![i * np + j, ni * np + nj, i * np + nj]);
        }
    }
    make::faceted_solid(&vertices, &faces).expect("faceted torus")
}

fn timed<F: FnOnce() -> String + Send + 'static>(label: &str, deadline: Duration, f: F) {
    let started = Instant::now();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(deadline) {
        Ok(result) => println!("{label:<52} {:>8.1} ms  {result}", started.elapsed().as_secs_f64() * 1e3),
        Err(_) => println!("{label:<52} > {:.0} s  ABANDONED (thread left running)", deadline.as_secs_f64()),
    }
}

fn main() {
    let deadline = Duration::from_secs(std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20));
    // Seated on the tube like an anchored part: the cylinder cuts through the torus at x = 10.
    let cylinder = || make::cylinder([10.0, 0.0, 0.0], 3.0, 3.0).unwrap();
    println!("== booleans: faceted torus (band stand-in) ∪ analytic cylinder through its tube");
    for (nt, np) in [(16, 8), (24, 12), (32, 16), (48, 24), (64, 32), (96, 48), (128, 64)] {
        let band = faceted_torus(10.0, 1.5, nt, np);
        let faces = band.faces.len();
        let tool = cylinder();
        timed(&format!("union faceted {faces} faces + cylinder"), deadline, move || {
            match brep::combine(band, tool, Operation::Union, 1e-6) {
                Ok(b) => format!("ok {} faces", b.faces.len()),
                Err(e) => format!("refused {e:?}"),
            }
        });
    }
    println!("== booleans: analytic pairs");
    let pairs: Vec<(&str, Body, Body)> = vec![
        ("box ∪ box (offset)", make::cuboid([0.0; 3], [4.0; 3]).unwrap(), make::cuboid([2.0; 3], [4.0; 3]).unwrap()),
        ("cylinder − cylinder (coaxial)", make::cylinder([0.0; 3], 3.3, 3.0).unwrap(), make::cylinder([0.0, 0.0, -1.0], 2.8, 5.0).unwrap()),
        ("cylinder ∪ cylinder (crossing axes)", make::cylinder([0.0; 3], 2.0, 8.0).unwrap(), brep::transform(&make::cylinder([0.0, 0.0, -4.0], 1.0, 8.0).unwrap(), &brep::Placement { x_axis: [0.0, 0.0, 1.0], y_axis: [0.0, 1.0, 0.0], z_axis: [-1.0, 0.0, 0.0], origin: [0.0, 0.0, 4.0] }).unwrap()),
        ("sphere ∪ cylinder (offset)", make::sphere([0.0; 3], 3.0).unwrap(), make::cylinder([1.0, 0.0, 0.0], 1.0, 6.0).unwrap()),
        ("torus ∪ cylinder", make::torus([0.0; 3], 10.0, 1.5).unwrap(), make::cylinder([10.0, 0.0, 0.0], 3.0, 3.0).unwrap()),
    ];
    for (label, a, b) in pairs {
        timed(label, deadline, move || match brep::combine(a, b, Operation::Union, 1e-6) {
            Ok(b) => format!("ok {} faces", b.faces.len()),
            Err(e) => format!("refused {e:?}"),
        });
    }
    println!("== other operations on growing bodies");
    for sides in [8usize, 16, 32, 64, 128] {
        let prism = make::pyramid_frustum([0.0; 3], 4.0, 4.0, 3.0, sides).unwrap();
        let faces = prism.faces.len();
        let edges: Vec<_> = prism.edges.iter().map(|(k, _)| k).take(sides).collect();
        timed(&format!("fillet {sides} edges of a {faces}-face prism"), deadline, move || match brep::fillet_edges(&prism, &edges, 0.3) {
            Ok(b) => format!("ok {} faces", b.faces.len()),
            Err(e) => format!("refused {e:?}"),
        });
    }
    for (nt, np) in [(32, 16), (64, 32), (128, 64)] {
        let band = faceted_torus(10.0, 1.5, nt, np);
        let faces = band.faces.len();
        timed(&format!("tessellate faceted {faces} faces"), deadline, move || {
            let m = brep::mesh::tessellate(&band, brep::mesh::TessellationTolerance::new(0.15, 1e-7).with_chordal_deflection(0.04));
            format!("{} triangles", m.mesh.triangles.len())
        });
        let band = faceted_torus(10.0, 1.5, nt, np);
        timed(&format!("slice faceted {faces} faces by a plane"), deadline, move || {
            match brep::slice_by_plane(&band, cadkernel::space::Plane::from_axes([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0])) {
                Ok(Some(s)) => format!("ok {} + {} faces", s.negative.faces.len(), s.positive.faces.len()),
                Ok(None) => "missed".into(),
                Err(e) => format!("refused {e:?}"),
            }
        });
    }
    println!("done; abandoned threads are still running if any were reported");
    std::process::exit(0);
}
