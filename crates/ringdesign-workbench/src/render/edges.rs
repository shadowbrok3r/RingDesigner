//! The crisp edge pass's buffers: every CAD part's B-rep edges and every builder part's creases,
//! each segment a quad the vertex stage widens on screen.
//!
//! The evaluation already places both where the build stood the part — a kernel body is
//! transformed by its seat before it is tessellated and sampled, and a builder's creases are
//! placed with its solid — so the polylines are drawn as they come.
use ringdesign_core::cad::{Attach, Evaluated, EvaluatedComponent};
use ringdesign_core::sketch::Id;

/// Floats per staged edge vertex: the segment's two ends (3 + 3) and the corner (end, side).
pub const EDGE_FLOATS: usize = 8;
/// Floats per lit edge vertex: an edge vertex, its width in points and its colour.
pub const LIT_FLOATS: usize = 13;
/// Vertices per segment: two triangles.
pub const SEGMENT_VERTICES: usize = 6;
/// The vertex stage's width attribute, points.
pub const ATTR_WIDTH: u32 = 3;
/// The vertex stage's colour attribute.
pub const ATTR_COLOR: u32 = 4;
/// A segment's quad, each corner `(end, side)`: `end` 0 at the segment's start and 1 at its
/// finish, `side` −1 and +1 either side of it.
pub const CORNERS: [[f32; 2]; SEGMENT_VERTICES] = [[0.0, -1.0], [1.0, -1.0], [1.0, 1.0], [0.0, -1.0], [1.0, 1.0], [0.0, 1.0]];
/// Segments staged at most; the rest are counted in [`StagedEdges::dropped`].
pub const MAX_SEGMENTS: usize = 1 << 18;
/// An edge stands this many pixels toward the eye, so it wins against the faces it bounds up to about 70° off square.
pub const BIAS_PX: f32 = 3.0;
/// And never less than this, mm: a part's tessellation sags up to its 0.04 mm chord from the edges sampled on its true surface.
pub const BIAS_FLOOR_MM: f32 = 0.05;

/// An edge as a pick names it: the feature and the index of its polyline.
pub type EdgeKey = (Id, u32);

/// One edge's run of staged vertices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeRun {
    pub key: EdgeKey,
    pub first: u32,
    pub count: u32,
}

/// Every drawn edge's quads, and where each edge's run lies in them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StagedEdges {
    pub verts: Vec<f32>,
    pub runs: Vec<EdgeRun>,
    /// Segments left out past [`MAX_SEGMENTS`].
    pub dropped: usize,
}

impl StagedEdges {
    /// Segments staged.
    pub fn segments(&self) -> usize {
        self.verts.len() / (EDGE_FLOATS * SEGMENT_VERTICES)
    }

    /// Bytes the buffer uploads.
    pub fn bytes(&self) -> usize {
        std::mem::size_of_val(self.verts.as_slice())
    }
}

/// How the pass draws: every edge in one colour and width, the chosen and the hovered in their own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeStyle {
    pub color: [f32; 4],
    pub width_pt: f32,
    pub chosen: [f32; 4],
    pub chosen_pt: f32,
    pub hover: [f32; 4],
    pub hover_pt: f32,
}

impl EdgeStyle {
    /// Ink on the metal; the theme's selection violet and the hover cue's aqua at the widths the painter drew them.
    pub const DESKTOP: EdgeStyle = EdgeStyle {
        color: [0.06, 0.06, 0.08, 0.9],
        width_pt: 1.25,
        chosen: [0.80, 0.573, 0.851, 1.0],
        chosen_pt: 2.0,
        hover: [0.169, 0.886, 0.839, 1.0],
        hover_pt: 2.5,
    };
    /// The same colours at the phone's finger-sized widths.
    pub const PHONE: EdgeStyle = EdgeStyle { width_pt: 1.0, chosen_pt: 3.0, hover_pt: 3.0, ..EdgeStyle::DESKTOP };
}

/// Whether the pass draws a part's edges: a reference stone is never metal, and a cut part's edges bound the tool, not the ring.
pub fn drawn(c: &EvaluatedComponent) -> bool {
    !c.settings.reference && c.attach != Attach::Cut
}

