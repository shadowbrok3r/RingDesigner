//! A CrossGems signet preset rebuilt with the lofted head, for a
//! side-by-side with the ring mesh the preset caches.
//!
//! cg_signet <decoded preset dir> <out prefix> [--loft 0..1] [--size 7] [--builtin cushion|round|oval] [--symmetric xy|x|y]
//!
//! Reads the params.json and curves.json that tools/harvest/cgpreset.py dump-all writes,
//! builds the same ring (band = table length across, shank from the side
//! width, thickness = side thickness + 0.25 as the presets cast it), writes
//! <prefix>.png, <prefix>.obj and <prefix>.ring.json, and prints crest
//! radius and width per angle off the head in the frame measure_cg.py
//! reports the cached mesh in.
use ringdesign_core::{
    build, castability,
    profile::{SIGNET_MIN_SHANK_FRAC, TOP_DEG},
    render, AlphaLibrary, BuildParams, CustomOutline, ProfileStyle, RingDesign, RingSize,
};

fn num(v: &serde_json::Value, key: &str, default: f64) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(default)
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: cg_signet <preset dir> <out prefix> [--loft F] [--size S]");
        std::process::exit(2);
    }
    let dir = std::path::Path::new(&args[1]);
    let prefix = &args[2];
    let (mut loft, mut size) = (1.0f64, 7.0f64);
    let mut builtin: Option<String> = None;
    let mut symmetric: Option<String> = None;
    let mut i = 3;
    while i + 1 < args.len() {
        match args[i].as_str() {
            "--loft" => loft = args[i + 1].parse()?,
            "--size" => size = args[i + 1].parse()?,
            "--builtin" => builtin = Some(args[i + 1].clone()),
            // `xy`, `x` or `y`: fold the imported plan symmetric about the
            // band axis (x) and/or the ring axis (y).
            "--symmetric" => symmetric = Some(args[i + 1].clone()),
            _ => {}
        }
        i += 2;
    }
    let params: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("params.json"))?)?;
    let curves: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("curves.json"))?)?;
    let pts: Vec<[f64; 2]> = curves["Table Custom Profile"]["points"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("no table curve in curves.json"))?
        .iter()
        .map(|p| [p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0)])
        .collect();
    // CrossGems: Width is along the ring, Length across the band.
    let table_w = num(&params, "Table Width", 12.0);
    let table_l = num(&params, "Table Length", 12.0);
    let height = num(&params, "Table Height", 3.0);
    let side_w = num(&params, "Side Width", 6.0);
    let side_t = num(&params, "Side Thickness", 1.5) + 0.25;
    let frontal = num(&params, "Frontal Distance", 2.0);
    let lateral = num(&params, "Lateral Distance", 2.0);

    let mut d = RingDesign::default();
    d.name = format!(
        "CG signet {}",
        dir.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
    );
    d.size = RingSize::new(size);
    // The presets' section is the top of a tall ellipse: crown 0.745 of
    // (thickness + 0.5) on a band (thickness + 0.25) thick, as they cast it.
    d.profile.apply_style(ProfileStyle::HalfRound);
    d.profile.width_mm = table_l;
    d.profile.thickness_mm = side_t;
    d.profile.crown_mm = (0.745 * (side_t + 0.25)).min(side_t - 0.25);
    d.profile.edge_round_mm = 0.05;
    d.shank.apply_signet(table_l);
    let shank_frac = (side_w / table_l).clamp(SIGNET_MIN_SHANK_FRAC, 1.0);
    d.shank.amount = ((1.0 - shank_frac) / (1.0 - SIGNET_MIN_SHANK_FRAC)).clamp(0.0, 1.0);
    let mut outline = CustomOutline::from_points("CG table", &pts)
        .ok_or_else(|| anyhow::anyhow!("the table curve does not make an outline"))?;
    if let Some(s) = &symmetric {
        outline.symmetrize(s.contains('y'), s.contains('x'));
    }
    let o = match builtin.as_deref() {
        Some("cushion") => ringdesign_core::field::SignetOutline::Cushion,
        Some("round") => ringdesign_core::field::SignetOutline::Round,
        Some("oval") => ringdesign_core::field::SignetOutline::Oval,
        _ => d.shank.adopt_outline(outline),
    };
    d.shank.head.outline = o;
    d.shank.head.length_mm = table_w;
    d.shank.head.rise_mm = (height - side_t).max(0.0);
    d.shank.head.rim_round_mm = 0.3;
    d.shank.head.dome = 0.0;
    d.shank.head.loft = loft;
    d.shank.head.loft_frontal_mm = frontal;
    d.shank.head.loft_lateral_mm = lateral;

    let lib = AlphaLibrary::default();
    let f = castability::analyze_field(&d, &lib, &d.draft, 256, 128);
    println!(
        "{}: {:.4}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm at {:.0} deg",
        f.verdict.label(),
        f.undercut_fraction() * 100.0,
        f.worst_draft_deg,
        f.thinnest_wall_mm,
        f.thinnest_wall_theta_deg
    );
    let r = build(&d, &lib, BuildParams { theta_steps: 512, profile_steps: 192, ..BuildParams::default() });
    render::write_png(format!("{prefix}.png"), &r.mesh, 0.55, 1.05, 480, render::GOLD)?;
    ringdesign_core::stl::write_obj(format!("{prefix}.obj"), &r.mesh, &d.name)?;
    std::fs::write(format!("{prefix}.ring.json"), serde_json::to_string_pretty(&d)?)?;

    // Crest radius and half-width per angle off the head, 5 degree buckets.
    let head = d.shank.head.theta_deg;
    let _ = TOP_DEG;
    let mut rows = vec![(f64::MIN, 0.0f64); 37];
    for v in &r.mesh.vertices {
        let (x, y, z) = (v.0 as f64, v.1 as f64, v.2 as f64);
        let off = (y.atan2(x).to_degrees() - head).rem_euclid(360.0);
        let off = if off > 180.0 { 360.0 - off } else { off };
        let b = ((off + 2.5) / 5.0) as usize;
        if b < rows.len() {
            rows[b].0 = rows[b].0.max(x.hypot(y));
            rows[b].1 = rows[b].1.max(z.abs());
        }
    }
    println!("theta  rho_max  y_halfwidth_all");
    for (b, (rho, w)) in rows.iter().enumerate() {
        println!("{:5}  {:7.3}  {:7.2}", b * 5, rho, w);
    }
    Ok(())
}
