use super::{Pointer, value};
use crate::icons::{self, Icon};
use egui::{Color32, Pos2, Rect, Stroke};
use ringdesign_core::{
    AlphaLibrary, Mesh, RingDesign,
    field::Decal,
    interaction::{
        ornament::{self, Operation, Target},
        picking, surface,
    },
};

#[derive(Default)]
pub struct TransformTool {
    target: Option<Target>,
    draft: Decal,
    source: Option<u64>,
    pending: Option<Operation>,
    drag: Option<(usize, Pos2, Decal)>,
    pub message: String,
}
impl TransformTool {
    pub fn stop_drag(&mut self) {
        self.drag = None;
    }
    fn load(&mut self, d: &RingDesign, target: Target) {
        if let Some((_, decal)) = ornament::get(d, &target) {
            self.draft = *decal;
            self.target = Some(target);
            self.source = Some(crate::key(d));
            self.drag = None;
            self.message.clear();
        }
    }
    pub fn apply_pending(&mut self, d: &mut RingDesign) -> Option<usize> {
        let op = self.pending.take()?;
        if self.source != Some(crate::key(d)) {
            self.message = "The ring changed. Reselect the ornament before applying.".into();
            return None;
        }
        match ornament::apply(d, self.target.as_ref()?, self.draft, op) {
            Ok(target) => {
                let index = target.path[0];
                self.load(d, target);
                self.message = "Ornament applied. Undo restores the previous placement.".into();
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
            ui.label("Bake the driven design to edit individual ornaments.");
            return;
        }
        let targets = ornament::targets(d);
        let name = targets
            .iter()
            .find(|(t, _)| Some(t) == self.target.as_ref())
            .map_or("Pick an ornament", |(_, n)| n.as_str());
        egui::ComboBox::from_id_salt("ornament-target")
            .selected_text(name)
            .width((ui.available_width() - 16.0).max(80.0))
            .truncate()
            .show_ui(ui, |ui| {
                for (t, n) in &targets {
                    if ui
                        .selectable_label(Some(t) == self.target.as_ref(), n)
                        .clicked()
                    {
                        self.load(d, t.clone());
                    }
                }
            });
        if targets.is_empty() {
            ui.label("Place a Stamp first. This tool edits stamps, including those inside manual groups.");
            return;
        }
        ui.label("Tap raised artwork to select it. Drag the centre to move, the circular handle left/right to rotate, or the square up/down to resize.");
        if self.target.is_some() {
            value(
                ui,
                "Around ring",
                &mut self.draft.theta_deg,
                0.0..=360.0,
                "°",
            );
            value(
                ui,
                "Across band",
                &mut self.draft.v_mm,
                0.0..=d.field_context().band_v_len_mm,
                " mm",
            );
            value(ui, "Size", &mut self.draft.size_mm, 0.1..=24.0, " mm");
            value(
                ui,
                "Rotation",
                &mut self.draft.rotation_deg,
                -180.0..=180.0,
                "°",
            );
            ui.checkbox(&mut self.draft.flip, "Flip artwork");
            ui.horizontal_wrapped(|ui| {
                for (icon, label, op) in [
                    (Icon::Check, "Apply", Operation::Replace),
                    (Icon::Duplicate, "Copy", Operation::Duplicate),
                    (Icon::Mirror, "Mirror", Operation::Mirror),
                ] {
                    if icons::button(ui, icon, label, false, egui::vec2(0.0, 28.0)).clicked() {
                        self.pending = Some(op);
                    }
                }
                if icons::button(ui, Icon::Reset, "Reset", false, egui::vec2(0.0, 28.0)).clicked() {
                    if let Some(t) = self.target.clone() {
                        self.load(d, t);
                    }
                }
            });
            ui.small("Aqua is the proposed footprint. Apply saves it. Copy adds beside it; Mirror adds across the band. Layer masks still apply.");
        }
        if !self.message.is_empty() {
            ui.colored_label(Color32::from_rgb(239, 179, 104), &self.message);
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        response: &egui::Response,
        d: &RingDesign,
        lib: &AlphaLibrary,
        mesh: &Mesh,
        project: impl Fn([f64; 3]) -> Pos2,
        ray: impl Fn(Pos2) -> ([f32; 3], [f32; 3]),
        pointer: Pointer,
    ) {
        if d.graph.is_some() || d.cad.is_some() {
            return;
        }
        if pointer.navigating {
            self.drag = None;
            return;
        }
        let mut used = false;
        if let Some(target) = self.target.as_ref() {
            if let Some((layer, _)) = ornament::get(d, target) {
                crate::artwork::preview(
                    ui,
                    rect,
                    d,
                    lib,
                    &layer.alpha,
                    &self.draft,
                    layer.feather_mm,
                    layer.invert,
                    &project,
                );
                let centre =
                    surface::points(d, lib, &[[self.draft.theta_deg / 360.0, self.draft.v_mm]])[0];
                let centre = project(centre);
                // Fixed screen offsets keep all grips usable even for tiny stamps.
                let handles = [
                    centre,
                    centre + egui::vec2(0.0, -50.0),
                    centre + egui::vec2(50.0, 0.0),
                ];
                let painter = ui.painter_at(rect);
                for (i, p) in handles.into_iter().enumerate() {
                    if !rect.contains(p) {
                        continue;
                    }
                    if i > 0 {
                        painter.line_segment([centre, p], Stroke::new(1.0, super::canvas::AQUA));
                    }
                    let r = ui.interact(
                        Rect::from_center_size(p, egui::Vec2::splat(34.0)).intersect(rect),
                        ui.id().with(("ornament-handle", i)),
                        egui::Sense::drag(),
                    );
                    painter.rect_filled(
                        Rect::from_center_size(p, egui::Vec2::splat(26.0)),
                        if i == 2 { 3.0 } else { 13.0 },
                        Color32::from_rgb(29, 32, 43),
                    );
                    let icon = [Icon::Move, Icon::Rotate, Icon::Scale][i];
                    icon.image(ui, 18.0)
                        .tint(super::canvas::AQUA)
                        .paint_at(ui, Rect::from_center_size(p, egui::Vec2::splat(18.0)));
                    let (title, what, how, when) = icon.hint();
                    icons::help(&r, title, what, how, when);
                    if pointer.accepted && r.drag_started() {
                        self.drag = Some((
                            i,
                            ui.input(|i| i.pointer.press_origin()).unwrap_or(p),
                            self.draft,
                        ));
                    }
                    used |= r.hovered() || r.dragged() || r.drag_stopped();
                }
            }
        }
        if let Some((kind, start, initial)) = self.drag {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                if pointer.accepted && ui.input(|i| i.pointer.any_down()) {
                    match kind {
                        0 => {
                            let (o, v) = ray(pos);
                            if let Some(hit) = picking::hit(d, lib, mesh, o, v)
                                .filter(|h| h.radial_wall_mm >= 0.05)
                            {
                                self.draft.theta_deg = hit.theta_deg;
                                self.draft.v_mm = hit.v_mm;
                            }
                        }
                        1 => {
                            self.draft.rotation_deg =
                                (initial.rotation_deg + (pos.x - start.x) as f64 * 0.8 + 180.0)
                                    .rem_euclid(360.0)
                                    - 180.0;
                        }
                        _ => {
                            self.draft.size_mm = (initial.size_mm
                                + (start.y - pos.y) as f64 * 0.03)
                                .clamp(0.1, 24.0);
                        }
                    }
                }
            }
            if ui.input(|i| !i.pointer.any_down()) {
                self.drag = None;
            }
            used = true;
        }
        if !used && pointer.accepted && response.clicked() {
            if let Some(pos) = response
                .interact_pointer_pos()
                .filter(|p| rect.contains(*p))
            {
                let (o, v) = ray(pos);
                if let Some(target) =
                    picking::hit(d, lib, mesh, o, v).and_then(|h| ornament::pick(d, lib, &h))
                {
                    self.load(d, target);
                } else {
                    self.message =
                        "Tap the raised part of a stamp, or choose its name above.".into();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn apply_is_once_and_refuses_draft_after_source_changes() {
        use ringdesign_core::field::{DecalLayer, Layer, LayerEntry};
        let mut d = RingDesign::default();
        let v = d.field_context().crest_v_mm;
        d.layers.layers.push(LayerEntry::new(
            "motif",
            Layer::Decals(DecalLayer {
                decals: vec![Decal {
                    v_mm: v,
                    ..Default::default()
                }],
                ..Default::default()
            }),
        ));
        let mut tool = TransformTool::default();
        tool.load(
            &d,
            Target {
                path: vec![0],
                instance: 0,
            },
        );
        tool.draft.size_mm = 2.0;
        tool.pending = Some(Operation::Replace);
        assert_eq!(tool.apply_pending(&mut d), Some(0));
        assert_eq!(tool.apply_pending(&mut d), None);
        d.name = "Another version".into();
        let before = crate::key(&d);
        tool.pending = Some(Operation::Duplicate);
        assert_eq!(tool.apply_pending(&mut d), None);
        assert_eq!(crate::key(&d), before);
        assert!(tool.message.contains("ring changed"));
    }
}
