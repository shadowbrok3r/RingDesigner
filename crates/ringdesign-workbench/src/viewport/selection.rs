//! What is hovered and what is chosen over the pick scene, and the weights that light it on the
//! ring. A click replaces, Shift adds, Ctrl removes, Alt cycles the depth stack first; Tab cycles
//! it without choosing. `tint` turns the selection into one weight per fused-mesh vertex for the
//! viewport's focus channel, so a face, an edge or a vertex of a part lights exactly where it lies.
use ringdesign_core::{
    BuildResult, Mesh, RingDesign,
    cad::EvaluatedComponent,
    interaction::{
        bvh::Bvh,
        pick::{Entity, Filter, Pick, Ray},
    },
    mesh::SOLID_VERTEX,
    sketch::Id,
};

/// One chosen thing. Ordinals are the part's B-rep order, as a `Pick` names them.
#[derive(Clone, Debug, PartialEq)]
pub enum Sel {
    Part(Id),
    Face { feature: Id, face: u32 },
    Edge { feature: Id, edge: u32 },
    Vertex { feature: Id, vertex: u32 },
    Stone(Vec<usize>),
    /// A point on the band: where the click landed, in the chart and in the world.
    BandPoint { theta_deg: f64, v_mm: f64, world: [f64; 3] },
    Layer(usize),
}

impl Sel {
    /// The selection a pick becomes; `band` reads the chart position of a band hit.
    pub fn of(pick: &Pick, band: impl FnOnce([f64; 3]) -> (f64, f64)) -> Self {
        match &pick.entity {
            Entity::Band => {
                let (theta_deg, v_mm) = band(pick.world);
                Sel::BandPoint { theta_deg, v_mm, world: pick.world }
            }
            Entity::Part { feature } => Sel::Part(*feature),
            Entity::Face { feature, face } => Sel::Face { feature: *feature, face: *face },
            Entity::Edge { feature, edge } => Sel::Edge { feature: *feature, edge: *edge },
            Entity::Vertex { feature, vertex } => Sel::Vertex { feature: *feature, vertex: *vertex },
            Entity::Stone { path } => Sel::Stone(path.clone()),
        }
    }

    /// The selection an entity is, without a pick; the band has no place to name and is none.
    pub fn of_entity(e: &Entity) -> Option<Self> {
        Some(match e {
            Entity::Band => return None,
            Entity::Part { feature } => Sel::Part(*feature),
            Entity::Face { feature, face } => Sel::Face { feature: *feature, face: *face },
            Entity::Edge { feature, edge } => Sel::Edge { feature: *feature, edge: *edge },
            Entity::Vertex { feature, vertex } => Sel::Vertex { feature: *feature, vertex: *vertex },
            Entity::Stone { path } => Sel::Stone(path.clone()),
        })
    }

    /// The CAD feature the selection is on, if any.
    pub fn feature(&self) -> Option<Id> {
        match self {
            Sel::Part(id) | Sel::Face { feature: id, .. } | Sel::Edge { feature: id, .. } | Sel::Vertex { feature: id, .. } => Some(*id),
            _ => None,
        }
    }

    /// The edge ordinal, when the selection is one.
    pub fn edge(&self) -> Option<u32> {
        match self {
            Sel::Edge { edge, .. } => Some(*edge),
            _ => None,
        }
    }

    /// Whether the selection is what a pick names; a band point is a place, never the band.
    pub fn is(&self, e: &Entity) -> bool {
        match (self, e) {
            (Sel::Part(a), Entity::Part { feature }) => a == feature,
            (Sel::Face { feature: a, face: b }, Entity::Face { feature, face }) => a == feature && b == face,
            (Sel::Edge { feature: a, edge: b }, Entity::Edge { feature, edge }) => a == feature && b == edge,
            (Sel::Vertex { feature: a, vertex: b }, Entity::Vertex { feature, vertex }) => a == feature && b == vertex,
            (Sel::Stone(a), Entity::Stone { path }) => a == path,
            _ => false,
        }
    }
}

/// The modifier keys held on a click.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// The viewport's selection: what is chosen, what the pointer is over, and the stack behind it.
#[derive(Clone, Debug)]
pub struct Selection {
    pub items: Vec<Sel>,
    /// The pick the pointer is over, at the depth the cursor has cycled to.
    pub hover: Option<Pick>,
    pub filter: Filter,
    /// What the context menu opened over.
    pub under: Option<Pick>,
    stack: Vec<Pick>,
    cursor: usize,
    dirty: bool,
    staged_for: usize,
}

