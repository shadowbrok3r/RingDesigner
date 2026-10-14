//! The trident cuff as a native design — a bangle's bore, the crest shank and an opening — against cuff_base's swept-and-cut part.
//! cargo run --release -p ringdesign-core --example cuff_native -- [OUT_DIR] [--reference trident-cuff-base-open.stl] [--theta N] [--rows N]
use anyhow::{Context, Result, bail, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, Mesh, RingDesign, castability, castability::CastProcess, csg, library, mesh, opening,
    profile::{CrestShank, TOP_DEG},
    render, stl,
};
use std::path::{Path, PathBuf};

/// Volume of cuff_base's open variant, swept at 1080 x 320 and cut by two radial boxes, mm³.
const EXAMPLE_OPEN_MM3: f64 = 24_901.0;
/// Volume of cuff_base's closed variant, back strap and all, mm³.
const EXAMPLE_CLOSED_MM3: f64 = 27_382.0;
/// Angles from the crest's tip the sections are compared at, degrees.
const STATIONS: [f64; 10] = [0.0, 5.0, 10.0, 20.0, 30.0, 45.0, 60.0, 90.0, 120.0, 150.0];

/// A section's extent: width along the finger, radial thickness, bore radius.
#[derive(Clone, Copy, Debug)]
struct Extent {
    width: f64,
    thick: f64,
    bore_r: f64,
}

/// What cuff_base sweeps at `phi` from the tip, read off its own formulas: the band, widened and thickened by the crest.
fn formula(d: &RingDesign, phi: f64) -> Extent {
    let c = d.shank.crest.unwrap_or_default();
    let share = c.share(phi);
    Extent {
        width: d.profile.width_mm + (c.width_mm - d.profile.width_mm) * share,
        thick: d.profile.thickness_mm + c.rise_mm * share,
        bore_r: d.inner_radius_mm(),
    }
}

/// The native design's own section at `phi` from the tip.
fn native(d: &RingDesign, lib: &AlphaLibrary, phi: f64, rows: usize) -> Extent {
    let s = castability::section_at(d, lib, TOP_DEG + phi, rows);
    Extent { width: s.max_z - s.min_z, thick: s.max_r - s.min_r, bore_r: s.min_r }
}

/// The vertices of a reference mesh lying on the radial half-plane at `theta_deg`, as an extent.
fn sliced(m: &Mesh, theta_deg: f64) -> Option<Extent> {
    let (mut z, mut r) = ((f64::MAX, f64::MIN), (f64::MAX, f64::MIN));
    for v in &m.vertices {
        let (x, y) = (f64::from(v.0), f64::from(v.1));
        let off = (y.atan2(x).to_degrees() - theta_deg + 180.0).rem_euclid(360.0) - 180.0;
        if off.abs() > 0.01 {
            continue;
        }
        let rr = x.hypot(y);
        z = (z.0.min(f64::from(v.2)), z.1.max(f64::from(v.2)));
        r = (r.0.min(rr), r.1.max(rr));
    }
    (z.0 < z.1).then_some(Extent { width: z.1 - z.0, thick: r.1 - r.0, bore_r: r.0 })
}

/// A binary STL's triangles, welded on exact vertex bits.
fn read_stl(path: &Path) -> Result<Mesh> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    ensure!(bytes.len() >= 84, "{} is too short for a binary STL", path.display());
    let count = u32::from_le_bytes(bytes[80..84].try_into()?) as usize;
    ensure!(bytes.len() >= 84 + 50 * count, "{} holds fewer than its {count} triangles", path.display());
    let mut index = std::collections::HashMap::new();
    let mut m = Mesh::default();
    for t in 0..count {
        let at = 84 + 50 * t + 12;
        let mut face = [0u32; 3];
        for (k, slot) in face.iter_mut().enumerate() {
            let f = |o: usize| f32::from_le_bytes(bytes[at + 12 * k + o..at + 12 * k + o + 4].try_into().unwrap());
            let p = mesh::Vec3(f(0), f(4), f(8));
            *slot = *index.entry((p.0.to_bits(), p.1.to_bits(), p.2.to_bits())).or_insert_with(|| {
                m.vertices.push(p);
                (m.vertices.len() - 1) as u32
            });
        }
        m.faces.push(face);
    }
    Ok(m)
}

