// Measure a real signet off its mesh, and put ours beside it.
//
// Everything printed here is read the same way from both, so the columns are
// comparable: width and crest per degree off the head, and the cross-section at
// a few stations across it.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::mesh::{self, BuildParams, Mesh};
use ringdesign_core::profile::{ShankKind, TOP_DEG};
use ringdesign_core::{ProfileStyle, RingDesign};

#[path = "common/raster.rs"]
mod raster;
#[path = "common/scan.rs"]
mod scan;

const W: usize = 820;
const H: usize = 820;
const DIR: &str = "/home/shadowbroker/jewelry-scan/RING/Signets";

fn views(name: &str, m: &Mesh, out: &str) {
    let save = |tag: &str, img: Vec<u8>| {
        image::save_buffer(
            format!("{out}/{name}_{tag}.png"),
            &img,
            W as u32,
            H as u32,
            image::ColorType::Rgb8,
        )
        .unwrap();
    };
    save("hero", raster::render(m, 0.5, 1.15, W, H));
    save("face", raster::render(m, 0.0, 1.571, W, H));
    save("under", raster::render(m, 0.0, -0.62, W, H));
    // A quarter turn stands the side view up, as the catalogue photographs do.
    let side = raster::render(m, 1.571, 1.571, W, H);
    let mut turned = vec![0u8; side.len()];
    for y in 0..H {
        for x in 0..W {
            let (nx, ny) = (H - 1 - y, x);
            for k in 0..3 {
                turned[(ny * H + nx) * 3 + k] = side[(y * W + x) * 3 + k];
            }
        }
    }
    save("side", turned);
}

