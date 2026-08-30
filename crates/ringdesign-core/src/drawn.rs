//! Hand-drawn alphas, stored as strokes rather than pixels.
//!
//! A painted motif could be baked to an `Alpha` and put in the library under a name, the way an
//! imported PNG is. That is the wrong trade for a design that has to travel: layers reference
//! alphas *by name* and nothing embeds them, so a `.ring.json` carrying a painted layer would
//! arrive somewhere else pointing at an alpha that is not there — and a missing alpha contributes
//! nothing, silently, with no error.
//!
//! So the strokes live in the design. That also makes the raster a rendering decision rather than a
//! stored property: the same [`DrawnAlpha`] rasterizes at 256 for a preview and 1024 for an export,
//! and stays editable — undo is dropping the last stroke and rasterizing again.
//!
//! Coordinates are normalized 0..1 over the alpha, so a drawing is independent of the resolution it
//! was made at.

use serde::{Deserialize, Serialize};

use crate::alpha::Alpha;

/// Widest raster a drawn alpha is built at. Higher than `alpha::MAX_ALPHA_EDGE`, which is a policy
/// for *decoded images* — one f32 per pixel means 4096x1024 is 16 MB, and a band-wide drawing needs
/// the width: 512 px across a size-7 circumference is 0.13 mm per pixel, about the resolution the
/// mesh itself has, so pen detail finer than that would quantize away.
pub const MAX_DRAWN_EDGE: u32 = 4096;
/// Ceiling on stroke count per alpha, so a runaway input loop cannot grow the design without bound.
pub const MAX_STROKES: usize = 4096;
/// Ceiling on points in one stroke.
pub const MAX_STROKE_POINTS: usize = 8192;

/// One continuous press of the pen.
///
/// A point list rather than one record per segment: a freehand line is hundreds of segments, and
/// storing each as its own record with its own radius and flags bloats the design file for nothing.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stroke {
    /// `[x, y, pressure]`, all 0..1. `x` runs along `u`, `y` across `v`.
    pub points: Vec<[f32; 3]>,
    /// Brush radius as a fraction of the alpha's width.
    pub radius: f32,
    /// Feather, 0 = hard edge, 1 = the whole radius fades.
    pub soft: f32,
    /// Cut into what is under it rather than adding to it.
    pub erase: bool,
    /// `[tilt, azimuth]` in radians, parallel to `points` — how the pen was
    /// held at each sample.
    ///
    /// A graver's cut section is set by how the tool is held, so tilt shapes the
    /// stamp into an ellipse: upright cuts a round bead, laid over cuts a long
    /// flat facet. Pressure still means depth; tilt shapes the tool, it does not
    /// change how deep it goes.
    ///
    /// `#[serde(default)]` and read positionally, so a design written before
    /// this existed loads with an empty array and rasterises exactly as it did.
    #[serde(default)]
    pub tilt: Vec<[f32; 2]>,
}

impl Stroke {
    pub fn new(radius: f32, soft: f32, erase: bool) -> Self {
        Self { points: Vec::new(), radius, soft, erase, tilt: Vec::new() }
    }

    /// Append a sample, dropping ones too close to the last to matter.
    pub fn push(&mut self, x: f32, y: f32, pressure: f32) {
        if self.points.len() >= MAX_STROKE_POINTS {
            return;
        }
        let p = [x, y, pressure.clamp(0.0, 1.0)];
        if let Some(last) = self.points.last() {
            let step = (self.radius * 0.25).max(1e-4);
            if (last[0] - x).abs() < step && (last[1] - y).abs() < step {
                return;
            }
        }
        self.points.push(p);
    }

    /// Append a sample and how the pen was held for it.
    ///
    /// The two arrays are kept the same length by construction: `push` may drop
    /// a sample as too close to the last, and a tilt written unconditionally
    /// would then slip out of step with the points it describes.
    pub fn push_held(&mut self, x: f32, y: f32, pressure: f32, tilt: f32, azimuth: f32) {
        let before = self.points.len();
        self.push(x, y, pressure);
        if self.points.len() > before {
            self.tilt.resize(self.points.len() - 1, [0.0, 0.0]);
            self.tilt.push([tilt, azimuth]);
        }
    }

