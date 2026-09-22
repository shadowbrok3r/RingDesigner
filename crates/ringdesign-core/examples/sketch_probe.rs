//! M8's sketch core measured: the edit operations on a 10 × 6 rectangle and a crossed line, a
//! washer and two squares swept as regions, a 2 mm post on the top face of a box seated on the
//! ring as the box grows, and timings (best of five) for regions and offsets on large sketches.
//!
//!     cargo run --release --example sketch_probe
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    cad::{self, Component, Document, FaceRef, Feature, Operation, Placement, Profile},
    sketch::{FaceAnchor, FaceFrame, Geometry, Id, Pattern, Sketch, Workplane, region},
};
use cadkernel::brep::{self, Body};
use std::{f64::consts::PI, time::Instant};

const RUNS: usize = 5;

/// Best wall time of `RUNS` calls, in milliseconds, with the last result.
fn best<T>(mut f: impl FnMut() -> T) -> (f64, T) {
    let mut fastest = f64::INFINITY;
    let mut out = None;
    for _ in 0..RUNS {
        let t = Instant::now();
        let r = f();
        fastest = fastest.min(t.elapsed().as_secs_f64() * 1e3);
        out = Some(r);
    }
    (fastest, out.unwrap())
}
fn length(s: &Sketch, id: Id) -> f64 {
    let e = s.entities.iter().find(|e| e.id == id).unwrap();
    s.curves_of(e).unwrap().iter().map(|c| c.length()).sum()
}
fn lines(s: &Sketch) -> Vec<f64> {
    let mut out: Vec<f64> = s
        .entities
        .iter()
        .flat_map(|e| s.curves_of(e).unwrap())
        .filter(|c| matches!(c, cadkernel::geom2d::Curve::Line(_)))
        .map(|c| c.length())
        .collect();
    out.sort_by(f64::total_cmp);
    out
}
fn feature(id: Id, operation: Operation) -> Feature {
    Feature { id, name: operation.label().into(), enabled: true, operation, component: Component::default() }
}
fn design(features: Vec<Feature>, outputs: Vec<Id>) -> RingDesign {
    let mut doc = Document::default();
    for f in features {
        doc.append(f).unwrap();
    }
    doc.outputs = outputs;
    RingDesign { cad: Some(doc), ..RingDesign::default() }
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
/// The frame of the planar face whose outward normal lies along `dir`.
fn face_along(body: &Body, dir: [f64; 3]) -> (usize, FaceFrame) {
    body.faces
        .iter()
        .enumerate()
        .find_map(|(i, (k, _))| {
            let p = brep::planar_face_profile(body, k)?;
            (dot(p.outward, dir) > 0.99).then(|| (i, FaceFrame::of(&p).unwrap()))
        })
        .unwrap()
}
/// A regular `n`-gon of radius `r` about `c` as one closed polyline.
fn gon(s: &mut Sketch, c: [f64; 2], r: f64, n: usize) -> Id {
    let points = (0..n)
        .map(|k| {
            let a = k as f64 / n as f64 * std::f64::consts::TAU;
            s.point([c[0] + r * a.cos(), c[1] + r * a.sin()])
        })
        .collect();
    s.entity(Geometry::Polyline { points, closed: true })
}

fn main() {
    let lib = AlphaLibrary::builtin();
    let export = BuildParams { theta_steps: 1024, profile_steps: 384, ..BuildParams::default() };

    println!("== edits on a 10 x 6 rectangle");
    let mut s = Sketch::rectangle(10.0, 6.0);
    let rect = s.entities[0].id;
    let grown = s.offset(&[rect], 1.0).unwrap()[0];
    let e = s.entities.iter().find(|e| e.id == grown).unwrap();
    let curves = s.curves_of(e).unwrap();
    let area = region::loop_moments(&curves, [0.0; 2]).unwrap()[0];
    let (lo, hi) = curves.iter().map(|c| c.point_at(0.0)).fold(([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]), |(lo, hi), p| {
        ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])
    });
    println!("  offset +1: {:.6} x {:.6}, area {area:.6} mm² (96)", hi[0] - lo[0], hi[1] - lo[1]);
    let mut inside = Sketch::rectangle(10.0, 6.0);
    println!("  offset -3: {}", inside.offset(&[rect], -3.0).err().unwrap());
    let mut f = Sketch::rectangle(10.0, 6.0);
    let corner = f.points[1].id;
    let arc = f.fillet_corner(corner, 1.0).unwrap();
    println!("  fillet r1: lines {:?}, arc {:.9} mm (pi/2 = {:.9})", lines(&f), length(&f, arc), PI / 2.0);
    let mut c = Sketch::rectangle(10.0, 6.0);
    let cut = c.chamfer_corner(c.points[1].id, 1.0).unwrap();
    println!("  chamfer 1: lines {:?}, cut {:.9} mm", lines(&c), length(&c, cut));

    println!("\n== trim the middle of a line crossed twice");
    let mut t = Sketch::default();
    let p = [[0.0, 0.0], [10.0, 0.0], [3.0, -2.0], [3.0, 2.0], [7.0, -2.0], [7.0, 2.0]].map(|p| t.point(p));
    let long = t.entity(Geometry::Line { a: p[0], b: p[1] });
    t.entity(Geometry::Line { a: p[2], b: p[3] });
    t.entity(Geometry::Line { a: p[4], b: p[5] });
    let left = t.trim(long, [5.0, 0.0]).unwrap();
    println!("  {} lines left of it: {:?}", left.len(), left.iter().map(|id| length(&t, *id)).collect::<Vec<_>>());

    println!("\n== regions swept (export chord)");
    let mut washer = Sketch::default();
    for r in [3.0, 2.0] {
        let centre = washer.point([0.0; 2]);
        let rim = washer.point([r, 0.0]);
        washer.entity(Geometry::Circle { center: centre, rim });
    }
    let expected = PI * (9.0 - 4.0) * 2.0;
    let d = design(vec![feature(1, Operation::Extrude { sketch: washer.clone().into(), height_mm: 2.0, draft_deg: 0.0 })], vec![1]);
    let (ms, e) = best(|| cad::evaluate(&d, &lib, export).unwrap());
    let w = &e.components[0];
    println!(
        "  washer 6/4 x 2: {:.4} mm³ against {expected:.4} ({:+.3}%), watertight {}, {} lump, {ms:.1} ms",
        w.mesh.volume_mm3(),
        (w.mesh.volume_mm3() / expected - 1.0) * 100.0,
        w.mesh.validate().watertight,
        w.body.roots.len()
    );
    let mut two = Sketch::default();
    for x in [0.0, 5.0] {
        let p = [[x, 0.0], [x + 2.0, 0.0], [x + 2.0, 2.0], [x, 2.0]].map(|p| two.point(p));
        two.entity(Geometry::Polyline { points: p.to_vec(), closed: true });
    }
    let d = design(
        vec![feature(1, Operation::Sketch { sketch: two }), feature(2, Operation::Extrude { sketch: Profile::Feature { feature: 1 }, height_mm: 1.5, draft_deg: 0.0 })],
        vec![2],
    );
    let e = cad::evaluate(&d, &lib, export).unwrap();
    let b = &e.components[0];
    println!("  two 2 mm squares x 1.5: {} lumps, {:.6} mm³ (12), watertight {}", b.body.roots.len(), b.mesh.volume_mm3(), b.mesh.validate().watertight);
    let mut section = Sketch { plane: Workplane::section(), ..Sketch::default() };
    for (lo, hi) in [([2.0, 0.0], [5.0, 4.0]), ([3.0, 1.0], [4.0, 3.0])] {
        let p = [lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]].map(|p| section.point(p));
        section.entity(Geometry::Polyline { points: p.to_vec(), closed: true });
    }
    for degrees in [360.0, 180.0] {
        let op = Operation::Revolve { sketch: section.clone().into(), pivot: [0.0; 3], axis: [0.0, 0.0, 1.0], degrees };
        let d = design(vec![feature(1, op)], vec![1]);
        let (ms, e) = best(|| cad::evaluate(&d, &lib, export).unwrap());
        let r = &e.components[0];
        let want = 70.0 * PI * degrees / 360.0;
        println!(
            "  square-with-hole revolved {degrees}°: {:.4} mm³ against {want:.4} ({:+.3}%), watertight {}, {ms:.1} ms",
            r.mesh.volume_mm3(),
            (r.mesh.volume_mm3() / want - 1.0) * 100.0,
            r.mesh.validate().watertight
        );
    }

    println!("\n== a 2 mm circle sketched on the top face of an 8 x 6 x h box seated at 90°, extruded 1");
    let seat = Placement::ring(90.0, 0.0);
    let boxed = |h: f64| Feature { component: Component { placement: seat.clone(), ..Component::default() }, ..feature(1, Operation::Box { size: [8.0, 6.0, h] }) };
    let first = cad::evaluate(&design(vec![boxed(2.0)], vec![1]), &lib, export).unwrap();
    let frame = seat.frame(&design(vec![boxed(2.0)], vec![1])).unwrap();
    let block = &first.components[0].body;
    let (top, _) = face_along(block, frame.z_axis);
    let mut circle = Sketch::circle(1.0);
    circle.plane.on_face = Some(FaceAnchor { feature: 1, face: FaceRef::signed(block, top, &frame) });
    let mut base0 = None;
    for h in [2.0, 3.0, 4.0] {
        let d = design(
            vec![
                boxed(h),
                feature(2, Operation::Sketch { sketch: circle.clone() }),
                feature(3, Operation::Extrude { sketch: Profile::Feature { feature: 2 }, height_mm: 1.0, draft_deg: 0.0 }),
            ],
            vec![1, 3],
        );
        let e = cad::evaluate(&d, &lib, export).unwrap();
        let (_, face) = face_along(&e.components[0].body, frame.z_axis);
        let (_, base) = face_along(&e.components[1].body, frame.z_axis.map(|v| -v));
        let off: f64 = base.origin.iter().zip(face.origin).map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt();
        let rise = base0.map_or(0.0, |b: [f64; 3]| dot(std::array::from_fn(|k| base.origin[k] - b[k]), frame.z_axis));
        base0.get_or_insert(base.origin);
        println!(
            "  h {h}: base centre {:.2e} mm off the face centroid, risen {rise:.9} mm, post {:.4} mm³ (pi = {:.4})",
            off,
            e.components[1].mesh.volume_mm3(),
            PI
        );
    }

    println!("\n== timings, best of {RUNS}");
    let mut grid = Sketch::default();
    for i in 0..5 {
        for j in 0..5 {
            let (x, y) = (i as f64 * 10.0, j as f64 * 10.0);
            for (lo, side) in [([x, y], 8.0), ([x + 2.0, y + 2.0], 4.0)] {
                let p = [lo, [lo[0] + side, lo[1]], [lo[0] + side, lo[1] + side], [lo[0], lo[1] + side]].map(|p| grid.point(p));
                grid.entity(Geometry::Polyline { points: p.to_vec(), closed: true });
            }
        }
    }
    let (ms, r) = best(|| grid.profile_regions().unwrap());
    println!("  profile_regions, 50 squares (200 segments) nesting into {} regions: {ms:.3} ms", r.len());
    let mut round = Sketch::default();
    gon(&mut round, [0.0; 2], 10.0, 200);
    let (ms, r) = best(|| round.profile_regions().unwrap());
    println!("  profile_regions, one 200-gon: {} region, {ms:.3} ms", r.len());
    let mut hundred = Sketch::default();
    let loop100 = gon(&mut hundred, [0.0; 2], 10.0, 100);
    let (ms, made) = best(|| hundred.clone().offset(&[loop100], 0.5).unwrap());
    println!("  offset +0.5 of a 100-gon (100 lines): {} polyline, {ms:.3} ms", made.len());
    let mut rounded = Sketch::default();
    let fifty = gon(&mut rounded, [0.0; 2], 10.0, 50);
    let corners: Vec<Id> = match &rounded.entities[0].geometry {
        Geometry::Polyline { points, .. } => points.clone(),
        _ => unreachable!(),
    };
    let t = Instant::now();
    for p in &corners {
        rounded.fillet_corner(*p, 0.2).unwrap();
    }
    let filleting = t.elapsed().as_secs_f64() * 1e3;
    let loop_ids = rounded.loop_through(fifty).unwrap();
    let (ms, made) = best(|| rounded.clone().offset(&loop_ids, 0.5).unwrap());
    println!(
        "  50 corner fillets on a 50-gon: {filleting:.3} ms; offset +0.5 of the {}-segment loop (lines and arcs): {} entities, {ms:.3} ms",
        loop_ids.len(),
        made.len()
    );
    let mut cross = Sketch::default();
    for k in 0..10 {
        let y = k as f64 * 2.0;
        let a = cross.point([-1.0, y]);
        let b = cross.point([21.0, y]);
        cross.entity(Geometry::Line { a, b });
        let a = cross.point([y, -1.0]);
        let b = cross.point([y, 21.0]);
        cross.entity(Geometry::Line { a, b });
    }
    let (ms, added) = best(|| cross.clone().split_at_intersections().unwrap());
    println!("  split a 10 x 10 line grid at its 100 crossings: {} pieces added, {ms:.3} ms", added.len());
    let mut spokes = Sketch::default();
    let hub = spokes.point([0.0; 2]);
    let tip = spokes.point([5.0, 0.0]);
    let spoke = spokes.entity(Geometry::Line { a: hub, b: tip });
    let (ms, copies) = best(|| spokes.clone().pattern(&[spoke], &Pattern::Polar { centre: [0.0; 2], count: 64, sweep_deg: 360.0 }).unwrap());
    println!("  polar pattern of a spoke, 64 round: {} copies, {ms:.3} ms", copies.len());

    println!("\n== sketch.rs's solver on the problems the kernel's planegcs port was measured on");
    let mut rect = Sketch::rectangle(8.0, 6.0);
    rect.points[1].xy = [5.0, -2.0];
    let (ms, solved) = best(|| rect.solve().unwrap());
    println!("  rectangle 8 x 6: residual {:.2e} mm, {} iterations, {ms:.3} ms", solved.residual_mm, solved.iterations);
    for n in [16, 64, 128, 256] {
        let s = staircase(n);
        let (ms, solved) = best(|| s.solve());
        match solved {
            Ok(solved) => println!("  staircase of {n} points: residual {:.2e} mm, {} iterations, {ms:.3} ms", solved.residual_mm, solved.iterations),
            Err(e) => println!("  staircase of {n} points: {e}"),
        }
    }
    rect.constraints.push(ringdesign_core::sketch::Constraint::Distance { a: rect.points[0].id, b: rect.points[2].id, mm: 100.0 });
    println!("  conflicting rectangle: {}", rect.solve().err().unwrap());
}

/// Deterministic noise in [-0.2, 0.2], the sequence the planegcs measurement used.
fn noise(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 0.4
}

/// `n` points, 1 mm treads and risers alternating, the first fixed and the rest knocked by up to 0.2 mm.
fn staircase(n: usize) -> Sketch {
    use ringdesign_core::sketch::Constraint;
    let mut s = Sketch::default();
    let mut seed = 7u64;
    let mut exact = [0.0, 0.0];
    let mut prev = s.point(exact);
    s.points[0].fixed = true;
    for i in 0..n - 1 {
        exact[i % 2] += 1.0;
        let next = s.point([exact[0] + noise(&mut seed), exact[1] + noise(&mut seed)]);
        s.constraints.push(if i % 2 == 0 { Constraint::Horizontal(prev, next) } else { Constraint::Vertical(prev, next) });
        s.constraints.push(Constraint::Distance { a: prev, b: next, mm: 1.0 });
        prev = next;
    }
    s
}
