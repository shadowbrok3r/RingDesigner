// Where a signet actually has edges: dihedral turn per unit length, mapped.
//
// The claim under test (Logan's): on the reference heart the face outline is
// the ONLY edge, sharp just at the point and the cleft; everything else is
// smooth. Run on the reference and on ours, same classifier, so the columns
// compare.
use std::collections::HashMap;

use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::ShankKind;
use ringdesign_core::{ProfileStyle, RingDesign};

#[path = "common/scan.rs"]
mod scan;

/// The table plane over the head: dominant facet-normal bin facing +Y.
fn table_plane(s: &scan::Scan) -> ([f64; 3], f64) {
    let mut bins: HashMap<[i32; 3], f64> = HashMap::new();
    for f in &s.mesh.faces {
        let (Some((a, b, c)), Some(n)) = (s.mesh.triangle(f), s.mesh.face_normal(f)) else {
            continue;
        };
        let p = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0];
        if p[1] < 0.0 || n[1] <= 0.2 {
            continue;
        }
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let x = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let area = 0.5 * (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
        let key =
            [(n[0] * 40.0).round() as i32, (n[1] * 40.0).round() as i32, (n[2] * 40.0).round() as i32];
        *bins.entry(key).or_default() += area;
    }
    let (&key, _) = bins.iter().max_by(|a, b| a.1.total_cmp(b.1)).unwrap();
    let v = [key[0] as f64, key[1] as f64, key[2] as f64];
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let dir = [v[0] / l, v[1] / l, v[2] / l];
    let mut off: Vec<f64> = Vec::new();
    for f in &s.mesh.faces {
        let (Some((a, b, c)), Some(n)) = (s.mesh.triangle(f), s.mesh.face_normal(f)) else {
            continue;
        };
        if n[0] * dir[0] + n[1] * dir[1] + n[2] * dir[2] < 0.999 {
            continue;
        }
        let p = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0];
        off.push(p[0] * dir[0] + p[1] * dir[1] + p[2] * dir[2]);
    }
    off.sort_by(f64::total_cmp);
    (dir, off[off.len() / 2])
}

/// Spin the scan so the measured plate normal is exactly +Y. The width-symmetry
/// alignment can sit degrees off the plate on a lobed outline.
fn plate_align(s: &mut scan::Scan) -> f64 {
    let (dir, d) = table_plane(s);
    let e = dir[0].atan2(dir[1]);
    let (sn, cs) = e.sin_cos();
    for v in s.mesh.vertices.iter_mut() {
        let (x, y) = (v.0 as f64, v.1 as f64);
        *v = ringdesign_core::mesh::Vec3((x * cs - y * sn) as f32, (x * sn + y * cs) as f32, v.2);
    }
    println!("   plate normal was {:+.1} deg off the width-symmetry head", e.to_degrees());
    d
}

