//! Orbit camera with an orthographic projection.
//!
//! World up is +Z, the finger axis, so a pitch of 90 degrees looks straight
//! down at the face of the ring. Matrices are column-major for OpenGL.

use ringdesign_core::mesh::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandardView {
    Face,
    Edge,
    Profile,
    Iso,
}

impl StandardView {
    pub const ALL: &'static [StandardView] = &[
        StandardView::Face,
        StandardView::Edge,
        StandardView::Profile,
        StandardView::Iso,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StandardView::Face => "Face",
            StandardView::Edge => "Edge",
            StandardView::Profile => "Profile",
            StandardView::Iso => "3/4",
        }
    }

    /// `(yaw, pitch)` in radians.
    fn angles(self) -> (f32, f32) {
        use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
        match self {
            StandardView::Face => (-FRAC_PI_2, FRAC_PI_2 - 0.001),
            StandardView::Edge => (-FRAC_PI_2, 0.0),
            StandardView::Profile => (0.0, 0.0),
            StandardView::Iso => (-FRAC_PI_2 - FRAC_PI_4 * 0.5, FRAC_PI_4 * 0.85),
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    /// Zoom factor: 1.0 frames the model exactly.
    pub zoom: f32,
    /// What the camera orbits and the view's window is centred on less the pan: the ring's middle until a fit moves it.
    pub target: [f32; 3],
    pub pan: [f32; 2],
    /// Radius of the fitted bounding sphere, mm.
    radius: f32,
    /// Turn about the view axis, radians: what lets the ring be seen upside down.
    #[serde(default)]
    pub roll: f32,
    /// The fitted bounds' middle, which a named view orbits; `None` while the target has never left it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    home: Option<[f32; 3]>,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        let (yaw, pitch) = StandardView::Iso.angles();
        Self {
            yaw,
            pitch,
            zoom: 1.0,
            target: [0.0; 3],
            pan: [0.0; 2],
            radius: 12.0,
            roll: 0.0,
            home: None,
        }
    }
}

/// The middle of a box and the radius of the sphere round it, mm.
fn sphere(min: Vec3, max: Vec3) -> ([f32; 3], f32) {
    let ext = [max.0 - min.0, max.1 - min.1, max.2 - min.2];
    ([(min.0 + max.0) * 0.5, (min.1 + max.1) * 0.5, (min.2 + max.2) * 0.5], 0.5 * dot(ext, ext).sqrt())
}

impl OrbitCamera {
    /// Recentre on new bounds, keeping the current orientation and zoom.
    pub fn fit(&mut self, bounds: Option<(Vec3, Vec3)>) {
        let Some((min, max)) = bounds else { return };
        let (centre, r) = sphere(min, max);
        self.target = centre;
        self.home = Some(centre);
        self.radius = r.max(1.0);
    }

    pub fn reset(&mut self) {
        let keep = (self.radius, self.home());
        *self = Self::default();
        self.radius = keep.0;
        self.target = keep.1;
        self.home = Some(keep.1);
    }

    /// The middle a named view orbits.
    fn home(&self) -> [f32; 3] {
        self.home.unwrap_or(self.target)
    }

    /// Orbits the ring's own middle again, centred on it.
    pub fn centre_home(&mut self) {
        self.target = self.home();
        self.pan = [0.0; 2];
    }

    /// Orbits the ring's own middle again without moving the picture.
    pub fn pivot_home(&mut self) {
        self.pivot_on(self.home());
    }

    /// The view's right, up and forward in the world.
    fn axes(&self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        let f = normalize(sub(self.target, self.eye()));
        let s = normalize(cross(f, self.up()));
        (s, cross(s, f), f)
    }

    /// Orbits `point` from now on without moving the picture: the pan takes up the difference.
    pub fn pivot_on(&mut self, point: [f32; 3]) {
        if !point.iter().all(|v| v.is_finite()) {
            return;
        }
        self.home = Some(self.home());
        let (s, u, _) = self.axes();
        let d = sub(point, self.target);
        self.pan = [self.pan[0] - dot(d, s), self.pan[1] - dot(d, u)];
        self.target = point;
    }

