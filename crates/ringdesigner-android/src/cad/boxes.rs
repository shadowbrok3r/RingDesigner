//! Box select on the phone's ring: a mode in which one finger draws a box instead of turning the ring.
use egui::Pos2;
use egui_mobile::egui;
use ringdesign_core::interaction::pick::Ray;
use ringdesign_workbench::{
    icons::Icon,
    touch::boxes::{BoxOp, Drag},
};

use super::{Cad, View, bar};

/// Box select's switch, what a box does, and the box a finger holds.
#[derive(Clone, Copy, Debug, Default)]
pub struct Boxing {
    /// A one-finger drag on the ring draws a box.
    pub on: bool,
    pub op: BoxOp,
    /// A finger is down on the ring and may yet draw a box.
    pressed: bool,
    drag: Option<Drag>,
}

impl Boxing {
    /// Whether a finger holds the ring for a box, so the view neither turns nor pans.
    pub fn holding(&self) -> bool {
        self.pressed || self.drag.is_some()
    }

    /// The box being drawn.
    pub fn drag(&self) -> Option<Drag> {
        self.drag
    }

    /// A finger came down on the ring.
    pub(super) fn press(&mut self) {
        self.pressed = self.on;
        self.drag = None;
    }

    /// The pressed finger left the slop at `to`, from `from`: the box starts; whether it did.
    pub(super) fn start(&mut self, from: Pos2, to: Pos2) -> bool {
        if !self.pressed {
            return false;
        }
        self.drag = Some(Drag { from, to });
        true
    }

    /// The finger moved on to `to`; whether a box follows it.
    pub(super) fn follow(&mut self, to: Pos2) -> bool {
        match &mut self.drag {
            Some(d) => {
                d.to = to;
                true
            }
            None => false,
        }
    }

    /// The finger lifted at `to`: the box drawn, if one was.
    pub(super) fn lift(&mut self, to: Pos2) -> Option<Drag> {
        self.pressed = false;
        self.drag.take().map(|d| Drag { to, ..d })
    }

    /// Drops the finger's box unfinished, as a second finger or a lost touch does.
    pub(super) fn let_go(&mut self) {
        self.pressed = false;
        self.drag = None;
    }

    /// Turns box select on or off.
    pub fn toggle(&mut self) {
        self.on = !self.on;
        self.let_go();
    }
}

/// What the box bar's buttons do after the ops, in their order.
const DONE: usize = BoxOp::ALL.len();

impl Cad {
    /// Chooses what the box `d` takes, as its op says; what it says.
    pub(super) fn finish_box(&mut self, v: &View, d: Drag) -> String {
        if !d.sized() {
            return "Box select: drag further for a box".into();
        }
        let Some(scene) = self.scene.clone() else {
            return "Box select: nothing on this ring to take; add a part or a stone first".into();
        };
        let ray = |p: Pos2| {
            let (o, dir) = v.ray(p);
            Ray { origin: o.map(f64::from), direction: dir.map(f64::from) }
        };
        let caught = d.catch(&scene, ray, self.selection.filter);
        self.selection.boxed(&caught, self.boxing.op.mods());
        self.walk.forget();
        self.planes.chosen = None;
        format!("{} box: {} selected", d.kind(), self.selection.items.len())
    }

    /// The box under the finger: a window drawn solid, a crossing dashed.
    pub(super) fn draw_box(&self, painter: &egui::Painter) {
        let Some(d) = self.boxing.drag() else { return };
        let r = d.rect();
        let aqua = crate::theme::AQUA;
        let stroke = egui::Stroke::new(2.0, aqua);
        painter.rect_filled(r, 0.0, aqua.gamma_multiply(0.10));
        if d.crossing() {
            let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
            painter.extend(egui::Shape::dashed_line(&corners, stroke, 8.0, 5.0));
        } else {
            painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Inside);
        }
        painter.text(r.left_top() + egui::vec2(4.0, -4.0), egui::Align2::LEFT_BOTTOM, d.kind(), egui::FontId::proportional(12.0), aqua);
    }

    /// The box bar at the ring's foot: how to draw a box, what it does to the choice, and Done.
    pub(super) fn box_bar(&mut self, ctx: &egui::Context, v: &View) {
        let lines = [
            ("Box select: drag left to right for a window, right to left for a crossing".to_string(), crate::theme::AQUA),
            (format!("{} chosen · pinch still zooms", self.selection.items.len()), crate::theme::INK_DIM),
        ];
        let icons = [Icon::Select, Icon::Add, Icon::Delete];
        let mut buttons: Vec<bar::Button> = BoxOp::ALL.iter().zip(icons).map(|(op, icon)| bar::Button { icon, label: op.label(), checked: self.boxing.op == *op, enabled: true }).collect();
        buttons.push(bar::Button { icon: Icon::Check, label: "Done", checked: false, enabled: true });
        match bar::show(ctx, v.rect, v.covered, &lines, &buttons) {
            Some(DONE) => {
                self.boxing.toggle();
                self.status("Box select put away: a drag turns the ring again");
            }
            Some(i) => {
                if let Some(op) = BoxOp::ALL.get(i) {
                    self.boxing.op = *op;
                }
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    #[test]
    fn a_box_follows_the_finger_only_while_the_switch_is_on_and_a_pinch_drops_it() {
        let mut b = Boxing::default();
        b.press();
        assert!(!b.holding() && !b.start(pos2(0.0, 0.0), pos2(20.0, 20.0)), "off, a drag is the ring's");
        b.toggle();
        assert!(b.on);
        b.press();
        assert!(b.holding(), "a finger down on the ring holds it from the press");
        assert!(b.start(pos2(10.0, 10.0), pos2(30.0, 25.0)));
        assert!(b.follow(pos2(80.0, 60.0)));
        assert_eq!(b.lift(pos2(90.0, 70.0)), Some(Drag { from: pos2(10.0, 10.0), to: pos2(90.0, 70.0) }));
        assert!(!b.holding());
        b.press();
        b.start(pos2(0.0, 0.0), pos2(40.0, 40.0));
        b.let_go();
        assert!(!b.holding() && b.lift(pos2(50.0, 50.0)).is_none(), "a second finger drops the box unfinished");
        b.toggle();
        assert!(!b.on && !b.holding());
    }
}
