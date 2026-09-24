//! Box select by one finger: left to right a window, right to left a crossing, the catch replacing, joining or leaving the choice, and whole parts or only their faces, edges or vertices taken.
use crate::icons::Icon;
use crate::viewport::{Mods, box_planes};
use egui::{Pos2, Rect};
use ringdesign_core::interaction::pick::{Entity, Filter, PickScene, Ray};

/// A box narrower or shorter than this on screen takes nothing, points.
pub const MIN_BOX_PT: f32 = 6.0;

/// What a box takes: whole parts with the stones, seats and stamps beside them, or only the faces, edges or vertices of parts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Takes {
    #[default]
    Parts,
    Faces,
    Edges,
    Vertices,
}

impl Takes {
    pub const ALL: [Self; 4] = [Self::Parts, Self::Faces, Self::Edges, Self::Vertices];

    pub fn label(self) -> &'static str {
        match self {
            Self::Parts => "Parts",
            Self::Faces => "Faces",
            Self::Edges => "Edges",
            Self::Vertices => "Vertices",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::Parts => Icon::CadBox,
            Self::Faces => Icon::Surface,
            Self::Edges => Icon::Wire,
            Self::Vertices => Icon::Measure,
        }
    }

    /// The pick classes it keeps.
    pub fn filter(self) -> Filter {
        match self {
            Self::Parts => Filter { parts: true, stones: true, made: true, ..Filter::none() },
            Self::Faces => Filter { faces: true, ..Filter::none() },
            Self::Edges => Filter { edges: true, ..Filter::none() },
            Self::Vertices => Filter { vertices: true, ..Filter::none() },
        }
    }
}

/// What a box does to the choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BoxOp {
    #[default]
    Replace,
    Add,
    Remove,
}

impl BoxOp {
    pub const ALL: [Self; 3] = [Self::Replace, Self::Add, Self::Remove];

    pub fn label(self) -> &'static str {
        match self {
            Self::Replace => "Replace",
            Self::Add => "Add",
            Self::Remove => "Remove",
        }
    }

    /// The keys a mouse holds for it: none, Shift or Ctrl.
    pub fn mods(self) -> Mods {
        match self {
            Self::Replace => Mods::default(),
            Self::Add => Mods { shift: true, ..Mods::default() },
            Self::Remove => Mods { ctrl: true, ..Mods::default() },
        }
    }
}

/// A box a finger dragged from `from` to `to`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    pub from: Pos2,
    pub to: Pos2,
}

impl Drag {
    pub fn rect(&self) -> Rect {
        Rect::from_two_pos(self.from, self.to)
    }

    /// Dragged right to left: a crossing, which takes whatever it touches; left to right a window, which takes what it holds whole.
    pub fn crossing(&self) -> bool {
        self.to.x < self.from.x
    }