/// Sharp-edge census: dihedral per interior edge, bucketed by where it lives.
fn probe(name: &str, s: &scan::Scan) {
    let m = &s.mesh;
    let (dir, d) = table_plane(s);
    let normals: Vec<Option<[f64; 3]>> = m.faces.iter().map(|f| m.face_normal(f)).collect();
    let on_plane = |p: [f64; 3]| (p[0] * dir[0] + p[1] * dir[1] + p[2] * dir[2] - d).abs() < 0.08;
    let is_table = |fi: usize| {
        let Some(n) = normals[fi] else { return false };
        if n[0] * dir[0] + n[1] * dir[1] + n[2] * dir[2] < 0.995 {
            return false;
        }
        let Some((a, b, c)) = m.triangle(&m.faces[fi]) else { return false };
        on_plane([(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0])
    };

    let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (fi, f) in m.faces.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (f[k], f[(k + 1) % 3]);
            edges.entry((a.min(b), a.max(b))).or_default().push(fi);
        }
    }

    // deg of turn, edge length, midpoint, borders-the-table.
    struct Sharp {
        turn: f64,
        len: f64,
        mid: [f64; 3],
        table: bool,
    }
    let mut sharp: Vec<Sharp> = Vec::new();
    let mut smooth_len = 0.0f64;
    for (&(a, b), fs) in &edges {
        if fs.len() != 2 {
            continue;
        }
        let (Some(n0), Some(n1)) = (normals[fs[0]], normals[fs[1]]) else { continue };
        let dot = (n0[0] * n1[0] + n0[1] * n1[1] + n0[2] * n1[2]).clamp(-1.0, 1.0);
        let turn = dot.acos().to_degrees();
        let (pa, pb) = (m.vertices[a as usize], m.vertices[b as usize]);
        let (pa, pb) =
            ([pa.0 as f64, pa.1 as f64, pa.2 as f64], [pb.0 as f64, pb.1 as f64, pb.2 as f64]);
        let len =
            ((pa[0] - pb[0]).powi(2) + (pa[1] - pb[1]).powi(2) + (pa[2] - pb[2]).powi(2)).sqrt();
        if turn < 15.0 {
            smooth_len += len;
            continue;
        }
        sharp.push(Sharp {
            turn,
            len,
            mid: [
                0.5 * (pa[0] + pb[0]),
                0.5 * (pa[1] + pb[1]),
                0.5 * (pa[2] + pb[2]),
            ],
            table: is_table(fs[0]) || is_table(fs[1]),
        });
    }

    // Regions: the face outline (borders a table facet), the bore's own edge
    // break (at bore radius), and everything else — which Logan says is empty.
    let bore = s.bore_r;
    let (mut t_len, mut b_len, mut o_len) = (0.0f64, 0.0f64, 0.0f64);
    let mut others: Vec<&Sharp> = Vec::new();
    for e in &sharp {
        let r = e.mid[0].hypot(e.mid[1]);
        if e.table || on_plane(e.mid) {
            t_len += e.len;
        } else if r < bore + 0.35 {
            b_len += e.len;
        } else {
            o_len += e.len;
            others.push(e);
        }
    }
    println!(
        "\n{name}: table at r {d:.2} tilt {:+.1} deg; edge turn >= 15 deg:",
        dir[2].asin().to_degrees()
    );
    println!(
        "   face outline {t_len:6.1} mm   bore edge {b_len:6.1} mm   ELSEWHERE {o_len:6.1} mm   (smooth-mesh edge length {smooth_len:.0} mm)"
    );

    // The elsewhere set, clustered so it reads: angle off the head, z, radius.
    others.sort_by(|a, b| b.turn.total_cmp(&a.turn));
    let mut shown: Vec<[f64; 3]> = Vec::new();
    for e in &others {
        let deg = e.mid[1].atan2(e.mid[0]).to_degrees() - 90.0;
        let key = [deg, e.mid[2], e.mid[0].hypot(e.mid[1])];
        if shown
            .iter()
            .any(|k| (k[0] - key[0]).abs() < 6.0 && (k[1] - key[1]).abs() < 0.5 && (k[2] - key[2]).abs() < 0.5)
        {
            continue;
        }
        shown.push(key);
        println!(
            "     {:5.1} deg turn at {:+6.1} deg off head, z {:+5.2}, r {:5.2}",
            e.turn, key[0], key[1], key[2]
        );
        if shown.len() >= 12 {
            break;
        }
    }

    // How crisp the outline's own edge is, around the face: total turn across
    // the plate's rim, sampled by station along the ring.
    let mut rim: HashMap<i32, (f64, f64)> = HashMap::new();
    for e in sharp.iter().filter(|e| e.table || on_plane(e.mid)) {
        let deg = e.mid[1].atan2(e.mid[0]).to_degrees() - 90.0;
        let slot = rim.entry((deg / 5.0).round() as i32).or_default();
        slot.0 = slot.0.max(e.turn);
        slot.1 += e.len;
    }
    let mut keys: Vec<i32> = rim.keys().copied().collect();
    keys.sort();
    print!("   rim turn by station:");
    for k in keys {
        print!("  {:+3}:{:2.0}", k * 5, rim[&k].0);
    }
    println!();

    // The rim's fillet, sized from the sub-sharp turn belt around the plate:
    // total turn against how far from the plane it happens. arc/turn = radius.
    let mut turn_sum = 0.0f64;
    let mut depth_max = 0.0f64;
    for (&(a, b), fs) in &edges {
        if fs.len() != 2 {
            continue;
        }
        let (Some(n0), Some(n1)) = (normals[fs[0]], normals[fs[1]]) else { continue };
        let dot = (n0[0] * n1[0] + n0[1] * n1[1] + n0[2] * n1[2]).clamp(-1.0, 1.0);
        let turn = dot.acos().to_degrees();
        if turn < 3.0 {
            continue;
        }
        let (pa, pb) = (m.vertices[a as usize], m.vertices[b as usize]);
        let mid = [
            0.5 * (pa.0 as f64 + pb.0 as f64),
            0.5 * (pa.1 as f64 + pb.1 as f64),
            0.5 * (pa.2 as f64 + pb.2 as f64),
        ];
        let depth = d - (mid[0] * dir[0] + mid[1] * dir[1] + mid[2] * dir[2]);
        if !(0.0..1.2).contains(&depth) || mid[0].hypot(mid[1]) < bore + 0.8 {
            continue;
        }
        turn_sum += turn;
        if turn > 8.0 {
            depth_max = depth_max.max(depth);
        }
    }
    println!("   rim belt: {turn_sum:.0} deg of turn within 1.2 mm of the plane, >8 deg turns reach {depth_max:.2} mm deep");
}

