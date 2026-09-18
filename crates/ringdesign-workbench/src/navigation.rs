//! Ring-relative views and the shared navigator: a view cube in the corner
//! of the viewport, in the manner of a CAD package's.
//!
//! The cube turns with the camera. A tap on a face looks straight at that
//! side of the ring, on an edge between two faces, on a corner between three;
//! a drag on the cube orbits; the arrows round it step to the face on that
//! side; a double tap goes home. Its faces are named for the ring rather than
//! for the world — FACE is wherever the head is — and for the *screen*: with
//! FACE toward you, the cube's right-hand face reads RIGHT and shows the
//! shoulder on your right.
use crate::focus::view_axes;
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
    /// `[yaw, pitch]`. Screen right is the way theta climbs from the head,
    /// so the shoulder on the viewer's right is looked at from `head + 90°`.
    pub fn angles(self, head_degrees: f32) -> [f32; 2] {
        let head = head_degrees.to_radians();
        match self {
            Self::Face => [head, 0.0],
            Self::Left => [head - FRAC_PI_2, 0.0],
            Self::Right => [head + FRAC_PI_2, 0.0],
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
    /// Quarter turns round the finger axis; positive carries the camera to screen right.
    Turn(f32),
    Tilt(f32),
    /// The same view from the other shoulder: the camera reflected in the
    /// plane through the finger axis and the head. The face never turns away.
    Mirror,
    /// The diametrically opposite side.
    Opposite,
    /// Straight to `[yaw, pitch]`.
    Look([f32; 2]),
    /// A drag on the cube, in points.
    Orbit(egui::Vec2),
}
impl Action {
    pub fn angles(self, yaw: f32, pitch: f32, head_degrees: f32) -> [f32; 2] {
        let [mut yaw, mut pitch] = match self {
            Self::View(v) => v.angles(head_degrees),
            Self::Turn(quarter) => [yaw + quarter * FRAC_PI_2, pitch],
            Self::Tilt(quarter) => [yaw, pitch + quarter * FRAC_PI_2],
            Self::Mirror => [2.0 * head_degrees.to_radians() - yaw, pitch],
            Self::Opposite => [yaw + PI, -pitch],
            Self::Look(angles) => angles,
            Self::Orbit(d) => [yaw - d.x * 0.012, (pitch + d.y * 0.012).clamp(-FRAC_PI_2 + 0.001, FRAC_PI_2 - 0.001)],
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

    /// Whether the view should recentre on the ring and may ease into the
    /// pose. A drag follows the finger and leaves the pan alone.
    pub fn recentres(self) -> bool {
        !matches!(self, Self::Orbit(_))
    }
}

pub struct Response {
    pub rect: Rect,
    pub action: Option<Action>,
    pub changed: bool,
    /// Exposed to the phone's layout report and device review tools.
    pub controls: Vec<(&'static str, Rect)>,
}

/// One face of the cube: its label, the view it stands for, its outward
/// normal and the two axes across it, all in world space.
struct CubeFace {
    label: &'static str,
    view: View,
    n: [f32; 3],
    a: [f32; 3],
    b: [f32; 3],
}

fn cube_faces(head: f32) -> [CubeFace; 6] {
    let h = [head.cos(), head.sin(), 0.0];
    let r = [-head.sin(), head.cos(), 0.0];
    let z = [0.0, 0.0, 1.0];
    let neg = |v: [f32; 3]| [-v[0], -v[1], -v[2]];
    [
        CubeFace { label: "FACE", view: View::Face, n: h, a: r, b: z },
        CubeFace { label: "PALM", view: View::Base, n: neg(h), a: r, b: z },
        CubeFace { label: "RIGHT", view: View::Right, n: r, a: h, b: z },
        CubeFace { label: "LEFT", view: View::Left, n: neg(r), a: h, b: z },
        CubeFace { label: "BORE", view: View::Opening, n: z, a: h, b: r },
        CubeFace { label: "BACK", view: View::Back, n: neg(z), a: h, b: r },
    ]
}

/// Where the edge and corner bands of a face begin, in face units.
const CUBE_BAND: f32 = 0.52;

/// What a point on the cube stands for: the face under it and which of its
/// edges or corners, as signs along the face's two axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubeHit {
    pub face: usize,
    pub along: [i8; 2],
}

/// The cube as drawn for one camera: the projection of a world vector, and
/// the faces that show.
pub struct Cube {
    centre: Pos2,
    half: f32,
    s: [f32; 3],
    u: [f32; 3],
    d: [f32; 3],
    faces: [CubeFace; 6],
    head: f32,
}

impl Cube {
    pub fn new(centre: Pos2, half: f32, angles: [f32; 2], head_degrees: f32) -> Self {
        let (s, u) = view_axes(angles[0], angles[1]);
        let d = [angles[1].cos() * angles[0].cos(), angles[1].cos() * angles[0].sin(), angles[1].sin()];
        let head = head_degrees.to_radians();
        Self { centre, half, s, u, d, faces: cube_faces(head), head }
    }

    fn flat(&self, w: [f32; 3]) -> egui::Vec2 {
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        egui::vec2(dot(w, self.s), -dot(w, self.u)) * self.half
    }

    fn facing(&self, i: usize) -> f32 {
        let n = self.faces[i].n;
        n[0] * self.d[0] + n[1] * self.d[1] + n[2] * self.d[2]
    }

    /// Faces toward the camera, nearest last so a painter can run in order.
    fn visible(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..6).filter(|i| self.facing(*i) > 0.04).collect();
        v.sort_by(|a, b| self.facing(*a).total_cmp(&self.facing(*b)));
        v
    }

    /// A point on a face: `along` in face units, -1..1 each way.
    fn point(&self, i: usize, along: [f32; 2]) -> Pos2 {
        let f = &self.faces[i];
        self.centre + self.flat(f.n) + self.flat(f.a) * along[0] + self.flat(f.b) * along[1]
    }

    pub fn hit(&self, p: Pos2) -> Option<CubeHit> {
        for &i in self.visible().iter().rev() {
            let f = &self.faces[i];
            let (a, b) = (self.flat(f.a), self.flat(f.b));
            let det = a.x * b.y - a.y * b.x;
            if det.abs() < 1e-3 {
                continue;
            }
            let q = p - (self.centre + self.flat(f.n));
            let (x, y) = ((q.x * b.y - q.y * b.x) / det, (a.x * q.y - a.y * q.x) / det);
            if x.abs() <= 1.0 && y.abs() <= 1.0 {
                let band = |v: f32| if v > CUBE_BAND { 1 } else if v < -CUBE_BAND { -1 } else { 0 };
                return Some(CubeHit { face: i, along: [band(x), band(y)] });
            }
        }
        None
    }

    /// The view a hit stands for: down the sum of the face normals it touches.
    pub fn look(&self, hit: CubeHit) -> [f32; 2] {
        let f = &self.faces[hit.face];
        if hit.along == [0, 0] {
            return f.view.angles(self.head.to_degrees());
        }
        let mut v = f.n;
        for k in 0..3 {
            v[k] += f.a[k] * f32::from(hit.along[0]) + f.b[k] * f32::from(hit.along[1]);
        }
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
        let yaw = if v[0].hypot(v[1]) > 0.05 { v[1].atan2(v[0]) } else { self.head - PI };
        [yaw, (v[2] / len).clamp(-1.0, 1.0).asin()]
    }

    fn region(&self, hit: CubeHit) -> Vec<Pos2> {
        let span = |s: i8| match s {
            0 => [-CUBE_BAND, CUBE_BAND],
            1 => [CUBE_BAND, 1.0],
            _ => [-1.0, -CUBE_BAND],
        };
        let (x, y) = (span(hit.along[0]), span(hit.along[1]));
        vec![self.point(hit.face, [x[0], y[0]]), self.point(hit.face, [x[1], y[0]]), self.point(hit.face, [x[1], y[1]]), self.point(hit.face, [x[0], y[1]])]
    }

    fn paint(&self, p: &egui::Painter, hover: Option<CubeHit>) {
        let ink = Color32::from_rgb(143, 137, 156);
        let aqua = Color32::from_rgb(43, 226, 214);
        let pink = Color32::from_rgb(255, 61, 139);
        for i in self.visible() {
            let toward = self.facing(i).clamp(0.0, 1.0);
            let shade = (30.0 + 26.0 * toward) as u8;
            let quad: Vec<Pos2> = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]].iter().map(|c| self.point(i, *c)).collect();
            let rim = if i == 0 { Stroke::new(1.5, pink) } else { Stroke::new(1.0, ink.gamma_multiply(0.8)) };
            p.add(egui::Shape::convex_polygon(quad, Color32::from_rgb(shade, shade - 6, shade + 10), rim));
            if let Some(h) = hover.filter(|h| h.face == i) {
                p.add(egui::Shape::convex_polygon(self.region(h), aqua.gamma_multiply(0.45), Stroke::NONE));
            }
            if toward > 0.42 {
                let size = 7.0 + 2.5 * toward;
                p.text(self.point(i, [0.0, 0.0]), egui::Align2::CENTER_CENTER, self.faces[i].label, egui::FontId::proportional(size), if i == 0 { Color32::WHITE } else { Color32::from_rgb(215, 212, 224) }.gamma_multiply(0.35 + 0.65 * toward));
            }
        }
    }
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
                    let (r, _) = ui.allocate_exact_size(egui::vec2(96.0, 92.0), egui::Sense::hover());
                    let ink = Color32::from_rgb(143, 137, 156);
                    let aqua = Color32::from_rgb(43, 226, 214);
                    let cube = Cube::new(r.center(), 19.0, angles, head);
                    let body = ui.interact(Rect::from_center_size(r.center(), egui::vec2(62.0, 62.0)), id.with("view-cube"), egui::Sense::click_and_drag());
                    controls.push(("View cube", body.rect));
                    let pointer = body.hover_pos().or(body.interact_pointer_pos());
                    let under = pointer.and_then(|p| cube.hit(p));
                    cube.paint(ui.painter(), under.filter(|_| !body.dragged()));
                    let held = icons::help(&body, "View cube", "Turns with the ring: FACE is the head, PALM the underside, BORE and BACK the two openings.", "Tap a face to look straight at it, an edge or a corner to look between faces. Drag the cube to orbit. Double-tap for the home view.", "The arrows step to the face on that side. Mirror shows the same view from the other shoulder.");
                    if body.dragged() {
                        action = Some(Action::Orbit(body.drag_delta()));
                    } else if body.double_clicked() {
                        action = Some(Action::View(View::ThreeQuarter));
                    } else if body.clicked() && !held {
                        if let Some(hit) = body.interact_pointer_pos().and_then(|p| cube.hit(p)) {
                            action = Some(Action::Look(cube.look(hit)));
                        }
                    }
                    // The arrows round the cube: the face on that side.
                    for (name, at, tip, command) in [
                        ("Turn left", egui::vec2(-40.0, 0.0), [egui::vec2(-5.0, 0.0), egui::vec2(4.0, -6.0), egui::vec2(4.0, 6.0)], Action::Turn(-1.0)),
                        ("Turn right", egui::vec2(40.0, 0.0), [egui::vec2(5.0, 0.0), egui::vec2(-4.0, -6.0), egui::vec2(-4.0, 6.0)], Action::Turn(1.0)),
                        ("Tilt up", egui::vec2(0.0, -39.0), [egui::vec2(0.0, -5.0), egui::vec2(-6.0, 4.0), egui::vec2(6.0, 4.0)], Action::Tilt(1.0)),
                        ("Tilt down", egui::vec2(0.0, 39.0), [egui::vec2(0.0, 5.0), egui::vec2(-6.0, -4.0), egui::vec2(6.0, -4.0)], Action::Tilt(-1.0)),
                    ] {
                        let centre = r.center() + at;
                        let size = if at.x == 0.0 { egui::vec2(40.0, 14.0) } else { egui::vec2(16.0, 40.0) };
                        let hit = ui.interact(Rect::from_center_size(centre, size), id.with(name), egui::Sense::click());
                        ui.painter().add(egui::Shape::convex_polygon(tip.iter().map(|t| centre + *t).collect(), if hit.hovered() { aqua } else { ink }, Stroke::NONE));
                        if hit.clicked() { action = Some(command); }
                        controls.push((name, hit.rect));
                    }
                    ui.horizontal(|ui| {
                        for (icon, name, tip, command) in [
                            (Icon::Reset, "Home view", "The three-quarter view of the head.", Action::View(View::ThreeQuarter)),
                            (Icon::Mirror, "Mirror", "The same view from the other shoulder. The face never turns away.", Action::Mirror),
                        ] {
                            let b = icons::compact(ui, icon, false);
                            controls.push((name, b.rect));
                            if b.response.clone().on_hover_text(tip).clicked() { action = Some(command); }
                        }
                        let menu = icons::compact(ui, Icon::View, false);
                        controls.push(("Views", menu.rect));
                        egui::Popup::menu(&menu.response).open_memory(if menu.clicked() { Some(egui::SetOpenCommand::Toggle) } else { None }).show(|ui| {
                            for view in View::ALL {
                                if ui.button(view.label()).clicked() { action = Some(Action::View(view)); ui.close(); }
                            }
                            ui.separator();
                            if ui.button("Opposite side").clicked() { action = Some(Action::Opposite); ui.close(); }
                            ui.small(if settings.locked { "Locked · empty space pans" } else { "Free · empty space orbits" });
                        });
                    });
                    ui.horizontal(|ui| {
                        let lock = icons::compact(ui, if settings.locked { Icon::Locked } else { Icon::Unlocked }, settings.locked);
                        controls.push(("Lock view", lock.rect));
                        if lock.clicked() { settings.locked = !settings.locked; }
                        let loupe = icons::compact(ui, Icon::Magnifier, settings.magnifier);
                        controls.push(("Magnifier", loupe.rect));
                        if loupe.clicked() { settings.magnifier = !settings.magnifier; }
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

/// Full magnification, and the floor it falls back to for a placement too wide
/// to hold at 2.5x. Below that a lens stops being a magnifier, and a stamp that
/// large reads in the viewport without one.
pub const LOUPE_ZOOM: f32 = 2.5;
const LOUPE_MIN_ZOOM: f32 = 1.5;
/// Points of clear band between the placement's outermost point and the rim.
const LOUPE_MARGIN: f32 = 7.0;

/// Keep the contact at the centre of the lens. Near the top, put the lens to
/// either side, never underneath the finger or the system bars. `reach` is how
/// far the placement extends from the contact in points; the lens grows to hold
/// it at full magnification, up to 45% of the shorter side of the viewport, and
/// only as far as a spot clear of the tools allows — an occluded palette is
/// worse than a smaller crop, and an occluded finger is worse than both.
pub fn loupe_rect(contact: Pos2, bounds: Rect, obstacles: &[Rect], reach: f32) -> Rect {
    let cap = (bounds.size().min_elem() * 0.45).clamp(120.0, 200.0);
    let fits = |d: f32| {
        d.min(bounds.width() - 8.0)
            .min(bounds.height() - 8.0)
            .max(32.0)
    };
    let want = fits(((reach.max(0.0) + LOUPE_MARGIN) * 2.0 * LOUPE_ZOOM).clamp(120.0, cap));
    let floor = fits(120.0).min(want);
    let mut fallback = None;
    for step in 0..5 {
        let rect = place(contact, bounds, obstacles, want + (floor - want) * step as f32 * 0.25);
        let covered = covered_area(rect, obstacles);
        if covered <= 0.0 {
            return rect;
        }
        if fallback.is_none_or(|(least, _)| covered < least) {
            fallback = Some((covered, rect));
        }
    }
    fallback.map(|(_, rect)| rect).unwrap_or(bounds)
}

fn covered_area(rect: Rect, obstacles: &[Rect]) -> f32 {
    obstacles
        .iter()
        .map(|o| rect.intersect(*o).area().max(0.0))
        .sum()
}

fn place(contact: Pos2, bounds: Rect, obstacles: &[Rect], diameter: f32) -> Rect {
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
            let finger = (radius + 42.0 - rect.center().distance(contact)).max(0.0);
            (finger * 10000.0
                + covered_area(*rect, obstacles)
                + rect.center().distance(contact + positions[0])) as i64
        })
        .unwrap()
}

/// The magnification a lens of this size can hold: full, reduced so the
/// placement's own edges land inside the circle rather than being cropped by it.
pub fn loupe_zoom(lens: Rect, reach: f32) -> f32 {
    let room = lens.width() * 0.5 - LOUPE_MARGIN;
    if reach <= 0.0 || room <= 0.0 {
        return LOUPE_ZOOM;
    }
    (room / reach).clamp(LOUPE_MIN_ZOOM, LOUPE_ZOOM)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn direction(a: [f32; 2]) -> [f32; 3] {
        [a[1].cos() * a[0].cos(), a[1].cos() * a[0].sin(), a[1].sin()]
    }
    #[test]
    fn ring_views_follow_head_and_opposite_is_an_involution() {
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
    fn mirror_swaps_the_shoulders_and_never_turns_the_face_away() {
        for head in [0.0f32, 90.0, 137.0, 270.0] {
            let h = direction([head.to_radians(), 0.0]);
            let toward_face = |a: [f32; 2]| direction(a).into_iter().zip(h).map(|(a, b)| a * b).sum::<f32>();
            // Face on, the mirror is the same view; the old flip showed the palm.
            let face = View::Face.angles(head);
            let mirrored = Action::Mirror.angles(face[0], face[1], head);
            assert!((toward_face(mirrored) - 1.0).abs() < 1e-5);
            // Left shoulder <-> right shoulder, and the three-quarter view to its twin.
            let left = View::Left.angles(head);
            let m = Action::Mirror.angles(left[0], left[1], head);
            for (a, b) in direction(m).into_iter().zip(direction(View::Right.angles(head))) {
                assert!((a - b).abs() < 1e-5);
            }
            for start in [View::ThreeQuarter.angles(head), [head.to_radians() + 0.9, 0.4]] {
                let m = Action::Mirror.angles(start[0], start[1], head);
                assert!((toward_face(m) - toward_face(start)).abs() < 1e-5, "as much of the face as before");
                assert!((m[1] - start[1]).abs() < 1e-5);
                let back = Action::Mirror.angles(m[0], m[1], head);
                for (a, b) in direction(back).into_iter().zip(direction(start)) {
                    assert!((a - b).abs() < 1e-5, "twice is where it began");
                }
            }
        }
    }
    #[test]
    fn the_cube_reads_like_the_screen_and_its_faces_edges_and_corners_look_where_they_point() {
        let head = 90.0f32;
        let centre = egui::pos2(100.0, 100.0);
        let cube = Cube::new(centre, 20.0, View::Face.angles(head), head);
        // Face on: FACE in the middle, RIGHT off to the right, BORE above.
        let middle = cube.hit(centre).expect("the middle of the cube");
        assert_eq!((cube.faces[middle.face].label, middle.along), ("FACE", [0, 0]));
        assert_eq!(cube.look(middle), View::Face.angles(head));
        let labels: Vec<&str> = cube.visible().iter().map(|i| cube.faces[*i].label).collect();
        assert_eq!(labels, ["FACE"], "a square-on view shows one face");
        // An edge band: FACE and RIGHT together is 45 degrees toward the viewer's right.
        let edge = cube.hit(centre + egui::vec2(16.0, 0.0)).unwrap();
        assert_eq!(edge.along, [1, 0]);
        let look = cube.look(edge);
        assert!((look[0] - (head.to_radians() + FRAC_PI_2 * 0.5)).abs() < 1e-4 && look[1].abs() < 1e-5, "{look:?}");
        // A corner: three faces, an isometric view from above the right shoulder.
        let corner = cube.hit(centre + egui::vec2(16.0, -16.0)).unwrap();
        assert_eq!(corner.along, [1, 1]);
        let look = cube.look(corner);
        assert!(look[0] > head.to_radians() && (look[1] - (1.0f32 / 3.0f32.sqrt()).asin()).abs() < 1e-4, "{look:?}");
        assert!(cube.hit(centre + egui::vec2(40.0, 0.0)).is_none());
        // From the three-quarter view three faces show, and where each is drawn agrees with its name.
        let iso = Cube::new(centre, 20.0, [head.to_radians() + 0.6, 0.5], head);
        let shown: Vec<&str> = iso.visible().iter().map(|i| iso.faces[*i].label).collect();
        assert_eq!(shown.len(), 3);
        assert!(shown.contains(&"FACE") && shown.contains(&"RIGHT") && shown.contains(&"BORE"), "{shown:?}");
        let at = |label: &str| iso.point(iso.faces.iter().position(|f| f.label == label).unwrap(), [0.0, 0.0]);
        assert!(at("RIGHT").x > at("FACE").x && at("BORE").y < at("FACE").y);
        // The views agree with the cube: tapping RIGHT is the Right shoulder view.
        let right = iso.hit(at("RIGHT")).unwrap();
        assert_eq!(iso.look(right), View::Right.angles(head));
        // The bore faces keep the head at the top of the screen.
        let top = Cube::new(centre, 20.0, [0.3, 1.2], head);
        let bore = top.hit(top.point(4, [0.0, 0.0])).unwrap();
        assert_eq!(top.look(bore), View::Opening.angles(head));
    }
    #[test]
    fn a_drag_on_the_cube_orbits_and_leaves_the_pan_alone() {
        let a = Action::Orbit(egui::vec2(10.0, -5.0)).angles(0.2, 0.1, 90.0);
        assert!(a[0] < 0.2 && a[1] < 0.1);
        assert!(!Action::Orbit(egui::Vec2::ZERO).recentres() && Action::Mirror.recentres());
        let pole = Action::Orbit(egui::vec2(0.0, 900.0)).angles(0.2, 0.1, 90.0);
        assert!(pole[1] < FRAC_PI_2 && (pole[0] - 0.2).abs() < 1e-5, "a drag stops at the pole rather than rolling over it");
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
            for reach in [0.0, 18.0, 44.0, 130.0] {
                let lens = loupe_rect(contact, bounds, &[toolbar], reach);
                assert!(bounds.contains_rect(lens));
                assert!(!lens.expand(20.0).contains(contact));
                assert!(!lens.intersects(toolbar));
            }
        }
        let c = egui::pos2(220.0, 450.0);
        assert!(loupe_rect(c, bounds, &[], 0.0).bottom() < c.y - 40.0);
    }
    #[test]
    fn the_lens_holds_the_whole_placement_until_magnifying_stops_paying() {
        let bounds = Rect::from_min_max(egui::pos2(0.0, 90.0), egui::pos2(410.0, 670.0));
        let contact = egui::pos2(220.0, 450.0);
        let mut last = 0.0;
        for reach in [0.0, 12.0, 20.0, 30.0, 44.0, 60.0, 130.0] {
            let lens = loupe_rect(contact, bounds, &[], reach);
            let zoom = loupe_zoom(lens, reach);
            assert!((LOUPE_MIN_ZOOM..=LOUPE_ZOOM).contains(&zoom));
            // Nothing shrinks as the placement grows, and the magnified edge
            // lands inside the rim wherever the floor has not been reached.
            assert!(lens.width() >= last);
            last = lens.width();
            if zoom > LOUPE_MIN_ZOOM {
                assert!(reach * zoom <= lens.width() * 0.5 - 1.0);
            }
        }
        // The fixed 2.5x crop showed 24 points either side of the contact.
        let reach = 44.0;
        let lens = loupe_rect(contact, bounds, &[], reach);
        assert!(reach * loupe_zoom(lens, reach) <= lens.width() * 0.5);
        assert_eq!(loupe_zoom(loupe_rect(contact, bounds, &[], 0.0), 0.0), LOUPE_ZOOM);
    }
}
