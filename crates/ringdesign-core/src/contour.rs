//! Turning a drawn sketch into a closed boundary.
//!
//! [`CustomOutline::from_points`](crate::field::CustomOutline::from_points) does
//! the hard parts of importing a plan — arc-length densification, circular
//! smoothing, aspect and `fair_r` from the hull defect — and takes a closed
//! polyline. Core had every other piece (an exact EDT, `content_bounds`) and no
//! way to *get* that polyline out of a raster, so a drawn face could only be
//! authored by typing coordinates.
//!
//! This is the missing step: threshold, walk the largest blob's boundary, and
//! decimate to something `from_points` can smooth.
//!
//! Deliberately not a model. The drawn line *is* the boundary the jeweller
//! meant, so anything generative here can only add error to exact data.

use crate::alpha::Alpha;

/// Directions the boundary is sampled in.
///
/// `from_points` densifies and resamples to a 720-entry polar table regardless,
/// so more than this buys nothing and costs the smoother.
pub const MAX_TRACE_POINTS: usize = 512;

/// Below this many boundary samples there is no shape to speak of —
/// `from_points` itself refuses fewer than 8.
const MIN_BOUNDARY: usize = 16;

/// A mark smaller than this is a speck, not a plan.
///
/// The radial sweep happily returns a full ring of samples for a single pixel,
/// so the size check has to be on the blob itself rather than on how many
/// samples came back.
const MIN_BLOB_PX: usize = 32;
/// …and it has to have extent in both axes, so a drawn hairline is refused too.
const MIN_BLOB_EXTENT: usize = 4;

/// Trace the largest blob in `a` above `threshold` as a closed polyline.
///
/// Returns points in the alpha's own normalized `(x, y)`, counter-clockwise,
/// ready for `CustomOutline::from_points`. `None` when nothing crosses the
/// threshold or the mark is too small to be a plan.
pub fn trace(a: &Alpha, threshold: f32) -> Option<Vec<[f64; 2]>> {
    let (w, h) = (a.width, a.height);
    if w < 3 || h < 3 {
        return None;
    }
    let inside = |x: usize, y: usize| a.data.get(y * w + x).is_some_and(|&v| v > threshold);

    let blob = largest_blob(w, h, &inside)?;
    if !big_enough(w, h, &blob) {
        return None;
    }
    let pts = radial_boundary(w, h, &blob, MAX_TRACE_POINTS);
    if pts.len() < MIN_BOUNDARY {
        return None;
    }
    // Normalized to the raster, and y flipped so the result reads in the
    // upright convention `CustomOutline` uses — its own `+y` runs across the
    // band toward the low edge.
    Some(
        pts.into_iter()
            .map(|(x, y)| [x as f64 / w as f64, 1.0 - y as f64 / h as f64])
            .collect(),
    )
}

/// Flood-fill every connected component and keep the biggest.
///
/// The biggest rather than the first: a sketch usually carries stray marks, and
/// the plan is the one with area behind it.
fn largest_blob(w: usize, h: usize, inside: &dyn Fn(usize, usize) -> bool) -> Option<Vec<bool>> {
    let mut label = vec![0u32; w * h];
    let mut best = (0usize, 0u32);
    let mut next = 1u32;

    for sy in 0..h {
        for sx in 0..w {
            if !inside(sx, sy) || label[sy * w + sx] != 0 {
                continue;
            }
            let id = next;
            next += 1;
            let mut n = 0usize;
            let mut stack = vec![(sx, sy)];
            label[sy * w + sx] = id;
            while let Some((x, y)) = stack.pop() {
                n += 1;
                let push = |nx: usize, ny: usize, st: &mut Vec<(usize, usize)>, lb: &mut Vec<u32>| {
                    if inside(nx, ny) && lb[ny * w + nx] == 0 {
                        lb[ny * w + nx] = id;
                        st.push((nx, ny));
                    }
                };
                if x > 0 {
                    push(x - 1, y, &mut stack, &mut label);
                }
                if x + 1 < w {
                    push(x + 1, y, &mut stack, &mut label);
                }
                if y > 0 {
                    push(x, y - 1, &mut stack, &mut label);
                }
                if y + 1 < h {
                    push(x, y + 1, &mut stack, &mut label);
                }
            }
            if n > best.0 {
                best = (n, id);
            }
        }
    }
    if best.0 == 0 {
        return None;
    }
    Some(label.iter().map(|&l| l == best.1).collect())
}

