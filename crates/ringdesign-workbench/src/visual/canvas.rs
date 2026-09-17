use super::{Tool, Visual};
use egui::{Color32, Pos2, Rect, Stroke};
use ringdesign_core::{
    AlphaLibrary, Mesh, RingDesign,
    interaction::{mould, picking, section},
};

pub(super) const AQUA: Color32 = Color32::from_rgb(43, 226, 214);
pub(super) const AMBER: Color32 = Color32::from_rgb(239, 179, 104);
pub(super) const PINK: Color32 = Color32::from_rgb(255, 61, 139);
const RED: Color32 = Color32::from_rgb(239, 76, 99);

#[derive(Clone, Copy)]
pub struct Pointer {
    pub pressure: f32,
    pub tilt: [f32; 2],
    pub accepted: bool,
    pub navigating: bool,
}
impl Default for Pointer {
    fn default() -> Self {
        Self {
            pressure: 1.0,
            tilt: [0.0; 2],
            accepted: true,
            navigating: false,
        }
    }
}
#[derive(Default)]
pub struct Edit {
    pub drawing: Option<usize>,
    pub layer: Option<usize>,
}
impl Edit {
    pub fn changed(&self) -> bool {
        self.drawing.is_some() || self.layer.is_some()
    }
}

/// egui turns a stationary touch into a secondary click and releases its drag
/// owner. Placement must keep its original contact until the finger lifts.
#[derive(Default)]
pub(super) struct StampContact {
    active: bool,
}
impl StampContact {
    fn update(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        enabled: bool,
    ) -> (Option<Pos2>, bool) {
        let (down, pressed, released, escape, pos) = ui.input(|i| {
            (
                i.pointer.primary_down(),
                i.pointer.button_pressed(egui::PointerButton::Primary),
                i.pointer.button_released(egui::PointerButton::Primary),
                i.key_pressed(egui::Key::Escape),
                i.pointer.interact_pos(),
            )
        });
        if !enabled || escape || (!down && !released) {
            self.active = false;
        }
        if pressed {
            self.active = enabled && (response.is_pointer_button_down_on() || response.clicked());
        }
        let position = if self.active {
            pos
        } else {
            response.hover_pos().or(response.interact_pointer_pos())
        };
        let place = enabled && ((self.active && released) || response.clicked());
        if released {
            self.active = false;
        }
        (position, place)
    }
}

