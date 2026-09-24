//! M1 spike R3: do the edge and face ordinals a Fillet, Chamfer or Shell stores keep naming the
//! same topology when the source feature's parameters change? Each starter operation is
//! evaluated over a parameter sweep and every edge gets a signature — the surface kinds of its
//! two faces, its curve kind, and its direction and midpoint normalised by the body's extent.
//! An ordinal is stable when the signature at that ordinal survives the sweep.
//!
//!     cargo run --release --example ref_probe
use cadkernel::brep::{Body, Curve3, Surface};
use ringdesign_core::{
    AlphaLibrary, BuildParams,
    cad::{self, Component, Document, Feature, Operation},
    sketch::Sketch,
};

#[derive(Clone, Debug, PartialEq)]
struct Signature {
    faces: [String; 2],
    curve: &'static str,
    direction: [i32; 3],
    midpoint: [i32; 3],
}

fn kind(s: &Surface) -> String {
    match s {
        Surface::Plane(_) => "plane",
        Surface::Cylinder(_) => "cylinder",
        Surface::Cone(_) => "cone",
        Surface::Sphere(_) => "sphere",
        Surface::Torus(_) => "torus",
        Surface::Nurbs(_) => "nurbs",
    }
    .into()
}

fn signatures(body: &Body) -> Vec<Signature> {
    let (lo, hi) = body.vertices.iter().fold(([f64::MAX; 3], [f64::MIN; 3]), |(lo, hi), (_, v)| {
        (std::array::from_fn(|k| lo[k].min(v.point[k])), std::array::from_fn(|k| hi[k].max(v.point[k])))
    });
    let span: [f64; 3] = std::array::from_fn(|k| (hi[k] - lo[k]).max(1e-9));
    body.edges
        .iter()
        .map(|(key, edge)| {
            let mut faces: Vec<String> = edge
                .coedges
                .iter()
                .filter_map(|c| body.coedges.get(*c))
                .filter_map(|c| body.loops.get(c.owner))
                .filter_map(|l| body.faces.get(l.owner))
                .filter_map(|f| body.surfaces.get(f.surface))
                .map(kind)
                .collect();
            faces.sort();
            faces.resize(2, "-".into());
            let curve = match body.curves.get(edge.curve) {
                Some(Curve3::Line(_)) => "line",
                Some(Curve3::Circle(_)) => "circle",
                Some(Curve3::Ellipse(_)) => "ellipse",
                Some(Curve3::PlanarSpline { .. }) => "spline",
                Some(Curve3::Nurbs(_)) => "nurbs",
                None => "?",
            };
            let (a, b) = body.edge_endpoints(key).unwrap_or(([0.0; 3], [0.0; 3]));
            // Quantised to tenths of the extent, so a resize keeps the signature and a swap breaks it.
            let q = |v: f64, k: usize| ((v - lo[k]) / span[k] * 10.0).round() as i32;
            let d: [i32; 3] = std::array::from_fn(|k| ((b[k] - a[k]) / span[k] * 4.0).round() as i32);
            Signature {
                faces: [faces[0].clone(), faces[1].clone()],
                curve,
                direction: d,
                midpoint: std::array::from_fn(|k| q((a[k] + b[k]) * 0.5, k)),
            }
        })
        .collect()
}

fn body_of(op: Operation) -> Body {
    let mut d = ringdesign_core::RingDesign::default();
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "probe".into(), enabled: true, operation: op, component: Component::default() }).unwrap();
    d.cad = Some(doc);
    let e = cad::evaluate(&d, &AlphaLibrary::builtin(), BuildParams::default()).expect("evaluates");
    e.components.into_iter().next().unwrap().body
}

fn sweep(label: &str, variants: Vec<Operation>) {
    let bodies: Vec<Body> = variants.into_iter().map(body_of).collect();
    let base = signatures(&bodies[0]);
    let mut worst = 0usize;
    let mut per = String::new();
    for b in &bodies[1..] {
        let s = signatures(b);
        let faces = base.iter().zip(&s).filter(|(a, b)| a.faces != b.faces || a.curve != b.curve).count();
        let direction = base.iter().zip(&s).filter(|(a, b)| a.faces == b.faces && a.curve == b.curve && a.direction != b.direction).count();
        let moved = faces + direction + base.len().abs_diff(s.len());
        per.push_str(&format!(" [{} edges, {faces} kind, {direction} dir]", s.len()));
        worst = worst.max(moved);
    }
    let verdict = if worst == 0 { "STABLE" } else { "RETARGETS" };
    println!("{label:<40} {:>3} edges  {verdict:<9}{per}", base.len());
}