    /// "Window" or "Crossing".
    pub fn kind(&self) -> &'static str {
        if self.crossing() { "Crossing" } else { "Window" }
    }

    /// Whether the box is big enough to mean one.
    pub fn sized(&self) -> bool {
        let r = self.rect();
        r.width() >= MIN_BOX_PT && r.height() >= MIN_BOX_PT
    }

    /// What the box takes of `scene`, the rays under its corners read by `ray`; nothing for a box too small to mean one.
    pub fn catch(&self, scene: &PickScene, ray: impl Fn(Pos2) -> Ray, filter: Filter) -> Vec<Entity> {
        if !self.sized() {
            return Vec::new();
        }
        let r = self.rect();
        let planes = box_planes([ray(r.left_top()), ray(r.right_top()), ray(r.right_bottom()), ray(r.left_bottom())]);
        scene.box_select(planes, self.crossing(), filter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::{Sel, Selection};
    use egui::pos2;
    use ringdesign_core::{
        AlphaLibrary, BuildParams, RingDesign,
        cad::{Attach, Component, Document, Feature, Operation, Placement},
        mesh, templates,
    };

    /// The Court band with two posts joined at its top, 20° apart.
    fn posts() -> RingDesign {
        let mut d = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        for (id, theta) in [(2, 80.0), (3, 100.0)] {
            let component = Component { attach: Attach::Join, placement: Placement::ring(theta, 0.0), ..Component::default() };
            doc.append(Feature { id, name: format!("Post {id}"), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, component }).unwrap();
        }
        d.cad = Some(doc);
        d
    }

    /// Looking down −y at the ring's top, 20 points a millimetre about screen (400, 400): x right, z up the screen.
    fn ray(p: Pos2) -> Ray {
        Ray { origin: [f64::from(p.x - 400.0) / 20.0, 40.0, -f64::from(p.y - 400.0) / 20.0], direction: [0.0, -1.0, 0.0] }
    }

    fn screen(x: f64, z: f64) -> Pos2 {
        pos2(400.0 + x as f32 * 20.0, 400.0 - z as f32 * 20.0)
    }

    #[test]
    fn a_window_takes_the_post_it_holds_whole_and_a_crossing_what_it_touches() {
        let d = posts();
        let built = mesh::build(&d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 256, profile_steps: 96, refine: None, ..BuildParams::default() });
        let scene = PickScene::build(&built, &d);
        let x = |id: u64| {
            let c = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == id).unwrap();
            c.frame.origin[0]
        };
        let (left, right) = (x(3), x(2));
        assert!(left < -1.5 && right > 1.5, "the post at 100° stands left of the one at 80° seen from above: {left} {right}");
        // Left to right round the post on the right: a window that holds it whole.
        let window = Drag { from: screen(right - 1.6, 1.6), to: screen(right + 1.6, -1.6) };
        assert!(!window.crossing() && window.kind() == "Window" && window.sized());
        let caught = window.catch(&scene, ray, Filter::default());
        assert!(caught.contains(&Entity::Part { feature: 2 }) && !caught.contains(&Entity::Part { feature: 3 }), "{caught:?}");
        assert!(!caught.contains(&Entity::Band), "the band runs out of any box round a post");
        // Right to left over the middle of both: a crossing touches both and holds neither whole.
        let crossing = Drag { from: screen(right, 0.4), to: screen(left, -0.4) };
        assert!(crossing.crossing() && crossing.kind() == "Crossing");
        let touched = crossing.catch(&scene, ray, Filter::default());
        assert!(touched.contains(&Entity::Part { feature: 2 }) && touched.contains(&Entity::Part { feature: 3 }), "{touched:?}");
        let held = Drag { from: crossing.to, to: crossing.from }.catch(&scene, ray, Filter::default());
        assert!(!held.iter().any(|e| matches!(e, Entity::Part { .. })), "the same box as a window holds no post whole: {held:?}");
        // A flick too short to be a box takes nothing.
        let flick = Drag { from: screen(right, 0.0), to: screen(right, 0.0) + egui::vec2(12.0, 4.0) };
        assert!(!flick.sized() && flick.catch(&scene, ray, Filter::default()).is_empty());
    }

    #[test]
    fn the_catch_replaces_joins_or_leaves_the_choice_as_the_op_says() {
        let d = posts();
        let built = mesh::build(&d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 256, profile_steps: 96, refine: None, ..BuildParams::default() });
        let scene = PickScene::build(&built, &d);
        let x = |id: u64| built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == id).unwrap().frame.origin[0];
        let round = |cx: f64| Drag { from: screen(cx - 1.6, 1.6), to: screen(cx + 1.6, -1.6) }.catch(&scene, ray, Filter::default());
        let (first, second) = (round(x(2)), round(x(3)));
        let mut s = Selection::default();
        s.boxed(&first, BoxOp::Replace.mods());
        assert_eq!(s.items, [Sel::Part(2)]);
        s.boxed(&second, BoxOp::Add.mods());
        assert_eq!(s.items, [Sel::Part(2), Sel::Part(3)]);
        s.boxed(&first, BoxOp::Remove.mods());
        assert_eq!(s.items, [Sel::Part(3)]);
        s.boxed(&first, BoxOp::Replace.mods());
        assert_eq!(s.items, [Sel::Part(2)]);
        assert_eq!(BoxOp::ALL.map(BoxOp::label), ["Replace", "Add", "Remove"]);
        assert_eq!(BoxOp::default(), BoxOp::Replace);
    }

    #[test]
    fn a_box_takes_whole_parts_or_only_their_faces_edges_or_vertices() {
        let d = posts();
        let built = mesh::build(&d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 256, profile_steps: 96, refine: None, ..BuildParams::default() });
        let scene = PickScene::build(&built, &d);
        let c = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap();
        let (faces, edges, vertices) = (c.trace.face_kind.len(), c.edges.len(), c.trace.vertices.len());
        // A 1 × 2 mm cylinder: a side and two ends, its two rims and their seams' ends.
        assert_eq!((faces, edges), (3, 3), "vertices {vertices}");
        let x = c.frame.origin[0];
        let window = Drag { from: screen(x - 1.6, 1.6), to: screen(x + 1.6, -1.6) };
        let take = |t: Takes| window.catch(&scene, ray, t.filter());
        assert_eq!(take(Takes::Parts), [Entity::Part { feature: 2 }]);
        assert_eq!(take(Takes::Faces), (0..faces as u32).map(|face| Entity::Face { feature: 2, face }).collect::<Vec<_>>());
        assert_eq!(take(Takes::Edges), (0..edges as u32).map(|edge| Entity::Edge { feature: 2, edge }).collect::<Vec<_>>());
        assert_eq!(take(Takes::Vertices), (0..vertices as u32).map(|vertex| Entity::Vertex { feature: 2, vertex }).collect::<Vec<_>>());
        // Whatever the scene gives holds the part with everything of it; the choice keeps the part alone.
        let all = window.catch(&scene, ray, Filter::default());
        assert_eq!(all.len(), 1 + faces + edges + vertices, "{all:?}");
        let mut s = Selection::default();
        s.boxed(&all, BoxOp::Replace.mods());
        assert_eq!(s.items, [Sel::Part(2)]);
        // Taking faces, the choice is the faces themselves.
        s.boxed(&take(Takes::Faces), BoxOp::Replace.mods());
        assert_eq!(s.items, (0..faces as u32).map(|face| Sel::Face { feature: 2, face }).collect::<Vec<_>>());
        // A crossing over both posts' middles takes the faces of both it touches, and the band never.
        let crossing = Drag { from: screen(x + 0.5, 0.4), to: screen(-x - 0.5, -0.4) };
        let touched = crossing.catch(&scene, ray, Takes::Faces.filter());
        assert!(touched.iter().any(|e| matches!(e, Entity::Face { feature: 2, .. })) && touched.iter().any(|e| matches!(e, Entity::Face { feature: 3, .. })), "{touched:?}");
        assert!(touched.iter().all(|e| matches!(e, Entity::Face { .. })));
        assert_eq!(Takes::ALL.map(Takes::label), ["Parts", "Faces", "Edges", "Vertices"]);
        assert_eq!(Takes::default(), Takes::Parts);
        assert!(!Takes::Parts.filter().band && Takes::Parts.filter().stones && Takes::Parts.filter().made);
    }
}