impl Default for Selection {
    fn default() -> Self {
        Self { items: Vec::new(), hover: None, filter: Filter::default(), under: None, stack: Vec::new(), cursor: 0, dirty: true, staged_for: 0 }
    }
}

impl Selection {
    /// The picks under the pointer, best first. The cursor survives while the stack names the
    /// same entities, so a Tab-cycled depth holds still under a pointer that barely moves.
    pub fn hovered(&mut self, picks: Vec<Pick>) {
        let same = picks.len() == self.stack.len() && picks.iter().zip(&self.stack).all(|(a, b)| a.entity == b.entity);
        if !same {
            self.cursor = 0;
        }
        self.stack = picks;
        self.refresh_hover();
    }

    /// The stack under the pointer, best first.
    pub fn stack(&self) -> &[Pick] {
        &self.stack
    }

    /// Where in the stack the hover sits: `(index, length)`.
    pub fn depth(&self) -> (usize, usize) {
        (self.cursor.min(self.stack.len().saturating_sub(1)), self.stack.len())
    }

    /// Step the hover one deeper into the stack, round to the top.
    pub fn cycle(&mut self) -> Option<&Pick> {
        if self.stack.len() > 1 {
            self.cursor = (self.cursor + 1) % self.stack.len();
        }
        self.refresh_hover();
        self.hover.as_ref()
    }

    fn refresh_hover(&mut self) {
        let next = self.stack.get(self.cursor).or(self.stack.first()).cloned();
        if self.hover.as_ref().map(|h| &h.entity) != next.as_ref().map(|h| &h.entity) {
            self.dirty = true;
        }
        self.hover = next;
    }

    /// A click on `sel`: replace, add with Shift, remove with Ctrl; a click on nothing clears
    /// unless a modifier is held.
    pub fn click(&mut self, sel: Option<Sel>, mods: Mods) {
        match (sel, mods.shift, mods.ctrl) {
            (Some(sel), true, _) => {
                if !self.items.contains(&sel) {
                    self.items.push(sel);
                    self.dirty = true;
                }
            }
            (Some(sel), false, true) => {
                let before = self.items.len();
                self.items.retain(|s| *s != sel);
                self.dirty |= self.items.len() != before;
            }
            (Some(sel), false, false) => {
                self.items.clear();
                self.items.push(sel);
                self.dirty = true;
            }
            (None, false, false) => self.clear(),
            (None, _, _) => {}
        }
    }

    /// A click where the pointer is: Alt cycles the stack first, then the hover is chosen.
    pub fn choose(&mut self, mods: Mods, band: impl FnOnce([f64; 3]) -> (f64, f64)) {
        if mods.alt {
            self.cycle();
        }
        let sel = self.hover.as_ref().map(|p| Sel::of(p, band));
        self.click(sel, mods);
    }

    pub fn clear(&mut self) {
        self.dirty |= !self.items.is_empty();
        self.items.clear();
        self.under = None;
    }

    /// A box's catch as the choice, a whole part standing for its faces, edges and vertices: Shift adds, Ctrl removes, else replaces.
    pub fn boxed(&mut self, entities: &[Entity], mods: Mods) {
        let parts: Vec<Id> = entities.iter().filter_map(|e| if let Entity::Part { feature } = e { Some(*feature) } else { None }).collect();
        let chosen: Vec<Sel> = entities
            .iter()
            .filter(|e| !matches!(e, Entity::Face { feature, .. } | Entity::Edge { feature, .. } | Entity::Vertex { feature, .. } if parts.contains(feature)))
            .filter_map(Sel::of_entity)
            .collect();
        match (mods.shift, mods.ctrl) {
            (true, _) => {
                for sel in chosen {
                    if !self.items.contains(&sel) {
                        self.items.push(sel);
                        self.dirty = true;
                    }
                }
            }
            (false, true) => {
                let before = self.items.len();
                self.items.retain(|s| !chosen.contains(s));
                self.dirty |= self.items.len() != before;
            }
            (false, false) => {
                self.dirty |= self.items != chosen;
                self.items = chosen;
                self.under = None;
            }
        }
    }

    pub fn is_selected(&self, e: &Entity) -> bool {
        self.items.iter().any(|s| s.is(e))
    }

    /// The one part every chosen item is on, the part itself or its faces, edges and vertices; `None` for anything else.
    pub fn one_part(&self) -> Option<Id> {
        let first = self.items.first()?.feature()?;
        self.items.iter().all(|s| s.feature() == Some(first)).then_some(first)
    }

