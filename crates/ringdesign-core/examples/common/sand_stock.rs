//! Native signet preparation shared by collection authors.
use anyhow::{Result, ensure};
use ringdesign_core::{field::smoothstep, imported_base::Source};

// Add vertices before shaping the drafted master: a flat source face may
// contain very large triangles, which would otherwise turn a smooth crown
// into two planes meeting at a ridge.
fn refine_master(s: &mut Source) -> Result<()> {
    for _ in 0..10 {
        let mut mids = std::collections::HashMap::new();
        for f in &s.faces {
            for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                let key = (a.min(b), a.max(b));
                if mids.contains_key(&key) {
                    continue;
                }
                let p = s.vertices[a as usize];
                let q = s.vertices[b as usize];
                if p.iter().zip(q).map(|(a, b)| (a - b).powi(2)).sum::<f64>() > 0.55_f64.powi(2) {
                    mids.insert(key, s.vertices.len() as u32);
                    s.vertices
                        .push(std::array::from_fn(|i| (p[i] + q[i]) * 0.5));
                }
            }
        }
        if mids.is_empty() {
            break;
        }
        let mut faces = Vec::new();
        for [a, b, c] in s.faces.drain(..) {
            let get = |a: u32, b: u32| mids.get(&(a.min(b), a.max(b))).copied();
            match (get(a, b), get(b, c), get(c, a)) {
                (None, None, None) => faces.push([a, b, c]),
                (Some(d), None, None) => faces.extend([[a, d, c], [d, b, c]]),
                (None, Some(e), None) => faces.extend([[b, e, a], [e, c, a]]),
                (None, None, Some(f)) => faces.extend([[c, f, b], [f, a, b]]),
                (Some(d), Some(e), None) => faces.extend([[d, b, e], [a, d, c], [d, e, c]]),
                (None, Some(e), Some(f)) => faces.extend([[e, c, f], [b, e, a], [e, f, a]]),
                (Some(d), None, Some(f)) => faces.extend([[f, a, d], [c, f, b], [f, d, b]]),
                (Some(d), Some(e), Some(f)) => {
                    faces.extend([[a, d, f], [d, b, e], [f, e, c], [d, e, f]])
                }
            }
        }
        s.faces = faces;
        ensure!(
            s.faces.len() <= 400_000,
            "Drafted master exceeds source budget"
        );
    }
    Ok(())
}
pub fn sand_stock(source: std::sync::Arc<Source>) -> Result<std::sync::Arc<Source>> {
    let mut s: Source = serde_json::from_value(serde_json::to_value(&*source)?)?;
    let bore = s.calibration.bore_radius_mm;
    let min_z = s
        .vertices
        .iter()
        .map(|p| p[2])
        .fold(f64::INFINITY, f64::min);
    let max_z = s
        .vertices
        .iter()
        .map(|p| p[2])
        .fold(f64::NEG_INFINITY, f64::max);
    let z0 = (min_z + max_z) * 0.5;
    // A sand master needs an actual edge loop at the parting plane. Merely
    // adding draft to triangles crossing it puts their maxima off-plane.
    // Clip the upper half and reflect it, retaining the imported contour.
    let old = s.vertices.clone();
    let old_faces = s.faces.clone();
    s.vertices.clear();
    s.faces.clear();
    let mut welded = std::collections::HashMap::new();
    let mut index = |p: [f64; 3], vertices: &mut Vec<[f64; 3]>| {
        let key = p.map(|x| (x * 1e6).round() as i64);
        *welded.entry(key).or_insert_with(|| {
            let id = vertices.len() as u32;
            vertices.push(p);
            id
        })
    };
    for f in old_faces {
        let tri = f.map(|i| {
            let p = old[i as usize];
            [p[0], p[1], p[2] - z0]
        });
        let mut poly = Vec::new();
        for i in 0..3 {
            let a = tri[i];
            let b = tri[(i + 1) % 3];
            if a[2] >= 0. {
                poly.push(a);
            }
            if (a[2] >= 0.) != (b[2] >= 0.) {
                let t = -a[2] / (b[2] - a[2]);
                poly.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, 0.]);
            }
        }
        for k in 1..poly.len().saturating_sub(1) {
            let tri = [poly[0], poly[k], poly[k + 1]];
            let upper = tri.map(|p| index(p, &mut s.vertices));
            let lower = tri.map(|p| index([p[0], p[1], -p[2]], &mut s.vertices));
            if upper[0] != upper[1] && upper[1] != upper[2] && upper[0] != upper[2] {
                s.faces.push(upper);
                s.faces.push([lower[0], lower[2], lower[1]]);
            }
        }
    }
    refine_master(&mut s)?;
    let mut half = vec![0_f64; 360];
    for p in &s.vertices {
        let t = p[1].atan2(p[0]).to_degrees().rem_euclid(360.).round() as usize % 360;
        for k in 0..=8 {
            for i in [(t + k) % 360, (t + 360 - k) % 360] {
                half[i] = half[i].max(p[2].abs());
            }
        }
    }
    for p in &mut s.vertices {
        let r = p[0].hypot(p[1]);
        let t = p[1].atan2(p[0]).to_degrees().rem_euclid(360.);
        let j = t.floor() as usize;
        let f = t - j as f64;
        let h = half[j] * (1. - f) + half[(j + 1) % 360] * f;
        let skin = smoothstep(bore + 0.12, bore + 0.65, r);
        let head = smoothstep(
            bore + s.calibration.head_height_mm * 0.35,
            bore + s.calibration.head_height_mm * 0.8,
            p[1],
        );
        let drafted = r + (0.045 + 0.16 * head) * (h - (p[2] * p[2] + 0.36).sqrt()).max(0.);
        let inner = bore + 0.012 * p[2].abs();
        let rr = if r < bore + 0.65 {
            inner * (1. - skin) + drafted * skin
        } else {
            drafted
        };
        p[0] *= rr / r;
        p[1] *= rr / r;
    }
    s.name = format!("{} / drafted workshop master", source.name);
    s.calibration.head_height_mm = s.vertices.iter().map(|p| p[1]).fold(0_f64, f64::max) - bore;
    s.calibration.palm_thickness_mm = s
        .vertices
        .iter()
        .filter(|p| p[1] < -bore && p[0].abs() < 0.12)
        .map(|p| p[0].hypot(p[1]) - bore)
        .fold(0_f64, f64::max);
    s.calibration.shoulder_end_mm = bore + s.calibration.head_height_mm * 0.8;
    Source::from_json(&serde_json::to_string(&s)?)
}
