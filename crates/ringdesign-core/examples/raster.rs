// Software z-buffer rasterizer shared by the preview examples.
use ringdesign_core::mesh::Mesh;

/// Gold-shaded render at the given orientation.
pub fn render(m: &Mesh, yaw: f64, pitch: f64, w: usize, h: usize) -> Vec<u8> {
    render_classed(m, yaw, pitch, w, h, None)
}

pub fn render_classed(
    m: &Mesh,
    yaw: f64,
    pitch: f64,
    w: usize,
    h: usize,
    classes: Option<&[ringdesign_core::FaceClass]>,
) -> Vec<u8> {
    let (min, max) = m.bounds().unwrap();
    let c = [
        (min.0 + max.0) as f64 * 0.5,
        (min.1 + max.1) as f64 * 0.5,
        (min.2 + max.2) as f64 * 0.5,
    ];
    let ext = ((max.0 - min.0).max(max.1 - min.1).max(max.2 - min.2)) as f64;
    let scale = w as f64 / (ext * 1.25);

    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let xf = |p: [f64; 3]| -> [f64; 3] {
        let (x, y, z) = (p[0] - c[0], p[1] - c[1], p[2] - c[2]);
        let (x1, y1) = (x * cy - y * sy, x * sy + y * cy);
        let (y2, z2) = (y1 * cp - z * sp, y1 * sp + z * cp);
        [x1, y2, z2]
    };

    let mut depth = vec![f64::NEG_INFINITY; w * h];
    let mut img = vec![18u8; w * h * 3];
    let light = {
        let l: [f64; 3] = [-0.35, -0.5, 0.79];
        let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        [l[0] / n, l[1] / n, l[2] / n]
    };

    for (fi, f) in m.faces.iter().enumerate() {
        let Some((a, b, cc)) = m.triangle(f) else { continue };
        let (ta, tb, tc) = (xf(a), xf(b), xf(cc));
        let n = {
            let e1 = [tb[0] - ta[0], tb[1] - ta[1], tb[2] - ta[2]];
            let e2 = [tc[0] - ta[0], tc[1] - ta[1], tc[2] - ta[2]];
            let v = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if l < 1e-12 { continue } else { [v[0] / l, v[1] / l, v[2] / l] }
        };
        if n[2] <= 0.0 {
            continue;
        }
        let px = |p: [f64; 3]| (p[0] * scale + w as f64 * 0.5, -p[1] * scale + h as f64 * 0.5);
        let (p0, p1, p2) = (px(ta), px(tb), px(tc));
        let minx = p0.0.min(p1.0).min(p2.0).floor().max(0.0) as usize;
        let maxx = (p0.0.max(p1.0).max(p2.0).ceil() as usize).min(w - 1);
        let miny = p0.1.min(p1.1).min(p2.1).floor().max(0.0) as usize;
        let maxy = (p0.1.max(p1.1).max(p2.1).ceil() as usize).min(h - 1);
        let area = (p1.0 - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (p1.1 - p0.1);
        if area.abs() < 1e-9 {
            continue;
        }
        let diff = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0);
        let spec = diff.powf(28.0);
        let shade = 0.16 + 0.72 * diff;
        let base = match classes {
            Some(cs) => cs.get(fi).map(|c| c.rgb()).unwrap_or([0.87, 0.71, 0.43]),
            None => [0.87, 0.71, 0.43],
        };
        let lit = if classes.is_some() { 0.35 + 0.65 * diff } else { shade };
        let hl = if classes.is_some() { 0.0 } else { spec * 0.85 };
        let col = [
            ((base[0] as f64 * lit + hl) * 255.0).min(255.0) as u8,
            ((base[1] as f64 * lit + hl) * 255.0).min(255.0) as u8,
            ((base[2] as f64 * lit + hl) * 255.0).min(255.0) as u8,
        ];
        for y in miny..=maxy {
            for x in minx..=maxx {
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                let w0 = ((p1.0 - p0.0) * (fy - p0.1) - (fx - p0.0) * (p1.1 - p0.1)) / area;
                let w1 = ((fx - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (fy - p0.1)) / area;
                if w0 < 0.0 || w1 < 0.0 || w0 + w1 > 1.0 {
                    continue;
                }
                let z = ta[2] + w1 * (tb[2] - ta[2]) + w0 * (tc[2] - ta[2]);
                let i = y * w + x;
                if z > depth[i] {
                    depth[i] = z;
                    img[i * 3] = col[0];
                    img[i * 3 + 1] = col[1];
                    img[i * 3 + 2] = col[2];
                }
            }
        }
    }
    img
}