/// The wall Logan is describing: outer r per z through the head, finely, so
/// the point wall and the cleft wall show their whole profile.
fn flank(name: &str, s: &scan::Scan) {
    println!("{name} wall profiles (outer r per z across the band):");
    let half = s.at(0.0).width * 0.62;
    const BINS: usize = 110;
    for d in [0.0f64, 12.0, 25.0] {
        let rows = s.section(d, BINS, half);
        let prof: Vec<(f64, f64)> = rows
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                r.map(|(_, outer)| (-half + 2.0 * half * (i as f64 + 0.5) / BINS as f64, outer))
            })
            .collect();
        let top = prof.iter().map(|p| p.1).fold(f64::MIN, f64::max);
        println!("  {d:4.0} deg, top r {top:.2}; z where the outer sits N mm below the top:");
        for drop in [0.05, 0.2, 0.5, 1.0, 1.5, 2.0] {
            let lo = prof.iter().take_while(|p| p.1 < top - drop).last().map(|p| p.0);
            let hi = prof.iter().rev().take_while(|p| p.1 < top - drop).last().map(|p| p.0);
            let (Some(lo), Some(hi)) = (lo, hi) else { continue };
            println!("      -{drop:4.2} mm: z {lo:+6.2} and {hi:+6.2}");
        }
    }
}

fn main() {
    let raw = scan::read_stl(
        "/home/shadowbroker/jewelry-scan/RING/Signets/Heart-signet-ring-size7to11.STL",
    );
    let mut scans: Vec<scan::Scan> = scan::components(&raw).iter().map(scan::Scan::of).collect();
    scans.sort_by(|a, b| a.bore_r.total_cmp(&b.bore_r));
    let r = &mut scans[0];
    plate_align(r);
    probe("REFERENCE heart", r);
    flank("REFERENCE heart", r);
    let r = &scans[0];

    let (head_w, thick) = (r.at(0.0).width, r.at(180.0).crest - r.bore_r);
    let mut d = RingDesign::default();
    d.size = ringdesign_core::sizing::RingSize::from_diameter_mm(r.bore_r * 2.0);
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = head_w;
    d.profile.thickness_mm = thick;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Signet;
    d.shank.apply_signet(head_w);
    d.shank.head.loft = 0.0; // The prism is the subject.
    d.shank.head.outline = ringdesign_core::field::SignetOutline::Heart;
    d.shank.head.fit_length_to(head_w);
    let built = mesh::build(
        &d,
        &AlphaLibrary::builtin(),
        BuildParams { theta_steps: 900, profile_steps: 256, ..Default::default() },
    );
    let mut mine = scan::Scan::of(&built.mesh);
    plate_align(&mut mine);
    probe("OURS heart", &mine);
    flank("OURS heart", &mine);

    // Decode the worst creases on the raw build grid: profile row j tells
    // which part of the section loop carries them.
    let m = &built.mesh;
    let p = 256usize;
    let normals: Vec<Option<[f64; 3]>> = m.faces.iter().map(|f| m.face_normal(f)).collect();
    let mut edges: std::collections::HashMap<(u32, u32), Vec<usize>> = std::collections::HashMap::new();
    for (fi, f) in m.faces.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (f[k], f[(k + 1) % 3]);
            edges.entry((a.min(b), a.max(b))).or_default().push(fi);
        }
    }
    let mut worst: Vec<(f64, usize, [f64; 3])> = Vec::new();
    for (&(a, b), fs) in &edges {
        if fs.len() != 2 {
            continue;
        }
        let (Some(n0), Some(n1)) = (normals[fs[0]], normals[fs[1]]) else { continue };
        let dot = (n0[0] * n1[0] + n0[1] * n1[1] + n0[2] * n1[2]).clamp(-1.0, 1.0);
        let turn = dot.acos().to_degrees();
        let pa = m.vertices[a as usize];
        let pb = m.vertices[b as usize];
        let mid = [
            0.5 * (pa.0 + pb.0) as f64,
            0.5 * (pa.1 + pb.1) as f64,
            0.5 * (pa.2 + pb.2) as f64,
        ];
        let r = mid[0].hypot(mid[1]);
        let deg = mid[1].atan2(mid[0]).to_degrees() - 90.0;
        // Only the elsewhere family: off the plate, off the bore, off the
        // point/cleft symmetry plane.
        if turn >= 40.0 && r > 9.2 && r < 13.0 && deg.abs() > 6.0 && deg.abs() < 80.0 {
            worst.push((turn, fs[0], mid));
        }
    }
    worst.sort_by(|x, y| y.0.total_cmp(&x.0));
    println!("grid decode of elsewhere creases (face row j of {p}):");
    for (turn, fi, mid) in worst.iter().take(10) {
        let j = (fi / 2) % p;
        let i = (fi / 2) / p;
        println!(
            "   turn {:5.1} at i {} (theta {:+7.2} deg off head) j {}   xyz ({:+.2}, {:+.2}, {:+.2})",
            turn,
            i,
            i * 360 / 900,
            j,
            mid[0],
            mid[1],
            mid[2]
        );
    }
}