/// Every drawn part's edges as quads.
pub fn stage_edges(evaluated: &Evaluated) -> StagedEdges {
    stage_polylines(evaluated.components.iter().filter(|c| drawn(c)).map(|c| (c.id, c.edges.as_slice())))
}

/// Polylines as quads, one run per polyline under its feature and index.
pub fn stage_polylines<'a>(parts: impl IntoIterator<Item = (Id, &'a [Vec<[f64; 3]>])>) -> StagedEdges {
    let mut out = StagedEdges::default();
    let mut segments = 0usize;
    for (id, polylines) in parts {
        for (i, poly) in polylines.iter().enumerate() {
            let first = out.verts.len() / EDGE_FLOATS;
            for w in poly.windows(2) {
                if !usable(w[0], w[1]) {
                    continue;
                }
                if segments == MAX_SEGMENTS {
                    out.dropped += 1;
                    continue;
                }
                segments += 1;
                push_segment(&mut out.verts, w[0], w[1], None);
            }
            let count = out.verts.len() / EDGE_FLOATS - first;
            if count > 0 {
                out.runs.push(EdgeRun { key: (id, i as u32), first: first as u32, count: count as u32 });
            }
        }
    }
    out
}

/// The chosen edges and the hovered one as quads carrying their own width and colour, the hovered last so it reads over a chosen one.
pub fn stage_lit(evaluated: &Evaluated, chosen: &[EdgeKey], hovered: Option<EdgeKey>, style: &EdgeStyle) -> Vec<f32> {
    let poly = |key: &EdgeKey| evaluated.components.iter().find(|c| c.id == key.0).and_then(|c| c.edges.get(key.1 as usize));
    let mut out = Vec::new();
    let lit = chosen.iter().map(|k| (k, style.chosen, style.chosen_pt)).chain(hovered.as_ref().map(|k| (k, style.hover, style.hover_pt)));
    for (key, color, width) in lit {
        for w in poly(key).map_or(&[][..], |p| p.as_slice()).windows(2) {
            if usable(w[0], w[1]) {
                push_segment(&mut out, w[0], w[1], Some((width, color)));
            }
        }
    }
    out
}

/// A segment both of whose ends are finite and apart.
fn usable(a: [f64; 3], b: [f64; 3]) -> bool {
    a.iter().chain(&b).all(|v| v.is_finite()) && a != b
}

/// Six corners of one segment's quad; with `lit`, each carries its width and colour.
fn push_segment(out: &mut Vec<f32>, a: [f64; 3], b: [f64; 3], lit: Option<(f32, [f32; 4])>) {
    let (a, b) = (a.map(|v| v as f32), b.map(|v| v as f32));
    for [end, side] in CORNERS {
        out.extend_from_slice(&[a[0], a[1], a[2], b[0], b[1], b[2], end, side]);
        if let Some((width, color)) = lit {
            out.push(width);
            out.extend_from_slice(&color);
        }
    }
}

