//! Where a view looks: the box a Fit view frames, the chosen parts before the parts shown alone before the whole ring, and the point on the ring a pinch leaves the pivot on.
use crate::viewport::{Sel, selection::feature_name};
use ringdesign_core::{
    BuildResult, Mesh, RingDesign, Vec3,
    interaction::{picking::raycast, pick::Ray},
    sketch::Id,
};

/// What a Fit view frames.
#[derive(Clone, Debug, PartialEq)]
pub enum Framed {
    /// The chosen parts, seats, stamps and stones, by name.
    Chosen(Vec<String>),
    /// The parts shown alone, by name.
    Alone(Vec<String>),
    /// The whole ring.
    Ring,
}

impl Framed {
    /// What the status line says of it.
    pub fn said(&self) -> String {
        let list = |names: &[String]| match names {
            [one] => one.clone(),
            [a, b] => format!("{a} and {b}"),
            more => format!("{} things", more.len()),
        };
        match self {
            Framed::Chosen(names) => format!("Fit view: {}, as chosen", list(names)),
            Framed::Alone(names) => format!("Fit view: {}, shown alone", list(names)),
            Framed::Ring => "Fit view: the whole ring".into(),
        }
    }
}

/// A box grown to hold points.
#[derive(Clone, Copy, Debug)]
struct Grow(Option<(Vec3, Vec3)>);

impl Grow {
    fn point(&mut self, p: [f32; 3]) {
        let v = Vec3(p[0], p[1], p[2]);
        self.0 = Some(match self.0 {
            None => (v, v),
            Some((lo, hi)) => (Vec3(lo.0.min(v.0), lo.1.min(v.1), lo.2.min(v.2)), Vec3(hi.0.max(v.0), hi.1.max(v.1), hi.2.max(v.2))),
        });
    }

    fn range(&mut self, b: Option<(Vec3, Vec3)>) {
        if let Some((lo, hi)) = b {
            self.point([lo.0, lo.1, lo.2]);
            self.point([hi.0, hi.1, hi.2]);
        }
    }
}

/// The box of what one chosen item covers on `built`, and its name; `None` for a place or a layer.
fn item_box(built: &BuildResult, design: &RingDesign, item: &Sel) -> Option<((Vec3, Vec3), String)> {
    let mesh = &built.mesh;
    let fused = |owns: &dyn Fn(u32) -> bool| {
        let mut g = Grow(None);
        for (v, o) in mesh.vertices.iter().zip(&mesh.origin) {
            if owns(*o) {
                g.point([v.0, v.1, v.2]);
            }
        }
        g.0
    };
    match item {
        Sel::Part(id) | Sel::Face { feature: id, .. } | Sel::Edge { feature: id, .. } | Sel::Vertex { feature: id, .. } => {
            let c = built.parts.evaluated.as_ref()?.components.iter().find(|c| c.id == *id)?;
            Some((c.mesh.bounds()?, feature_name(*id, design, Some(built))))
        }
        Sel::Seat(path) => {
            let solids = &built.solids;
            let b = fused(&|o| solids.stone_of(o).is_some_and(|s| solids.paths[s] == *path))?;
            Some((b, "the seat".into()))
        }
        Sel::Stamp(index) => {
            let b = fused(&|o| built.solids.stamp_of(o) == Some(*index))?;
            let name = design.stamps.get(*index).map_or_else(|| format!("stamp {index}"), |s| format!("\"{}\"", s.name));
            Some((b, name))
        }
        Sel::Stone(path) => {
            let (st, f) = ringdesign_core::stones::stone_frames(design).into_iter().find(|(s, _)| s.path == *path)?;
            let r = f.reach.max(f.pavilion) as f32;
            let g = f.girdle.map(|x| x as f32);
            Some(((Vec3(g[0] - r, g[1] - r, g[2] - r), Vec3(g[0] + r, g[1] + r, g[2] + r)), format!("the {:.1} mm stone", st.gem.w_mm)))
        }
        Sel::BandPoint { .. } | Sel::Layer(_) => None,
    }
}

