//! Touch-sized targets: how near a finger must land to take a gizmo handle, and which of the things under it a tap takes.
use crate::gizmo::{Handle, Layout, Mark};
use crate::viewport::Sel;
use egui::Pos2;
use ringdesign_core::interaction::pick::{Entity, Pick};

/// Half a fingertip: a handle whose mark lies within this of the touch is taken, points.
pub const FINGER_PT: f32 = 22.0;
/// The least height of a touch target such as a menu row or a strip chip, points.
pub const TARGET_PT: f32 = 44.0;
/// How far from a finger a part's vertex or edge still answers a pick, points.
pub const APERTURE_PT: f32 = 12.0;
/// A second tap within this of the first is on the same spot, points.
pub const SAME_SPOT_PT: f32 = 24.0;
/// Dots stand on the part, so they take a finger nearer than lines do: this share of the reach.
const DOT_SHARE: f32 = 0.6;
/// What a dot's small size wins it over a line at the same distance, points.
const DOT_FAVOUR: f32 = 4.0;

fn segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

fn polyline(p: Pos2, points: &[Pos2]) -> f32 {
    points.windows(2).map(|w| segment(p, w[0], w[1])).fold(f32::INFINITY, f32::min)
}

/// The handle a finger at `p` takes within `reach` points: dots favoured, and only dots inside the part's disc `part`.
pub fn handle_at(layout: &Layout, p: Pos2, reach: f32, part: Option<(Pos2, f32)>) -> Option<Handle> {
    let on_part = part.is_some_and(|(c, r)| c.distance(p) < r);
    layout
        .marks
        .iter()
        .filter_map(|(h, m)| {
            let (d, tolerance, favour) = match m {
                Mark::Arrow { base, tip } if !on_part => (segment(p, *base, *tip), reach, 0.0),
                Mark::Ring { points, .. } if !on_part => (polyline(p, points), reach, 0.0),
                Mark::Dot { at, radius } => ((p.distance(*at) - radius).max(0.0), reach * DOT_SHARE, DOT_FAVOUR),
                _ => return None,
            };
            (d <= tolerance).then_some((*h, d - favour))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(h, _)| h)
}

/// A pick stack as a finger wants it: whole parts before stones, their faces, edges and vertices, and the band last.
pub fn coarse_first(mut picks: Vec<Pick>) -> Vec<Pick> {
    let order = |e: &Entity| match e {
        Entity::Part { .. } => 0,
        Entity::Stone { .. } => 1,
        Entity::Face { .. } => 2,
        Entity::Edge { .. } => 3,
        Entity::Vertex { .. } => 4,
        Entity::Band => 5,
    };
    picks.sort_by_key(|p| order(&p.entity));
    picks
}

/// What a long press opens its menu on: the pick already chosen, else a part's face, else the part, a stone or the band.
pub fn menu_pick(picks: &[Pick], chosen: &[Sel]) -> Option<Pick> {
    let class = |f: fn(&Entity) -> bool| picks.iter().find(|p| f(&p.entity));
    picks
        .iter()
        .rev()
        .find(|p| chosen.iter().any(|s| s.is(&p.entity)))
        .or_else(|| class(|e| matches!(e, Entity::Face { .. })))
        .or_else(|| class(|e| matches!(e, Entity::Part { .. })))
        .or_else(|| class(|e| matches!(e, Entity::Stone { .. })))
        .or_else(|| class(|e| matches!(e, Entity::Band)))
        .or(picks.first())
        .cloned()
}

/// Which of the things under a finger a tap takes: the first on a fresh spot, the next each time the same spot is tapped again.
#[derive(Clone, Debug, Default)]
pub struct DepthWalk {
    spot: Option<Pos2>,
    stack: Vec<Entity>,
    cursor: usize,
}

impl DepthWalk {
    /// The index into `picks` a tap at `p` takes; `None` with nothing under it, which also forgets the spot.
    pub fn tap(&mut self, p: Pos2, picks: &[Pick]) -> Option<usize> {
        if picks.is_empty() {
            self.forget();
            return None;
        }
        let stack: Vec<Entity> = picks.iter().map(|k| k.entity.clone()).collect();
        let again = self.spot.is_some_and(|s| s.distance(p) <= SAME_SPOT_PT) && self.stack == stack;
        self.cursor = if again { (self.cursor + 1) % stack.len() } else { 0 };
        self.spot = Some(p);
        self.stack = stack;
        Some(self.cursor)
    }

    /// Forgets the spot, so the next tap starts from the top.
    pub fn forget(&mut self) {
        self.spot = None;
        self.stack.clear();
        self.cursor = 0;
    }

    /// Where in the stack the last tap sat: `(index, length)`.
    pub fn depth(&self) -> (usize, usize) {
        (self.cursor, self.stack.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Axis;
    use egui::pos2;

    /// A seated part's gizmo as it lays out: the part 16 pt across its reach at (400, 300), arrows from 26 pt out, the spin ring at 46.
    fn layout() -> Layout {
        let ring: Vec<Pos2> = (0..=64)
            .map(|i| {
                let a = i as f32 / 64.0 * std::f32::consts::TAU;
                pos2(400.0 + 46.0 * a.cos(), 300.0 + 46.0 * a.sin())
            })
            .collect();
        Layout {
            marks: vec![
                (Handle::Move(Axis::Theta), Mark::Arrow { base: pos2(426.0, 300.0), tip: pos2(482.0, 300.0) }),
                (Handle::Move(Axis::Height), Mark::Arrow { base: pos2(400.0, 274.0), tip: pos2(400.0, 218.0) }),
                (Handle::Turn(Axis::Spin), Mark::Ring { far: vec![false; ring.len()], points: ring }),
                (Handle::Dial, Mark::Dot { at: pos2(400.0, 306.0), radius: 6.0 }),
                (Handle::Grip(0), Mark::Dot { at: pos2(386.0, 292.0), radius: 4.5 }),
            ],
            dial: None,
            lines: Vec::new(),
        }
    }

    #[test]
    fn a_finger_takes_a_handle_the_mouse_would_miss_and_misses_one_a_thumb_away() {
        let l = layout();
        let beside = pos2(470.0, 318.0);
        assert_eq!(l.hit(beside), None, "18 pt under the arrow is past the mouse's 7");
        assert_eq!(handle_at(&l, beside, FINGER_PT, None), Some(Handle::Move(Axis::Theta)));
        assert_eq!(handle_at(&l, pos2(470.0, 330.0), FINGER_PT, None), None, "30 pt from the arrow and from the ring is a thumb away from both");
        // Where the height arrow and the spin ring are both in reach the nearer wins: 10 pt against 14.8, then 30 against 4.
        assert_eq!(handle_at(&l, pos2(410.0, 240.0), FINGER_PT, None), Some(Handle::Move(Axis::Height)));
        assert_eq!(handle_at(&l, pos2(440.0, 270.0), FINGER_PT, None), Some(Handle::Turn(Axis::Spin)));
    }

    #[test]
    fn a_finger_on_the_part_chooses_the_part_and_only_the_dots_standing_on_it_answer_there() {
        let l = layout();
        let part = Some((pos2(400.0, 300.0), 16.0));
        // 12.6 pt from the height arrow's base, but on the part.
        let on = pos2(404.0, 286.0);
        assert_eq!(handle_at(&l, on, FINGER_PT, None), Some(Handle::Move(Axis::Height)));
        assert_eq!(handle_at(&l, on, FINGER_PT, part), None);
        // The grip and the dial stand on the part and take a finger right on them, the nearer dot first.
        assert_eq!(handle_at(&l, pos2(388.0, 293.0), FINGER_PT, part), Some(Handle::Grip(0)));
        assert_eq!(handle_at(&l, pos2(401.0, 311.0), FINGER_PT, part), Some(Handle::Dial));
        // 17.2 pt from the round arrow's base, 14 past the dial's rim and 21.6 past the grip's: a dot's reach is 13.2.
        let edge = pos2(412.0, 290.0);
        assert_eq!(handle_at(&l, edge, FINGER_PT, None), Some(Handle::Move(Axis::Theta)));
        assert_eq!(handle_at(&l, edge, FINGER_PT, part), None);
    }

    fn pick(entity: Entity) -> Pick {
        Pick { entity, world: [0.0; 3], normal: [0.0, 0.0, 1.0], depth: 1.0, px: 0.0 }
    }

    #[test]
    fn a_tap_takes_the_whole_part_first_and_the_same_spot_again_walks_down_to_its_edges() {
        let ranked = vec![
            pick(Entity::Vertex { feature: 3, vertex: 1 }),
            pick(Entity::Edge { feature: 3, edge: 2 }),
            pick(Entity::Face { feature: 3, face: 0 }),
            pick(Entity::Part { feature: 3 }),
        ];
        let stack = coarse_first(ranked);
        let names: Vec<Entity> = stack.iter().map(|p| p.entity.clone()).collect();
        assert_eq!(names, [Entity::Part { feature: 3 }, Entity::Face { feature: 3, face: 0 }, Entity::Edge { feature: 3, edge: 2 }, Entity::Vertex { feature: 3, vertex: 1 }]);
        let mut walk = DepthWalk::default();
        let at = pos2(200.0, 200.0);
        let taken: Vec<usize> = (0..5).map(|k| walk.tap(at + egui::vec2(k as f32 * 3.0, 0.0), &stack).unwrap()).collect();
        assert_eq!(taken, [0, 1, 2, 3, 0], "each tap on the spot goes one deeper, round to the top");
        assert_eq!(walk.depth(), (0, 4));
        assert_eq!(walk.tap(pos2(300.0, 200.0), &stack), Some(0), "a fresh spot starts from the top");
        assert_eq!(walk.tap(pos2(300.0, 200.0), &stack[..2]), Some(0), "so does a different stack under the same spot");
        assert_eq!(walk.tap(pos2(300.0, 200.0), &[]), None);
        assert_eq!(walk.tap(pos2(300.0, 200.0), &stack[..2]), Some(0), "nothing under the finger forgot the spot");
    }

    #[test]
    fn a_long_press_menu_opens_on_what_is_chosen_else_a_face_else_the_part_or_the_band() {
        let stack = coarse_first(vec![pick(Entity::Edge { feature: 3, edge: 2 }), pick(Entity::Face { feature: 3, face: 4 }), pick(Entity::Part { feature: 3 })]);
        assert_eq!(menu_pick(&stack, &[]).map(|p| p.entity), Some(Entity::Face { feature: 3, face: 4 }), "a face carries the part's items and its own");
        let chosen = [Sel::Edge { feature: 3, edge: 2 }];
        assert_eq!(menu_pick(&stack, &chosen).map(|p| p.entity), Some(Entity::Edge { feature: 3, edge: 2 }), "an edge walked to stays the subject");
        assert_eq!(menu_pick(&[pick(Entity::Part { feature: 9 })], &[]).map(|p| p.entity), Some(Entity::Part { feature: 9 }));
        assert_eq!(menu_pick(&[pick(Entity::Band)], &chosen).map(|p| p.entity), Some(Entity::Band));
        assert!(menu_pick(&[], &chosen).is_none());
    }
}