    /// Whether the focus channel must be restaged: something changed, or the mesh under it did.
    pub fn needs_stage(&mut self, mesh_key: usize) -> bool {
        let due = self.dirty || self.staged_for != mesh_key;
        self.dirty = false;
        self.staged_for = mesh_key;
        due
    }
}

/// The planes through the rays under a rectangle's corners, taken round it, as `a·x + b·y + c·z + d ≥ 0` inside.
pub fn box_planes(corners: [Ray; 4]) -> [[f64; 4]; 4] {
    let unit = |v: [f64; 3]| {
        let l = dot3(v, v).sqrt();
        if l > 0.0 { v.map(|x| x / l) } else { v }
    };
    let dirs = corners.map(|r| unit(r.direction));
    let mid_o: [f64; 3] = std::array::from_fn(|k| corners.iter().map(|r| r.origin[k]).sum::<f64>() / 4.0);
    let mid_d = unit(std::array::from_fn(|k| dirs.iter().map(|d| d[k]).sum::<f64>()));
    let probe: [f64; 3] = std::array::from_fn(|k| mid_o[k] + mid_d[k]);
    std::array::from_fn(|i| {
        let j = (i + 1) % 4;
        let p0 = corners[i].origin;
        let p1: [f64; 3] = std::array::from_fn(|k| p0[k] + dirs[i][k]);
        let p2: [f64; 3] = std::array::from_fn(|k| corners[j].origin[k] + dirs[j][k]);
        let n = unit(cross3(sub3(p1, p0), sub3(p2, p0)));
        let d = -dot3(n, p0);
        if dot3(n, probe) + d < 0.0 { [-n[0], -n[1], -n[2], -d] } else { [n[0], n[1], n[2], d] }
    })
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// A fused vertex within this of a part's own face tessellation is on that face.
const FACE_MM: f64 = 1e-3;
/// An edge polyline is sampled on the true curve the tessellation chords; its chord sag is 0.04 mm.
const EDGE_MM: f64 = 0.1;
const VERTEX_MM: f64 = 0.05;
/// Radius of the disc lit round a band point.
const DISC_MM: f64 = 0.9;

fn component(built: &BuildResult, feature: Id) -> Option<&EvaluatedComponent> {
    built.parts.evaluated.as_ref()?.components.iter().find(|c| c.id == feature)
}

fn at(m: &Mesh, i: usize) -> [f64; 3] {
    let v = m.vertices[i];
    [v.0 as f64, v.1 as f64, v.2 as f64]
}

fn dist2(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Squared distance from `p` to the segment `ab`.
fn segment_dist2(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let len2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
    let t = if len2 > 0.0 { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1] + (p[2] - a[2]) * d[2]) / len2).clamp(0.0, 1.0) } else { 0.0 };
    dist2(p, [a[0] + d[0] * t, a[1] + d[1] * t, a[2] + d[2] * t])
}

/// Set `weight` on every fused vertex the entity covers.
fn paint(w: &mut [f32], built: &BuildResult, target: &Sel, weight: f32) {
    let mesh = &built.mesh;
    let origin = &mesh.origin;
    let owned = |i: usize, feature: Id| origin.get(i).and_then(|o| built.parts.feature_of(*o)) == Some(feature);
    match target {
        Sel::Part(feature) => {
            for i in 0..w.len() {
                if owned(i, *feature) {
                    w[i] = weight;
                }
            }
        }
        Sel::Face { feature, face } => {
            let Some(c) = component(built, *feature) else { return };
            let faces: Vec<[u32; 3]> = c.mesh.faces.iter().enumerate().filter(|(t, _)| c.trace.tri_face.get(*t) == Some(face)).map(|(_, f)| *f).collect();
            if faces.is_empty() {
                return;
            }
            let sub = Mesh { vertices: c.mesh.vertices.clone(), faces, ..Default::default() };
            let bvh = Bvh::build(&sub);
            for i in 0..w.len() {
                if owned(i, *feature) && bvh.nearest(&sub, at(mesh, i), FACE_MM).is_some() {
                    w[i] = weight;
                }
            }
        }
        Sel::Edge { feature, edge } => {
            let Some(poly) = component(built, *feature).and_then(|c| c.edges.get(*edge as usize)) else { return };
            let tol = EDGE_MM * EDGE_MM;
            for i in 0..w.len() {
                if !owned(i, *feature) {
                    continue;
                }
                let p = at(mesh, i);
                let near = if poly.len() == 1 { dist2(p, poly[0]) <= tol } else { poly.windows(2).any(|s| segment_dist2(p, s[0], s[1]) <= tol) };
                if near {
                    w[i] = weight;
                }
            }
        }
        Sel::Vertex { feature, vertex } => {
            let Some(v) = component(built, *feature).and_then(|c| c.trace.vertices.get(*vertex as usize)) else { return };
            let tol = VERTEX_MM * VERTEX_MM;
            for i in 0..w.len() {
                if owned(i, *feature) && dist2(at(mesh, i), *v) <= tol {
                    w[i] = weight;
                }
            }
        }
        Sel::BandPoint { world, .. } => {
            let tol = DISC_MM * DISC_MM;
            for i in 0..w.len() {
                let band = origin.get(i).is_none_or(|o| *o < SOLID_VERTEX);
                if band && dist2(at(mesh, i), *world) <= tol {
                    w[i] = weight;
                }
            }
        }
        Sel::Stone(_) | Sel::Layer(_) => {}
    }
}

