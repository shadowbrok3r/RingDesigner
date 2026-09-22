//! M3's foundation measured: a pick scene over each M2 template with a joined bezel and a cut
//! pilot, at preview and export — BVH and scene build time, then a thousand random picks, two
//! hundred nearest-point queries and a box selection, per ring.
//!
//!     cargo run --release --example pick_probe
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    cad::{Attach, Component, Document, Feature, Operation, Placement, Stage},
    interaction::{
        bvh::Bvh,
        pick::{Entity, Filter, PickScene, Ray, ViewScale},
    },
    mesh, templates,
};
use std::time::Instant;

/// A small deterministic generator for the rays.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

fn part(id: u64, name: &str, operation: Operation, attach: Attach, height: f64) -> Feature {
    Feature {
        id,
        name: name.into(),
        enabled: true,
        operation,
        component: Component { attach, stage: Stage::Cast, placement: Placement::ring(90.0, height), ..Default::default() },
    }
}

/// The template with a bezel joined at the top of the ring and a pilot cut through it.
fn dressed(mut d: RingDesign) -> RingDesign {
    let mut doc = Document::default();
    doc.append(Feature { id: 0, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    doc.append(part(1, "bezel", Operation::Cylinder { radius_mm: 3.0, height_mm: 2.5 }, Attach::Join, 1.25)).unwrap();
    doc.append(part(2, "pilot", Operation::Cylinder { radius_mm: 1.2, height_mm: 12.0 }, Attach::Cut, 0.0)).unwrap();
    d.cad = Some(doc);
    d
}

fn main() {
    let lib = AlphaLibrary::builtin();
    let view = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 10.0 };
    for (label, params) in [
        ("preview 256×128", BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() }),
        ("export 1024×384", BuildParams { theta_steps: 1024, profile_steps: 384, ..BuildParams::default() }),
    ] {
        println!("== {label}");
        for t in templates::all().iter().filter(|t| ["Court band", "Heart signet", "Braided band", "Cathedral solitaire stock"].contains(&t.name)) {
            let d = dressed(t.design());
            let started = Instant::now();
            let built = match mesh::try_build(&d, &lib, params) {
                Ok(b) => b,
                Err(e) => {
                    println!("  {:<28} build failed: {e:#}", t.name);
                    continue;
                }
            };
            let build_ms = started.elapsed().as_secs_f64() * 1e3;
            let started = Instant::now();
            let bvh = Bvh::build(&built.mesh);
            let bvh_ms = started.elapsed().as_secs_f64() * 1e3;
            let started = Instant::now();
            let scene = PickScene::build(&built, &d);
            let scene_ms = started.elapsed().as_secs_f64() * 1e3;
            println!(
                "  {:<28} {} faces ({} joined, {} cut, notes {}), {} parts, {} stones: build {build_ms:.0} ms, bvh {bvh_ms:.1} ms, scene {scene_ms:.1} ms",
                t.name,
                built.mesh.faces.len(),
                built.parts.joined,
                built.parts.cut,
                built.parts.notes.len(),
                scene.parts(),
                scene.stones()
            );
            // Rays from a sphere round the ring at random points of its box.
            let (lo, hi) = built.mesh.bounds().unwrap();
            let mut rng = Lcg(9);
            let rays: Vec<Ray> = (0..1000)
                .map(|_| {
                    let (th, ph) = (rng.range(0.0, std::f64::consts::TAU), rng.range(-1.0, 1.0f64).acos());
                    let o = [40.0 * ph.sin() * th.cos(), 40.0 * ph.sin() * th.sin(), 40.0 * ph.cos()];
                    let target = [rng.range(lo.0 as f64, hi.0 as f64), rng.range(lo.1 as f64, hi.1 as f64), rng.range(lo.2 as f64, hi.2 as f64)];
                    Ray { origin: o, direction: std::array::from_fn(|k| target[k] - o[k]) }
                })
                .collect();
            let started = Instant::now();
            let mut classes = [0usize; 6];
            let mut hits = 0;
            for r in &rays {
                let picks = scene.pick(*r, &view, 8.0, Filter::default());
                if let Some(p) = picks.first() {
                    hits += 1;
                    classes[match p.entity {
                        Entity::Vertex { .. } => 0,
                        Entity::Edge { .. } => 1,
                        Entity::Face { .. } => 2,
                        Entity::Part { .. } => 3,
                        Entity::Stone { .. } => 4,
                        Entity::Band => 5,
                    }] += 1;
                }
            }
            let pick_us = started.elapsed().as_secs_f64() * 1e6 / rays.len() as f64;
            let started = Instant::now();
            let mut near = 0;
            for r in rays.iter().take(200) {
                let p: [f64; 3] = std::array::from_fn(|k| r.origin[k] + r.direction[k]);
                near += usize::from(bvh.nearest(&built.mesh, p, 2.0).is_some());
            }
            let nearest_us = started.elapsed().as_secs_f64() * 1e6 / 200.0;
            let started = Instant::now();
            let window = [[1.0, 0.0, 0.0, 3.5], [-1.0, 0.0, 0.0, 3.5], [0.0, 0.0, 1.0, 3.5], [0.0, 0.0, -1.0, 3.5]];
            let selected = scene.box_select(window, true, Filter::default());
            let box_ms = started.elapsed().as_secs_f64() * 1e3;
            println!(
                "    1000 picks {pick_us:.1} µs each ({hits} hit: vertex {} edge {} face {} part {} stone {} band {}), 200 nearest {nearest_us:.1} µs each ({near} within 2 mm), crossing box {box_ms:.1} ms → {} entities",
                classes[0], classes[1], classes[2], classes[3], classes[4], classes[5], selected.len()
            );
            // The top of the bezel, straight down: what the first pick names.
            let picks = scene.pick(Ray { origin: [0.0, 40.0, 0.0], direction: [0.0, -1.0, 0.0] }, &view, 8.0, Filter::default());
            println!("    straight down at the top: {:?}", picks.iter().map(|p| (&p.entity, p.px)).collect::<Vec<_>>());
        }
    }
}
