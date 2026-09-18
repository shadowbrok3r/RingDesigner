//! Rasterize metal intervals along the pull. Every upper-half metal interval
//! must extend continuously to the parting plane; likewise below it. A sand
//! interval followed by metal in the withdrawal direction is an obstruction.
//! This detects finite obstructions independently of their surface-area share.

use super::{BoreStrategy, Setup};
use crate::mesh::{Mesh, cross, norm, sub};
use serde::Serialize;

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn unit(v: [f64; 3]) -> [f64; 3] {
    let n = norm(v);
    v.map(|x| x / n)
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Frame {
    pub x: [f64; 3],
    pub y: [f64; 3],
    pub z: [f64; 3],
}
impl Frame {
    pub fn new(pull: [f64; 3]) -> anyhow::Result<Self> {
        anyhow::ensure!(
            pull.iter().all(|v| v.is_finite() && v.abs() < 1e6) && norm(pull) > 1e-9,
            "Pull direction must be a finite nonzero vector"
        );
        let z = unit(pull);
        let seed = if z[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let y = unit(cross(z, seed));
        Ok(Self {
            x: unit(cross(y, z)),
            y,
            z,
        })
    }
    pub fn project(&self, p: [f64; 3]) -> [f64; 3] {
        [dot(p, self.x), dot(p, self.y), dot(p, self.z)]
    }
    pub fn world(&self, p: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|i| self.x[i] * p[0] + self.y[i] * p[1] + self.z[i] * p[2])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Status {
    Clear,
    Review,
    Blocked,
    Invalid,
    NotApplicable,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Clear => "No sampled obstruction",
            Self::Review => "Review needed",
            Self::Blocked => "Pattern is obstructed",
            Self::Invalid => "Invalid geometry",
            Self::NotApplicable => "Pull not required",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Half {
    Cope,
    Drag,
}
impl Half {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cope => "Upper mold",
            Self::Drag => "Lower mold",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Obstruction {
    pub half: Half,
    pub point: [f64; 3],
    pub world: [f64; 3],
    pub depth_mm: f64,
    pub projected_area_mm2: f64,
    pub samples: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct SandFinding {
    pub point: [f64; 3],
    pub width_mm: f64,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct Column {
    pub x: f64,
    pub y: f64,
    pub intervals: Vec<[f64; 2]>,
    pub upper_block_mm: f64,
    pub lower_block_mm: f64,
    pub unresolved: bool,
}
impl Column {
    pub fn metal_at(&self, z: f64) -> bool {
        self.intervals.iter().any(|[a, b]| z >= *a && z <= *b)
    }
    pub fn top(&self, parting: f64) -> f64 {
        self.intervals.last().map_or(parting, |v| v[1].max(parting))
    }
    pub fn bottom(&self, parting: f64) -> f64 {
        self.intervals
            .first()
            .map_or(parting, |v| v[0].min(parting))
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ReleaseReport {
    pub status: Status,
    pub frame: Frame,
    pub parting_mm: f64,
    pub suggested_parting_mm: f64,
    pub bounds: [[f64; 3]; 2],
    pub grid: [usize; 2],
    pub cell_mm: [f64; 2],
    pub tolerance_mm: f64,
    pub unresolved_rays: usize,
    pub occupied_rays: usize,
    pub obstructions: Vec<Obstruction>,
    pub sand_findings: Vec<SandFinding>,
    pub worst_draft_deg: f64,
    pub low_draft_area_mm2: f64,
    pub fits_flask: bool,
    pub notes: Vec<String>,
    #[serde(skip)]
    pub columns: Vec<Column>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Orientation {
    pub pull: [f64; 3],
    pub parting_mm: f64,
    pub status: Status,
    pub obstructions: usize,
    pub blocked_area_mm2: f64,
    pub low_draft_area_mm2: f64,
    pub fits_flask: bool,
}

/// Compare axis-aligned withdrawals on one already compensated pattern.
/// Results are suggestions; changing a setup remains an explicit user action.
pub fn compare_orientations(mesh: &Mesh, setup: &Setup) -> anyhow::Result<Vec<Orientation>> {
    let mut candidates = Vec::new();
    for pull in [
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
    ] {
        let mut candidate = setup.clone();
        candidate.pull = pull;
        candidate.auto_parting = true;
        let r = analyze(mesh, &candidate)?;
        candidates.push(Orientation {
            pull,
            parting_mm: r.parting_mm,
            status: r.status,
            obstructions: r.obstructions.len(),
            blocked_area_mm2: r.obstructions.iter().map(|o| o.projected_area_mm2).sum(),
            low_draft_area_mm2: r.low_draft_area_mm2,
            fits_flask: r.fits_flask,
        });
    }
    candidates.sort_by(|a, b| {
        (!a.fits_flask)
            .cmp(&(!b.fits_flask))
            .then(a.blocked_area_mm2.total_cmp(&b.blocked_area_mm2))
            .then(a.low_draft_area_mm2.total_cmp(&b.low_draft_area_mm2))
    });
    Ok(candidates)
}

/// The depth of the largest trapped sand interval on either side of the split.
fn blocked(intervals: &[[f64; 2]], p: f64, tol: f64) -> [f64; 2] {
    let (mut upper, mut lower) = (0.0_f64, 0.0_f64);
    let mut cursor = p;
    for &[a, b] in intervals.iter().filter(|v| v[1] > p + tol) {
        if a > cursor + tol {
            upper = upper.max(a - cursor);
        }
        cursor = cursor.max(b);
    }
    cursor = p;
    for &[a, b] in intervals.iter().rev().filter(|v| v[0] < p - tol) {
        if b < cursor - tol {
            lower = lower.max(cursor - b);
        }
        cursor = cursor.min(a);
    }
    [upper, lower]
}

pub fn analyze(mesh: &Mesh, setup: &Setup) -> anyhow::Result<ReleaseReport> {
    setup.validate()?;
    let frame = Frame::new(setup.pull)?;
    let mut out = ReleaseReport {
        status: Status::Invalid,
        frame,
        parting_mm: setup.parting_mm,
        suggested_parting_mm: setup.parting_mm,
        bounds: [[0.0; 3]; 2],
        grid: [0, 0],
        cell_mm: [0.0, 0.0],
        tolerance_mm: setup.tolerance_mm,
        unresolved_rays: 0,
        occupied_rays: 0,
        obstructions: Vec::new(),
        sand_findings: Vec::new(),
        worst_draft_deg: 0.0,
        low_draft_area_mm2: 0.0,
        fits_flask: false,
        notes: Vec::new(),
        columns: Vec::new(),
    };
    if mesh.faces.is_empty()
        || mesh.vertices.iter().any(|v| !v.is_finite())
        || mesh
            .faces
            .iter()
            .any(|f| f.iter().any(|&i| i as usize >= mesh.vertices.len()))
        || !mesh.validate().watertight
        || mesh.volume_mm3() <= 1e-9
    {
        out.notes
            .push("A nonempty, finite, closed mesh with positive volume is required".into());
        return Ok(out);
    }
    let vertices: Vec<_> = mesh
        .vertices
        .iter()
        .map(|p| frame.project([p.0 as f64, p.1 as f64, p.2 as f64]))
        .collect();
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in &vertices {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    out.bounds = [lo, hi];
    let nx = ((hi[0] - lo[0]) / setup.sample_pitch_mm)
        .ceil()
        .clamp(4.0, 384.0) as usize;
    let ny = ((hi[1] - lo[1]) / setup.sample_pitch_mm)
        .ceil()
        .clamp(4.0, 384.0) as usize;
    let dx = (hi[0] - lo[0]) / nx as f64;
    let dy = (hi[1] - lo[1]) / ny as f64;
    if dx <= 1e-9 || dy <= 1e-9 {
        out.notes.push("Degenerate projection".into());
        return Ok(out);
    }
    out.grid = [nx, ny];
    out.cell_mm = [dx, dy];
    let mut hits: Vec<Vec<f64>> = vec![Vec::new(); nx * ny];
    // Rasterizing each triangle's projected bounding rectangle avoids one
    // ray/triangle traversal per grid cell. No dependency on the ring's chart.
    for f in &mesh.faces {
        let [a, b, c] = [
            vertices[f[0] as usize],
            vertices[f[1] as usize],
            vertices[f[2] as usize],
        ];
        let det = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
        if det.abs() < 1e-14 {
            continue;
        }
        let imin = (((a[0].min(b[0]).min(c[0]) - lo[0]) / dx - 0.5).ceil() as isize)
            .clamp(0, nx as isize - 1) as usize;
        let imax = (((a[0].max(b[0]).max(c[0]) - lo[0]) / dx - 0.5).floor() as isize)
            .clamp(0, nx as isize - 1) as usize;
        let jmin = (((a[1].min(b[1]).min(c[1]) - lo[1]) / dy - 0.5).ceil() as isize)
            .clamp(0, ny as isize - 1) as usize;
        let jmax = (((a[1].max(b[1]).max(c[1]) - lo[1]) / dy - 0.5).floor() as isize)
            .clamp(0, ny as isize - 1) as usize;
        for j in jmin..=jmax {
            for i in imin..=imax {
                let x = lo[0] + (i as f64 + 0.5) * dx;
                let y = lo[1] + (j as f64 + 0.5) * dy;
                let u = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / det;
                let v = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / det;
                let w = 1.0 - u - v;
                if u >= -1e-9 && v >= -1e-9 && w >= -1e-9 {
                    hits[j * nx + i].push(u * a[2] + v * b[2] + w * c[2]);
                }
            }
        }
    }
    out.columns = hits
        .into_iter()
        .enumerate()
        .map(|(k, mut hits)| {
            hits.sort_by(f64::total_cmp);
            // Merge duplicate hits from triangles sharing an edge, not separate
            // surfaces within the user tolerance: thin metal must stay measurable.
            hits.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
            let unresolved = hits.len() % 2 != 0;
            let intervals = if unresolved {
                Vec::new()
            } else {
                hits.chunks_exact(2).map(|h| [h[0], h[1]]).collect()
            };
            Column {
                x: lo[0] + (k % nx) as f64 * dx + dx * 0.5,
                y: lo[1] + (k / nx) as f64 * dy + dy * 0.5,
                intervals,
                unresolved,
                ..Default::default()
            }
        })
        .collect();
    out.unresolved_rays = out.columns.iter().filter(|c| c.unresolved).count();
    out.occupied_rays = out
        .columns
        .iter()
        .filter(|c| !c.intervals.is_empty())
        .count();
    if out.occupied_rays == 0 {
        out.notes
            .push("The sampling grid missed the geometry; decrease sample pitch".into());
        return Ok(out);
    }
    let mid = (lo[2] + hi[2]) * 0.5;
    let mut best = (f64::INFINITY, mid);
    for k in 0..=64 {
        let p = lo[2] + (hi[2] - lo[2]) * k as f64 / 64.0;
        let score = out
            .columns
            .iter()
            .map(|c| {
                blocked(&c.intervals, p, setup.tolerance_mm)
                    .iter()
                    .sum::<f64>()
            })
            .sum::<f64>();
        if score < best.0 - 1e-9
            || ((score - best.0).abs() < 1e-9 && (p - mid).abs() < (best.1 - mid).abs())
        {
            best = (score, p);
        }
    }
    out.suggested_parting_mm = best.1;
    let p = if setup.auto_parting {
        best.1
    } else {
        setup.parting_mm
    };
    out.parting_mm = p;
    for col in &mut out.columns {
        let b = blocked(&col.intervals, p, setup.tolerance_mm);
        col.upper_block_mm = b[0];
        col.lower_block_mm = b[1];
    }
    for half in [Half::Cope, Half::Drag] {
        let depth = |c: &Column| {
            if half == Half::Cope {
                c.upper_block_mm
            } else {
                c.lower_block_mm
            }
        };
        let mut visited = vec![false; nx * ny];
        for seed in 0..nx * ny {
            if visited[seed] || depth(&out.columns[seed]) == 0.0 {
                continue;
            }
            let mut queue = vec![seed];
            visited[seed] = true;
            let mut count = 0;
            let mut worst = seed;
            while let Some(k) = queue.pop() {
                count += 1;
                if depth(&out.columns[k]) > depth(&out.columns[worst]) {
                    worst = k;
                }
                let (x, y) = (k % nx, k / nx);
                for (xx, yy) in [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ] {
                    if xx < nx && yy < ny {
                        let q = yy * nx + xx;
                        if !visited[q] && depth(&out.columns[q]) > 0.0 {
                            visited[q] = true;
                            queue.push(q);
                        }
                    }
                }
            }
            let c = &out.columns[worst];
            let z = if half == Half::Cope {
                c.intervals.last().map_or(p, |v| v[0])
            } else {
                c.intervals.first().map_or(p, |v| v[1])
            };
            let point = [c.x, c.y, z];
            out.obstructions.push(Obstruction {
                half,
                point,
                world: frame.world(point),
                depth_mm: depth(c),
                projected_area_mm2: count as f64 * dx * dy,
                samples: count,
            });
        }
    }
    out.obstructions
        .sort_by(|a, b| b.depth_mm.total_cmp(&a.depth_mm));
    // Local draft is a separate risk check. A small area is never used to
    // discard a confirmed obstruction; coarse normals alone do not confirm it.
    let mut negative = false;
    for f in &mesh.faces {
        let [a, b, c] = [
            vertices[f[0] as usize],
            vertices[f[1] as usize],
            vertices[f[2] as usize],
        ];
        let normal = cross(sub(b, a), sub(c, a));
        let len = norm(normal);
        if len <= 1e-12 {
            continue;
        }
        let cz = (a[2] + b[2] + c[2]) / 3.0;
        if (cz - p).abs() <= setup.tolerance_mm {
            continue;
        }
        let angle = ((normal[2] / len) * if cz > p { 1.0 } else { -1.0 })
            .clamp(-1.0, 1.0)
            .asin()
            .to_degrees();
        out.worst_draft_deg = out.worst_draft_deg.min(angle);
        if angle < setup.recipe.min_draft_deg {
            out.low_draft_area_mm2 += len * 0.5;
        }
        if angle < -2.5 {
            negative = true;
        }
    }
    // Inspect sand slots in slices parallel to the parting plane. This is a
    // width warning, not a material-strength prediction or a connectivity proof.
    for z in [p, (p + hi[2]) * 0.5, (p + lo[2]) * 0.5] {
        for axis in 0..2 {
            let (lines, steps, pitch) = if axis == 0 {
                (ny, nx, dx)
            } else {
                (nx, ny, dy)
            };
            for line in 0..lines {
                let idx = |s: usize| {
                    if axis == 0 {
                        line * nx + s
                    } else {
                        s * nx + line
                    }
                };
                let mut start = None;
                let mut previous_metal = false;
                for step in 0..steps {
                    let metal = out.columns[idx(step)].metal_at(z);
                    if !metal && previous_metal {
                        start = Some(step);
                    }
                    if metal {
                        if let Some(a) = start.take() {
                            let width = (step - a) as f64 * pitch;
                            if width < setup.recipe.min_sand_web_mm && out.sand_findings.len() < 128
                            {
                                let col = &out.columns[idx((a + step) / 2)];
                                if !out.sand_findings.iter().any(|f| {
                                    (f.point[0] - col.x).hypot(f.point[1] - col.y)
                                        < setup.recipe.min_sand_web_mm
                                }) {
                                    out.sand_findings.push(SandFinding {point:[col.x,col.y,z],width_mm:width,message:format!("Sand slot about {width:.2} mm wide; inspect its support and depth")});
                                }
                            }
                        }
                    }
                    previous_metal = metal;
                }
            }
        }
    }
    let m = setup.recipe.sand_margin_mm;
    let f = &setup.flask;
    out.fits_flask = lo[0] >= -f.width_mm * 0.5 + m
        && hi[0] <= f.width_mm * 0.5 - m
        && lo[1] >= -f.length_mm * 0.5 + m
        && hi[1] <= f.length_mm * 0.5 - m
        && hi[2] - p + m <= f.cope_mm
        && p - lo[2] + m <= f.drag_mm
        && p >= lo[2]
        && p <= hi[2];
    if !out.fits_flask {
        out.notes
            .push("Pattern or sand margin does not fit the flask at this parting plane".into());
    }
    for c in &setup.channels {
        let r = c.diameter_mm * 0.5;
        if [c.start, c.end].iter().any(|v| {
            v[0].abs() + r > f.width_mm * 0.5
                || v[1].abs() + r > f.length_mm * 0.5
                || v[2] + r > p + f.cope_mm
                || v[2] - r < p - f.drag_mm
        }) {
            out.notes.push(format!(
                "{} extends outside the flask; confirm its inlet/exit",
                c.kind.label()
            ));
        }
    }
    out.status = if !out.obstructions.is_empty() {
        Status::Blocked
    } else if negative
        || out.unresolved_rays > 0
        || out.low_draft_area_mm2 > 0.01
        || !out.sand_findings.is_empty()
        || !out.fits_flask
    {
        Status::Review
    } else {
        Status::Clear
    };
    if out.low_draft_area_mm2 > 0.01 {
        out.notes.push(format!("{:.2} mm² carries less than {:.1}° draft, including bore walls; review drag and pattern finish",out.low_draft_area_mm2,setup.recipe.min_draft_deg));
    }
    if setup.bore == BoreStrategy::SeparateCore {
        out.notes.push("Separate core selected: core geometry, prints, and removal sequence require review; no bore samples were exempted".into());
        if out.status == Status::Clear {
            out.status = Status::Review;
        }
    }
    if setup.recipe.process == crate::castability::CastProcess::LostWax {
        out.status = Status::NotApplicable;
    }
    out.notes.push(format!("Sample spacing {:.3} × {:.3} mm; obstruction tolerance {:.3} mm. Features between samples may be missed. This checks rigid withdrawal, not sand strength or metal flow.",dx,dy,setup.tolerance_mm));
    if dx > setup.sample_pitch_mm * 1.01 || dy > setup.sample_pitch_mm * 1.01 {
        out.notes
            .push("Grid capped at 384 cells per axis; requested pitch was not reached".into());
    }
    if negative && out.obstructions.is_empty() {
        out.notes.push("Some faces have negative draft without a resolved interval obstruction; inspect at finer sampling".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn box_mesh(lo: [f64; 3], hi: [f64; 3]) -> Mesh {
        let vertices = vec![
            [lo[0], lo[1], lo[2]],
            [hi[0], lo[1], lo[2]],
            [hi[0], hi[1], lo[2]],
            [lo[0], hi[1], lo[2]],
            [lo[0], lo[1], hi[2]],
            [hi[0], lo[1], hi[2]],
            [hi[0], hi[1], hi[2]],
            [lo[0], hi[1], hi[2]],
        ]
        .into_iter()
        .map(|v| crate::Vec3(v[0] as f32, v[1] as f32, v[2] as f32))
        .collect();
        Mesh {
            vertices,
            normals: Vec::new(),
            faces: vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 3, 7],
                [2, 7, 6],
                [3, 0, 4],
                [3, 4, 7],
            ],
            ..Default::default()
        }
    }
    fn setup() -> Setup {
        Setup {
            auto_parting: false,
            ..Default::default()
        }
    }
    #[test]
    fn closed_box_is_clear_of_obstructions_but_parallel_walls_are_reviewed() {
        let r = analyze(&box_mesh([-3.0; 3], [3.0; 3]), &setup()).unwrap();
        assert!(r.obstructions.is_empty());
        assert_eq!(r.status, Status::Review);
        assert!(r.fits_flask);
    }
    #[test]
    fn small_detached_overhang_is_never_diluted_by_total_area() {
        let mut m = box_mesh([-10.0, -10.0, -2.0], [10.0, 10.0, 2.0]);
        let cap = box_mesh([10.1, -0.3, 0.5], [10.8, 0.3, 1.0]);
        let offset = m.vertices.len() as u32;
        m.vertices.extend(cap.vertices);
        m.faces
            .extend(cap.faces.iter().map(|f| f.map(|i| i + offset)));
        let r = analyze(&m, &setup()).unwrap();
        assert_eq!(r.status, Status::Blocked);
        assert!(
            r.obstructions
                .iter()
                .any(|o| o.half == Half::Cope && o.depth_mm >= 0.49)
        );
    }
    #[test]
    fn interval_gaps_block_the_appropriate_half() {
        assert_eq!(blocked(&[[-2.0, 2.0]], 0.0, 0.01), [0.0, 0.0]);
        assert_eq!(blocked(&[[0.5, 2.0]], 0.0, 0.01), [0.5, 0.0]);
        assert_eq!(blocked(&[[-2.0, -0.5]], 0.0, 0.01), [0.0, 0.5]);
        assert_eq!(blocked(&[[-2.0, 0.5], [1.0, 2.0]], 0.0, 0.01), [0.5, 0.0]);
    }
    #[test]
    fn invalid_meshes_and_directions_do_not_pass() {
        assert_eq!(
            analyze(&Mesh::default(), &setup()).unwrap().status,
            Status::Invalid
        );
        let mut s = setup();
        s.pull = [0.0; 3];
        assert!(analyze(&box_mesh([-1.0; 3], [1.0; 3]), &s).is_err());
    }
    #[test]
    fn pull_frames_round_trip_and_flipped_pulls_exchange_halves() {
        let f = Frame::new([1.0, 2.0, 3.0]).unwrap();
        let p = [-4.0, 2.0, 7.0];
        for (a, b) in f.world(f.project(p)).into_iter().zip(p) {
            assert!((a - b).abs() < 1e-10);
        }
        let m = box_mesh([-1.0, -1.0, 0.5], [1.0, 1.0, 2.0]);
        let a = analyze(&m, &setup()).unwrap();
        let mut s = setup();
        s.pull = [0.0, 0.0, -1.0];
        let b = analyze(&m, &s).unwrap();
        assert!(a.obstructions.iter().all(|o| o.half == Half::Cope));
        assert!(b.obstructions.iter().all(|o| o.half == Half::Drag));
    }
    #[test]
    fn automatic_parting_finds_a_plane_inside_offset_geometry() {
        let mut s = setup();
        s.auto_parting = true;
        let r = analyze(&box_mesh([-2.0, -2.0, 4.0], [2.0, 2.0, 6.0]), &s).unwrap();
        assert!((r.parting_mm - 5.0).abs() < 1e-8);
        assert!(r.obstructions.is_empty());
    }
    #[test]
    fn rotating_mesh_and_pull_preserves_release_and_fragile_slots_are_reported() {
        let mut m = box_mesh([-3.0, -2.0, -1.0], [-0.15, 2.0, 1.0]);
        let other = box_mesh([0.15, -2.0, -1.0], [3.0, 2.0, 1.0]);
        let offset = m.vertices.len() as u32;
        m.vertices.extend(other.vertices);
        m.faces
            .extend(other.faces.iter().map(|f| f.map(|v| v + offset)));
        let mut s = setup();
        s.sample_pitch_mm = 0.05;
        let before = analyze(&m, &s).unwrap();
        assert!(before.obstructions.is_empty());
        assert!(before.sand_findings.iter().any(|f| f.width_mm < 0.4));
        let frame = Frame::new([0.3, 0.7, 1.0]).unwrap();
        for p in &mut m.vertices {
            let q = frame.world([p.0 as f64, p.1 as f64, p.2 as f64]);
            *p = crate::Vec3(q[0] as f32, q[1] as f32, q[2] as f32);
        }
        s.pull = frame.z;
        let after = analyze(&m, &s).unwrap();
        assert!(after.obstructions.is_empty());
        assert!(!after.sand_findings.is_empty());
    }
    #[test]
    fn grid_cap_and_nonfinite_geometry_are_explicit() {
        let mut s = setup();
        s.sample_pitch_mm = 0.025;
        let mut m = box_mesh([-10.0; 3], [10.0; 3]);
        let r = analyze(&m, &s).unwrap();
        assert_eq!(r.grid, [384, 384]);
        assert!(r.notes.iter().any(|n| n.contains("capped")));
        m.vertices[0].0 = f32::NAN;
        assert_eq!(analyze(&m, &s).unwrap().status, Status::Invalid);
    }
}
