//! Times the UI-thread and worker stages the desktop runs for one design, headless.
use std::time::Instant;

use ringdesign_core::interaction::pick::{Filter, PickScene, Ray, ViewScale};
use ringdesign_core::interaction::picking;
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::{AlphaLibrary, castability, library};
use ringdesign_workbench::render;
use ringdesign_workbench::viewport::{Selection, tint, tint_in};

fn timed<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let out = f();
    println!("{label:52} {:>9.1} ms", start.elapsed().as_secs_f64() * 1e3);
    out
}

fn mb(floats: usize) -> f64 {
    floats as f64 * 4.0 / 1e6
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("usage: perf_probe <design.ring.json>");
    let mut lib = AlphaLibrary::installed();
    let design = timed("load_design", || library::load_design(&path))?;
    timed("unpack_embedded + bake_all", || {
        design.unpack_embedded(&mut lib);
        design.bake_all(&mut lib);
    });
    let (_, theta, profile) = BuildParams::PRESETS[3];
    let params = BuildParams { theta_steps: theta, profile_steps: profile, ..BuildParams::default() };
    let result = timed("try_build (Export preset)", || mesh::try_build(&design, &lib, params))?;
    let m = &result.mesh;
    println!("  mesh: {} vertices, {} faces, procedural band: {}", m.vertices.len(), m.faces.len(), design.band_is_procedural());
    if !design.band_is_procedural() {
        use ringdesign_core::cad;
        let never = std::sync::atomic::AtomicBool::new(false);
        let cache = std::sync::Mutex::new(cad::Cache::default());
        let stored = design.cad.as_ref().and_then(|d| d.features.iter().find_map(|f| match &f.operation {
            cad::Operation::Stored { mesh, .. } => Some(mesh.clone()),
            _ => None,
        }));
        if let Some(packed) = stored {
            let u = timed("  stored: Packed::decode", || packed.decode())?;
            let solid = ringdesign_core::csg::Solid { v: u.positions, f: u.triangles };
            timed("  stored: Solid::open_edges (closed check)", || solid.open_edges());
            let named = ringdesign_core::setting::Named { solid, patch: u.face_of, names: vec!["Face 0".into()] };
            timed("  stored: builders::creases", || cad::builders::creases(&named, cad::builders::CREASE_DEG).len());
        }
        let evaluate = || cad::evaluate_memo(&design, &lib, params, &cad::BuildCtx::new(&never), cad::Memo::new(&cache));
        let evaluated = timed("  cad::evaluate_memo, cold cache", evaluate)?;
        drop(timed("  cad::evaluate_memo, warm cache", evaluate)?);
        let (fused, _) = timed("  parts::assembled", || ringdesign_core::parts::assembled(evaluated, Some(&never)))?;
        timed("  mesh.validate", || fused.validate());
        timed("  mesh.quality", || fused.quality());
        timed("  mesh.surface_area_mm2", || fused.surface_area_mm2());
    }

    println!("-- worker, per build");
    let scene = timed("PickScene::build", || PickScene::build(&result, &design));
    let metal = timed("render::stage_cad", || render::stage_cad(m));
    println!("  metal VBO: {:.1} MB ({} B/vertex)", mb(metal.len()), render::MESH_FLOATS * 4);
    drop(metal);
    let edges = timed("render::stage_edges", || result.parts.evaluated.as_ref().map(render::stage_edges).unwrap_or_default());
    println!("  edge VBO: {:.1} MB, {} runs", mb(edges.verts.len()), edges.runs.len());
    timed("castability::analyze_at", || castability::analyze_at(m, &design.draft, design.inner_radius_mm(), 0.0));

    println!("-- UI thread, every frame");
    timed("mesh.bounds() (draw_grid, plane_shapes: x2/frame)", || m.bounds());
    if let Some(e) = &result.parts.evaluated {
        timed("component volume_mm3 (CAD panel, per frame)", || e.components.iter().map(|c| c.mesh.volume_mm3()).sum::<f64>());
        if let Some(c) = e.components.first() {
            println!("-- UI thread, every frame while a part is selected (gizmo_of x2/frame)");
            let centre = timed("part mesh.bounds()", || {
                let (lo, hi) = c.mesh.bounds().expect("bounds");
                [(lo.0 + hi.0) as f64 * 0.5, (lo.1 + hi.1) as f64 * 0.5, (lo.2 + hi.2) as f64 * 0.5]
            });
            timed("gizmo::reach", || ringdesign_workbench::gizmo::reach(&c.mesh, centre));
        }
    }

    println!("-- UI thread, per hover sample / hover change");
    let top = m.vertices.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1)).expect("vertices");
    let ray = Ray { origin: [top.0 as f64, top.1 as f64 + 100.0, top.2 as f64], direction: [0.0, -1.0, 0.0] };
    let view = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 12.0 };
    let picks = timed("scene.pick (hover sample, BVH)", || scene.pick(ray, &view, 8.0, Filter::default()));
    println!("  picks: {:?}", picks.iter().map(|p| &p.entity).collect::<Vec<_>>());
    timed("picking::raycast (brute force: click, Measure, Paint)", || {
        picking::raycast(m, ray.origin.map(|v| v as f32), ray.direction.map(|v| v as f32))
    });
    for pick in picks.iter().take(2) {
        let mut sel = Selection::default();
        sel.hovered(vec![pick.clone()]);
        let weights = timed(&format!("tint, hover {:?}", pick.entity), || tint(&sel, &result));
        let scened = timed(&format!("tint_in (scene), hover {:?}", pick.entity), || tint_in(&sel, &result, Some(&scene)));
        println!("  same weights: {}", weights == scened);
        let staged = timed("render::stage_select", || render::stage_select(m, &weights));
        println!("  select VBO: {:.1} MB", mb(staged.len()));
    }

    println!("-- UI thread, every 30 s (eframe auto-save) and per committed edit");
    let json = timed("session_design: clone + embed_alphas + design_json", || {
        let mut d = design.clone();
        d.embed_alphas(&lib);
        library::design_json(&d)
    })?;
    println!("  session design: {:.1} MB", json.len() as f64 / 1e6);
    timed("History::describe (2x serde_json::to_value)", || {
        let a = serde_json::to_value(&design).expect("value");
        let b = serde_json::to_value(&design).expect("value");
        a == b
    });
    Ok(())
}
