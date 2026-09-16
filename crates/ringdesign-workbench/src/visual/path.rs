use super::value;
use egui::{Color32, Pos2, Rect, Stroke};
use ringdesign_core::{
    AlphaLibrary, RingDesign,
    curve::{CurveLayer, WireProfile},
    field::{Blend, Layer},
    interaction::{picking, surface},
};

pub struct PathTool {
    pub curve: CurveLayer,
    pub engrave: bool,
    pub snap: bool,
    pub selected: Option<usize>,
    target: Option<usize>,
    source: Option<u64>,
    dragging: bool,
    apply: bool,
    cached: Option<String>,
    handles: Vec<[f64; 3]>,
    lines: Vec<Vec<[f64; 3]>>,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inspector_applies_once_and_refuses_stale_drafts() {
        let mut d = RingDesign::default();
        let mut tool = PathTool::default();
        let v = d.field_context().crest_v_mm;
        tool.curve.points = vec![[0.1, v], [0.2, v]];
        tool.source = Some(crate::key(&d));
        tool.apply = true;
        assert_eq!(tool.apply_pending(&mut d), Some(0));
        assert_eq!(tool.apply_pending(&mut d), None);
        assert_eq!(d.layers.layers.len(), 1);
        tool.curve.width_mm = 0.9;
        tool.apply = true;
        assert_eq!(tool.apply_pending(&mut d), Some(0));
        assert_eq!(d.layers.layers.len(), 1);
        d.name = "Different project".into();
        let before = crate::key(&d);
        tool.apply = true;
        assert_eq!(tool.apply_pending(&mut d), None);
        assert_eq!(crate::key(&d), before);
        assert!(tool.message.contains("ring changed"));
    }
}
impl Default for PathTool {
    fn default() -> Self {
        Self {
            curve: CurveLayer {
                points: vec![],
                repeats_around: 1,
                height_mm: 0.12,
                width_mm: 0.6,
                ..Default::default()
            },
            engrave: false,
            snap: false,
            selected: None,
            target: None,
            source: None,
            dragging: false,
            apply: false,
            cached: None,
            handles: vec![],
            lines: vec![],
            message: String::new(),
        }
    }
}
impl PathTool {
    pub fn invalidate(&mut self) {
        self.cached = None;
    }
    pub fn stop_drag(&mut self) {
        self.dragging = false;
    }
    pub fn apply_pending(&mut self, d: &mut RingDesign) -> Option<usize> {
        if !std::mem::take(&mut self.apply) {
            return None;
        }
        if self.source != Some(crate::key(d)) {
            self.message =
                "The ring changed. Clear the draft or reload its path before applying.".into();
            return None;
        }
        match surface::apply_path(d, self.curve.clone(), self.engrave, self.target) {
            Ok(index) => {
                self.target = Some(index);
                self.source = Some(crate::key(d));
                self.message = "Path applied. Undo restores the previous shape.".into();
                self.cached = None;
                Some(index)
            }
            Err(e) => {
                self.message = e;
                None
            }
        }
    }
    pub fn controls(&mut self, ui: &mut egui::Ui, d: &RingDesign) {
        if d.graph.is_some() || d.cad.is_some() {
            ui.label("Bake the driven design to edit surface paths.");
            return;
        }
        let width = (ui.available_width() - 18.0).max(90.0);
        let label = self
            .target
            .and_then(|i| d.layers.layers.get(i))
            .map_or("New path", |e| e.name.as_str());
        egui::ComboBox::from_id_salt("viewport-path-layer")
            .selected_text(label)
            .width(width)
            .truncate()
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(self.target.is_none(), "New path")
                    .clicked()
                {
                    *self = Self::default();
                }
                for (i, e) in d.layers.layers.iter().enumerate() {
                    if let Layer::Curve(curve) = &e.layer {
                        if ui
                            .selectable_label(self.target == Some(i), &e.name)
                            .clicked()
                        {
                            *self = Self::default();
                            self.curve = curve.clone();
                            self.target = Some(i);
                            self.engrave = e.blend == Blend::Subtract;
                            self.source = Some(crate::key(d));
                        }
                    }
                }
            });
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.engrave, false, "Raise");
            ui.selectable_value(&mut self.engrave, true, "Engrave");
        });
        value(ui, "Width", &mut self.curve.width_mm, 0.1..=6.0, " mm");
        value(ui, "Depth", &mut self.curve.height_mm, 0.01..=1.6, " mm");
        let old_repeats = self.curve.repeats_around.max(1);
        ui.horizontal_wrapped(|ui| {
            ui.label("Copies");
            ui.add(
                egui::DragValue::new(&mut self.curve.repeats_around)
                    .range(1..=surface::MAX_REPEATS),
            );
            ui.checkbox(&mut self.curve.mirror_v, "Mirror sides");
        });
        if old_repeats != self.curve.repeats_around {
            for p in &mut self.curve.points {
                p[0] *= self.curve.repeats_around as f64 / old_repeats as f64;
            }
        }
        egui::ComboBox::from_id_salt("path-cross-section")
            .selected_text(self.curve.profile.label())
            .width(width)
            .truncate()
            .show_ui(ui, |ui| {
                for &p in WireProfile::ALL {
                    ui.selectable_value(&mut self.curve.profile, p, p.label());
                }
            });
        value(ui, "End taper", &mut self.curve.taper, 0.0..=0.5, "");
        ui.checkbox(&mut self.snap, "Snap to 5° / 0.25 mm");
        ui.checkbox(&mut self.curve.closed, "Join around repeat");
        if let Some(i) = self.selected.filter(|i| *i < self.curve.points.len()) {
            let p = &mut self.curve.points[i];
            let mut angle = p[0] * 360.0 / self.curve.repeats_around as f64;
            value(ui, "Point angle", &mut angle, -360.0..=720.0, "°");
            p[0] = angle / 360.0 * self.curve.repeats_around as f64;
            value(
                ui,
                "Across band",
                &mut p[1],
                0.0..=d.field_context().band_v_len_mm,
                " mm",
            );
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !self.curve.points.is_empty(),
                    egui::Button::new("Remove point"),
                )
                .clicked()
            {
                let i = self
                    .selected
                    .unwrap_or(self.curve.points.len() - 1)
                    .min(self.curve.points.len() - 1);
                self.curve.points.remove(i);
                self.selected = None;
            }
            if ui.button("Clear draft").clicked() {
                *self = Self::default();
            }
            if ui
                .add_enabled(
                    self.curve.points.len() >= 2,
                    egui::Button::new(if self.target.is_some() {
                        "Apply path"
                    } else {
                        "Add path"
                    }),
                )
                .clicked()
            {
                self.apply = true;
            }
        });
        ui.label("Tap points on the ring; drag a point to reshape. Select lets you orbit. Apply keeps one editable layer.");
        if d.draft.process == ringdesign_core::castability::CastProcess::SandTwoPart {
            ui.small("Keep raised paths shallow on the crown. Check casting after applying.");
        }
        ui.small(format!("{} / 64 points", self.curve.points.len()));
        if !self.message.is_empty() {
            ui.colored_label(Color32::from_rgb(239, 179, 104), &self.message);
        }
    }
    fn refresh(&mut self, d: &RingDesign, lib: &AlphaLibrary) {
        if self.curve.points.iter().flatten().any(|v| !v.is_finite()) {
            self.handles.clear();
            self.lines.clear();
            self.message = "Enter finite point coordinates.".into();
            return;
        }
        let key = serde_json::to_string(&self.curve).unwrap_or_default();
        if self.cached.as_ref() == Some(&key) {
            return;
        }
        let repeats = self.curve.repeats_around.clamp(1, surface::MAX_REPEATS) as f64;
        let chart: Vec<_> = self
            .curve
            .points
            .iter()
            .map(|p| [p[0] / repeats, p[1]])
            .collect();
        self.handles = surface::points(d, lib, &chart);
        self.lines.clear();
        // Preview each instance with the same spline as the relief evaluator.
        let sampled = self.curve.sample_path(6);
        let budget = (2048
            / self.curve.repeats_around.clamp(1, surface::MAX_REPEATS) as usize
            / if self.curve.mirror_v { 2 } else { 1 })
        .max(2);
        let sampled: Vec<_> = if sampled.len() > budget {
            (0..budget)
                .map(|i| sampled[i * (sampled.len() - 1) / (budget - 1)])
                .collect()
        } else {
            sampled
        };
        for i in 0..self.curve.repeats_around.clamp(1, surface::MAX_REPEATS) {
            let chart: Vec<_> = sampled
                .iter()
                .map(|p| [(p[0] + i as f64) / repeats, p[1]])
                .collect();
            self.lines.push(surface::points(d, lib, &chart));
            if self.curve.mirror_v {
                let band = d.field_context().band_v_len_mm;
                let chart: Vec<_> = chart.iter().map(|p| [p[0], band - p[1]]).collect();
                self.lines.push(surface::points(d, lib, &chart));
            }
        }
        self.cached = Some(key);
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        response: &egui::Response,
        d: &mut RingDesign,
        lib: &AlphaLibrary,
        mesh: &ringdesign_core::Mesh,
        project: impl Fn([f64; 3]) -> Pos2,
        ray: impl Fn(Pos2) -> ([f32; 3], [f32; 3]),
        pointer: super::Pointer,
    ) -> Option<usize> {
        if pointer.navigating {
            self.dragging = false;
            return None;
        }
        if d.graph.is_some() || d.cad.is_some() {
            return None;
        }
        if let Some(index) = self.apply_pending(d) {
            return Some(index);
        }
        self.refresh(d, lib);
        let pos = response
            .interact_pointer_pos()
            .or(response.hover_pos())
            .filter(|p| rect.contains(*p));
        if pointer.accepted {
            if let Some(pos) = pos {
                let closest = self
                    .handles
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (i, project(*p).distance(pos)))
                    .filter(|(_, distance)| *distance < 20.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(i, _)| i);
                if response.drag_started() {
                    let start = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
                    self.selected = self
                        .handles
                        .iter()
                        .enumerate()
                        .find(|(_, p)| project(**p).distance(start) < 24.0)
                        .map(|(i, _)| i);
                    self.dragging = self.selected.is_some();
                }
                if response.clicked() && closest.is_some() {
                    self.selected = closest;
                } else if (response.clicked() || (response.dragged() && self.dragging))
                    && self.curve.points.len() <= 64
                {
                    let (origin, dir) = ray(pos);
                    if let Some(hit) =
                        picking::hit(d, lib, mesh, origin, dir).filter(|h| h.radial_wall_mm >= 0.05)
                    {
                        if self.source.is_none() {
                            self.source = Some(crate::key(d));
                        }
                        let repeats =
                            self.curve.repeats_around.clamp(1, surface::MAX_REPEATS) as f64;
                        let mut p = [hit.theta_deg / 360.0, hit.v_mm];
                        if self.snap {
                            p[0] = (p[0] * 72.0).round() / 72.0;
                            p[1] = (p[1] * 4.0).round() / 4.0;
                        }
                        p[1] = p[1].clamp(0.0, d.field_context().band_v_len_mm);
                        let old = if self.dragging {
                            self.selected.and_then(|i| self.curve.points.get(i))
                        } else {
                            self.curve.points.last()
                        };
                        if let Some(old) = old {
                            p[0] = surface::near_turn(p[0], old[0] / repeats);
                        }
                        p[0] *= repeats;
                        if self.dragging {
                            if let Some(i) = self.selected {
                                self.curve.points[i] = p;
                            }
                        } else if self.curve.points.len() < 64 {
                            self.curve.points.push(p);
                            self.selected = Some(self.curve.points.len() - 1);
                        }
                        self.cached = None;
                        ui.ctx().request_repaint();
                    }
                }
            }
        }
        if ui.input(|i| i.pointer.any_released()) {
            self.dragging = false;
        }
        let painter = ui.painter_at(rect);
        for line in &self.lines {
            for pair in line.windows(2) {
                painter.line_segment(
                    [project(pair[0]), project(pair[1])],
                    Stroke::new(1.5, super::canvas::PINK),
                );
            }
        }
        for (i, p) in self.handles.iter().enumerate() {
            let pos = project(*p);
            let color = if self.selected == Some(i) {
                super::canvas::AMBER
            } else {
                super::canvas::AQUA
            };
            painter.circle_filled(pos, 7.0, color);
            painter.text(
                pos + egui::vec2(10.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                (i + 1).to_string(),
                egui::FontId::proportional(12.0),
                color,
            );
        }
        None
    }
}