/// The table itself: the outermost flat, found as the plane facing the head.
///
/// Measured and not inferred from the silhouette, because the head's widest
/// point is its corner and the corner is not on the table's axis.
fn table(s: &scan::Scan) {
    use std::collections::HashMap;
    // The biggest flat, by area of parallel facets. Reading the outermost point
    // instead finds the table's *corner*, which is not on its axis and not even
    // on the table for a face that leans.
    let mut bins: HashMap<[i32; 3], f64> = HashMap::new();
    let face = |f: &[u32; 3]| {
        let (a, b, c) = s.mesh.triangle(f)?;
        let n = s.mesh.face_normal(f)?;
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let x = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let area = 0.5 * (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
        Some((n, area, [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0]))
    };
    for f in &s.mesh.faces {
        let Some((n, area, p)) = face(f) else { continue };
        // Outward-facing only, and only over the head.
        if p[1] < 0.0 || n[1] <= 0.2 {
            continue;
        }
        let key = [(n[0] * 40.0).round() as i32, (n[1] * 40.0).round() as i32, (n[2] * 40.0).round() as i32];
        *bins.entry(key).or_default() += area;
    }
    let Some((&key, &area)) = bins.iter().max_by(|a, b| a.1.total_cmp(b.1)) else {
        println!("   no table found");
        return;
    };
    let dir = {
        let v = [key[0] as f64, key[1] as f64, key[2] as f64];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / l, v[1] / l, v[2] / l]
    };
    // Every facet parallel to it and at the same offset is the table.
    let mut off = Vec::new();
    let mut pts: Vec<[f64; 3]> = Vec::new();
    for f in &s.mesh.faces {
        let Some((n, _, p)) = face(f) else { continue };
        if n[0] * dir[0] + n[1] * dir[1] + n[2] * dir[2] < 0.999 {
            continue;
        }
        off.push(p[0] * dir[0] + p[1] * dir[1] + p[2] * dir[2]);
        let (a, b, c) = s.mesh.triangle(f).unwrap();
        pts.extend([a, b, c]);
    }
    off.sort_by(f64::total_cmp);
    let d = off[off.len() / 2];
    pts.retain(|p| (p[0] * dir[0] + p[1] * dir[1] + p[2] * dir[2] - d).abs() < 0.05);
    let span = |k: usize| {
        let lo = pts.iter().map(|p| p[k]).fold(f64::MAX, f64::min);
        let hi = pts.iter().map(|p| p[k]).fold(f64::MIN, f64::max);
        (hi - lo, 0.5 * (lo + hi))
    };
    let (len, cx) = span(0);
    let (across, mid) = span(2);
    let tilt = dir[2].asin().to_degrees();
    println!(
        "   table: {len:.2} round the ring x {across:.2} across, {area:.1} mm2, at radius \
         {d:.2} ({:.2} over the bore), centred {mid:+.2} along the finger, tilted {tilt:+.1} deg, \
         corner reach {:.2}",
        d - s.bore_r,
        (len * 0.5).hypot(d) - s.bore_r
    );

    // The table's own outline, drawn. This is the shape `SignetOutline` has to
    // reproduce, and the only honest way to compare the two.
    const ROWS: usize = 30;
    const COLS: usize = 62;
    let half = 0.5 * len.max(across) + 0.3;
    let mut grid = vec![[b' '; COLS]; ROWS];
    // Rasterized from the facets, not filled between the boundary vertices: a
    // heart's dimple is a *concavity*, and filling each row between its ends
    // closes it.
    let cell = |p: [f64; 3]| {
        (
            ((p[2] - mid + half) / (2.0 * half) * ROWS as f64).clamp(0.0, ROWS as f64 - 1.0),
            ((p[0] - cx + half) / (2.0 * half) * COLS as f64).clamp(0.0, COLS as f64 - 1.0),
        )
    };
    for f in &s.mesh.faces {
        let Some((n, _, p)) = face(f) else { continue };
        if n[0] * dir[0] + n[1] * dir[1] + n[2] * dir[2] < 0.999
            || (p[0] * dir[0] + p[1] * dir[1] + p[2] * dir[2] - d).abs() >= 0.05
        {
            continue;
        }
        let (a, b, c) = s.mesh.triangle(f).unwrap();
        let t = [cell(a), cell(b), cell(c)];
        let (r0, r1) = (
            t.iter().map(|v| v.0).fold(f64::MAX, f64::min) as usize,
            t.iter().map(|v| v.0).fold(f64::MIN, f64::max) as usize,
        );
        let (c0, c1) = (
            t.iter().map(|v| v.1).fold(f64::MAX, f64::min) as usize,
            t.iter().map(|v| v.1).fold(f64::MIN, f64::max) as usize,
        );
        let area = (t[1].0 - t[0].0) * (t[2].1 - t[0].1) - (t[2].0 - t[0].0) * (t[1].1 - t[0].1);
        for r in r0..=r1.min(ROWS - 1) {
            for col in c0..=c1.min(COLS - 1) {
                let (pr, pc) = (r as f64 + 0.5, col as f64 + 0.5);
                let e = |i: usize, j: usize| {
                    (t[j].0 - t[i].0) * (pc - t[i].1) - (t[j].1 - t[i].1) * (pr - t[i].0)
                };
                let (x, y, z) = (e(0, 1), e(1, 2), e(2, 0));
                if area.abs() < 1e-12
                    || ((x >= 0.0) == (area > 0.0)
                        && (y >= 0.0) == (area > 0.0)
                        && (z >= 0.0) == (area > 0.0))
                {
                    grid[r][col] = b'#';
                }
            }
        }
    }
    for row in grid.iter().rev() {
        println!("     |{}|", String::from_utf8(row.to_vec()).unwrap());
    }

    // The same outline as `SignetOutline::extent` reports it: reach across the
    // band per station around the ring, both normalized to the plate's own box.
    let mut lo = [f64::MAX; 11];
    let mut hi = [f64::MIN; 11];
    for p in &pts {
        let x = ((p[0] - cx) / (len * 0.5)).clamp(-1.0, 1.0);
        let i = ((x.abs() * 10.0).round() as usize).min(10);
        let y = (p[2] - mid) / (across * 0.5);
        lo[i] = lo[i].min(y);
        hi[i] = hi[i].max(y);
    }
    print!("     x     ");
    for i in 0..11 {
        print!("{:6.1}", i as f64 / 10.0);
    }
    println!();
    print!("     lo    ");
    for v in lo {
        print!("{:6.2}", if v > 1e30 { 0.0 } else { v });
    }
    println!();
    print!("     hi    ");
    for v in hi {
        print!("{:6.2}", if v < -1e30 { 0.0 } else { v });
    }
    println!();
}

fn report(name: &str, s: &scan::Scan) {
    let head = s.at(0.0);
    let back = s.at(180.0);
    println!(
        "\n{name}: bore {:.2}  head {:.2} wide x {:.2} thick  shank {:.2} x {:.2}",
        s.bore_r,
        head.width,
        head.crest - s.bore_r,
        back.width,
        back.crest - s.bore_r
    );
    println!("   off   width  w_norm   crest  thick  centre");
    for step in 0..=18 {
        let d = step as f64 * 5.0;
        let a = s.at(d);
        println!(
            "  {d:4.0}  {:6.2}  {:6.3}  {:6.2} {:6.2}  {:+6.2}",
            a.width,
            (a.width - back.width) / (head.width - back.width).max(1e-9),
            a.crest,
            a.crest - s.bore_r,
            a.centre
        );
    }
}