fn main() {
    sweep("box (resize each axis)", vec![
        Operation::Box { size: [8.0, 6.0, 3.0] },
        Operation::Box { size: [2.0, 6.0, 3.0] },
        Operation::Box { size: [8.0, 20.0, 3.0] },
        Operation::Box { size: [8.0, 6.0, 0.5] },
    ]);
    sweep("cylinder (radius, height)", vec![
        Operation::Cylinder { radius_mm: 4.0, height_mm: 3.0 },
        Operation::Cylinder { radius_mm: 1.0, height_mm: 3.0 },
        Operation::Cylinder { radius_mm: 4.0, height_mm: 12.0 },
    ]);
    sweep("torus (radii)", vec![
        Operation::Torus { major_mm: 10.0, minor_mm: 1.5 },
        Operation::Torus { major_mm: 6.0, minor_mm: 1.5 },
        Operation::Torus { major_mm: 10.0, minor_mm: 3.0 },
    ]);
    sweep("extrude rectangle (height, draft)", vec![
        Operation::Extrude { sketch: Sketch::rectangle(8.0, 6.0).into(), height_mm: 3.0, draft_deg: 0.0 },
        Operation::Extrude { sketch: Sketch::rectangle(8.0, 6.0).into(), height_mm: 9.0, draft_deg: 0.0 },
        Operation::Extrude { sketch: Sketch::rectangle(8.0, 6.0).into(), height_mm: 3.0, draft_deg: 10.0 },
        Operation::Extrude { sketch: Sketch::rectangle(3.0, 12.0).into(), height_mm: 3.0, draft_deg: 0.0 },
    ]);
    sweep("extrude circle (height)", vec![
        Operation::Extrude { sketch: Sketch::circle(3.0).into(), height_mm: 2.0, draft_deg: 0.0 },
        Operation::Extrude { sketch: Sketch::circle(3.0).into(), height_mm: 8.0, draft_deg: 0.0 },
        Operation::Extrude { sketch: Sketch::circle(1.0).into(), height_mm: 2.0, draft_deg: 0.0 },
    ]);
    let section = |w: f64, h: f64, offset: f64| {
        let mut s = Sketch::rectangle(w, h);
        s.plane = ringdesign_core::sketch::Workplane::section();
        for p in &mut s.points {
            p.xy[0] += offset;
        }
        s
    };
    sweep("revolve section (size, radius, angle)", vec![
        Operation::Revolve { sketch: section(2.0, 5.0, 10.0).into(), pivot: [0.0; 3], axis: [0.0, 0.0, 1.0], degrees: 360.0, in_plane: false },
        Operation::Revolve { sketch: section(2.0, 5.0, 14.0).into(), pivot: [0.0; 3], axis: [0.0, 0.0, 1.0], degrees: 360.0, in_plane: false },
        Operation::Revolve { sketch: section(4.0, 2.0, 10.0).into(), pivot: [0.0; 3], axis: [0.0, 0.0, 1.0], degrees: 360.0, in_plane: false },
        Operation::Revolve { sketch: section(2.0, 5.0, 10.0).into(), pivot: [0.0; 3], axis: [0.0, 0.0, 1.0], degrees: 180.0, in_plane: false },
    ]);
    let top = |w: f64, h: f64, z: f64| {
        let mut s = Sketch::rectangle(w, h);
        s.plane.origin[2] = z;
        s
    };
    sweep("loft two rectangles (top size, height)", vec![
        Operation::Loft { sections: vec![Sketch::rectangle(10.0.into(), 8.0).into(), top(8.0.into(), 6.0.into(), 5.0).into()] },
        Operation::Loft { sections: vec![Sketch::rectangle(10.0.into(), 8.0).into(), top(4.0.into(), 3.0.into(), 5.0).into()] },
        Operation::Loft { sections: vec![Sketch::rectangle(10.0.into(), 8.0).into(), top(8.0.into(), 6.0.into(), 12.0).into()] },
    ]);
    sweep("sweep circle on a 3-station path", vec![
        Operation::Sweep { sketch: Sketch::circle(1.0).into(), path: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 5.0], [2.0, 0.0, 8.0]] },
        Operation::Sweep { sketch: Sketch::circle(0.5).into(), path: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 5.0], [2.0, 0.0, 8.0]] },
        Operation::Sweep { sketch: Sketch::circle(1.0).into(), path: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 9.0], [4.0, 0.0, 12.0]] },
    ]);
    // A boolean's edge list depends on where the cut lands: the case a stored ordinal fears most.
    let boolean = |offset: f64| {
        let mut d = ringdesign_core::RingDesign::default();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "a".into(), enabled: true, operation: Operation::Box { size: [8.0, 6.0, 3.0] }, component: Component::default() }).unwrap();
        doc.append(Feature { id: 2, name: "b".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 6.0 }, component: Component::default() }).unwrap();
        doc.append(Feature { id: 3, name: "t".into(), enabled: true, operation: Operation::Transform { source: 2, translation: [offset, 0.0, 0.0], rotation_deg: [0.0; 3] }, component: Component::default() }).unwrap();
        doc.append(Feature { id: 4, name: "cut".into(), enabled: true, operation: Operation::Boolean { a: 1, b: 3, kind: cad::Boolean::Subtract }, component: Component::default() }).unwrap();
        d.cad = Some(doc);
        match cad::evaluate(&d, &AlphaLibrary::builtin(), BuildParams::default()) {
            Ok(e) => Some(e.components.into_iter().next().unwrap().body),
            Err(e) => {
                println!("box − cylinder at x={offset}: {e:#}");
                None
            }
        }
    };
    let bodies: Vec<Body> = [0.0, 1.0, 2.0, 3.0, 4.5].into_iter().filter_map(boolean).collect();
    if bodies.len() < 2 {
        println!("{:<40} no two evaluable variants", "box − moved cylinder");
        return;
    }
    let base = signatures(&bodies[0]);
    let mut worst = 0;
    for b in &bodies[1..] {
        let s = signatures(b);
        worst = worst.max(base.iter().zip(&s).filter(|(a, b)| a.faces != b.faces || a.curve != b.curve || a.direction != b.direction).count() + base.len().abs_diff(s.len()));
    }
    println!("{:<40} {:>3} edges  {}", "box − moved cylinder", base.len(), if worst == 0 { "STABLE" } else { "RETARGETS" });
}
