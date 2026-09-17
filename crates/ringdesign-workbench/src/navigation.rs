//! Ring-relative views and a small, shared touch/pen navigator.
use crate::icons::{self, Icon};
use egui::{Color32, Pos2, Rect, Stroke};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub locked: bool,
    pub magnifier: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            locked: false,
            magnifier: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum View {
    Face,
    Left,
    Right,
    Base,
    Opening,
    Back,
    ThreeQuarter,
}
impl View {
    pub const ALL: [Self; 7] = [
        Self::Face,
        Self::Left,
        Self::Right,
        Self::Base,
        Self::Opening,
        Self::Back,
        Self::ThreeQuarter,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Face => "Signet face",
            Self::Left => "Left shoulder",
            Self::Right => "Right shoulder",
            Self::Base => "Underside",
            Self::Opening => "Through opening",
            Self::Back => "Other opening",
            Self::ThreeQuarter => "Signet 3/4",
        }
    }
    pub fn angles(self, head_degrees: f32) -> [f32; 2] {
        let head = head_degrees.to_radians();
        match self {
            Self::Face => [head, 0.0],
            Self::Left => [head + FRAC_PI_2, 0.0],
            Self::Right => [head - FRAC_PI_2, 0.0],
            Self::Base => [head + PI, 0.0],
            Self::Opening => [head - PI, FRAC_PI_2],
            Self::Back => [head - PI, -FRAC_PI_2],
            Self::ThreeQuarter => [head - 0.48, -0.55],
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Action {
    View(View),
    Turn(f32),
    Tilt(f32),
    Opposite,
}
impl Action {
    pub fn angles(self, yaw: f32, pitch: f32, head_degrees: f32) -> [f32; 2] {
        let [mut yaw, mut pitch] = match self {
            Self::View(v) => v.angles(head_degrees),
            Self::Turn(quarter) => [yaw + quarter * FRAC_PI_2, pitch],
            Self::Tilt(quarter) => [yaw, pitch + quarter * FRAC_PI_2],
            Self::Opposite => [yaw + PI, -pitch],
        };
        pitch = (pitch + PI).rem_euclid(TAU) - PI;
        if pitch > FRAC_PI_2 {
            pitch = PI - pitch;
            yaw += PI;
        }
        if pitch < -FRAC_PI_2 {
            pitch = -PI - pitch;
            yaw += PI;
        }
        [(yaw + PI).rem_euclid(TAU) - PI, pitch]
    }
}

pub struct Response {
    pub rect: Rect,
    pub action: Option<Action>,
    pub changed: bool,
    /// Exposed to the phone's layout report and device review tools.
    pub controls: Vec<(&'static str, Rect)>,
}

pub fn show(
    ui: &egui::Ui,
    viewport: Rect,
    id: egui::Id,
    settings: &mut Settings,
    angles: [f32; 2],
    head: f32,
) -> Response {
    let before = *settings;
    let mut action = None;
    let mut controls = vec![];
    let area = egui::Area::new(id.with("ring-navigator"))
        .order(egui::Order::Foreground).movable(false)
        .fixed_pos(viewport.right_top() + egui::vec2(-110.0, 7.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::new().fill(Color32::from_rgba_unmultiplied(24, 22, 32, 238))
                .stroke(Stroke::new(1.0, Color32::from_rgb(70, 61, 77)))
                .corner_radius(10).inner_margin(5).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                    let (r, _) = ui.allocate_exact_size(egui::vec2(96.0, 78.0), egui::Sense::hover());
                    let at = |x, y| r.min + egui::vec2(x, y);
                    let p = ui.painter();
                    let ink = Color32::from_rgb(143, 137, 156);
                    let aqua = Color32::from_rgb(43, 226, 214);
                    let pink = Color32::from_rgb(255, 61, 139);
                    // A signet, with its face and shoulders as selectable parts.
                    p.add(egui::epaint::EllipseShape::stroke(at(48.0, 45.0), egui::vec2(29.0, 30.0), Stroke::new(8.0, ink)));
                    p.add(egui::epaint::EllipseShape::stroke(at(48.0, 45.0), egui::vec2(24.0, 25.0), Stroke::new(1.0, aqua.gamma_multiply(0.45))));
                    p.add(egui::Shape::convex_polygon(vec![at(29.0, 5.0), at(66.0, 5.0), at(75.0, 16.0), at(64.0, 27.0), at(31.0, 27.0), at(21.0, 16.0)], Color32::from_rgb(44, 35, 51), Stroke::new(1.5, pink)));
                    for (view, centre, size) in [
                        (View::Face, at(48.0, 16.0), egui::vec2(58.0, 25.0)),
                        (View::Left, at(18.0, 44.0), egui::vec2(28.0, 28.0)),
                        (View::Right, at(78.0, 44.0), egui::vec2(28.0, 28.0)),
                        (View::Base, at(48.0, 68.0), egui::vec2(30.0, 20.0)),
                        (View::Opening, at(48.0, 44.0), egui::vec2(30.0, 27.0)),
                    ] {
                        let hit = ui.interact(Rect::from_center_size(centre, size), id.with(view.label()), egui::Sense::click());
                        let target = view.angles(head);
                        let selected = ((angles[0] - target[0] + PI).rem_euclid(TAU) - PI).abs() < 0.02 && (angles[1] - target[1]).abs() < 0.02;
                        ui.painter().circle_filled(centre, if selected { 4.0 } else { 2.5 }, if selected || hit.hovered() { aqua } else { ink });
                        let held = icons::help(&hit, view.label(), "Look straight at this part of the ring.", "Tap a part of the small ring. The arrows turn by 90°; Flip shows the opposite side.", "Lock the angle for precise decoration. Empty-space drags then pan.");
                        if hit.clicked() && !held { action = Some(Action::View(view)); }
                        controls.push((view.label(), hit.rect));
                    }
                    ui.horizontal(|ui| {
                        for (icon, name, command) in [(Icon::TurnLeft, "Turn left", Action::Turn(1.0)), (Icon::Opposite, "Opposite", Action::Opposite), (Icon::TurnRight, "Turn right", Action::Turn(-1.0))] {
                            let b = icons::compact(ui, icon, false);
                            controls.push((name, b.rect));
                            if b.clicked() { action = Some(command); }
                        }
                    });
                    ui.horizontal(|ui| {
                        let lock = icons::compact(ui, if settings.locked { Icon::Locked } else { Icon::Unlocked }, settings.locked);
                        controls.push(("Lock view", lock.rect));
                        if lock.clicked() { settings.locked = !settings.locked; }
                        let loupe = icons::compact(ui, Icon::Magnifier, settings.magnifier);
                        controls.push(("Magnifier", loupe.rect));
                        if loupe.clicked() { settings.magnifier = !settings.magnifier; }
                        let menu = icons::compact(ui, Icon::View, false);
                        controls.push(("Views", menu.rect));
                        egui::Popup::menu(&menu.response).open_memory(if menu.clicked() { Some(egui::SetOpenCommand::Toggle) } else { None }).show(|ui| {
                            for view in View::ALL {
                                if ui.button(view.label()).clicked() { action = Some(Action::View(view)); ui.close(); }
                            }
                            ui.separator();
                            for (name, command) in [("Tilt up 90°", Action::Tilt(1.0)), ("Tilt down 90°", Action::Tilt(-1.0))] {
                                if ui.button(name).clicked() { action = Some(command); ui.close(); }
                            }
                            ui.small(if settings.locked { "Locked · empty space pans" } else { "Free · empty space orbits" });
                        });
                    });
                });
        });
    Response {
        rect: area.response.rect,
        action,
        changed: before != *settings,
        controls,
    }
}

/// Keep the contact at the centre of the lens. Near the top, put the lens to
/// either side, never underneath the finger or the system bars.
pub fn loupe_rect(contact: Pos2, bounds: Rect, obstacles: &[Rect]) -> Rect {
    let diameter = 120.0_f32
        .min(bounds.width() - 8.0)
        .min(bounds.height() - 8.0)
        .max(32.0);
    let radius = diameter * 0.5;
    let offset = radius + 52.0;
    let allowed = bounds.shrink(radius + 3.0);
    let positions = [
        egui::vec2(0.0, -offset),
        egui::vec2(offset, 0.0),
        egui::vec2(-offset, 0.0),
        egui::vec2(offset, -offset),
        egui::vec2(-offset, -offset),
    ];
    positions
        .into_iter()
        .map(|offset| {
            let centre = allowed.clamp(contact + offset);
            Rect::from_center_size(centre, egui::Vec2::splat(diameter))
        })
        .min_by_key(|rect| {
            let covered: f32 = obstacles
                .iter()
                .map(|o| rect.intersect(*o).area().max(0.0))
                .sum();
            let finger = (radius + 42.0 - rect.center().distance(contact)).max(0.0);
            (finger * 10000.0 + covered + rect.center().distance(contact + positions[0])) as i64
        })
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn direction(a: [f32; 2]) -> [f32; 3] {
        [a[1].cos() * a[0].cos(), a[1].cos() * a[0].sin(), a[1].sin()]
    }
    #[test]
    fn ring_views_follow_head_and_flip_is_an_involution() {
        for head in [0.0, 90.0, 137.0, 270.0] {
            let face = View::Face.angles(head);
            let left = View::Left.angles(head);
            let a = direction(face);
            let b = direction(left);
            assert!(a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>().abs() < 1e-5);
            for view in View::ALL {
                let start = view.angles(head);
                let opposite = Action::Opposite.angles(start[0], start[1], head);
                for (a, b) in direction(start).into_iter().zip(direction(opposite)) {
                    assert!((a + b).abs() < 1e-5);
                }
                let end = Action::Opposite.angles(opposite[0], opposite[1], head);
                for (a, b) in direction(start).into_iter().zip(direction(end)) {
                    assert!((a - b).abs() < 1e-5);
                }
            }
        }
    }
    #[test]
    fn quarter_turns_preserve_angle_and_tilts_cross_poles() {
        let start = [0.37, -0.41];
        let mut a = start;
        for _ in 0..4 {
            a = Action::Turn(1.0).angles(a[0], a[1], 90.0);
        }
        for (a, b) in direction(start).into_iter().zip(direction(a)) {
            assert!((a - b).abs() < 1e-5);
        }
        let a = Action::Tilt(1.0).angles(0.0, 0.4, 90.0);
        assert!(a[1] <= FRAC_PI_2);
        let before = direction([0.0, 0.4]);
        let after = direction(a);
        assert!(
            before
                .into_iter()
                .zip(after)
                .map(|(a, b)| a * b)
                .sum::<f32>()
                .abs()
                < 1e-5
        );
    }
    #[test]
    fn loupe_stays_visible_and_clear_of_contact_and_tools() {
        let bounds = Rect::from_min_max(egui::pos2(0.0, 90.0), egui::pos2(410.0, 670.0));
        let toolbar = Rect::from_min_size(egui::pos2(2.0, 100.0), egui::vec2(90.0, 330.0));
        for contact in [
            egui::pos2(220.0, 450.0),
            egui::pos2(210.0, 110.0),
            egui::pos2(200.0, 630.0),
        ] {
            let lens = loupe_rect(contact, bounds, &[toolbar]);
            assert!(bounds.contains_rect(lens));
            assert!(!lens.expand(20.0).contains(contact));
            assert!(!lens.intersects(toolbar));
        }
        let c = egui::pos2(220.0, 450.0);
        assert!(loupe_rect(c, bounds, &[]).bottom() < c.y - 40.0);
    }
}