    /// How the pen was held at sample `i`; upright when nothing was recorded.
    pub fn held_at(&self, i: usize) -> [f32; 2] {
        self.tilt.get(i).copied().unwrap_or([0.0, 0.0])
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

/// The signed angle from `a` to `b`, taking the short way round.
///
/// Azimuth wraps, so interpolating 350° to 10° as plain numbers spins the tool
/// backwards through nearly a full turn in the middle of one stroke.
fn shortest_turn(a: f32, b: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let d = (b - a) % tau;
    if d > tau * 0.5 {
        d - tau
    } else if d < -tau * 0.5 {
        d + tau
    } else {
        d
    }
}

/// A named alpha defined by the strokes that drew it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DrawnAlpha {
    pub name: String,
    /// Raster the design builds this at. Clamped by [`MAX_DRAWN_EDGE`].
    pub width: u32,
    pub height: u32,
    /// Wrap the brush in `x`, for a motif that has to meet itself around the ring.
    #[serde(default)]
    pub wrap_x: bool,
    /// Wrap in `y` too, which is what a tile needs to be seamless in both axes.
    #[serde(default)]
    pub wrap_y: bool,
    pub strokes: Vec<Stroke>,
}

impl DrawnAlpha {
    pub fn new(name: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            name: name.into(),
            width: width.clamp(8, MAX_DRAWN_EDGE),
            height: height.clamp(8, MAX_DRAWN_EDGE),
            wrap_x: false,
            wrap_y: false,
            strokes: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.strokes.iter().all(Stroke::is_empty)
    }

    /// Rasterize at the stored size.
    pub fn rasterize(&self) -> Alpha {
        self.rasterize_at(self.width, self.height)
    }

    /// Rasterize at an arbitrary size — the whole point of storing strokes.
    pub fn rasterize_at(&self, width: u32, height: u32) -> Alpha {
        let w = width.clamp(8, MAX_DRAWN_EDGE) as usize;
        let h = height.clamp(8, MAX_DRAWN_EDGE) as usize;
        let mut buf = vec![0.0f32; w * h];
        for stroke in self.strokes.iter().take(MAX_STROKES) {
            self.draw_stroke(&mut buf, w, h, stroke);
        }
        Alpha::new(self.name.clone(), w, h, buf)
    }

    fn draw_stroke(&self, buf: &mut [f32], w: usize, h: usize, stroke: &Stroke) {
        let r = stroke.radius.max(1e-4);
        match stroke.points.len() {
            0 => {}
            1 => {
                let p = stroke.points[0];
                let [tilt, az] = stroke.held_at(0);
                self.stamp(buf, w, h, p[0], p[1], r, stroke.soft, p[2], stroke.erase, tilt, az);
            }
            _ => {
                for (i, pair) in stroke.points.windows(2).enumerate() {
                    let (a, b) = (pair[0], pair[1]);
                    let ([ta, aa], [tb, ab]) = (stroke.held_at(i), stroke.held_at(i + 1));
                    // Stamp along the segment at ~radius/2 so discs overlap into a line.
                    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                    let len = (dx * dx + dy * dy).sqrt();
                    let steps = ((len / (r * 0.5).max(1e-4)).ceil() as usize).clamp(1, 4096);
                    for i in 0..=steps {
                        let t = i as f32 / steps as f32;
                        self.stamp(
                            buf,
                            w,
                            h,
                            a[0] + dx * t,
                            a[1] + dy * t,
                            r,
                            stroke.soft,
                            a[2] + (b[2] - a[2]) * t,
                            stroke.erase,
                            ta + (tb - ta) * t,
                            // Azimuth is an angle: interpolating 350° to 10°
                            // the short way keeps the tool from spinning
                            // backwards through the stroke.
                            aa + shortest_turn(aa, ab) * t,
                        );
                    }
                }
            }
        }
    }

