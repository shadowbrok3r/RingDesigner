//! One-shot camera clearance when menus or tool palettes open.
use egui_mobile::egui::{Id, Rect, Vec2, pos2};

#[derive(Default)]
pub struct MenuAvoidance {
    previous: Vec<(Id, Rect)>,
    pending: u8,
    pub shifts: u32,
    pub last_delta: [f32; 2],
}

impl MenuAvoidance {
    /// Opening starts a short layout settle. Closing, dragging and ordinary
    /// frames never start one. Manual navigation cancels a pending adjustment.
    pub fn observe(&mut self, overlays: &[(Id, Rect)], manual: bool, mesh_ready: bool) -> bool {
        let opened = overlays
            .iter()
            .any(|(id, _)| !self.previous.iter().any(|(old, _)| old == id));
        let closed = self
            .previous
            .iter()
            .any(|(id, _)| !overlays.iter().any(|(new, _)| new == id));
        let settling = overlays.iter().any(|(id, r)| {
            self.previous
                .iter()
                .find(|(old, _)| old == id)
                .is_some_and(|(_, old)| {
                    (r.min - old.min).length() > 0.5 || (r.size() - old.size()).length() > 0.5
                })
        });
        self.previous = overlays.to_vec();
        if manual || (closed && !opened) {
            self.pending = 0;
        } else if opened || (self.pending > 0 && settling) {
            self.pending = 2;
        } else if self.pending > 0 && mesh_ready {
            self.pending -= 1;
            return self.pending == 0;
        }
        false
    }

    pub fn settling(&self) -> bool {
        self.pending > 0
    }

    pub fn record_shift(&mut self, delta: Vec2) {
        self.shifts += 1;
        self.last_delta = [delta.x, delta.y];
    }
}

/// Translation with the most visible projected ring area, preferring the
/// smallest movement on ties. Scale and orientation are deliberately unchanged.
/// Overlapping menus count once. If the whole ring cannot fit, expose as much
/// as possible without zooming away from the user's chosen detail level.
pub fn clearance(view: Rect, ring: Rect, overlays: &[Rect]) -> Vec2 {
    if !view.is_positive() || !ring.is_positive() || !ring.is_finite() {
        return Vec2::ZERO;
    }
    let obstacles: Vec<_> = overlays
        .iter()
        .map(|r| r.expand(6.0).intersect(view))
        .filter(|r| r.is_positive())
        .collect();
    if obstacles.is_empty() {
        return Vec2::ZERO;
    }
    let visible = |r: Rect| {
        let clipped = r.intersect(view);
        if !clipped.is_positive() {
            return 0.0;
        }
        let covered: Vec<_> = obstacles
            .iter()
            .map(|o| o.intersect(clipped))
            .filter(|r| r.is_positive())
            .collect();
        clipped.area() - union_area(&covered)
    };
    let half = ring.size() * 0.5;
    let clamp = |v: f32, a: f32, b: f32| if a <= b { v.clamp(a, b) } else { (a + b) * 0.5 };
    let mut xs = vec![ring.center().x, view.left() + half.x, view.right() - half.x];
    let mut ys = vec![ring.center().y, view.top() + half.y, view.bottom() - half.y];
    for r in &obstacles {
        xs.extend([r.left() - half.x, r.right() + half.x]);
        ys.extend([r.top() - half.y, r.bottom() + half.y]);
    }
    let mut best = Vec2::ZERO;
    let mut score = visible(ring);
    for x in xs {
        for &y in &ys {
            let centre = pos2(
                clamp(x, view.left() + half.x, view.right() - half.x),
                clamp(y, view.top() + half.y, view.bottom() - half.y),
            );
            let delta = centre - ring.center();
            let candidate = visible(ring.translate(delta));
            if candidate > score + 0.5
                || ((candidate - score).abs() <= 0.5 && delta.length_sq() < best.length_sq())
            {
                score = candidate;
                best = delta;
            }
        }
    }
    best
}

fn union_area(rects: &[Rect]) -> f32 {
    let mut xs: Vec<_> = rects.iter().flat_map(|r| [r.left(), r.right()]).collect();
    xs.sort_by(f32::total_cmp);
    xs.dedup();
    let mut area = 0.0;
    for x in xs.windows(2) {
        let mut ys: Vec<_> = rects
            .iter()
            .filter(|r| r.left() < x[1] && r.right() > x[0])
            .map(|r| (r.top(), r.bottom()))
            .collect();
        ys.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut end = f32::NEG_INFINITY;
        let mut height = 0.0;
        for (top, bottom) in ys {
            height += (bottom - top.max(end)).max(0.0);
            end = end.max(bottom);
        }
        area += (x[1] - x[0]) * height;
    }
    area
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(pos2(x, y), Vec2::new(w, h))
    }

    #[test]
    fn moves_clear_of_menus_without_changing_ring_size() {
        let view = rect(0., 0., 400., 500.);
        let ring = rect(120., 110., 160., 180.);
        let menus = [rect(0., 0., 190., 300.), rect(300., 0., 100., 120.)];
        let shift = clearance(view, ring, &menus);
        let moved = ring.translate(shift);
        assert!(view.contains_rect(moved));
        assert!(menus.iter().all(|r| !r.expand(5.9).intersects(moved)));
        assert_eq!(moved.size(), ring.size());
        assert_eq!(clearance(view, moved, &menus), Vec2::ZERO);
    }

    #[test]
    fn overlapping_menus_and_impossible_fit_remain_finite() {
        let menu = rect(0., 0., 250., 220.);
        assert_eq!(union_area(&[menu, menu]), menu.area());
        let shift = clearance(
            rect(0., 0., 320., 200.),
            rect(-100., -80., 500., 350.),
            &[menu, menu],
        );
        assert!(shift.is_finite());
    }

    #[test]
    fn only_opening_repositions_and_manual_pan_cancels_pending_work() {
        let mut state = MenuAvoidance::default();
        let menu = [(Id::new("File"), rect(0., 0., 180., 300.))];
        assert!(!state.observe(&menu, false, true));
        assert!(!state.observe(&menu, false, true));
        assert!(state.observe(&menu, false, true));
        for _ in 0..5 {
            assert!(!state.observe(&menu, true, true));
        }
        for _ in 0..5 {
            assert!(!state.observe(&menu, false, true));
        }
        assert!(!state.observe(&[], false, true));
        assert!(!state.observe(&menu, false, true));
        assert!(!state.observe(&menu, true, true));
        for _ in 0..5 {
            assert!(!state.observe(&menu, false, true));
        }
        assert!(!state.observe(&[], false, true));
        assert!(!state.observe(&menu, false, true));
        assert!(!state.observe(&menu, false, true));
        assert!(state.observe(&menu, false, true));
    }
}