/// The head's flank, drawn: the section's metal from bore to crest, per station
/// along the finger. `#` is metal, and the bore is on the left.
fn sections(s: &scan::Scan, half: f64) {
    const BINS: usize = 34;
    const COLS: usize = 46;
    let r0 = s.bore_r - 0.4;
    let r1 = s.at(0.0).crest.max(s.at(40.0).crest) + 0.4;
    println!(
        "   sections, bore {r0:.1} to {r1:.1} mm across, +-{half:.1} mm along the finger:"
    );
    for d in [0.0, 15.0, 30.0, 45.0, 60.0, 90.0, 180.0] {
        let rows = s.section(d, BINS, half);
        println!("  {d:5.0} deg");
        for (i, slot) in rows.iter().enumerate().rev() {
            let z = -half + 2.0 * half * (i as f64 + 0.5) / BINS as f64;
            let mut line = vec![b' '; COLS];
            if let Some((inner, outer)) = *slot {
                let col = |r: f64| {
                    (((r - r0) / (r1 - r0) * COLS as f64) as isize).clamp(0, COLS as isize - 1)
                        as usize
                };
                for c in line.iter_mut().take(col(outer) + 1).skip(col(inner)) {
                    *c = b'#';
                }
            }
            println!("    {z:+6.1} |{}|", String::from_utf8(line).unwrap());
        }
    }
}

/// Ours, built to the same bore, head width and thickness.
fn ours(bore_r: f64, head_w: f64, thick: f64, outline: ringdesign_core::field::SignetOutline) -> RingDesign {
    let mut d = RingDesign::default();
    d.size = ringdesign_core::sizing::RingSize::from_diameter_mm(bore_r * 2.0);
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = head_w;
    d.profile.thickness_mm = thick;
    // The references' plates have a 0.2 mm break and nothing else: a squared
    // side is what lets the table reach the head's own silhouette.
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Signet;
    d.shank.apply_signet(head_w);
    d.shank.head.loft = 0.0; // The prism is the subject.
    d.shank.head.outline = outline;
    d.shank.head.fit_length_to(head_w);
    d
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp".into());
    let files = [
        ("heart", "Heart-signet-ring-size7to11.STL"),
        ("hexa", "Hexa-signet-size6to13.STL"),
        ("oval", "Signet Oval R.stl"),
    ];

    for (name, file) in files {
        let raw = scan::read_stl(&format!("{DIR}/{file}"));
        let parts = scan::components(&raw);
        println!("\n=== {file}: {} tris, {} rings ===", raw.faces.len(), parts.len());
        // The smallest is the nearest to a size 7, which is what we model.
        let mut scans: Vec<scan::Scan> = parts.iter().map(scan::Scan::of).collect();
        scans.sort_by(|a, b| a.bore_r.total_cmp(&b.bore_r));
        let s = &scans[0];
        for other in &scans {
            print!("  bore {:.2}", other.bore_r);
        }
        println!();
        report(name, s);
        table(s);
        sections(s, s.at(0.0).width * 0.55);
        views(&format!("ref_{name}"), &s.mesh, &out);
    }

    // Ours, matched to the heart reference.
    let raw = scan::read_stl(&format!("{DIR}/Heart-signet-ring-size7to11.STL"));
    let mut scans: Vec<scan::Scan> =
        scan::components(&raw).iter().map(scan::Scan::of).collect();
    scans.sort_by(|a, b| a.bore_r.total_cmp(&b.bore_r));
    let r = &scans[0];
    // Matched on the shank's thickness, not the head's: ours adds the table's
    // rise on top of the band, and the reference's head is its shank plus that
    // rise already.
    let (head_w, thick) =
        (r.at(0.0).width, r.at(180.0).crest - r.bore_r);
    let d = ours(r.bore_r, head_w, thick, ringdesign_core::field::SignetOutline::Heart);
    let built = mesh::build(
        &d,
        &AlphaLibrary::builtin(),
        BuildParams { theta_steps: 900, profile_steps: 256, ..Default::default() },
    );
    println!(
        "\n=== ours, matched: bore {:.2}, head {head_w:.2} wide, {thick:.2} thick ===",
        d.inner_radius_mm()
    );
    let mine = scan::Scan::of(&built.mesh);
    report("ours", &mine);
    table(&mine);
    sections(&mine, head_w * 0.55);
    views("ours_heart", &built.mesh, &out);
    let _ = TOP_DEG;
}