    /// One feathered stamp. Paint takes the max toward `value`, erase takes the min toward 0, so
    /// overlapping passes of the same stroke do not accumulate into a ridge.
    ///
    /// `tilt` and `azimuth` shape it into an ellipse: the minor axis shortens as
    /// `cos(tilt)`, so an upright pen (tilt 0) is exactly the round disc this
    /// used to be — bit for bit, because the rotation and the scale both fall
    /// out to the identity — and a pen laid at 60° cuts a footprint twice as
    /// long as it is wide, along the direction it leans.
    #[allow(clippy::too_many_arguments)]
    fn stamp(
        &self,
        buf: &mut [f32],
        w: usize,
        h: usize,
        cx: f32,
        cy: f32,
        r: f32,
        soft: f32,
        value: f32,
        erase: bool,
        tilt: f32,
        azimuth: f32,
    ) {
        let value = value.clamp(0.0, 1.0);
        if value <= 0.0 && !erase {
            return;
        }
        let inner = r * (1.0 - soft.clamp(0.0, 1.0));
        let (wf, hf) = (w as f32, h as f32);
        // Aspect: the brush is round on screen, and the alpha may not be square.
        let rx = r;
        let ry = r * wf / hf.max(1.0);

        // The tool's own shape: the contact patch lengthens along the lean while
        // its width across is unchanged, so the major semi-axis is `r/cos(tilt)`
        // and the minor stays `r`. That makes the eccentricity exactly
        // `sin(tilt)`. Clamped short of 90° so a pen laid flat on the glass
        // still leaves a bounded mark rather than an infinite streak.
        let squash = tilt.clamp(0.0, 1.4).cos().max(0.15);
        let (sa, ca) = if tilt > 1e-4 { azimuth.sin_cos() } else { (0.0, 1.0) };

        // The rotated ellipse's bounding box, so a laid-over stamp is not
        // clipped by the round brush's extent.
        let ext = 1.0 / squash;
        let x0 = ((cx - rx * ext) * wf).floor() as i64;
        let x1 = ((cx + rx * ext) * wf).ceil() as i64;
        let y0 = ((cy - ry * ext) * hf).floor() as i64;
        let y1 = ((cy + ry * ext) * hf).ceil() as i64;

        for py in y0..=y1 {
            let (iy, v) = match wrap_or_clamp(py, h, self.wrap_y) {
                Some(t) => t,
                None => continue,
            };
            for px in x0..=x1 {
                let (ix, u) = match wrap_or_clamp(px, w, self.wrap_x) {
                    Some(t) => t,
                    None => continue,
                };
                let dx = (u - cx) / rx.max(1e-6);
                let dy = (v - cy) / ry.max(1e-6);
                // Into the tool's frame: along the lean, then across it. At
                // tilt 0 this is the identity and `d` is the old radius.
                let along = (dx * ca + dy * sa) * squash;
                let across = -dx * sa + dy * ca;
                let d = (along * along + across * across).sqrt();
                if d > 1.0 {
                    continue;
                }
                let falloff = if inner >= r || d <= inner / r {
                    1.0
                } else {
                    let t = (1.0 - d) / (1.0 - inner / r).max(1e-6);
                    t.clamp(0.0, 1.0)
                };
                let slot = &mut buf[iy * w + ix];
                if erase {
                    *slot = slot.min(1.0 - falloff);
                } else {
                    *slot = slot.max(value * falloff);
                }
            }
        }
    }
}

/// Map a pixel index to `(index, centre in 0..1)`, wrapping or clamping out-of-range.
/// `None` when it falls outside and wrapping is off.
fn wrap_or_clamp(p: i64, n: usize, wrap: bool) -> Option<(usize, f32)> {
    let n_i = n as i64;
    let idx = if wrap {
        p.rem_euclid(n_i)
    } else if p < 0 || p >= n_i {
        return None;
    } else {
        p
    };
    // The coordinate keeps the *unwrapped* position so a brush straddling the seam stays round.
    Some((idx as usize, (p as f32 + 0.5) / n as f32))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dot(wrap_x: bool) -> DrawnAlpha {
        let mut d = DrawnAlpha::new("dot", 64, 64);
        d.wrap_x = wrap_x;
        let mut s = Stroke::new(0.1, 0.5, false);
        s.points.push([0.5, 0.5, 1.0]);
        d.strokes.push(s);
        d
    }

    #[test]
    fn a_stamp_marks_its_centre_and_leaves_the_corner_alone() {
        let a = dot(false).rasterize();
        assert!(a.sample(0.5, 0.5) > 0.9, "centre should be full");
        assert_eq!(a.sample(0.0, 0.0), 0.0, "a corner is far outside the disc");
    }

    #[test]
    fn rasterizing_bigger_gives_the_same_picture() {
        let d = dot(false);
        let small = d.rasterize_at(64, 64);
        let big = d.rasterize_at(256, 256);
        assert_eq!(big.width, 256);
        // Same shape, sampled in normalized space.
        for (x, y) in [(0.5, 0.5), (0.5, 0.42), (0.2, 0.2)] {
            assert!(
                (small.sample(x, y) - big.sample(x, y)).abs() < 0.15,
                "({x},{y}) differs: {} vs {}",
                small.sample(x, y),
                big.sample(x, y)
            );
        }
    }

    #[test]
    fn wrap_x_makes_a_stroke_on_the_seam_meet_itself() {
        let mut d = DrawnAlpha::new("seam", 64, 64);
        d.wrap_x = true;
        let mut s = Stroke::new(0.1, 0.2, false);
        s.points.push([0.0, 0.5, 1.0]);
        d.strokes.push(s);
        let a = d.rasterize();
        // A disc centred on x=0 must appear at both edges, or the motif breaks at the joint.
        assert!(a.sample(0.0, 0.5) > 0.5, "left edge");
        assert!(a.sample(1.0, 0.5) > 0.5, "right edge should carry the other half");
    }

    #[test]
    fn without_wrap_the_same_stroke_stays_on_its_own_side() {
        let mut d = DrawnAlpha::new("noseam", 64, 64);
        let mut s = Stroke::new(0.1, 0.2, false);
        s.points.push([0.0, 0.5, 1.0]);
        d.strokes.push(s);
        let a = d.rasterize();
        assert!(a.sample(0.0, 0.5) > 0.5);
        assert_eq!(a.sample(1.0, 0.5), 0.0);
    }

    #[test]
    fn pressure_scales_the_mark() {
        let mut d = DrawnAlpha::new("soft", 64, 64);
        let mut s = Stroke::new(0.1, 0.0, false);
        s.points.push([0.5, 0.5, 0.25]);
        d.strokes.push(s);
        let a = d.rasterize();
        let v = a.sample(0.5, 0.5);
        assert!((v - 0.25).abs() < 0.02, "light pressure should leave a light mark, got {v}");
    }

    #[test]
    fn erase_cuts_back_what_paint_laid_down() {
        let mut d = DrawnAlpha::new("cut", 64, 64);
        let mut paint = Stroke::new(0.3, 0.0, false);
        paint.points.push([0.5, 0.5, 1.0]);
        let mut cut = Stroke::new(0.1, 0.0, true);
        cut.points.push([0.5, 0.5, 1.0]);
        d.strokes.push(paint);
        d.strokes.push(cut);
        let a = d.rasterize();
        assert!(a.sample(0.5, 0.5) < 0.1, "the eraser should have taken the centre back");
        assert!(a.sample(0.5, 0.72) > 0.5, "and left the ring of paint outside it");
    }

    #[test]
    fn a_dragged_stroke_is_continuous_between_its_points() {
        let mut d = DrawnAlpha::new("line", 128, 128);
        let mut s = Stroke::new(0.05, 0.1, false);
        s.points.push([0.2, 0.5, 1.0]);
        s.points.push([0.8, 0.5, 1.0]);
        d.strokes.push(s);
        let a = d.rasterize();
        for x in [0.25, 0.4, 0.55, 0.7] {
            assert!(a.sample(x, 0.5) > 0.5, "gap at x={x}");
        }
    }

    #[test]
    fn push_drops_samples_too_close_to_matter() {
        let mut s = Stroke::new(0.1, 0.0, false);
        s.push(0.5, 0.5, 1.0);
        s.push(0.5001, 0.5001, 1.0);
        assert_eq!(s.points.len(), 1, "a sub-step jitter sample is not worth storing");
        s.push(0.9, 0.9, 1.0);
        assert_eq!(s.points.len(), 2);
    }

    #[test]
    fn sizes_are_clamped_so_a_loaded_design_cannot_ask_for_gigabytes() {
        let d = DrawnAlpha::new("huge", 100_000, 100_000);
        assert_eq!(d.width, MAX_DRAWN_EDGE);
        assert_eq!(d.height, MAX_DRAWN_EDGE);
    }

    /// Footprint of one stamp, as (width, height) in pixels above a threshold.
    fn footprint(tilt: f32, azimuth: f32) -> (usize, usize) {
        let mut d = DrawnAlpha::new("t", 128, 128);
        let mut st = Stroke::new(0.15, 0.0, false);
        st.push_held(0.5, 0.5, 1.0, tilt, azimuth);
        d.strokes.push(st);
        let a = d.rasterize();
        let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0usize, usize::MAX, 0usize);
        for y in 0..a.height {
            for x in 0..a.width {
                if a.data[y * a.width + x] > 0.5 {
                    x0 = x0.min(x);
                    x1 = x1.max(x);
                    y0 = y0.min(y);
                    y1 = y1.max(y);
                }
            }
        }
        assert!(x0 != usize::MAX, "the stamp left no mark at tilt {tilt}");
        (x1 - x0 + 1, y1 - y0 + 1)
    }