fn crossings(m: &Mesh) -> usize {
    let v = m.vertices.iter().map(|p| [f64::from(p.0), f64::from(p.1), f64::from(p.2)]).collect();
    csg::self_crossings(&csg::Solid { v, f: m.faces.clone() })
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut out = PathBuf::from("target/cuff-native");
    let mut reference: Option<PathBuf> = None;
    let (mut n_theta, mut n_prof) = (1080, 320);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--reference" => reference = Some(args.next().context("--reference needs an STL")?.into()),
            "--theta" => n_theta = args.next().context("--theta needs a count")?.parse()?,
            "--rows" => n_prof = args.next().context("--rows needs a count")?.parse()?,
            flag if flag.starts_with("--") => bail!("unknown option {flag}"),
            other => out = PathBuf::from(other),
        }
    }
    std::fs::create_dir_all(&out)?;
    let mut d = opening::measured_cuff();
    d.name = "Trident cuff, native".into();
    d.draft.process = CastProcess::LostWax;
    let crest = d.shank.crest.unwrap_or_default();
    ensure!(crest == CrestShank::default(), "the native cuff carries the crest's defaults");
    let lib = AlphaLibrary::builtin();
    let arc = d.sweep_arc();
    println!(
        "{}: bore {:.2} ({}), band {:.2} x {:.2}, crest {:.2} wide and {:.2} proud, gap {:.1} mm: the band runs {:.3}° from {:.3}°",
        d.name,
        d.size.inner_diameter_mm(),
        d.size.display(),
        d.profile.width_mm,
        d.profile.thickness_mm,
        crest.width_mm,
        crest.rise_mm,
        d.opening.map(|o| o.gap_mm).unwrap_or_default(),
        arc.span_deg,
        arc.start_deg
    );

    let params = BuildParams { theta_steps: n_theta, profile_steps: n_prof, ..BuildParams::default() };
    let built = mesh::try_build(&d, &lib, params)?;
    let v = built.mesh.validate();
    let crossed = crossings(&built.mesh);
    println!(
        "\nbuilt {} x {}: {} triangles, watertight {}, boundary {}, non-manifold {}, self-crossing face pairs {crossed}",
        n_theta, n_prof, v.triangle_count, v.watertight, v.boundary_edges, v.non_manifold_edges
    );
    ensure!(v.watertight && crossed == 0, "the native cuff is not a closed, uncrossed solid");

    let theirs = reference.as_deref().map(read_stl).transpose()?;
    println!("\nfrom the tip   formula w / t / bore r     native w / t / bore r      native - formula{}", if theirs.is_some() { "    native - cuff_base" } else { "" });
    let mut worst: (f64, f64) = (0.0, 0.0);
    for phi in STATIONS {
        let (f, n) = (formula(&d, phi), native(&d, &lib, phi, n_prof));
        let dn = (n.width - f.width).abs().max((n.thick - f.thick).abs()).max((n.bore_r - f.bore_r).abs());
        worst.0 = worst.0.max(dn);
        let mut line = format!(
            "  {phi:6.1}°     {:6.3} / {:5.3} / {:6.3}    {:6.3} / {:5.3} / {:6.3}    {dn:.4} mm",
            f.width, f.thick, f.bore_r, n.width, n.thick, n.bore_r
        );
        if let Some(m) = &theirs {
            let r = sliced(m, TOP_DEG + phi).with_context(|| format!("cuff_base has no section at {phi}°"))?;
            let dr = (n.width - r.width).abs().max((n.thick - r.thick).abs()).max((n.bore_r - r.bore_r).abs());
            worst.1 = worst.1.max(dr);
            line.push_str(&format!("            {dr:.4} mm"));
        }
        println!("{line}");
    }
    println!("worst section difference: {:.4} mm against the formula{}", worst.0, if theirs.is_some() { format!(", {:.4} mm against cuff_base", worst.1) } else { String::new() });

    let volume = built.report.volume_mm3;
    let gap = (volume - EXAMPLE_OPEN_MM3) / EXAMPLE_OPEN_MM3;
    println!("\nvolume {volume:.0} mm³ against cuff_base's open {EXAMPLE_OPEN_MM3:.0} ({:+.3}%) and closed {EXAMPLE_CLOSED_MM3:.0}", gap * 100.0);
    if let Some(m) = &theirs {
        println!("cuff_base's open STL: {:.0} mm³", m.volume_mm3());
    }
    let b = built.report.bounds_mm;
    println!("bounds {:.2} x {:.2} x {:.2} mm, measured bore {:.3} mm", b[0], b[1], b[2], built.report.inner_diameter_mm);

    let field = castability::analyze_field(&d, &lib, &d.draft, 720, 192);
    println!(
        "\nfield verdict {:?} under {:?}: undercut {:.4}% (worst {:.2}°), vertical {:.1} mm² of {:.1} mm², thinnest wall {:.3} mm",
        field.verdict,
        field.process,
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.vertical_area_mm2,
        field.total_area_mm2,
        field.thinnest_wall_mm
    );
    for n in &field.notes {
        println!("  {n}");
    }
    ensure!(field.undercut_area_mm2 == 0.0, "the cuff undercuts");

    stl::write_stl(out.join("trident-cuff-native.stl"), &built.mesh, &d.name)?;
    let path = out.join("trident-cuff-native.ring.json");
    library::save_design(&path, &d)?;
    let again = mesh::try_build(&library::load_design(&path)?, &lib, params)?;
    ensure!(again.mesh.vertices == built.mesh.vertices && again.mesh.faces == built.mesh.faces, "{} does not rebuild to its own mesh", path.display());
    render::write_png_parts(out.join("trident-cuff-native-hero.png"), &[render::Part::metal(&built.mesh, render::GOLD)], 0.55, 1.05, 900)?;
    println!("\nwritten to {}", out.display());
    Ok(())
}
