//! A drafted extrusion as a mesh, for the outlines the kernel's tapered extrude refuses: Béziers, and
//! outlines whose inset drops a piece (sharp points, notches narrower than the draft's inset). The
//! outline is walked to the chord, the far end is its inset with every edge that shrinks to nothing
//! taken out where the wavefront passes it, and each side is the plane between an edge and its inset.
use super::SurfaceKind;
use super::builders::{self, Made};
use super::twist;
use crate::csg::{P3, Solid};
use crate::setting::Named;
use crate::sketch::region;
use anyhow::{Context, Result, bail, ensure};
use cadkernel::geom2d::Curve;
use cadkernel::space::Plane;

/// The key a drafted extrusion's mesh carries as a mesh value.
pub const DRAFT: &str = "extrude.draft";
/// The key the mesh of a sketch's meeting loops, each extruded alone and joined, carries.
pub const LOOPS: &str = "extrude.loops";

fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn dot2(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn cross2(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

/// One edge of the walked outline: where it starts, its unit direction, and its unit normal to the left (inward on a counter-clockwise ring).
#[derive(Clone, Copy)]
struct Edge {
    at: [f64; 2],
    t: [f64; 2],
    n: [f64; 2],
}
impl Edge {
    /// Where the lines of `self` and `next`, each moved `by` mm along its normal, meet; `None` when they run back along each other.
    fn meet(&self, next: &Edge, by: f64) -> Option<[f64; 2]> {
        let (p, q) = ([self.at[0] + self.n[0] * by, self.at[1] + self.n[1] * by], [next.at[0] + next.n[0] * by, next.at[1] + next.n[1] * by]);
        let c = cross2(self.t, next.t);
        if c.abs() < 1e-12 {
            // Parallel: the same line once moved when they run the same way, nothing when they run back.
            return (dot2(self.t, next.t) > 0.0).then(|| {
                let s = dot2(sub2(q, p), self.t);
                [p[0] + self.t[0] * s, p[1] + self.t[1] * s]
            });
        }
        let u = cross2(sub2(q, p), next.t) / c;
        Some([p[0] + self.t[0] * u, p[1] + self.t[1] * u])
    }
}

/// `ring` (counter-clockwise, no repeated points) moved `by` mm to the left of every edge (inward; outward when negative),
/// every edge that shrinks to nothing on the way taken out: the far ring's points, and for each point of `ring` the far point it rises to.
fn inset(ring: &[[f64; 2]], by: f64) -> Result<(Vec<[f64; 2]>, Vec<usize>)> {
    let m = ring.len();
    let sign = by.signum();
    let edges: Vec<Edge> = (0..m)
        .map(|i| {
            let d = sub2(ring[(i + 1) % m], ring[i]);
            let l = d[0].hypot(d[1]);
            let t = [d[0] / l, d[1] / l];
            Edge { at: ring[i], t, n: [-t[1] * sign, t[0] * sign] }
        })
        .collect();
    let (mut prev, mut next): (Vec<usize>, Vec<usize>) = ((0..m).map(|i| (i + m - 1) % m).collect(), (0..m).map(|i| (i + 1) % m).collect());
    let mut alive = vec![true; m];
    let mut count = m;
    let (mut at, end) = (0.0_f64, by.abs());
    let closes = || anyhow::anyhow!("Drafted extrusion: the draft closes the outline before the far end; draft it less, or widen it");
    loop {
        ensure!(count >= 3, closes());
        // The edge that shrinks to nothing first, and how far in.
        let length = |i: usize, by: f64| -> Option<f64> {
            let (a, b) = (edges[prev[i]].meet(&edges[i], by)?, edges[i].meet(&edges[next[i]], by)?);
            Some(dot2(sub2(b, a), edges[i].t))
        };
        let mut first: Option<(f64, usize)> = None;
        for i in (0..m).filter(|i| alive[*i]) {
            let (Some(now), Some(later)) = (length(i, at), length(i, at + 1.0)) else { return Err(closes()) };
            let rate = later - now;
            let gone = if now <= 1e-12 { at } else if rate < -1e-12 { at - now / rate } else { continue };
            if gone <= end && first.is_none_or(|(g, _)| gone < g) {
                first = Some((gone, i));
            }
        }
        let Some((gone, i)) = first else { break };
        at = gone.max(at);
        alive[i] = false;
        count -= 1;
        let (p, n) = (prev[i], next[i]);
        next[p] = n;
        prev[n] = p;
    }
    // Each live edge's start at the far end; a point of the ring rises to the start of the first live edge from its own.
    let mut slot = vec![usize::MAX; m];
    let mut far = Vec::with_capacity(count);
    let start = (0..m).find(|i| alive[*i]).context("Drafted extrusion: no edge survives the draft")?;
    let mut i = start;
    loop {
        slot[i] = far.len();
        far.push(edges[prev[i]].meet(&edges[i], end).ok_or_else(closes)?);
        i = next[i];
        if i == start {
            break;
        }
    }
    let rises = (0..m)
        .map(|j| {
            let mut k = j;
            while !alive[k] {
                k = (k + 1) % m;
            }
            slot[k]
        })
        .collect();
    Ok((far, rises))
}

/// One closed `outline` on `plane` extruded along `direction`, its far end inset by the draft `draft_rad` (positive narrows it), walked within `chord` mm.
pub fn extrude(plane: Plane, outline: &[Curve], direction: [f64; 3], draft_rad: f64, chord: f64) -> Result<Made> {
    ensure!(draft_rad.is_finite() && draft_rad.abs() < std::f64::consts::FRAC_PI_2, "Drafted extrusion: the draft must be below 90 degrees");
    let senses = region::senses(outline).context("Drafted extrusion: the outline does not close into one loop")?;
    let normal = plane.normal().context("Drafted extrusion: the sketch plane has no normal")?;
    let height = (0..3).map(|k| direction[k] * normal[k]).sum::<f64>();
    ensure!(height.abs() > 1e-9, "Drafted extrusion: the extrusion runs along its own plane");
    let (walked, pieces) = twist::section(outline, &senses, chord, f64::INFINITY)?;
    // Repeated points out, counter-clockwise on the plane.
    let mut ring: Vec<([f64; 2], bool, usize)> = Vec::with_capacity(walked.len());
    for p in walked {
        if ring.last().is_none_or(|q| sub2(q.0, p.0)[0].hypot(sub2(q.0, p.0)[1]) > 1e-9) {
            ring.push(p);
        }
    }
    while ring.len() > 1 && sub2(ring[0].0, ring[ring.len() - 1].0)[0].hypot(sub2(ring[0].0, ring[ring.len() - 1].0)[1]) <= 1e-9 {
        ring.pop();
    }
    let m = ring.len();
    ensure!(m >= 3, "Drafted extrusion: the outline has {m} points round; it needs three");
    let area = (0..m).map(|j| cross2(ring[j].0, ring[(j + 1) % m].0)).sum::<f64>() * 0.5;
    ensure!(area.abs() > 1e-12, "Drafted extrusion: the outline encloses no area");
    // Each point's leaving edge keeps the piece it runs along when the ring turns round.
    let mut piece: Vec<usize> = ring.iter().map(|p| p.2).collect();
    if area < 0.0 {
        ring.reverse();
        piece = (0..m).map(|j| ring[(j + 1) % m].2).collect();
    }
    let near: Vec<[f64; 2]> = ring.iter().map(|p| p.0).collect();
    let (far, rises) = inset(&near, height.abs() * draft_rad.tan())?;
    let k = far.len();
    let mut v: Vec<P3> = near.iter().map(|p| plane.point_at(*p)).collect();
    v.extend(far.iter().map(|p| std::array::from_fn(|c| plane.point_at(*p)[c] + direction[c])));
    // Faces as seen with the extrusion running along the normal; mirrored when it runs against it.
    let flip = height < 0.0;
    let mut f: Vec<[u32; 3]> = Vec::with_capacity(2 * m + 2 * k);
    let mut patch: Vec<u32> = Vec::with_capacity(2 * m + 2 * k);
    let mut push = |t: [u32; 3], p: usize| {
        f.push(if flip { [t[0], t[2], t[1]] } else { t });
        patch.push(p as u32);
    };
    for j in 0..m {
        let (a, b) = (j as u32, ((j + 1) % m) as u32);
        let (c, d) = ((m + rises[(j + 1) % m]) as u32, (m + rises[j]) as u32);
        push([a, b, c], piece[j]);
        if c != d {
            push([a, c, d], piece[j]);
        }
    }
    let caps = pieces.len();
    for t in twist::fill(&near, "Drafted extrusion")? {
        push([t[0], t[2], t[1]], caps);
    }
    let far_cap = twist::fill(&far, "Drafted extrusion").map_err(|_| anyhow::anyhow!("Drafted extrusion: the draft closes the outline across a neck or notch before the far end; draft it less, or widen it"))?;
    for t in far_cap {
        push(t.map(|i| i + m as u32), caps + 1);
    }
    let mut names: Vec<String> = (0..caps).map(|p| if caps == 1 { "Side".to_string() } else { format!("Side {}", p + 1) }).collect();
    names.extend(["Start cap".to_string(), "End cap".to_string()]);
    let mut kinds: Vec<SurfaceKind> = outline
        .iter()
        .map(|c| match c {
            Curve::Line(_) => SurfaceKind::Plane,
            Curve::Arc(_) | Curve::Circle(_) => SurfaceKind::Cone,
            _ => SurfaceKind::Freeform,
        })
        .collect();
    kinds.extend([SurfaceKind::Plane; 2]);
    let solid = Solid { v, f };
    let (open, repeated) = solid.open_edges();
    if open != 0 || repeated != 0 {
        bail!("Drafted extrusion did not close ({open} open edges, {repeated} repeated)");
    }
    let crossings = crate::csg::self_crossings(&solid);
    ensure!(crossings == 0, "Drafted extrusion crosses itself ({crossings} pairs of faces meet); draft it less, or widen the outline");
    let named = Named { solid, patch, names };
    let creases = builders::creases(&named, builders::CREASE_DEG);
    Ok(Made { key: DRAFT.to_string(), named, kinds, creases, gem: None, seat: None, stations: Vec::new() })
}

/// Drafted regions as one mesh: one region as it is, several side by side, each patch named by its region.
pub fn gathered(mut made: Vec<Made>) -> Result<Made> {
    ensure!(!made.is_empty(), "Sketch has no profile geometry");
    if made.len() == 1 {
        return Ok(made.remove(0));
    }
    let mut out = Made { key: DRAFT.to_string(), named: Named { solid: Solid { v: Vec::new(), f: Vec::new() }, patch: Vec::new(), names: Vec::new() }, kinds: Vec::new(), creases: Vec::new(), gem: None, seat: None, stations: Vec::new() };
    for (r, m) in made.into_iter().enumerate() {
        let (v0, p0) = (out.named.solid.v.len() as u32, out.named.names.len() as u32);
        out.named.solid.v.extend(m.named.solid.v);
        out.named.solid.f.extend(m.named.solid.f.iter().map(|t| t.map(|i| i + v0)));
        out.named.patch.extend(m.named.patch.iter().map(|p| p + p0));
        out.named.names.extend(m.named.names.iter().map(|n| format!("Region {} {}", r + 1, n.to_lowercase())));
        out.kinds.extend(m.kinds);
        out.creases.extend(m.creases);
    }
    Ok(out)
}

/// Each loop's extrusion joined into one by csg, every face keeping its loop's patch, named by its loop.
pub fn joined(parts: Vec<Named>) -> Result<Made> {
    let mut all: Option<Named> = None;
    for (k, mut part) in parts.into_iter().enumerate() {
        part.names = part.names.iter().map(|n| format!("Loop {} {}", k + 1, n.to_lowercase())).collect();
        all = Some(match all {
            None => part,
            Some(so_far) => builders::combined(&so_far, &part, crate::csg::Op::Union).with_context(|| format!("Joining loop {} to the loops before it", k + 1))?,
        });
    }
    let named = all.context("Sketch has no profile geometry")?;
    let (open, repeated) = named.solid.open_edges();
    ensure!(open == 0 && repeated == 0 && !named.solid.is_empty(), "The joined loops did not close ({open} open edges, {repeated} repeated)");
    let kinds = vec![SurfaceKind::Freeform; named.names.len()];
    let creases = builders::creases(&named, builders::CREASE_DEG);
    Ok(Made { key: LOOPS.to_string(), named, kinds, creases, gem: None, seat: None, stations: Vec::new() })
}

#[cfg(test)]
mod tests {
    use crate::cad::{Component, Document, Feature, Operation, evaluate};
    use crate::sketch::{Geometry, Sketch, Workplane};
    use crate::{AlphaLibrary, BuildParams, Mesh, RingDesign};
    use cadkernel::brep;

    const PREVIEW: BuildParams = BuildParams { theta_steps: 128, profile_steps: 64, min_wall_mm: crate::mesh::MIN_WALL_MM, adaptive: false, refine: None, soften_mm: 0.0 };

    /// The drafted extrusion of `s`, `height` mm at `draft_deg`: its closed mesh and whether the kernel built it.
    fn drafted(s: &Sketch, height: f64, draft_deg: f64) -> (Mesh, bool) {
        let mut d = RingDesign::default();
        let mut doc = Document::default();
        let op = Operation::Extrude { sketch: s.clone().into(), height_mm: height, draft_deg };
        doc.append(Feature { id: 1, name: op.label().into(), enabled: true, operation: op, component: Component::default() }).unwrap();
        d.cad = Some(doc);
        let e = evaluate(&d, &AlphaLibrary::builtin(), PREVIEW).unwrap();
        assert!(e.first_error().is_none(), "{:?}", e.first_error());
        let c = &e.components[0];
        let v = c.mesh.validate();
        assert!(v.watertight && v.boundary_edges == 0 && v.non_manifold_edges == 0, "{v:?}");
        (c.mesh.clone(), c.brep().is_some())
    }
    fn poly(pts: &[[f64; 2]]) -> Sketch {
        let mut s = Sketch::default();
        let ids = pts.iter().map(|p| s.point(*p)).collect();
        s.entity(Geometry::Polyline { points: ids, closed: true });
        s
    }
    /// Whether the kernel's tapered extrude refuses `s`.
    fn kernel_refuses(s: &Sketch, height: f64, draft_deg: f64) -> bool {
        brep::extrude_tapered(Workplane::default().plane().unwrap(), &s.profile_curves().unwrap(), [0.0, 0.0, height], draft_deg.to_radians()).is_none()
    }
    /// The area of a polygon.
    fn area(p: &[[f64; 2]]) -> f64 {
        (0..p.len()).map(|i| p[i][0] * p[(i + 1) % p.len()][1] - p[(i + 1) % p.len()][0] * p[i][1]).sum::<f64>().abs() / 2.0
    }

    /// Ogiva's crockets are Bézier leaves raised 3° along the pull; the kernel's tapered extrude takes only lines and arcs.
    #[test]
    fn a_bezier_outline_drafts() {
        let mut s = Sketch::default();
        let [a, b, c1, c2, tip, d1, d2] = [[-1.0, 0.0], [1.0, 0.0], [1.0, 1.5], [0.3, 2.5], [0.0, 3.0], [-0.3, 2.5], [-1.0, 1.5]].map(|p| s.point(p));
        s.entity(Geometry::Line { a, b });
        s.entity(Geometry::Bezier { points: [b, c1, c2, tip] });
        s.entity(Geometry::Bezier { points: [tip, d1, d2, a] });
        assert!(kernel_refuses(&s, 1.0, 3.0), "the kernel now drafts Béziers itself");
        let (up, brep) = drafted(&s, 1.0, 3.0);
        assert!(!brep);
        // The straight extrusion less the draft's wedge round the rim, to first order.
        let walked: Vec<[f64; 2]> = s.profile_curves().unwrap().iter().flat_map(|c| (0..200).map(move |k| c.point_at(k as f64 / 200.0))).collect();
        let rim: f64 = (0..walked.len()).map(|i| (walked[(i + 1) % walked.len()][0] - walked[i][0]).hypot(walked[(i + 1) % walked.len()][1] - walked[i][1])).sum();
        let expect = area(&walked) - rim * 3f64.to_radians().tan() / 2.0;
        assert!((up.volume_mm3() - expect).abs() < 0.02, "{} against {expect}", up.volume_mm3());
        // Run below the plane, the same solid mirrored.
        let (down, _) = drafted(&s, -1.0, 3.0);
        assert!((down.volume_mm3() - up.volume_mm3()).abs() < 1e-9);
        // Every point of the far end lies inside the near one by the draft's inset at least.
        let top: Vec<_> = up.vertices.iter().filter(|v| (v.2 - 1.0).abs() < 1e-6).collect();
        assert!(top.iter().all(|v| v.1 < 3.0 - 0.05 && v.1 > 0.05), "the far end is inset");
    }

    /// A sharp point whose tip edge is shorter than the draft's inset, and a notch narrower than
    /// twice it under a draft that widens the far end: the kernel's inset drops a piece and it refused both.
    #[test]
    fn an_inset_that_drops_a_piece_drafts() {
        let tip = poly(&[[0.0, 0.0], [3.0, 0.0], [3.0, 1.0], [1.51, 2.0], [1.49, 2.0], [0.0, 1.0]]);
        assert!(kernel_refuses(&tip, 1.0, 5.0), "the kernel now drafts this point itself");
        let (mesh, brep) = drafted(&tip, 1.0, 5.0);
        assert!(!brep);
        // The tip edge is gone at the far end: one far corner where the two slopes meet.
        let top: Vec<_> = mesh.vertices.iter().filter(|v| (v.2 - 1.0).abs() < 1e-6).collect();
        assert_eq!(top.len(), 5, "{top:?}");
        let near = area(&[[0.0, 0.0], [3.0, 0.0], [3.0, 1.0], [1.51, 2.0], [1.49, 2.0], [0.0, 1.0]]);
        assert!(mesh.volume_mm3() < near && mesh.volume_mm3() > near - 9.0 * 5f64.to_radians().tan(), "{}", mesh.volume_mm3());

        let notch = poly(&[[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [2.05, 3.0], [2.0, 1.0], [1.95, 3.0], [0.0, 3.0]]);
        assert!(kernel_refuses(&notch, 1.0, -5.0), "the kernel now drafts this notch itself");
        let (mesh, _) = drafted(&notch, 1.0, -5.0);
        // The notch fills before the far end, whose rim is the rectangle grown by the inset, with
        // the corner where the notch closed left on its top edge.
        let grow = 5f64.to_radians().tan() as f32;
        let top: Vec<_> = mesh.vertices.iter().filter(|v| (v.2 - 1.0).abs() < 1e-6).collect();
        assert_eq!(top.len(), 5, "{top:?}");
        let on = |x: f32, at: f32| (x - at).abs() < 1e-5;
        assert!(top.iter().all(|v| on(v.0, -grow) || on(v.0, 4.0 + grow) || on(v.1, -grow) || on(v.1, 3.0 + grow)), "{top:?}");
        // A draft that would carry the notch's root across the outline is refused by name.
        let e = {
            let mut d = RingDesign::default();
            let mut doc = Document::default();
            doc.append(Feature { id: 1, name: "x".into(), enabled: true, operation: Operation::Extrude { sketch: notch.into(), height_mm: 1.0, draft_deg: 5.0 }, component: Component::default() }).unwrap();
            d.cad = Some(doc);
            evaluate(&d, &AlphaLibrary::builtin(), PREVIEW).unwrap()
        };
        assert!(e.first_error().is_some_and(|m| m.contains("closes the outline")), "{:?}", e.first_error());
    }

    /// What the kernel drafts, it still drafts: the body stays its own.
    #[test]
    fn a_draft_the_kernel_takes_is_still_its_body() {
        let (mesh, brep) = drafted(&Sketch::rectangle(4.0, 3.0), 1.0, 5.0);
        assert!(brep);
        assert!(mesh.volume_mm3() < 12.0);
    }
}
