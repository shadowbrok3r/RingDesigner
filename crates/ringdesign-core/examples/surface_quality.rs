//! Reproduce surface seams independently of viewport lighting.
//! cargo run -p ringdesign-core --example surface_quality -- SOURCE OUTPUT
use ringdesign_core::{AlphaLibrary, BuildParams, library, manufacturing, mesh, render};
use std::{io::Write, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    anyhow::ensure!(args.len() == 2, "surface_quality SOURCE OUTPUT");
    let source = library::load_design(&args[0])?;
    let out = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&out)?;
    let empty = AlphaLibrary::default();
    let lib = manufacturing::source_library(&source, &empty).into_owned();
    let mut mods = std::io::BufWriter::new(std::fs::File::create(out.join("modulation.csv"))?);
    writeln!(mods, "theta,width,crest,r,cap,crown,crown_min,dome,facet")?;
    for i in 0..1800 {
        let theta = i as f64 / 10.0;
        let m = source.modulation_at(
            theta,
            source.inner_radius_mm(),
            source.reference_loop().crest_radius_mm,
        );
        writeln!(
            mods,
            "{theta},{},{},{},{},{},{},{},{}",
            m.width_scale,
            m.crest_span.unwrap_or((0.0, 0.0)).1,
            m.outer_r.unwrap_or(0.0),
            m.outer_max_r.unwrap_or(0.0),
            m.crown_scale,
            m.crown_min_mm.unwrap_or(0.0),
            m.dome_drop,
            m.facet_half
        )?;
    }
    for bare in [true, false] {
        let mut d = source.clone();
        if bare {
            d.layers.layers.clear();
        }
        let name = if bare { "bare" } else { "ornament" };
        let params = BuildParams {
            theta_steps: 720,
            profile_steps: 320,
            ..Default::default()
        };
        let built = mesh::build(&d, &lib, params);
        render::write_png(
            out.join(format!("{name}.png")),
            &built.mesh,
            0.48,
            0.7,
            1100,
            render::GOLD,
        )?;
        let mut csv =
            std::io::BufWriter::new(std::fs::File::create(out.join(format!("{name}.csv")))?);
        writeln!(csv, "theta,row,x,y,z,nx,ny,nz")?;
        for (i, (v, n)) in built
            .mesh
            .vertices
            .iter()
            .zip(&built.mesh.normals)
            .enumerate()
        {
            writeln!(
                csv,
                "{},{},{},{},{},{},{},{}",
                i / 320,
                i % 320,
                v.0,
                v.1,
                v.2,
                n.0,
                n.1,
                n.2
            )?;
        }
        println!("{name}: {:?}", built.mesh.validate());
    }
    Ok(())
}
