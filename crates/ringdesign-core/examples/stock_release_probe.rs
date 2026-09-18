//! Isolate withdrawal findings in an imported composition.
use anyhow::Result;
use ringdesign_core::{AlphaLibrary, BuildParams, library, manufacturing as mf};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let d = library::load_design(&args[0])?;
    let lib = mf::source_library(&d, &AlphaLibrary::default()).into_owned();
    let setup = d.manufacturing.as_ref().unwrap();
    if args.iter().any(|s| s == "--wall") {
        let built = ringdesign_core::mesh::try_build(
            &d,
            &lib,
            BuildParams {
                theta_steps: 256,
                profile_steps: 160,
                ..d.build
            },
        )?;
        if let Some(i) = args.iter().position(|s| s == "--mesh") {
            ringdesign_core::stl::write_stl(&args[i + 1], &built.mesh, &d.name)?;
        }
        let screen =
            ringdesign_core::cad::measure::thickness(&built.mesh, setup.recipe.min_section_mm);
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"mesh_triangles":built.mesh.faces.len(),"wall":screen,"stone_count":ringdesign_core::stones::stone_frames(&d).len()})
            )?
        );
        return Ok(());
    }

    if args.iter().any(|s| s == "--quality") {
        let built = ringdesign_core::mesh::try_build(&d, &lib, d.build)?;
        let m = &built.mesh;
        fn norm(a: [f64; 3]) -> f64 {
            a.iter().map(|x| x * x).sum::<f64>().sqrt()
        }
        fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
            std::array::from_fn(|i| a[i] - b[i])
        }
        fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        }
        let mut bad = Vec::new();
        for (i, f) in m.faces.iter().enumerate() {
            let (a, b, c) = m.triangle(f).unwrap();
            let edge = norm(sub(a, b)).max(norm(sub(b, c))).max(norm(sub(c, a)));
            let aspect = edge * edge / norm(cross(sub(b, a), sub(c, a))).max(1e-20);
            if aspect > 1000. {
                bad.push((aspect, i, *f, [a, b, c]));
            }
        }
        bad.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!(
            "{} poor triangles; worst: {:#?}",
            bad.len(),
            &bad[..bad.len().min(5)]
        );
        return Ok(());
    }

    let mut variants = vec![("bare".to_owned(), None)];
    if args.iter().any(|a| a == "--each") {
        variants.extend(
            d.layers
                .layers
                .iter()
                .enumerate()
                .filter(|(_, e)| e.enabled && !e.bench_only)
                .map(|(i, e)| (e.name.clone(), Some(i))),
        );
    }
    for (name, index) in variants {
        let mut v = d.clone();
        for (i, e) in v.layers.layers.iter_mut().enumerate() {
            e.enabled = index == Some(i);
        }
        let built = mf::prepare(
            &v,
            &lib,
            setup,
            BuildParams {
                theta_steps: 768,
                profile_steps: 320,
                ..d.build
            },
        )?;
        let release = mf::release::analyze(&built.mesh, setup)?;
        println!(
            "{name}: {} obstructions / {} rays; max {:.3} mm, {:?}",
            release.obstructions.len(),
            release
                .obstructions
                .iter()
                .map(|o| o.samples)
                .sum::<usize>(),
            release
                .obstructions
                .iter()
                .map(|o| o.depth_mm)
                .fold(0_f64, f64::max),
            release.obstructions.first().map(|o| o.world)
        );
    }
    Ok(())
}