impl Visual {
    fn handle_world(&self, mesh: &Mesh) -> [f64; 3] {
        let (lo, hi) = mesh.bounds().unwrap_or_default();
        let lo = section::coords(lo);
        let hi = section::coords(hi);
        let mut p = std::array::from_fn(|i| (lo[i] + hi[i]) * 0.5);
        p[self.plane.axis.min(2)] = self.plane.offset;
        let edge = (self.plane.axis + 1) % 3;
        p[edge] = hi[edge] + 1.2;
        p
    }
    pub fn blocks_orbit(
        &self,
        pos: Option<Pos2>,
        rect: Rect,
        mesh: &Mesh,
        project: impl Fn([f64; 3]) -> Pos2,
        navigating: bool,
    ) -> bool {
        if navigating {
            return false;
        }
        let Some(pos) = pos.filter(|p| rect.contains(*p)) else {
            return false;
        };
        self.is_painting()
            || self.tool == Tool::Measure
            || (self.tool == Tool::Section
                && (self.section_dragging || pos.distance(project(self.handle_world(mesh))) < 24.0))
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        response: &egui::Response,
        d: &mut RingDesign,
        lib: &AlphaLibrary,
        mesh: &Mesh,
        project: impl Fn([f64; 3]) -> Pos2,
        ray: impl Fn(Pos2) -> ([f32; 3], [f32; 3]),
        pointer: Pointer,
    ) -> Edit {
        let mut edit = Edit::default();
        let painter = ui.painter_at(rect);
        match self.tool {
            Tool::Transform => self
                .transform
                .draw(ui, rect, response, d, lib, mesh, project, ray, pointer),
            Tool::Path => {
                edit.layer = self
                    .path
                    .draw(ui, rect, response, d, lib, mesh, project, ray, pointer);
            }
            Tool::Measure => {
                if pointer.accepted && !pointer.navigating && response.clicked() {
                    if let Some(pos) = response
                        .interact_pointer_pos()
                        .filter(|p| rect.contains(*p))
                    {
                        let (origin, dir) = ray(pos);
                        if let Some((_, world)) = picking::raycast(mesh, origin, dir) {
                            if self.measure.len() == 2 {
                                self.measure.clear();
                            }
                            self.measure.push(world.map(f64::from));
                        }
                    }
                }
                for p in &self.measure {
                    painter.circle_filled(project(*p), 5.0, AQUA);
                }
                if let [a, b] = self.measure.as_slice() {
                    let a2 = project(*a);
                    let b2 = project(*b);
                    painter.line_segment([a2, b2], Stroke::new(2.0, AMBER));
                    tag(
                        &painter,
                        rect,
                        a2.lerp(b2, 0.5),
                        &format!("{:.3} mm", section::distance(*a, *b)),
                        AMBER,
                    );
                }
            }
            Tool::Paint | Tool::Stamp => {
                if pointer.navigating {
                    self.gesture = Default::default();
                    self.stamp_contact = Default::default();
                    return edit;
                }
                let editable = d.graph.is_none() && d.cad.is_none() && pointer.accepted;
                let (position, place_stamp) = if self.tool == Tool::Stamp {
                    self.stamp_contact.update(ui, response, editable)
                } else {
                    (
                        response.hover_pos().or(response.interact_pointer_pos()),
                        false,
                    )
                };
                let position = position.filter(|p| rect.contains(*p));
                if let Some(pos) = position {
                    let (origin, dir) = ray(pos);
                    self.cursor = picking::hit(d, lib, mesh, origin, dir)
                        .filter(|hit| hit.radial_wall_mm >= 0.05);
                } else {
                    self.cursor = None;
                }
                if self.tool == Tool::Paint
                    && editable
                    && (response.is_pointer_button_down_on() || response.clicked())
                {
                    if let Some(hit) = &self.cursor {
                        self.gesture
                            .push(d, &self.brush, hit, pointer.pressure, pointer.tilt);
                    } else {
                        self.gesture.miss();
                    }
                }
                if self.tool == Tool::Paint && ui.input(|i| i.pointer.any_released()) {
                    edit.drawing = self.gesture.commit(d, &self.brush);
                }
                if self.tool == Tool::Stamp && place_stamp {
                    if let Some(hit) = &self.cursor {
                        edit.layer = ringdesign_core::interaction::paint::stamp_pattern(
                            d,
                            lib,
                            &self.brush,
                            hit,
                            self.arrangement,
                        );
                    }
                }
                if self.tool == Tool::Stamp {
                    if let Some(hit) = &self.cursor {
                        let decal = ringdesign_core::field::Decal {
                            theta_deg: hit.theta_deg,
                            v_mm: hit.v_mm,
                            size_mm: self.brush.diameter_mm,
                            rotation_deg: self.brush.rotation_deg,
                            ..Default::default()
                        };
                        crate::artwork::preview(
                            ui,
                            rect,
                            d,
                            lib,
                            &self.brush.stamp,
                            &decal,
                            (self.brush.diameter_mm * 0.06).clamp(0.05, 0.4),
                            false,
                            &project,
                        );
                    }
                }
                if let Some(hit) = &self.cursor {
                    let world = hit.world.map(f64::from);
                    let theta = hit.theta_deg.to_radians();
                    let across = [-theta.sin(), theta.cos(), 0.0];
                    let other = std::array::from_fn(|i| {
                        world[i] + across[i] * self.brush.diameter_mm * 0.5
                    });
                    let centre = project(world);
                    let radius = centre.distance(project(other)).max(4.0);
                    painter.circle_stroke(
                        centre,
                        radius,
                        Stroke::new(1.5, if self.brush.engrave { AMBER } else { AQUA }),
                    );
                    let depth = self.brush.depth_at(
                        d,
                        hit,
                        if self.tool == Tool::Stamp {
                            1.0
                        } else {
                            pointer.pressure
                        },
                    );
                    tag(
                        &painter,
                        rect,
                        centre + egui::vec2(0.0, -radius - 17.0),
                        &format!("{:.2} mm / {:.2} mm deep", self.brush.diameter_mm, depth),
                        AQUA,
                    );
                }
                for points in self.gesture.world.windows(2) {
                    if let [Some(a), Some(b)] = points {
                        painter.line_segment(
                            [project(a.map(f64::from)), project(b.map(f64::from))],
                            Stroke::new(2.0, PINK),
                        );
                    }
                }
            }
            Tool::Section => {
                let handle = self.handle_world(mesh);
                let hp = project(handle);
                let mut along = handle;
                along[self.plane.axis.min(2)] += 1.0;
                let screen_axis = project(along) - hp;
                let grip = ui.interact(
                    Rect::from_center_size(hp, egui::vec2(44.0, 44.0)),
                    ui.id().with("section-plane-handle"),
                    egui::Sense::drag(),
                );
                self.section_dragging = grip.dragged() && !pointer.navigating;
                if self.section_dragging {
                    // Mouse movement before the press can arrive in the same
                    // frame. Anchor to the press, never that frame's delta.
                    let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(hp);
                    let (start, offset) =
                        *self.section_grab.get_or_insert((origin, self.plane.offset));
                    let delta = grip.interact_pointer_pos().unwrap_or(start) - start;
                    if screen_axis.length_sq() > 0.01 {
                        let range = self.plane.range(mesh);
                        self.plane.offset = (offset
                            + (delta.dot(screen_axis) / screen_axis.length_sq()) as f64)
                            .clamp(range[0], range[1]);
                    }
                } else {
                    self.section_grab = None;
                }
                let key = (self.plane.axis, self.plane.offset.to_bits());
                if self.cut_key != Some(key) {
                    self.cut = section::cut(mesh, self.plane);
                    self.cap = self.cut.cap(self.plane.axis);
                    self.cut_key = Some(key);
                    self.wall = None;
                }
                let (_, view_direction) = ray(rect.center());
                if view_direction[self.plane.axis.min(2)] * if self.plane.flip { -1.0 } else { 1.0 }
                    < 0.0
                {
                    let mut cap = egui::Mesh::default();
                    for triangle in &self.cap {
                        let base = cap.vertices.len() as u32;
                        for p in triangle {
                            cap.colored_vertex(project(*p), Color32::from_rgb(151, 116, 68));
                        }
                        cap.add_triangle(base, base + 1, base + 2);
                    }
                    painter.add(egui::Shape::mesh(cap));
                }
                for [a, b] in &self.cut.segments {
                    painter.line_segment([project(*a), project(*b)], Stroke::new(1.8, AMBER));
                }
                let (lo, hi) = mesh.bounds().unwrap_or_default();
                let lo = section::coords(lo);
                let hi = section::coords(hi);
                let axes: Vec<_> = (0..3).filter(|&i| i != self.plane.axis.min(2)).collect();
                let corners: Vec<_> = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]
                    .into_iter()
                    .map(|s| {
                        let mut p = [0.0; 3];
                        p[self.plane.axis.min(2)] = self.plane.offset;
                        for j in 0..2 {
                            let k = axes[j];
                            p[k] = (lo[k] + hi[k]) * 0.5 + s[j] * ((hi[k] - lo[k]) * 0.5 + 1.2);
                        }
                        p
                    })
                    .collect();
                for i in 0..4 {
                    painter.line_segment(
                        [project(corners[i]), project(corners[(i + 1) % 4])],
                        Stroke::new(1.0, AQUA.gamma_multiply(0.6)),
                    );
                }
                let hp = project(self.handle_world(mesh));
                painter.circle_filled(hp, 7.0, AQUA);
                painter.circle_stroke(hp, 14.0, Stroke::new(1.0, AQUA));
                tag(
                    &painter,
                    rect,
                    hp + egui::vec2(0.0, -24.0),
                    &format!(
                        "{} {:.2} mm",
                        ["X", "Y", "Z"][self.plane.axis.min(2)],
                        self.plane.offset
                    ),
                    AQUA,
                );
                if response.clicked() && !grip.dragged() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let mut near = None;
                        let mut distance = 18.0;
                        for (i, [a, b]) in self.cut.segments.iter().enumerate() {
                            let a = project(*a);
                            let b = project(*b);
                            let v = b - a;
                            let t = ((pos - a).dot(v) / v.length_sq().max(1e-8)).clamp(0.0, 1.0);
                            let dd = pos.distance(a + v * t);
                            if dd < distance {
                                distance = dd;
                                near = Some((i, t));
                            }
                        }
                        if let Some((i, t)) = near {
                            self.wall = self.cut.wall_at(i, t as f64, self.plane.axis);
                        }
                    }
                }
                if let Some([a, b]) = self.wall {
                    let pa = project(a);
                    let pb = project(b);
                    painter.line_segment([pa, pb], Stroke::new(3.0, PINK));
                    tag(
                        &painter,
                        rect,
                        pa.lerp(pb, 0.5),
                        &format!("Wall chord {:.2} mm", section::distance(a, b)),
                        PINK,
                    );
                }
            }
            Tool::Clearance => {
                self.refresh_clearance(d);
                if response.clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        self.selected_stone = self
                            .envelopes
                            .iter()
                            .enumerate()
                            .map(|(i, e)| (i, pos.distance(project(e.centre))))
                            .filter(|(_, distance)| *distance < 28.0)
                            .min_by(|a, b| a.1.total_cmp(&b.1))
                            .map(|(i, _)| i);
                    }
                }
                for envelope in &self.envelopes {
                    let bad = envelope.worst_gap < self.gap_mm;
                    let color = if bad { RED } else { AQUA };
                    for ring in [&envelope.girdle, &envelope.deep] {
                        for i in 0..ring.len() {
                            painter.line_segment(
                                [project(ring[i]), project(ring[(i + 1) % ring.len()])],
                                Stroke::new(
                                    if std::ptr::eq(ring, &envelope.girdle) {
                                        1.7
                                    } else {
                                        0.8
                                    },
                                    color.gamma_multiply(0.8),
                                ),
                            );
                        }
                    }
                    for i in (0..envelope.girdle.len()).step_by(12) {
                        painter.line_segment(
                            [project(envelope.girdle[i]), project(envelope.deep[i])],
                            Stroke::new(0.8, color),
                        );
                    }
                }
                if let Some(pair) = self.crowding.as_ref().and_then(|r| r.closest.as_ref()) {
                    if let (Some(a), Some(b)) = (
                        self.envelopes.iter().find(|e| e.label == pair.a),
                        self.envelopes.iter().find(|e| e.label == pair.b),
                    ) {
                        let a = project(a.centre);
                        let b = project(b.centre);
                        let color = if pair.worst_mm() < self.gap_mm {
                            RED
                        } else {
                            AQUA
                        };
                        painter.line_segment([a, b], Stroke::new(2.0, color));
                        tag(
                            &painter,
                            rect,
                            a.lerp(b, 0.5),
                            &format!("Closest gap {:.2} mm", pair.worst_mm()),
                            color,
                        );
                    }
                }
            }
            Tool::Mould => {
                if let Some(study) = &self.study {
                    if self.playing {
                        self.phase += ui.input(|i| i.stable_dt.min(0.1)) as f64 / 6.0;
                        self.opening_mm = 8.0 * (1.0 - (self.phase * std::f64::consts::TAU).cos());
                        ui.ctx().request_repaint();
                    }
                    let (_, direction) = ray(rect.center());
                    let depth = |p: [f64; 3]| {
                        p.into_iter()
                            .zip(direction)
                            .map(|(a, b)| a * b as f64)
                            .sum::<f64>()
                    };
                    let mut triangles = Vec::new();
                    for (upper, show, faces, color) in [
                        (
                            true,
                            self.show_upper,
                            &study.upper,
                            Color32::from_rgba_unmultiplied(94, 161, 213, 100),
                        ),
                        (
                            false,
                            self.show_lower,
                            &study.lower,
                            Color32::from_rgba_unmultiplied(206, 157, 88, 100),
                        ),
                    ] {
                        if !show {
                            continue;
                        }
                        for face in faces {
                            let p = face.map(|p| {
                                mould::translated(p, study.report.frame.z, self.opening_mm, upper)
                            });
                            let z = p.map(depth).iter().sum::<f64>() / 3.0;
                            triangles.push((z, p, color));
                        }
                    }
                    triangles.sort_by(|a, b| b.0.total_cmp(&a.0));
                    let mut overlay = egui::Mesh::default();
                    for (_, triangle, color) in triangles {
                        let base = overlay.vertices.len() as u32;
                        for p in triangle {
                            overlay.colored_vertex(project(p), color);
                        }
                        overlay.add_triangle(base, base + 1, base + 2);
                    }
                    painter.add(egui::Shape::mesh(overlay));
                    for o in study.report.obstructions.iter().take(80) {
                        let p = project(o.world);
                        painter.circle_stroke(p, 5.0, Stroke::new(2.0, RED));
                    }
                    tag(
                        &painter,
                        rect,
                        rect.center_top() + egui::vec2(0.0, 25.0),
                        &format!("Cavity X-ray · {:.1} mm opening / half", self.opening_mm),
                        AMBER,
                    );
                }
            }
            Tool::Select => {}
        }
        edit
    }
}
fn tag(painter: &egui::Painter, rect: Rect, position: Pos2, text: &str, color: Color32) {
    let galley = painter.layout(
        text.to_string(),
        egui::FontId::proportional(12.0),
        color,
        (rect.width() - 20.0).max(20.0),
    );
    let size = galley.size();
    let x = (position.x - size.x * 0.5).clamp(
        rect.left() + 6.0,
        (rect.right() - size.x - 6.0).max(rect.left() + 6.0),
    );
    let y = (position.y - size.y * 0.5).clamp(
        rect.top() + 6.0,
        (rect.bottom() - size.y - 6.0).max(rect.top() + 6.0),
    );
    let pos = egui::pos2(x, y);
    painter.rect_filled(
        Rect::from_min_size(pos, size).expand(4.0),
        4.0,
        Color32::from_rgba_unmultiplied(18, 18, 20, 228),
    );
    painter.galley(pos, galley, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_contact_survives_long_touch_and_commits_only_on_release() {
        let ctx = egui::Context::default();
        let mut contact = StampContact::default();
        let pos = egui::pos2(80.0, 90.0);
        let mut saw_long_touch = false;
        let mut commits = 0;
        // These are the device-observed phases: press, stationary preview,
        // Android/egui's long-touch conversion, then lift without a drag.
        for (frame, time) in [0.0, 0.1, 0.3, 1.0, 1.5, 2.0, 2.1].into_iter().enumerate() {
            let mut events = Vec::new();
            if frame == 1 || frame == 5 {
                let pressed = frame == 1;
                events.push(egui::Event::PointerMoved(pos));
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                });
                events.push(egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(1),
                    phase: if pressed {
                        egui::TouchPhase::Start
                    } else {
                        egui::TouchPhase::End
                    },
                    pos,
                    force: None,
                });
            }
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(300.0, 400.0))),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        let (_, response) = ui.allocate_exact_size(
                            ui.available_size(),
                            egui::Sense::click_and_drag(),
                        );
                        saw_long_touch |= response.long_touched();
                        let (preview, place) = contact.update(ui, &response, true);
                        if (1..=5).contains(&frame) {
                            assert_eq!(preview, Some(pos), "preview lost on frame {frame}");
                        }
                        assert_eq!(place, frame == 5, "unexpected commit on frame {frame}");
                        commits += usize::from(place);
                    });
                },
            );
            output.textures_delta.clear();
        }
        assert!(
            saw_long_touch,
            "test must exercise egui's context-gesture conversion"
        );
        assert_eq!(commits, 1);
        assert!(!contact.active);
    }

    #[test]
    fn section_drag_ignores_motion_before_press_and_keeps_its_anchor() {
        let ctx = egui::Context::default();
        let mut visual = Visual::default();
        visual.select(Tool::Section);
        let mut design = RingDesign::default();
        let lib = AlphaLibrary::default();
        let mesh = Mesh {
            vertices: vec![
                ringdesign_core::mesh::Vec3(-5.0, -5.0, -5.0),
                ringdesign_core::mesh::Vec3(5.0, 5.0, 5.0),
            ],
            ..Default::default()
        };
        let project =
            |p: [f64; 3]| egui::pos2(300.0 + p[0] as f32 * 10.0, 300.0 - p[2] as f32 * 10.0);
        let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(600.0, 600.0));
        let start = project(visual.handle_world(&mesh));
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let frames = [
            (vec![egui::Event::PointerMoved(egui::pos2(10.0, 10.0))], 0.0),
            (vec![], 0.0),
            (
                vec![egui::Event::PointerMoved(start), button(start, true)],
                0.0,
            ),
            (
                vec![egui::Event::PointerMoved(start - egui::vec2(0.0, 10.0))],
                1.0,
            ),
            (
                vec![egui::Event::PointerMoved(start - egui::vec2(0.0, 40.0))],
                4.0,
            ),
            (vec![button(start - egui::vec2(0.0, 40.0), false)], 4.0),
        ];
        for (i, (events, expected)) in frames.into_iter().enumerate() {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        let (rect, response) = ui.allocate_exact_size(
                            ui.available_size(),
                            egui::Sense::click_and_drag(),
                        );
                        visual.draw(
                            ui,
                            rect,
                            &response,
                            &mut design,
                            &lib,
                            &mesh,
                            project,
                            |_| ([0.0, 0.0, 30.0], [0.0, 0.0, -1.0]),
                            Pointer::default(),
                        );
                    });
                },
            );
            output.textures_delta.clear();
            assert!(
                (visual.plane.offset - expected).abs() < 0.001,
                "frame {i}: {} != {expected}",
                visual.plane.offset
            );
            if i == 4 {
                assert!(visual.blocks_orbit(Some(start), screen, &mesh, project, false));
            }
        }
        assert!(!visual.section_dragging);
    }
}