    /// The whole compatibility claim: an upright pen — and every design written
    /// before tilt existed, which reads back as tilt 0 — rasterises exactly as
    /// it did. The rotation and the squash both fall out to the identity.
    #[test]
    fn an_upright_pen_still_cuts_the_round_disc() {
        let (w, h) = footprint(0.0, 0.0);
        assert_eq!(w, h, "a round stamp is as wide as it is tall: {w}x{h}");
    }

    #[test]
    fn a_stroke_without_tilt_data_rasterizes_identically_to_one_with_zeroes() {
        let mut bare = DrawnAlpha::new("t", 96, 96);
        let mut s1 = Stroke::new(0.2, 0.3, false);
        s1.push(0.4, 0.5, 1.0);
        s1.push(0.6, 0.5, 0.8);
        bare.strokes.push(s1);

        let mut held = DrawnAlpha::new("t", 96, 96);
        let mut s2 = Stroke::new(0.2, 0.3, false);
        s2.push_held(0.4, 0.5, 1.0, 0.0, 0.0);
        s2.push_held(0.6, 0.5, 0.8, 0.0, 0.0);
        held.strokes.push(s2);

        assert_eq!(bare.rasterize().data, held.rasterize().data, "bit for bit");
    }

    /// A pen laid over cuts a long flat facet, along the way it leans.
    #[test]
    fn tilt_stretches_the_stamp_along_its_azimuth() {
        // Azimuth 0 leans along +x, so the footprint grows in x.
        let (w0, h0) = footprint(0.0, 0.0);
        let (w, h) = footprint(1.0, 0.0);
        assert!(w > w0, "laid over, it should reach further along: {w} vs {w0}");
        assert!(
            (h as i64 - h0 as i64).abs() <= 2,
            "and not much further across: {h} vs {h0}"
        );

        // Turned a quarter, the same lean stretches across instead.
        let (wq, hq) = footprint(1.0, std::f32::consts::FRAC_PI_2);
        assert!(hq > h0, "azimuth rotates the major axis: {hq} vs {h0}");
        assert!(wq < w, "and it is no longer the long way in x: {wq} vs {w}");
    }