    /// Moves the pivot onto the middle of `bounds` without moving the picture; the pose that then frames them, the look kept, centred, their sphere filling the view's shorter side.
    pub fn framing(&mut self, bounds: (Vec3, Vec3)) -> ringdesign_workbench::focus::Pose {
        let (centre, r) = sphere(bounds.0, bounds.1);
        self.pivot_on(centre);
        let zoom = (self.radius / r.max(1e-3)).clamp(0.15, 24.0);
        ringdesign_workbench::focus::Pose { pan: [0.0; 2], zoom, ..self.pose() }
    }

    /// Takes `bounds` as the ring's own without moving the picture: the radius it is framed by and the middle a named view orbits.
    pub fn refit(&mut self, bounds: (Vec3, Vec3)) {
        let (centre, r) = sphere(bounds.0, bounds.1);
        let r = r.max(1.0);
        self.zoom = (self.zoom * r / self.radius.max(1e-3)).clamp(0.15, 24.0);
        self.radius = r;
        self.home = Some(centre);
    }

    /// The pose as the shared focus and navigator maths hold it.
    pub fn pose(&self) -> ringdesign_workbench::focus::Pose {
        ringdesign_workbench::focus::Pose { yaw: self.yaw, pitch: self.pitch, roll: self.roll, zoom: self.zoom, pan: self.pan }
    }

    pub fn set_pose(&mut self, pose: ringdesign_workbench::focus::Pose) {
        self.yaw = pose.yaw;
        self.pitch = pose.pitch;
        self.roll = pose.roll;
        self.zoom = pose.zoom.clamp(0.15, 24.0);
        self.pan = pose.pan;
    }

    /// The pose that looks straight at a patch of the ring.
    pub fn aimed_at(&self, aim: &ringdesign_workbench::focus::Aim) -> ringdesign_workbench::focus::Pose {
        ringdesign_workbench::focus::aim_pose(self.pose(), self.target, self.radius, aim)
    }

    pub fn set_view(&mut self, view: StandardView) {
        let (yaw, pitch) = view.angles();
        self.yaw = yaw;
        self.pitch = pitch;
        self.target = self.home();
        self.pan = [0.0, 0.0];
        self.roll = 0.0;
    }

    pub fn orbit(&mut self, delta: egui::Vec2) {
        use std::f32::consts::{PI, TAU};
        // A drag is in screen axes; rolled, those are not the camera's own.
        let (sin, cos) = self.roll.sin_cos();
        let delta = egui::vec2(delta.x * cos + delta.y * sin, delta.y * cos - delta.x * sin);
        // Tumbling runs on over the poles. Past one the camera is upside down, and a turn about the
        // finger axis moves the surface the other way across the screen, so the turn does too.
        let over = if self.pitch.cos() < 0.0 { -1.0 } else { 1.0 };
        self.yaw -= delta.x * 0.008 * over;
        self.pitch = (self.pitch + delta.y * 0.008 + PI).rem_euclid(TAU) - PI;
    }

    pub fn zoom_by(&mut self, scroll: f32) {
        self.zoom = (self.zoom * (1.0 + scroll * 0.0015)).clamp(0.15, 24.0);
    }

    pub fn pan_by(&mut self, delta: egui::Vec2, rect: egui::Rect) {
        let scale = self.half_extent() * 2.0 / rect.width().min(rect.height()).max(1.0);
        self.pan[0] -= delta.x * scale;
        self.pan[1] += delta.y * scale;
    }

    /// Half-extent along the shorter viewport dimension, in mm.
    pub fn half_extent(&self) -> f32 {
        self.radius * 1.15 / self.zoom.max(1e-3)
    }