/// Whether the blob is a shape rather than a speck or a hairline.
fn big_enough(w: usize, h: usize, blob: &[bool]) -> bool {
    let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0usize, usize::MAX, 0usize);
    let mut n = 0usize;
    for y in 0..h {
        for x in 0..w {
            if blob[y * w + x] {
                n += 1;
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    n >= MIN_BLOB_PX
        && x1.saturating_sub(x0) + 1 >= MIN_BLOB_EXTENT
        && y1.saturating_sub(y0) + 1 >= MIN_BLOB_EXTENT
}

/// The blob's boundary, sampled as a radius per angle about its centroid.
///
/// Radial rather than contour-following, because the destination is a *polar*
/// table: `CustomOutline` stores one radius per `OUTLINE_STEPS` direction, so
/// walking a pixel contour only to resample it into polar is a detour that can
/// also fail — Moore tracing oscillates on a solid region, which is exactly
/// what a filled pen stroke is. A radial sweep cannot oscillate, is O(n) in the
/// samples asked for, and loses nothing the destination could have stored.
fn radial_boundary(w: usize, h: usize, blob: &[bool], samples: usize) -> Vec<(f32, f32)> {
    let filled = |x: i64, y: i64| {
        x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && blob[y as usize * w + x as usize]
    };

    // Centroid of the blob, which for a plan is inside it.
    let (mut sx, mut sy, mut n) = (0f64, 0f64, 0usize);
    for y in 0..h {
        for x in 0..w {
            if blob[y * w + x] {
                sx += x as f64;
                sy += y as f64;
                n += 1;
            }
        }
    }
    if n == 0 {
        return Vec::new();
    }
    let (cx, cy) = (sx / n as f64, sy / n as f64);
    let reach = (w.max(h) as f64) * 1.5;

    let mut out = Vec::with_capacity(samples);
    for i in 0..samples {
        let th = i as f64 / samples as f64 * std::f64::consts::TAU;
        let (s, c) = th.sin_cos();
        // Walk outward and keep the last filled step: the *outer* boundary in
        // this direction, so a concave plan reads its true silhouette rather
        // than stopping at the first gap.
        let mut last: Option<(f64, f64)> = None;
        let mut t = 0.0f64;
        while t <= reach {
            let (px, py) = (cx + c * t, cy + s * t);
            if filled(px.round() as i64, py.round() as i64) {
                last = Some((px, py));
            }
            t += 0.5;
        }
        if let Some((px, py)) = last {
            out.push((px as f32, py as f32));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A filled disc, as a sketch would leave it.
    fn disc(n: usize, r: f32) -> Alpha {
        let mut data = vec![0.0f32; n * n];
        let c = n as f32 * 0.5;
        for y in 0..n {
            for x in 0..n {
                let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
                if (dx * dx + dy * dy).sqrt() <= r {
                    data[y * n + x] = 1.0;
                }
            }
        }
        Alpha::new("disc", n, n, data)
    }

    #[test]
    fn a_drawn_circle_traces_to_a_closed_ring_of_points() {
        let a = disc(96, 30.0);
        let pts = trace(&a, 0.5).expect("traced");
        assert!(pts.len() >= 16, "{} points", pts.len());
        assert!(pts.len() <= MAX_TRACE_POINTS);
        // Every point sits about one radius from the centre.
        for p in &pts {
            let (dx, dy) = (p[0] - 0.5, p[1] - 0.5);
            let r = (dx * dx + dy * dy).sqrt();
            assert!((r - 30.0 / 96.0).abs() < 0.05, "r={r} at {p:?}");
        }
        // Closed: the last point is a neighbour of the first.
        let (a0, b0) = (pts[0], pts[pts.len() - 1]);
        assert!((a0[0] - b0[0]).hypot(a0[1] - b0[1]) < 0.12, "ends do not meet");
    }

    /// The plan is the mark with area behind it, not whatever the scan hits
    /// first — a sketch usually carries strays.
    #[test]
    fn the_largest_mark_wins_over_a_stray_speck() {
        let mut a = disc(96, 24.0);
        // A speck in the corner, scanned before the disc.
        for y in 2..5 {
            for x in 2..5 {
                a.data[y * 96 + x] = 1.0;
            }
        }
        let pts = trace(&a, 0.5).expect("traced");
        let far = pts.iter().any(|p| (p[0] - 0.5).hypot(p[1] - 0.5) > 0.4);
        assert!(!far, "the speck was traced instead of the disc");
    }

    #[test]
    fn a_traced_circle_survives_from_points() {
        let a = disc(128, 40.0);
        let pts = trace(&a, 0.5).expect("traced");
        let o = crate::field::CustomOutline::from_points("drawn", &pts).expect("accepted");
        // Round: the plan's own aspect is close to 1.
        assert!((o.aspect - 1.0).abs() < 0.15, "aspect {}", o.aspect);
    }

    /// A cross is the concave case, and the destination stores one radius per
    /// direction — so the sweep must read the *outer* silhouette rather than
    /// stopping at the first gap it crosses.
    #[test]
    fn a_concave_plan_reads_its_outer_silhouette() {
        let n = 96usize;
        let mut data = vec![0.0f32; n * n];
        for y in 0..n {
            for x in 0..n {
                let bar_x = (36..60).contains(&x);
                let bar_y = (36..60).contains(&y);
                if (bar_x && (18..78).contains(&y)) || (bar_y && (18..78).contains(&x)) {
                    data[y * n + x] = 1.0;
                }
            }
        }
        let a = Alpha::new("cross", n, n, data);
        let pts = trace(&a, 0.5).expect("traced");
        let far = |t: f64| {
            let (s, c) = t.sin_cos();
            pts.iter()
                .map(|p| (p[0] - 0.5) * c + (p[1] - 0.5) * s)
                .fold(f64::MIN, f64::max)
        };
        // The arms reach further than the re-entrant corners between them.
        let arm = far(0.0);
        let corner = far(std::f64::consts::FRAC_PI_4);
        assert!(arm > corner, "arm {arm:.3} should out-reach the corner {corner:.3}");
    }

    #[test]
    fn a_blank_sketch_traces_nothing_rather_than_panicking() {
        let a = Alpha::new("blank", 64, 64, vec![0.0; 64 * 64]);
        assert!(trace(&a, 0.5).is_none());
    }

    #[test]
    fn a_single_speck_is_refused_as_too_small_to_be_a_plan() {
        let mut data = vec![0.0f32; 64 * 64];
        data[32 * 64 + 32] = 1.0;
        let a = Alpha::new("dot", 64, 64, data);
        assert!(trace(&a, 0.5).is_none());
    }

    #[test]
    fn a_tiny_raster_is_refused_rather_than_indexed_out_of_bounds() {
        let a = Alpha::new("tiny", 2, 2, vec![1.0; 4]);
        assert!(trace(&a, 0.5).is_none());
    }
}