/// One weight per fused-mesh vertex: 0 untouched, 1 selected, 2 hovered (hover wins). Empty when
/// nothing is chosen or hovered, which clears the channel.
pub fn tint(sel: &Selection, built: &BuildResult) -> Vec<f32> {
    if sel.items.is_empty() && sel.hover.is_none() {
        return Vec::new();
    }
    let mut w = vec![0.0; built.mesh.vertices.len()];
    for item in &sel.items {
        paint(&mut w, built, item, 1.0);
    }
    if let Some(h) = &sel.hover {
        let target = Sel::of(h, |_| (0.0, 0.0));
        paint(&mut w, built, &target, 2.0);
    }
    w
}

/// The name of a CAD feature: the document's, else the evaluation's, else its number.
pub fn feature_name(feature: Id, design: &RingDesign, built: Option<&BuildResult>) -> String {
    design
        .cad
        .as_ref()
        .and_then(|d| d.features.iter().find(|f| f.id == feature).map(|f| f.name.clone()))
        .or_else(|| built.and_then(|b| component(b, feature)).map(|c| c.name.clone()))
        .unwrap_or_else(|| format!("part #{feature}"))
}

/// What an entity is called on screen: "Cylinder face 3 (plane)", "Cylinder edge 2", "the band".
pub fn label(entity: &Entity, design: &RingDesign, built: Option<&BuildResult>) -> String {
    match entity {
        Entity::Band => "the band".into(),
        Entity::Part { feature } => feature_name(*feature, design, built),
        Entity::Face { feature, face } => {
            if let Some(patch) = built.and_then(|b| component(b, *feature)).and_then(|c| c.trace.patch(*face)) {
                return format!("{patch} of {}", feature_name(*feature, design, built));
            }
            let kind = built.and_then(|b| component(b, *feature)).and_then(|c| c.trace.face_kind.get(*face as usize)).map(|k| format!(" ({})", format!("{k:?}").to_lowercase())).unwrap_or_default();
            format!("{} face {face}{kind}", feature_name(*feature, design, built))
        }
        Entity::Edge { feature, edge } => format!("{} edge {edge}", feature_name(*feature, design, built)),
        Entity::Vertex { feature, vertex } => format!("{} vertex {vertex}", feature_name(*feature, design, built)),
        Entity::Stone { path } => match path.first().and_then(|i| design.layers.layers.get(*i)) {
            Some(e) => format!("stone on {}", e.name),
            None => "stone".into(),
        },
    }
}

