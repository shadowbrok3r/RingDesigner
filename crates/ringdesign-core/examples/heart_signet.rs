// A heart signet: narrow shank swelling into a broad heart-shaped head.
//
// Renders the three views the reference photographs use — three-quarter, face
// on, and the silhouette down the ring's own plane — and prints the band's
// width against `BlankSignet.obj`, which is the shape the swell has to match.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability;
use ringdesign_core::field::SignetOutline;
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::{ShankKind, TOP_DEG};
use ringdesign_core::{ProfileStyle, RingDesign};

#[path = "common/raster.rs"]
mod raster;

const W: usize = 820;
const H: usize = 820;

/// The reference signet's width per degree off the top, as a fraction of its
/// head. A 14.7 mm round face on a 20 mm bore, 7 mm shank, 1.75 mm thick.
const REF: [(f64, f64); 19] = [
    (0., 0.9992),
    (5., 0.9946),
    (10., 0.9809),
    (15., 0.9539),
    (20., 0.9141),
    (25., 0.8666),
    (30., 0.8041),
    (35., 0.7446),
    (40., 0.6803),
    (45., 0.6189),
    (50., 0.5720),
    (55., 0.5266),
    (60., 0.4977),
    (65., 0.4755),
    (70., 0.4601),
    (75., 0.4509),
    (80., 0.4436),
    (85., 0.4397),
    (90., 0.4387),
];

fn signet(outline: SignetOutline, head_w: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = head_w;
    d.profile.thickness_mm = env("RD_THICK", 2.0);
    d.profile.edge_round_mm = env("RD_EDGE", 0.5);
    d.profile.side_draft_deg = env("RD_DRAFT", 0.0);
    d.shank.kind = ShankKind::Signet;
    d.shank.apply_signet(head_w);
    d.shank.head.loft = 0.0; // The prism is the subject.
    d.shank.head.outline = outline;
    d.shank.head.rise_mm = env("RD_RISE", 0.3);
    d.shank.head.fit_length_to(head_w);
    d
}

fn env(key: &str, fallback: f64) -> f64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(fallback)
}

/// Quarter turn so the profile view stands the head up, as the reference does.
fn turn(img: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; img.len()];
    for y in 0..h {
        for x in 0..w {
            let (nx, ny) = (h - 1 - y, x);
            for k in 0..3 {
                out[(ny * h + nx) * 3 + k] = img[(y * w + x) * 3 + k];
            }
        }
    }
    out
}

fn save(name: &str, img: &[u8]) {
    image::save_buffer(name, img, W as u32, H as u32, image::ColorType::Rgb8).unwrap();
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp".into());
    let lib = AlphaLibrary::builtin();
    let params = match std::env::args().nth(2).as_deref() {
        Some("draft") => BuildParams { theta_steps: 192, profile_steps: 96, ..Default::default() },
        _ => BuildParams { theta_steps: 900, profile_steps: 256, ..Default::default() },
    };

    // The swell, against the reference. Read on a round head, which is what the
    // reference is — a heart's lobes are a separate question.
    let round = signet(SignetOutline::Round, 14.7);
    let (inner_r, crest_r) = (round.inner_radius_mm(), round.reference_loop().crest_radius_mm);
    let at = |d: &RingDesign, deg: f64| {
        d.shank.signet_width_frac(TOP_DEG + deg, d.inner_radius_mm(), crest_r, &d.profile)
    };
    let (head, shank) = (at(&round, 0.0), at(&round, 90.0));
    println!("swell against BlankSignet.obj  (bore {inner_r:.2}, crest {crest_r:.2})");
    println!("  deg    ref    mine    delta");
    let mut worst: f64 = 0.0;
    for (deg, want) in REF {
        // Both normalized to their own head and shank, so the comparison is of
        // the swell's shape and not of two rings' proportions.
        let mine = (at(&round, deg) - shank) / (head - shank).max(1e-9);
        let r = (want - REF[18].1) / (REF[0].1 - REF[18].1);
        println!("  {deg:3.0}  {r:.4}  {mine:.4}  {:+.4}", mine - r);
        worst = worst.max((mine - r).abs());
    }
    println!("  worst {worst:.4}");

    // Where the silhouette kinks. A crease is a step in slope, so this walks
    // the width and reports the worst change per degree and where it is.
    println!("\nworst slope step in the silhouette, per outline:");
    for &o in SignetOutline::ALL {
        let d = signet(o, 13.0);
        let cr = d.reference_loop().crest_radius_mm;
        let w = |t: f64| d.shank.signet_width_frac(TOP_DEG + t, d.inner_radius_mm(), cr, &d.profile);
        const STEP: f64 = 0.05;
        let slope = |t: f64| (w(t + STEP) - w(t)) / STEP;
        let (mut worst, mut at) = (0.0f64, 0.0);
        let mut prev = slope(0.0);
        let mut t = STEP;
        while t < 120.0 {
            let s = slope(t);
            if (s - prev).abs() > worst {
                worst = (s - prev).abs();
                at = t;
            }
            prev = s;
            t += STEP;
        }
        println!("  {:<10} {worst:.5} per degree at {at:.2} deg", o.label());
    }

    for (name, o) in [("heart", SignetOutline::Heart), ("round", SignetOutline::Round)] {
        let d = signet(o, 13.0);
        let built = mesh::build(&d, &lib, params);
        let rep = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
        println!(
            "\n{name}: {} {:.3}% undercut, worst {:+.2} deg, watertight {}",
            rep.verdict.label(),
            rep.undercut_fraction() * 100.0,
            rep.worst_draft_deg,
            built.report.validation.watertight,
        );
        // Where the undercut faces sit, in the ring's own coordinates.
        let mut worst: Vec<(f64, f64, f64, f64)> = Vec::new();
        for (i, f) in built.mesh.faces.iter().enumerate() {
            if rep.classes.get(i) != Some(&ringdesign_core::FaceClass::Undercut) {
                continue;
            }
            let Some((a, b, c)) = built.mesh.triangle(f) else { continue };
            let p = [
                (a[0] + b[0] + c[0]) / 3.0,
                (a[1] + b[1] + c[1]) / 3.0,
                (a[2] + b[2] + c[2]) / 3.0,
            ];
            let deg = p[1].atan2(p[0]).to_degrees() - TOP_DEG;
            let n = built.mesh.face_normal(f).unwrap_or([0.0; 3]);
            worst.push((deg, p[0].hypot(p[1]), p[2], n[2].asin().to_degrees()));
        }
        worst.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap());
        for &(deg, r, z, draft) in worst.iter().take(6) {
            println!("    {deg:+7.2} deg  r {r:5.2}  z {z:+5.2}  draft {draft:+6.1}");
        }
        if worst.len() > 6 {
            println!("    ... {} more", worst.len() - 6);
        }
        save(&format!("{out}/{name}_hero.png"), &raster::render(&built.mesh, 0.5, 1.15, W, H));
        save(&format!("{out}/{name}_face.png"), &raster::render(&built.mesh, 0.0, 1.571, W, H));
        let side = raster::render(&built.mesh, 1.571, 1.571, W, H);
        save(&format!("{out}/{name}_side.png"), &turn(&side, W, H));
        // Into the bore from the shank side, which is where a body that is
        // really the face extruded shows itself.
        save(&format!("{out}/{name}_under.png"), &raster::render(&built.mesh, 0.0, -0.62, W, H));

        let dir = ringdesign_core::library::default_design_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(format!("{name}_signet.json"));
        ringdesign_core::library::save_design(&file, &d).unwrap();
        println!("  saved {}", file.display());
    }
}