    /// Screen up in world space: the finger axis (or, looking down it, the
    /// way out to the head), turned about the view axis by the roll.
    fn up(&self) -> [f32; 3] {
        // The way the eye moves as it tilts: the finger axis at any tilt short of a pole, out to the head
        // at one, and on past it continuously — so a tumble never flips.
        let (sp, cp) = self.pitch.sin_cos();
        let (sy, cy) = self.yaw.sin_cos();
        let base = [-sp * cy, -sp * sy, cp];
        if self.roll == 0.0 {
            return base;
        }
        let f = normalize(sub(self.target, self.eye()));
        let s = normalize(cross(f, base));
        let u = cross(s, f);
        let (sin, cos) = self.roll.sin_cos();
        [u[0] * cos - s[0] * sin, u[1] * cos - s[1] * sin, u[2] * cos - s[2] * sin]
    }

    fn eye(&self) -> [f32; 3] {
        let d = self.radius * 4.0;
        let (sp, cp) = self.pitch.sin_cos();
        let (sy, cy) = self.yaw.sin_cos();
        [
            self.target[0] + d * cp * cy,
            self.target[1] + d * cp * sy,
            self.target[2] + d * sp,
        ]
    }

    /// `(mvp, normal_matrix)` for the given viewport rect.
    pub fn matrices(&self, rect: egui::Rect) -> ([f32; 16], [f32; 9]) {
        let eye = self.eye();
        let up = self.up();
        let view = look_at(eye, self.target, up);

        let aspect = (rect.width() / rect.height().max(1.0)).max(1e-3);
        let hh = self.half_extent() / aspect.min(1.0);
        let hw = hh * aspect;
        let far = self.radius * 12.0;
        let proj = ortho(
            -hw + self.pan[0],
            hw + self.pan[0],
            -hh + self.pan[1],
            hh + self.pan[1],
            -far,
            far,
        );

        let mvp = mat4_mul(&proj, &view);
        // View rotation is orthonormal, so it is its own normal matrix.
        let normal = [
            view[0], view[1], view[2], view[4], view[5], view[6], view[8], view[9], view[10],
        ];
        (mvp, normal)
    }

    /// World-space ray under a screen position: the orthographic unproject.
    /// Origin sits on the eye plane, direction is the view forward.
    pub fn ray(&self, rect: egui::Rect, pos: egui::Pos2) -> ([f32; 3], [f32; 3]) {
        let eye = self.eye();
        let up = self.up();
        let f = normalize(sub(self.target, eye));
        let s = normalize(cross(f, up));
        let u = cross(s, f);

        let centre = rect.center();
        let half = rect.size() * 0.5;
        let x_ndc = (pos.x - centre.x) / half.x.max(1.0);
        let y_ndc = -(pos.y - centre.y) / half.y.max(1.0);
        let aspect = (rect.width() / rect.height().max(1.0)).max(1e-3);
        let hh = self.half_extent() / aspect.min(1.0);
        let vx = self.pan[0] + x_ndc * hh * aspect;
        let vy = self.pan[1] + y_ndc * hh;

        let origin = [
            eye[0] + s[0] * vx + u[0] * vy,
            eye[1] + s[1] * vx + u[1] * vy,
            eye[2] + s[2] * vx + u[2] * vy,
        ];
        (origin, f)
    }

    /// The projection for one viewport rect, with the matrices resolved once.
    ///
    /// Taken once per overlay rather than per point: the ground grid alone
    /// projects over a hundred, and every one would rebuild the matrices.
    pub fn projector(&self, rect: egui::Rect) -> Projector {
        let (mvp, _) = self.matrices(rect);
        Projector {
            mvp,
            centre: rect.center(),
            half: rect.size() * 0.5,
        }
    }
}

/// World-to-screen for a fixed camera and rect.
#[derive(Clone, Copy)]
pub struct Projector {
    mvp: [f32; 16],
    centre: egui::Pos2,
    half: egui::Vec2,
}

impl Projector {
    pub fn at(&self, p: [f32; 3]) -> egui::Pos2 {
        let m = &self.mvp;
        let x = m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12];
        let y = m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13];
        egui::pos2(
            self.centre.x + x * self.half.x,
            self.centre.y - y * self.half.y,
        )
    }
}

fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [f32; 16] {
    let f = normalize(sub(target, eye));
    let s = normalize(cross(f, up));
    let u = cross(s, f);
    [
        s[0],
        u[0],
        -f[0],
        0.0,
        s[1],
        u[1],
        -f[1],
        0.0,
        s[2],
        u[2],
        -f[2],
        0.0,
        -dot(s, eye),
        -dot(u, eye),
        dot(f, eye),
        1.0,
    ]
}

fn ortho(l: f32, r: f32, b: f32, t: f32, n: f32, f: f32) -> [f32; 16] {
    let (rl, tb, fnn) = ((r - l).max(1e-6), (t - b).max(1e-6), (f - n).max(1e-6));
    [
        2.0 / rl,
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / tb,
        0.0,
        0.0,
        0.0,
        0.0,
        -2.0 / fnn,
        0.0,
        -(r + l) / rl,
        -(t + b) / tb,
        -(f + n) / fnn,
        1.0,
    ]
}

/// Column-major 4x4 product `a * b`.
fn mat4_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[k * 4 + row] * b[col * 4 + k];
            }
            out[col * 4 + row] = sum;
        }
    }
    out
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(a: [f32; 3]) -> [f32; 3] {
    let len = dot(a, a).sqrt();
    if len > 1e-9 {
        [a[0] / len, a[1] / len, a[2] / len]
    } else {
        [0.0, 0.0, 1.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0))
    }

    #[test]
    fn origin_projects_to_the_centre_when_centred() {
        let cam = OrbitCamera::default();
        let p = cam.projector(rect()).at([0.0, 0.0, 0.0]);
        assert!((p.x - rect().center().x).abs() < 1.0);
        assert!((p.y - rect().center().y).abs() < 1.0);
    }

    #[test]
    fn fit_centres_on_the_bounds() {
        let mut cam = OrbitCamera::default();
        cam.fit(Some((Vec3(-2.0, -4.0, -1.0), Vec3(4.0, 2.0, 3.0))));
        assert!((cam.target[0] - 1.0).abs() < 1e-6);
        assert!((cam.target[1] + 1.0).abs() < 1e-6);
        assert!((cam.target[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn fitted_model_stays_visible_in_narrow_and_wide_panes() {
        let mut cam = OrbitCamera::default();
        cam.fit(Some((Vec3(-12.0, -9.0, -7.0), Vec3(15.0, 10.0, 8.0))));
        for size in [egui::vec2(220.0, 850.0), egui::vec2(850.0, 220.0)] {
            let rect = egui::Rect::from_min_size(egui::pos2(335.0, 70.0), size);
            for view in StandardView::ALL {
                cam.set_view(*view);
                for x in [-12.0, 15.0] {
                    for y in [-9.0, 10.0] {
                        for z in [-7.0, 8.0] {
                            assert!(rect.contains(cam.projector(rect).at([x, y, z])));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn picking_and_panning_match_the_projection_in_narrow_panes() {
        let mut cam = OrbitCamera::default();
        let rect = egui::Rect::from_min_size(egui::pos2(335.0, 70.0), egui::vec2(220.0, 850.0));
        let point = [3.0, -2.0, 1.5];
        let screen = cam.projector(rect).at(point);
        let (origin, direction) = cam.ray(rect, screen);
        let error = cross(sub(point, origin), direction);
        assert!(dot(error, error) < 1e-7);
        let delta = egui::vec2(16.0, -23.0);
        cam.pan_by(delta, rect);
        let after = cam.projector(rect).at(point);
        assert!((after - screen - delta).length() < 1e-3);
    }

    #[test]
    fn zoom_is_clamped() {
        let mut cam = OrbitCamera::default();
        for _ in 0..500 {
            cam.zoom_by(1000.0);
        }
        assert!(cam.zoom <= 24.0);
        for _ in 0..1000 {
            cam.zoom_by(-1000.0);
        }
        assert!(cam.zoom >= 0.15);
    }

    #[test]
    fn a_rolled_camera_stands_the_ring_on_its_head_and_still_picks_true() {
        let mut cam = OrbitCamera::default();
        cam.fit(Some((Vec3(-11., -11., -4.), Vec3(11., 14., 4.))));
        cam.set_view(StandardView::Face);
        let r = rect();
        let head = [0.0, 12.0, 0.0];
        let upright = cam.projector(r).at(head);
        cam.roll = std::f32::consts::PI;
        let flipped = cam.projector(r).at(head);
        let c = r.center();
        assert!(((upright - c) + (flipped - c)).length() < 0.01, "a half roll is a point reflection of the screen: {upright:?} {flipped:?}");
        let (o, d) = cam.ray(r, flipped);
        let t = (0..3).map(|k| (head[k] - o[k]) * d[k]).sum::<f32>();
        let miss = (0..3).map(|k| (o[k] + d[k] * t - head[k]).powi(2)).sum::<f32>().sqrt();
        assert!(miss < 0.01, "{miss}");
        let yaw = cam.yaw;
        cam.orbit(egui::vec2(20.0, 0.0));
        assert!(cam.yaw > yaw);
        cam.set_view(StandardView::Face);
        assert_eq!(cam.roll, 0.0);
    }

    #[test]
    fn a_drag_tumbles_on_over_the_poles_without_a_flip() {
        // Rotating the face away runs on past the back view instead of turning the ring over: the head
        // moves a little for a little drag the whole way round, and a full turn comes back to the start.
        let mut cam = OrbitCamera::default();
        cam.fit(Some((Vec3(-11., -11., -4.), Vec3(11., 14., 4.))));
        cam.set_view(StandardView::Edge);
        let r = rect();
        let head = [0.0, 12.0, 0.0];
        let start = cam.projector(r).at(head);
        let mut last = start;
        let step = 4.0;
        let steps = (std::f32::consts::TAU / (step * 0.008)).round() as usize;
        let mut rose = false;
        for _ in 0..steps {
            cam.orbit(egui::vec2(0.0, step));
            let now = cam.projector(r).at(head);
            assert!((now - last).length() < 20.0, "the head jumped from {last:?} to {now:?} at pitch {}", cam.pitch);
            rose |= now.y < start.y - 50.0;
            last = now;
        }
        assert!(rose, "the head went over the top");
        assert!((last - start).length() < 2.0, "a whole turn is home again: {last:?} against {start:?}");
        // Upside down, a sideways drag still carries the surface under the finger.
        let mut up = OrbitCamera::default();
        up.fit(Some((Vec3(-11., -11., -4.), Vec3(11., 14., 4.))));
        up.set_view(StandardView::Edge);
        up.orbit(egui::vec2(0.0, std::f32::consts::PI / 0.008));
        let before = up.projector(r).at(head);
        up.orbit(egui::vec2(10.0, 0.0));
        assert!(up.projector(r).at(head).x > before.x, "the head follows a drag to the right");
    }

    /// A camera fitted to a ring 25 mm across, turned to the three-quarter view.
    fn ringed() -> OrbitCamera {
        let mut cam = OrbitCamera::default();
        cam.fit(Some((Vec3(-11., -11., -3.), Vec3(11., 12.5, 3.))));
        cam
    }

    fn apart(a: egui::Pos2, b: egui::Pos2) -> f32 {
        (a - b).length()
    }

    /// A 2 × 1 × 2 block at the ring's top, its sphere 1.5 mm round.
    const BLOCK: (Vec3, Vec3) = (Vec3(-1.0, 10.4, -1.0), Vec3(1.0, 11.4, 1.0));

    #[test]
    fn a_fit_moves_the_pivot_without_a_jump_centres_what_it_frames_and_turns_about_it() {
        let r = rect();
        let mut cam = ringed();
        cam.zoom = 3.0;
        cam.pan = [4.0, -2.5];
        let middle = [0.0, 10.9, 0.0];
        let probes = [[0.0, 0.0, 0.0], [10.0, 2.0, -1.0], middle, [-5.0, -9.0, 2.0]];
        let before = probes.map(|p| cam.projector(r).at(p));
        let to = cam.framing(BLOCK);
        assert_eq!(cam.target, middle);
        for (p, was) in probes.iter().zip(before) {
            assert!(apart(cam.projector(r).at(*p), was) < 0.01, "{p:?} moved from {was:?}");
        }
        cam.set_pose(to);
        assert!(apart(cam.projector(r).at(middle), r.center()) < 0.01);
        // Its 1.5 mm sphere spans the shorter side less the 15% margin: 600 pt / 2 / 1.15 = 261 pt from the middle.
        let (s, _, _) = cam.axes();
        let reach = 1.5 * apart(cam.projector(r).at([s[0], 10.9 + s[1], s[2]]), r.center());
        assert!((reach - 300.0 / 1.15).abs() < 0.5, "{reach}");
        assert_eq!((to.yaw, to.pitch, to.roll), (ringed().yaw, ringed().pitch, 0.0), "the look is kept");
        // A turn holds the block where it stands; about the ring's middle it would swing away.
        let mut turned = cam;
        turned.orbit(egui::vec2(60.0, 25.0));
        assert!(apart(turned.projector(r).at(middle), r.center()) < 0.05);
        let mut about_ring = cam;
        about_ring.pivot_home();
        assert!(apart(about_ring.projector(r).at(middle), r.center()) < 0.01);
        about_ring.orbit(egui::vec2(60.0, 25.0));
        assert!(apart(about_ring.projector(r).at(middle), r.center()) > 150.0);
        // A speck is framed at the camera's closest, not past it.
        assert_eq!(ringed().framing((Vec3(0.0, 10.0, 0.0), Vec3(0.01, 10.01, 0.01))).zoom, 24.0);
        // New bounds for the ring keep the picture while the radius follows them.
        let mut grown = ringed();
        let was = grown.projector(r).at([5.0, 5.0, 1.0]);
        grown.refit((Vec3(-16., -16., -3.), Vec3(16., 17., 3.)));
        assert!(apart(grown.projector(r).at([5.0, 5.0, 1.0]), was) < 0.01);
        assert!((grown.half_extent() - ringed().half_extent()).abs() < 1e-4);
    }

    #[test]
    fn a_named_view_orbits_the_rings_middle_again_and_a_stored_camera_keeps_it() {
        let r = rect();
        let mut cam = ringed();
        cam.framing(BLOCK);
        let home = [0.0, 0.75, 0.0];
        let mut back = cam;
        let still = back.projector(r).at([4.0, 3.0, 1.0]);
        back.pivot_home();
        assert_eq!(back.target, home);
        assert!(apart(back.projector(r).at([4.0, 3.0, 1.0]), still) < 0.01, "home again without a jump");
        // Centred on home instead, the ring's middle stands in the middle of the view.
        let mut centred = cam;
        centred.centre_home();
        assert_eq!((centred.target, centred.pan), (home, [0.0; 2]));
        assert!(apart(centred.projector(r).at(home), r.center()) < 0.01);
        // Stored and restored, the pivoted camera still knows its home.
        let mut stored: OrbitCamera = serde_json::from_str(&serde_json::to_string(&cam).unwrap()).unwrap();
        assert_eq!(stored.target, [0.0, 10.9, 0.0]);
        stored.set_view(StandardView::Face);
        assert_eq!((stored.target, stored.pan), (home, [0.0; 2]));
        cam.reset();
        assert_eq!((cam.target, cam.pan, cam.zoom), (home, [0.0; 2], 1.0));
        // A camera stored before homes were kept has never left its middle.
        let mut json = serde_json::to_value(ringed()).unwrap();
        json.as_object_mut().unwrap().remove("home");
        let mut old: OrbitCamera = serde_json::from_value(json).unwrap();
        old.set_view(StandardView::Edge);
        assert_eq!(old.target, home);
    }

    #[test]
    fn mat4_mul_has_identity() {
        let id: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let m = ortho(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0);
        let out = mat4_mul(&m, &id);
        for i in 0..16 {
            assert!((out[i] - m[i]).abs() < 1e-6);
        }
    }
}