/// What a selection is called on screen.
pub fn describe(sel: &Sel, design: &RingDesign, built: Option<&BuildResult>) -> String {
    match sel {
        Sel::Part(id) => label(&Entity::Part { feature: *id }, design, built),
        Sel::Face { feature, face } => label(&Entity::Face { feature: *feature, face: *face }, design, built),
        Sel::Edge { feature, edge } => label(&Entity::Edge { feature: *feature, edge: *edge }, design, built),
        Sel::Vertex { feature, vertex } => label(&Entity::Vertex { feature: *feature, vertex: *vertex }, design, built),
        Sel::Stone(path) => label(&Entity::Stone { path: path.clone() }, design, built),
        Sel::BandPoint { theta_deg, v_mm, .. } => format!("band at {theta_deg:.0}°, v {v_mm:.2}"),
        Sel::Layer(i) => design.layers.layers.get(*i).map_or_else(|| format!("layer {i}"), |e| e.name.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::{
        AlphaLibrary, BuildParams, mesh,
        cad::{Attach, Component, Document, Feature, Operation, Placement, Stage},
        interaction::pick::{PickScene, Ray, ViewScale},
        templates,
    };

    fn pick(entity: Entity) -> Pick {
        Pick { entity, world: [0.0, 10.0, 0.0], normal: [0.0, 1.0, 0.0], depth: 1.0, px: 0.0 }
    }
    fn part() -> Pick {
        pick(Entity::Part { feature: 1 })
    }
    fn face(face: u32) -> Pick {
        pick(Entity::Face { feature: 1, face })
    }
    fn band() -> Pick {
        pick(Entity::Band)
    }
    fn chart(_: [f64; 3]) -> (f64, f64) {
        (90.0, 4.5)
    }

    #[test]
    fn a_click_replaces_shift_adds_ctrl_removes_and_nothing_clears() {
        let mut s = Selection::default();
        s.hovered(vec![face(1), part()]);
        s.choose(Mods::default(), chart);
        assert_eq!(s.items, [Sel::Face { feature: 1, face: 1 }]);
        s.hovered(vec![band()]);
        s.choose(Mods { shift: true, ..Default::default() }, chart);
        assert_eq!(s.items.len(), 2);
        assert_eq!(s.items[1], Sel::BandPoint { theta_deg: 90.0, v_mm: 4.5, world: [0.0, 10.0, 0.0] });
        // Adding what is already chosen changes nothing.
        s.choose(Mods { shift: true, ..Default::default() }, chart);
        assert_eq!(s.items.len(), 2);
        s.hovered(vec![face(1), part()]);
        s.choose(Mods { ctrl: true, ..Default::default() }, chart);
        assert_eq!(s.items, [Sel::BandPoint { theta_deg: 90.0, v_mm: 4.5, world: [0.0, 10.0, 0.0] }]);
        assert!(s.is_selected(&Entity::Band) == false, "a band point is a place, not the band");
        // A modified click on nothing keeps the selection; a plain one clears it.
        s.hovered(vec![]);
        s.choose(Mods { shift: true, ..Default::default() }, chart);
        assert_eq!(s.items.len(), 1);
        s.choose(Mods::default(), chart);
        assert!(s.items.is_empty());
        s.hovered(vec![part()]);
        s.choose(Mods::default(), chart);
        assert!(s.is_selected(&Entity::Part { feature: 1 }));
    }

    #[test]
    fn tab_cycles_the_depth_stack_alt_click_chooses_the_next_and_the_cursor_survives_a_still_pointer() {
        let mut s = Selection::default();
        s.hovered(vec![pick(Entity::Vertex { feature: 1, vertex: 0 }), pick(Entity::Edge { feature: 1, edge: 2 }), face(0), part()]);
        assert_eq!(s.hover.as_ref().map(|p| p.entity.clone()), Some(Entity::Vertex { feature: 1, vertex: 0 }));
        assert_eq!(s.depth(), (0, 4));
        s.cycle();
        assert_eq!(s.hover.as_ref().map(|p| p.entity.clone()), Some(Entity::Edge { feature: 1, edge: 2 }));
        // The same stack again, as a throttled hover resamples it: the cursor holds.
        s.hovered(vec![pick(Entity::Vertex { feature: 1, vertex: 0 }), pick(Entity::Edge { feature: 1, edge: 2 }), face(0), part()]);
        assert_eq!(s.depth(), (1, 4));
        s.choose(Mods { alt: true, ..Default::default() }, chart);
        assert_eq!(s.items, [Sel::Face { feature: 1, face: 0 }]);
        s.cycle();
        s.cycle();
        assert_eq!(s.depth(), (0, 4), "the cycle wraps");
        // A different stack resets the cursor.
        s.hovered(vec![band()]);
        assert_eq!(s.depth(), (0, 1));
        s.hovered(vec![]);
        assert!(s.hover.is_none());
        s.clear();
        assert!(s.items.is_empty());
    }

    #[test]
    fn one_part_is_chosen_by_its_faces_edges_and_vertices_and_nothing_else_mixes_in() {
        let mut s = Selection::default();
        assert_eq!(s.one_part(), None);
        s.click(Some(Sel::Face { feature: 3, face: 1 }), Mods::default());
        assert_eq!(s.one_part(), Some(3));
        s.click(Some(Sel::Edge { feature: 3, edge: 2 }), Mods { shift: true, ..Default::default() });
        s.click(Some(Sel::Vertex { feature: 3, vertex: 0 }), Mods { shift: true, ..Default::default() });
        s.click(Some(Sel::Part(3)), Mods { shift: true, ..Default::default() });
        assert_eq!(s.one_part(), Some(3), "a part and its own faces, edges and vertices are one part");
        s.click(Some(Sel::Part(4)), Mods { shift: true, ..Default::default() });
        assert_eq!(s.one_part(), None, "two parts are not one");
        s.click(Some(Sel::Part(3)), Mods::default());
        s.click(Some(Sel::BandPoint { theta_deg: 90.0, v_mm: 0.0, world: [0.0, 10.0, 0.0] }), Mods { shift: true, ..Default::default() });
        assert_eq!(s.one_part(), None, "a band point beside it is not a part");
        s.click(Some(Sel::Stone(vec![0])), Mods::default());
        assert_eq!(s.one_part(), None);
    }

    #[test]
    fn the_channel_is_restaged_on_a_change_or_a_new_mesh_and_not_otherwise() {
        let mut s = Selection::default();
        assert!(s.needs_stage(1));
        assert!(!s.needs_stage(1));
        assert!(s.needs_stage(2), "a new mesh restages");
        s.hovered(vec![part()]);
        assert!(s.needs_stage(2));
        s.hovered(vec![part()]);
        assert!(!s.needs_stage(2), "the same hover again is not a change");
        s.click(Some(Sel::Part(1)), Mods::default());
        assert!(s.needs_stage(2));
    }

    #[test]
    fn a_window_round_the_bezel_boxes_its_part_and_the_modifiers_add_and_remove() {
        let design = court_with_bezel();
        let lib = AlphaLibrary::builtin();
        let built = mesh::build(&design, &lib, BuildParams { theta_steps: 256, profile_steps: 128, ..Default::default() });
        let scene = PickScene::build(&built, &design);
        // Looking down −y at the top of the ring: a screen corner at (x, z) is a ray from y = 40.
        let corner = |x: f64, z: f64| Ray { origin: [x, 40.0, z], direction: [0.0, -1.0, 0.0] };
        let window = box_planes([corner(-2.0, -2.0), corner(2.0, -2.0), corner(2.0, 2.0), corner(-2.0, 2.0)]);
        let inside = |p: [f64; 3]| window.iter().all(|pl| pl[0] * p[0] + pl[1] * p[1] + pl[2] * p[2] + pl[3] >= 0.0);
        assert!(inside([0.0, 10.0, 0.0]) && inside([1.9, -30.0, -1.9]) && !inside([2.1, 10.0, 0.0]) && !inside([0.0, 10.0, -2.1]));
        let caught = scene.box_select(window, false, Filter::default());
        assert!(caught.contains(&Entity::Part { feature: 1 }), "{caught:?}");
        assert!(caught.iter().any(|e| matches!(e, Entity::Face { .. })) && !caught.contains(&Entity::Band));
        let mut s = Selection::default();
        s.click(Some(Sel::BandPoint { theta_deg: 0.0, v_mm: 0.0, world: [10.0, 0.0, 0.0] }), Mods::default());
        s.boxed(&caught, Mods::default());
        assert_eq!(s.items, [Sel::Part(1)], "a whole part stands for its faces, edges and vertices");
        // A crossing that only clips the rim still takes the part; Ctrl takes it away, Shift puts it back.
        let crossing = box_planes([corner(1.2, -0.3), corner(2.5, -0.3), corner(2.5, 0.3), corner(1.2, 0.3)]);
        let clipped = scene.box_select(crossing, true, Filter::default());
        assert!(clipped.contains(&Entity::Part { feature: 1 }) && clipped.contains(&Entity::Band), "{clipped:?}");
        assert!(scene.box_select(crossing, false, Filter { parts: true, ..Filter::none() }).is_empty(), "a window must hold the whole part");
        s.boxed(&clipped, Mods { ctrl: true, ..Default::default() });
        assert!(s.items.is_empty());
        s.boxed(&clipped, Mods { shift: true, ..Default::default() });
        assert_eq!(s.items, [Sel::Part(1)], "the band is never boxed");
        s.boxed(&[], Mods::default());
        assert!(s.items.is_empty(), "an empty plain box clears");
    }

    /// The Court band with a joined cylinder at its top, as the pick scene's own tests build it.
    fn court_with_bezel() -> RingDesign {
        let mut d = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 0, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(Feature {
            id: 1,
            name: "Bezel".into(),
            enabled: true,
            operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 },
            component: Component { attach: Attach::Join, stage: Stage::Cast, placement: Placement::ring(90.0, 0.25), ..Default::default() },
        })
        .unwrap();
        d.cad = Some(doc);
        d
    }

    #[test]
    fn the_tint_lights_a_part_its_top_face_an_edge_a_vertex_and_a_disc_on_the_band() {
        let design = court_with_bezel();
        let lib = AlphaLibrary::builtin();
        let built = mesh::build(&design, &lib, BuildParams { theta_steps: 256, profile_steps: 128, ..Default::default() });
        let scene = PickScene::build(&built, &design);
        let view = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 10.0 };
        let down = |x: f64, z: f64| Ray { origin: [x, 40.0, z], direction: [0.0, -1.0, 0.0] };
        let top = scene.pick(down(0.0, 0.0), &view, 0.0, Filter::default());
        let top_face = top.iter().find(|p| matches!(p.entity, Entity::Face { .. })).expect("the top face").clone();
        let part_verts = built.mesh.origin.iter().filter(|o| built.parts.feature_of(**o) == Some(1)).count();
        assert!(part_verts > 100, "the fused mesh names the bezel's vertices: {part_verts}");

        let mut s = Selection::default();
        s.click(Some(Sel::Part(1)), Mods::default());
        let w = tint(&s, &built);
        assert_eq!(w.len(), built.mesh.vertices.len());
        let lit = w.iter().filter(|v| **v == 1.0).count();
        assert_eq!(lit, part_verts, "a part lights every vertex its origin names");

        s.hovered(vec![top_face.clone()]);
        let w = tint(&s, &built);
        let hovered: Vec<usize> = (0..w.len()).filter(|i| w[*i] == 2.0).collect();
        assert!(hovered.len() > 20 && hovered.len() < part_verts, "the top face is part of the part: {} of {part_verts}", hovered.len());
        // Every lit vertex lies on the plane of the part's own top-face triangles.
        let c = component(&built, 1).unwrap();
        let Entity::Face { face: top_ord, .. } = top_face.entity else { unreachable!() };
        let (a, b, cc) = c.mesh.faces.iter().enumerate().find(|(t, _)| c.trace.tri_face[*t] == top_ord).and_then(|(_, f)| c.mesh.triangle(f)).unwrap();
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [cc[0] - a[0], cc[1] - a[1], cc[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let n = n.map(|x| x / len);
        let plane = n[0] * a[0] + n[1] * a[1] + n[2] * a[2];
        for i in &hovered {
            let p = at(&built.mesh, *i);
            assert!((n[0] * p[0] + n[1] * p[1] + n[2] * p[2] - plane).abs() < 2e-3, "off the top plane");
        }
        let top_y = hovered.iter().map(|i| built.mesh.vertices[*i].1 as f64).fold(f64::MIN, f64::max);
        assert!(w.iter().filter(|v| **v == 1.0).count() + hovered.len() == part_verts, "hover wins where the two overlap");

        // The rim between the top and the wall: on the plane, and at the cylinder's radius.
        let edges = scene.pick(down(1.5, 0.0), &view, 6.0, Filter { edges: true, ..Filter::none() });
        let rim = edges.first().expect("the rim").clone();
        s.clear();
        s.hovered(vec![rim.clone()]);
        let w = tint(&s, &built);
        let on_rim: Vec<usize> = (0..w.len()).filter(|i| w[*i] == 2.0).collect();
        assert!(!on_rim.is_empty(), "the rim lights");
        for i in &on_rim {
            let v = built.mesh.vertices[*i];
            let r = (v.0 as f64).hypot(v.2 as f64);
            assert!((r - 1.5).abs() < 0.11 && (v.1 as f64 - top_y).abs() < 0.11, "rim vertex off the rim: r {r:.3} y {:.3}", v.1);
        }
        let Entity::Edge { edge, .. } = rim.entity else { unreachable!() };
        assert_eq!(component(&built, 1).unwrap().edges[edge as usize].len() > 8, true);

        // A B-rep vertex: the buried bottom seam vertex lights nothing, the top one lights itself.
        let c = component(&built, 1).unwrap();
        let mut hit_any = false;
        s.hovered(vec![]);
        for (vi, v) in c.trace.vertices.iter().enumerate() {
            s.clear();
            s.click(Some(Sel::Vertex { feature: 1, vertex: vi as u32 }), Mods::default());
            let w = tint(&s, &built);
            let lit: Vec<usize> = (0..w.len()).filter(|i| w[*i] == 1.0).collect();
            for i in &lit {
                assert!(dist2(at(&built.mesh, *i), *v).sqrt() <= VERTEX_MM);
            }
            hit_any |= !lit.is_empty();
        }
        assert!(hit_any, "the top rim's seam vertex is a fused vertex");

        // A band point lights a disc of band vertices only.
        // Beside the bezel on the 4 mm band: past the 1.5 mm radius, short of the edge.
        let band = scene.pick(down(0.0, -1.8), &view, 0.0, Filter::default());
        let hit = band.iter().find(|p| p.entity == Entity::Band).expect("the band beside the bezel").clone();
        s.clear();
        s.click(Some(Sel::of(&hit, |_| (90.0, 0.0))), Mods::default());
        let w = tint(&s, &built);
        let disc: Vec<usize> = (0..w.len()).filter(|i| w[*i] == 1.0).collect();
        assert!(disc.len() > 3, "a disc of vertices: {}", disc.len());
        for i in &disc {
            assert!(dist2(at(&built.mesh, *i), hit.world).sqrt() <= DISC_MM);
            assert!(built.mesh.origin[*i] < SOLID_VERTEX, "band vertices only");
        }
        // Nothing chosen and nothing hovered is an empty channel.
        s.clear();
        s.hovered(vec![]);
        assert!(tint(&s, &built).is_empty());
        assert_eq!(label(&top_face.entity, &design, Some(&built)), format!("Bezel face {} (plane)", match top_face.entity { Entity::Face { face, .. } => face, _ => 0 }));
        assert_eq!(label(&Entity::Band, &design, Some(&built)), "the band");
        assert_eq!(label(&Entity::Part { feature: 7 }, &design, None), "part #7");
    }

    /// The viewport's hover path on an export build, timed: `cargo test --release -p
    /// ringdesign-workbench -- --ignored a_hover_frame`.
    #[test]
    #[ignore]
    fn a_hover_frame_on_an_export_build_costs_under_a_millisecond() {
        use std::time::Instant;
        let design = court_with_bezel();
        let lib = AlphaLibrary::builtin();
        let built = mesh::build(&design, &lib, BuildParams { theta_steps: 1024, profile_steps: 384, ..Default::default() });
        let t = Instant::now();
        let scene = PickScene::build(&built, &design);
        let scene_ms = t.elapsed().as_secs_f64() * 1e3;
        // An orthographic top view at 20 px/mm: the ray under a pixel and its two neighbours.
        let ray = |p: egui::Pos2| -> ([f32; 3], [f32; 3]) { ([p.x * 0.05 - 14.0, 40.0, p.y * 0.05 - 4.0], [0.0, -1.0, 0.0]) };
        let mut sink = 0usize;
        let (mut worst, mut total) = (0.0f64, 0.0f64);
        let n = 1000;
        for i in 0..n {
            let pos = egui::pos2(((i * 37) % 560) as f32, ((i * 91) % 160) as f32);
            let t = Instant::now();
            let picks = crate::hover::pick_at(&scene, pos, &ray, 8.0, Filter::default());
            let us = t.elapsed().as_secs_f64() * 1e6;
            worst = worst.max(us);
            total += us;
            sink += picks.len();
        }
        let top = scene.pick(Ray { origin: [0.0, 40.0, 0.0], direction: [0.0, -1.0, 0.0] }, &ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 20.0 }, 8.0, Filter::default());
        let mut s = Selection::default();
        s.hovered(top);
        let t = Instant::now();
        let face = tint(&s, &built);
        let face_ms = t.elapsed().as_secs_f64() * 1e3;
        s.click(Some(Sel::Part(1)), Mods::default());
        s.hovered(vec![]);
        let t = Instant::now();
        let part = tint(&s, &built);
        let part_ms = t.elapsed().as_secs_f64() * 1e3;
        eprintln!(
            "{} faces: scene {scene_ms:.1} ms; hover pick mean {:.1} µs, worst {worst:.1} µs ({sink} picks); tint face {face_ms:.1} ms ({} lit), part {part_ms:.1} ms ({} lit)",
            scene.faces(),
            total / n as f64,
            face.iter().filter(|w| **w > 0.0).count(),
            part.iter().filter(|w| **w > 0.0).count()
        );
        let mean_us = total / n as f64;
        assert!(mean_us < 1000.0, "a hover pick must stay under a millisecond: {mean_us:.1} µs");
    }
}