/// The box a Fit view frames on `built` and what it is: the chosen parts, seats, stamps and stones, a face, edge or vertex standing for its part; else the parts `isolated` shows alone; else the whole ring.
pub fn framed(built: &BuildResult, design: &RingDesign, items: &[Sel], isolated: &[Id]) -> Option<((Vec3, Vec3), Framed)> {
    let mut chosen = Grow(None);
    let mut names: Vec<String> = Vec::new();
    for item in items {
        if let Some((b, name)) = item_box(built, design, item) {
            chosen.range(Some(b));
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    if let Some(b) = chosen.0 {
        return Some((b, Framed::Chosen(names)));
    }
    let mut alone = Grow(None);
    let mut shown = Vec::new();
    for id in isolated {
        if let Some(c) = built.parts.evaluated.as_ref().and_then(|e| e.components.iter().find(|c| c.id == *id)) {
            alone.range(c.mesh.bounds());
            shown.push(feature_name(*id, design, Some(built)));
        }
    }
    if let Some(b) = alone.0 {
        return Some((b, Framed::Alone(shown)));
    }
    built.mesh.bounds().map(|b| (b, Framed::Ring))
}

/// The first point of `mesh` a ray meets.
fn hit(mesh: &Mesh, ray: Ray) -> Option<[f64; 3]> {
    raycast(mesh, ray.origin.map(|v| v as f32), ray.direction.map(|v| v as f32)).map(|(_, p)| p.map(f64::from))
}

/// Where a pinch leaves the view's pivot on `mesh`: the metal under the fingers where they lifted, else under the view's middle; `None` when neither meets the ring.
pub fn pivot_after_pinch(mesh: &Mesh, fingers: Ray, middle: Ray) -> Option<[f64; 3]> {
    hit(mesh, fingers).or_else(|| hit(mesh, middle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::Selection;
    use ringdesign_core::{
        AlphaLibrary, BuildParams,
        cad::{Attach, Component, Document, Feature, Operation, Placement},
        mesh, templates,
    };

    /// The Court band with a post at its top and a block round the side at 0°.
    fn parts() -> RingDesign {
        let mut d = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        let at = |theta: f64| Component { attach: Attach::Join, placement: Placement::ring(theta, 0.0), ..Component::default() };
        doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, component: at(90.0) }).unwrap();
        doc.append(Feature { id: 3, name: "Block".into(), enabled: true, operation: Operation::Box { size: [2.0, 2.0, 1.0] }, component: at(0.0) }).unwrap();
        d.cad = Some(doc);
        d
    }

    fn built(d: &RingDesign) -> BuildResult {
        mesh::build(d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 192, profile_steps: 96, refine: None, ..BuildParams::default() })
    }

    fn centre(b: (Vec3, Vec3)) -> [f32; 3] {
        [(b.0.0 + b.1.0) * 0.5, (b.0.1 + b.1.1) * 0.5, (b.0.2 + b.1.2) * 0.5]
    }

    #[test]
    fn a_fit_frames_what_is_chosen_else_the_parts_shown_alone_else_the_whole_ring() {
        let d = parts();
        let b = built(&d);
        let component = |id: Id| b.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == id).unwrap().mesh.bounds().unwrap();
        // Nothing chosen, nothing alone: the whole ring, some 25 mm across.
        let (ring, what) = framed(&b, &d, &[], &[]).unwrap();
        assert_eq!((ring, what.clone()), (b.mesh.bounds().unwrap(), Framed::Ring));
        assert_eq!(what.said(), "Fit view: the whole ring");
        assert!(ring.1.0 - ring.0.0 > 20.0);
        // The post chosen by a face of it: the post's own box, a couple of millimetres at the ring's top.
        let (post, what) = framed(&b, &d, &[Sel::Face { feature: 2, face: 0 }], &[]).unwrap();
        assert_eq!((post, what.clone()), (component(2), Framed::Chosen(vec!["Post".into()])));
        assert!(post.1.0 - post.0.0 < 2.5 && centre(post)[1] > 9.0, "{post:?}");
        assert_eq!(what.said(), "Fit view: Post, as chosen");
        // Two chosen: the box round both, named together; a band point adds nothing.
        let at = Sel::BandPoint { theta_deg: 180.0, v_mm: 1.0, world: [-10.0, 0.0, 0.0] };
        let (both, what) = framed(&b, &d, &[Sel::Part(2), at.clone(), Sel::Edge { feature: 3, edge: 0 }], &[]).unwrap();
        let (p, k) = (component(2), component(3));
        assert_eq!(both, (Vec3(p.0.0.min(k.0.0), p.0.1.min(k.0.1), p.0.2.min(k.0.2)), Vec3(p.1.0.max(k.1.0), p.1.1.max(k.1.1), p.1.2.max(k.1.2))));
        assert_eq!(what.said(), "Fit view: Post and Block, as chosen");
        // Nothing chosen but parts shown alone: theirs.
        let (alone, what) = framed(&b, &d, &[at], &[3]).unwrap();
        assert_eq!((alone, what.said()), (component(3), "Fit view: Block, shown alone".to_string()));
        // A choice still wins over the parts alone.
        assert_eq!(framed(&b, &d, &[Sel::Part(2)], &[3]).unwrap().0, component(2));
        // Three or more are counted.
        assert_eq!(Framed::Chosen(vec!["a".into(), "b".into(), "c".into()]).said(), "Fit view: 3 things, as chosen");
        let mut s = Selection::default();
        s.items = vec![Sel::Part(9)];
        assert_eq!(framed(&b, &d, &s.items, &[]).unwrap().1, Framed::Ring, "a part the build does not hold frames the ring");
    }

    #[test]
    fn a_pinch_leaves_the_pivot_on_the_metal_under_the_fingers_else_under_the_middle() {
        let d = parts();
        let b = built(&d);
        let down = |x: f64, z: f64| Ray { origin: [x, 40.0, z], direction: [0.0, -1.0, 0.0] };
        // Down onto the post's end: its top, 2 mm over the crest.
        let post = component_top(&b, 2);
        let p = pivot_after_pinch(&b.mesh, down(0.2, 0.1), down(30.0, 0.0)).unwrap();
        assert!((p[1] - post).abs() < 1e-3 && (p[0] - 0.2).abs() < 1e-6 && (p[2] - 0.1).abs() < 1e-6, "{p:?} against {post}");
        // Fingers over nothing: the metal under the middle of the view.
        let q = pivot_after_pinch(&b.mesh, down(30.0, 0.0), down(4.0, 0.2)).unwrap();
        assert!(q[1] > 8.0 && q[1] < post, "the band beside the post: {q:?}");
        // Neither meets the ring: no pivot.
        assert_eq!(pivot_after_pinch(&b.mesh, down(30.0, 0.0), down(0.0, 20.0)), None);
    }

    /// The highest point of part `id` as built.
    fn component_top(b: &BuildResult, id: Id) -> f64 {
        let c = b.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == id).unwrap();
        f64::from(c.mesh.bounds().unwrap().1.1)
    }
}