/// The world direction toward the eye and the world size of one pixel, read off an orthographic
/// column-major `mvp` and the viewport's size in pixels; `None` for a matrix that sees nothing.
pub fn view_scale(mvp: &[f32; 16], viewport_px: [f32; 2]) -> Option<([f32; 3], f32)> {
    let row = |r: usize| [mvp[r], mvp[4 + r], mvp[8 + r]];
    let (x, y, z) = (row(0), row(1), row(2));
    let len = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let (lx, ly) = (len(x), len(y));
    if lx <= f32::EPSILON || ly <= f32::EPSILON || viewport_px[0] < 1.0 || viewport_px[1] < 1.0 {
        return None;
    }
    // The axis that moves neither screen coordinate is the view's; nearer is smaller depth.
    let axis = [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
    let la = len(axis);
    if la <= f32::EPSILON {
        return None;
    }
    let sign = if axis[0] * z[0] + axis[1] * z[1] + axis[2] * z[2] > 0.0 { -1.0 } else { 1.0 };
    let toward = axis.map(|v| v * sign / la);
    // One pixel is 2 / width of NDC across, and a millimetre is |row| of NDC.
    let px_mm = (2.0 / (viewport_px[0] * lx)).max(2.0 / (viewport_px[1] * ly));
    Some((toward, px_mm))
}

/// How far an edge stands toward the eye at this pixel size, mm.
pub fn bias_mm(px_mm: f32) -> f32 {
    (BIAS_PX * px_mm).max(BIAS_FLOOR_MM)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::cad;

    /// The vertex at `i` of a buffer `stride` floats wide.
    fn vertex(buf: &[f32], stride: usize, i: usize) -> &[f32] {
        &buf[i * stride..(i + 1) * stride]
    }

    #[test]
    fn a_segment_is_six_corners_three_either_side_and_the_runs_name_their_edges() {
        let square = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]];
        let dot = vec![[5.0, 5.0, 5.0]];
        let broken = vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [f64::NAN, 0.0, 0.0], [2.0, 0.0, 1.0], [3.0, 0.0, 1.0]];
        let parts = [square, dot, broken];
        let staged = stage_polylines([(7, &parts[..])]);
        assert_eq!(staged.segments(), 5, "four sides, then one of four in the broken line: a repeat and both halves of the NaN drop");
        assert_eq!(staged.verts.len(), 5 * SEGMENT_VERTICES * EDGE_FLOATS);
        assert_eq!(staged.bytes(), 5 * 6 * 8 * 4);
        assert_eq!(staged.runs, vec![EdgeRun { key: (7, 0), first: 0, count: 24 }, EdgeRun { key: (7, 2), first: 24, count: 6 }], "the one-point polyline has no run");
        let corners: Vec<[f32; 2]> = (0..6).map(|i| { let v = vertex(&staged.verts, EDGE_FLOATS, i); [v[6], v[7]] }).collect();
        assert_eq!(corners, CORNERS.to_vec());
        assert_eq!(corners.iter().filter(|c| c[1] < 0.0).count(), 3);
        assert_eq!(corners.iter().filter(|c| c[0] == 0.0).count(), 3, "three corners at each end");
        for i in 0..6 {
            assert_eq!(&vertex(&staged.verts, EDGE_FLOATS, i)[..6], &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0], "every corner carries both ends");
        }
        let last = vertex(&staged.verts, EDGE_FLOATS, 24);
        assert_eq!(&last[..6], &[2.0, 0.0, 1.0, 3.0, 0.0, 1.0]);
    }

    #[test]
    fn a_huge_polyline_stops_at_the_cap_and_says_how_much_it_left() {
        let line: Vec<[f64; 3]> = (0..=MAX_SEGMENTS + 10).map(|i| [i as f64 * 0.01, 0.0, 0.0]).collect();
        let staged = stage_polylines([(1, std::slice::from_ref(&line))]);
        assert_eq!((staged.segments(), staged.dropped), (MAX_SEGMENTS, 10));
        assert_eq!(staged.runs.len(), 1);
    }

    #[test]
    fn the_claw_solitaire_draws_its_heads_creases_and_neither_its_stone_nor_its_bur() {
        let d = cad::examples::design("claw-solitaire").unwrap();
        let built = ringdesign_core::mesh::try_build(&d, &ringdesign_core::AlphaLibrary::default(), ringdesign_core::mesh::BuildParams { theta_steps: 192, profile_steps: 96, ..Default::default() }).unwrap();
        let e = built.parts.evaluated.as_ref().expect("the parts are evaluated");
        let head = e.components.iter().find(|c| c.name == "Four-claw head").unwrap();
        let (stone, bur) = (e.components.iter().find(|c| c.settings.reference).unwrap(), e.components.iter().find(|c| c.attach == Attach::Cut).unwrap());
        assert!(!stone.edges.is_empty() && !bur.edges.is_empty(), "both have creases to leave out");
        let staged = stage_edges(e);
        let head_segments: usize = head.edges.iter().map(|p| p.windows(2).filter(|w| usable(w[0], w[1])).count()).sum();
        assert_eq!(staged.segments(), head_segments);
        assert!(staged.runs.iter().all(|r| r.key.0 == head.id));
        assert_eq!(staged.dropped, 0);
        // The creases lie on the head as built: every staged end is a vertex of its placed mesh.
        let on_mesh = |p: &[f32]| head.mesh.vertices.iter().any(|v| (v.0 - p[0]).abs() < 1e-4 && (v.1 - p[1]).abs() < 1e-4 && (v.2 - p[2]).abs() < 1e-4);
        for i in (0..staged.verts.len() / EDGE_FLOATS).step_by(SEGMENT_VERTICES * 7) {
            let v = vertex(&staged.verts, EDGE_FLOATS, i);
            assert!(on_mesh(&v[..3]) && on_mesh(&v[3..6]), "segment at vertex {i} is off the head");
        }
        println!("claw head: {} creases, {} segments, {} bytes", staged.runs.len(), staged.segments(), staged.bytes());
    }

    #[test]
    fn the_lit_edges_carry_their_width_and_colour_and_the_hover_draws_last() {
        let d = cad::examples::design("claw-solitaire").unwrap();
        let built = ringdesign_core::mesh::try_build(&d, &ringdesign_core::AlphaLibrary::default(), ringdesign_core::mesh::BuildParams { theta_steps: 192, profile_steps: 96, ..Default::default() }).unwrap();
        let e = built.parts.evaluated.as_ref().unwrap();
        let head = e.components.iter().find(|c| c.name == "Four-claw head").unwrap();
        let segs = |i: usize| head.edges[i].windows(2).filter(|w| usable(w[0], w[1])).count();
        let style = EdgeStyle::DESKTOP;
        let lit = stage_lit(e, &[(head.id, 0), (head.id, 1)], Some((head.id, 2)), &style);
        let n = segs(0) + segs(1) + segs(2);
        assert_eq!(lit.len(), n * SEGMENT_VERTICES * LIT_FLOATS);
        let widths: Vec<f32> = (0..n * SEGMENT_VERTICES).map(|i| vertex(&lit, LIT_FLOATS, i)[8]).collect();
        let chosen = (segs(0) + segs(1)) * SEGMENT_VERTICES;
        assert!(widths[..chosen].iter().all(|w| *w == style.chosen_pt) && widths[chosen..].iter().all(|w| *w == style.hover_pt));
        assert_eq!(&vertex(&lit, LIT_FLOATS, 0)[9..], &style.chosen);
        assert_eq!(&vertex(&lit, LIT_FLOATS, chosen)[9..], &style.hover);
        assert!(stage_lit(e, &[(head.id, 99_999), (4242, 0)], None, &style).is_empty(), "an edge that is not there lights nothing");
        assert_eq!(EdgeStyle::PHONE.chosen_pt, 3.0);
    }

    #[test]
    fn the_bias_leans_toward_the_eye_by_three_pixels_of_the_view() {
        // An orthographic view down −z of a 40 × 20 mm window on an 800 × 400 px viewport: 0.05 mm a pixel.
        let (hw, hh, far) = (20.0f32, 10.0f32, 100.0f32);
        let mut mvp = [0.0f32; 16];
        mvp[0] = 1.0 / hw;
        mvp[5] = 1.0 / hh;
        mvp[10] = -1.0 / far;
        mvp[15] = 1.0;
        let (toward, px_mm) = view_scale(&mvp, [800.0, 400.0]).unwrap();
        assert!((px_mm - 0.05).abs() < 1e-6, "{px_mm}");
        assert!((toward[0]).abs() < 1e-6 && (toward[1]).abs() < 1e-6 && (toward[2] - 1.0).abs() < 1e-6, "{toward:?}");
        // Lifting a point toward the eye makes it nearer, which is a smaller depth.
        let depth = |p: [f32; 3]| mvp[2] * p[0] + mvp[6] * p[1] + mvp[10] * p[2] + mvp[14];
        let lifted = [toward[0] * bias_mm(px_mm), toward[1] * bias_mm(px_mm), toward[2] * bias_mm(px_mm)];
        assert!(depth(lifted) < depth([0.0; 3]));
        assert!((bias_mm(px_mm) - 0.15).abs() < 1e-6);
        assert_eq!(bias_mm(0.001), BIAS_FLOOR_MM, "zoomed in, the chord's sag decides");
        assert_eq!(view_scale(&[0.0; 16], [800.0, 400.0]), None);
    }
}