    /// Tilt shapes the tool; it must not quietly deepen the cut.
    #[test]
    fn tilt_does_not_change_the_peak_value() {
        let mut round = DrawnAlpha::new("t", 96, 96);
        let mut a = Stroke::new(0.15, 0.0, false);
        a.push_held(0.5, 0.5, 1.0, 0.0, 0.0);
        round.strokes.push(a);

        let mut laid = DrawnAlpha::new("t", 96, 96);
        let mut b = Stroke::new(0.15, 0.0, false);
        b.push_held(0.5, 0.5, 1.0, 1.0, 0.0);
        laid.strokes.push(b);

        let peak = |d: &DrawnAlpha| d.rasterize().data.iter().cloned().fold(0.0f32, f32::max);
        assert!((peak(&round) - peak(&laid)).abs() < 1e-6);
    }

    #[test]
    fn push_held_keeps_the_two_arrays_in_step() {
        let mut s = Stroke::new(0.1, 0.0, false);
        s.push_held(0.5, 0.5, 1.0, 0.3, 0.1);
        // Dropped as too close — and its tilt must be dropped with it.
        s.push_held(0.5001, 0.5001, 1.0, 0.9, 0.9);
        assert_eq!(s.points.len(), s.tilt.len(), "a slipped array mis-shapes every later stamp");
        assert_eq!(s.points.len(), 1);
        assert_eq!(s.held_at(0), [0.3, 0.1]);
    }

    #[test]
    fn azimuth_interpolation_takes_the_short_way_round() {
        let tau = std::f32::consts::TAU;
        assert!((shortest_turn(0.1, tau - 0.1) + 0.2).abs() < 1e-5, "backwards, not forwards");
        assert!((shortest_turn(tau - 0.1, 0.1) - 0.2).abs() < 1e-5);
        assert!((shortest_turn(0.0, 1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn an_empty_drawing_rasterizes_to_nothing_rather_than_failing() {
        let d = DrawnAlpha::new("blank", 32, 32);
        assert!(d.is_empty());
        let a = d.rasterize();
        assert_eq!(a.width, 32);
        assert!(a.data.iter().all(|&v| v == 0.0));
    }
}
