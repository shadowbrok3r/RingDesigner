use super::{Editor, HandleDrag, Mode, Parameter, Sheet};
use crate::{camera::OrbitCamera, theme};
use egui_mobile::egui;
use ringdesign_core::RingDesign;

pub fn dimension(d: &RingDesign, parameter: Parameter) -> ([f32; 3], [f32; 3]) {
    let inner = d.inner_radius_mm() as f32;
    let outer = inner + d.profile.thickness_mm as f32;
    let width = d.profile.width_mm as f32;
    let theta = d.shank.head.theta_deg.to_radians();
    let normal = [theta.cos() as f32, theta.sin() as f32, 0.0];
    let tangent = [-normal[1], normal[0], 0.0];
    let height = outer + d.shank.head.rise_mm as f32;
    let center = normal.map(|x| x * height);
    match parameter {
        Parameter::Bore => ([-inner, 0.0, 0.0], [inner, 0.0, 0.0]),
        Parameter::Width => {
            let center = if d.shank.kind == ringdesign_core::ShankKind::Signet {
                center
            } else {
                [0.0, -outer, 0.0]
            };
            (
                [center[0], center[1], -width * 0.5],
                [center[0], center[1], width * 0.5],
            )
        }
        Parameter::Thickness => ([0.0, -inner, 0.0], [0.0, -outer, 0.0]),
        Parameter::HeadLength => {
            let half = d.shank.head.length_mm as f32 * 0.5;
            (
                std::array::from_fn(|i| center[i] - tangent[i] * half),
                std::array::from_fn(|i| center[i] + tangent[i] * half),
            )
        }
        Parameter::HeadRise => (normal.map(|x| x * outer), center),
        _ => (center, center),
    }
}

pub fn blocks_orbit(
    editor: &Editor,
    d: &RingDesign,
    camera: &OrbitCamera,
    rect: egui::Rect,
    pointer: Option<egui::Pos2>,
) -> bool {
    if editor.drag.is_some() {
        return true;
    }
    if !editor.guides
        || editor.mode != Mode::Shape
        || !editor.parameter.has_handle()
        || (editor.part == super::ShapePart::Head
            && d.shank.kind != ringdesign_core::ShankKind::Signet)
    {
        return false;
    }
    let Some(pointer) = pointer else {
        return false;
    };
    let (a, b) = dimension(d, editor.parameter);
    let projection = camera.projector(rect);
    [projection.at(a), projection.at(b)]
        .iter()
        .any(|p| p.distance(pointer) <= 23.0)
}

/// Keep dimension labels inside their viewport, even after zooming or rotating.
pub fn tag(
    ui: &egui::Ui,
    rect: egui::Rect,
    at: egui::Pos2,
    text: &str,
    color: egui::Color32,
) -> egui::Rect {
    let painter = ui.painter().with_clip_rect(rect);
    let max_width = (rect.width() - 24.0).max(1.0);
    let galley = painter.layout(
        text.into(),
        egui::FontId::proportional(12.0),
        color,
        max_width,
    );
    let size = galley.size() + egui::vec2(12.0, 8.0);
    let min = egui::pos2(
        (at.x - size.x * 0.5)
            .max(rect.left() + 4.0)
            .min((rect.right() - size.x - 4.0).max(rect.left() + 4.0)),
        (at.y - size.y * 0.5)
            .max(rect.top() + 4.0)
            .min((rect.bottom() - size.y - 4.0).max(rect.top() + 4.0)),
    );
    let bounds = egui::Rect::from_min_size(min, size);
    painter.rect_filled(
        bounds,
        5.0,
        egui::Color32::from_rgba_unmultiplied(12, 15, 19, 238),
    );
    painter.rect_stroke(
        bounds,
        5.0,
        egui::Stroke::new(1.0, color.gamma_multiply(0.45)),
        egui::StrokeKind::Inside,
    );
    painter.galley(min + egui::vec2(6.0, 4.0), galley, color);
    bounds
}

pub fn draw(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    camera: &OrbitCamera,
    editor: &mut Editor,
    d: &mut RingDesign,
    selected_layer: Option<usize>,
    parting_z: f64,
    can_edit: bool,
) -> (bool, Option<usize>) {
    if !editor.guides || editor.hold_before {
        return (false, None);
    }
    let projection = camera.projector(rect);
    let painter = ui.painter().with_clip_rect(rect);
    let mut changed = false;
    let mut stone_pick = None;
    match editor.mode {
        Mode::Shape
            if editor.parameter.has_handle()
                && (editor.part != super::ShapePart::Head
                    || d.shank.kind == ringdesign_core::ShankKind::Signet) =>
        {
            let parameter = editor.parameter;
            let (a, b) = dimension(d, parameter);
            let (a, b) = (projection.at(a), projection.at(b));
            painter.line_segment([a, b], egui::Stroke::new(1.5, theme::AQUA));
            let axis = (b - a).normalized();
            let normal = egui::vec2(-axis.y, axis.x);
            for (i, point) in [a, b].into_iter().enumerate() {
                if !rect.shrink(4.0).contains(point) {
                    continue;
                }
                painter.line_segment(
                    [point - normal * 7.0, point + normal * 7.0],
                    egui::Stroke::new(1.4, theme::AQUA),
                );
                painter.circle_filled(point, 6.0, theme::AQUA);
                painter.circle_stroke(
                    point,
                    10.0,
                    egui::Stroke::new(1.0, theme::AQUA.gamma_multiply(0.5)),
                );
                if !can_edit {
                    continue;
                }
                let hit =
                    egui::Rect::from_center_size(point, egui::vec2(44.0, 44.0)).intersect(rect);
                let response = ui.interact(
                    hit,
                    ui.id().with(("dimension-handle", parameter, i)),
                    egui::Sense::click_and_drag(),
                );
                if response.clicked() {
                    if editor.sheet.is_some() { editor.sheet = Some(Sheet::Edit); }
                    editor.active_field = parameter.label().into();
                }
                if response.drag_started() && b.distance(a) >= 4.0 {
                    let initial = parameter.value(d);
                    let symmetric = matches!(
                        parameter,
                        Parameter::Bore | Parameter::Width | Parameter::HeadLength
                    );
                    let sign = if i == 0 { -1.0 } else { 1.0 };
                    editor.drag = Some(HandleDrag {
                        parameter,
                        initial_value: initial,
                        screen_axis: axis * sign,
                        points_per_mm: (b.distance(a)
                            / initial.abs().max(0.05) as f32
                            / if symmetric { 2.0 } else { 1.0 })
                        .max(0.1),
                        pointer_start: ui
                            .input(|input| input.pointer.press_origin())
                            .unwrap_or(point),
                    });
                    editor.sheet = Some(Sheet::Edit);
                    editor.active_field = parameter.label().into();
                }
                if response.dragged() {
                    if let (Some(drag), Some(pos)) = (&editor.drag, response.interact_pointer_pos())
                    {
                        let delta =
                            (pos - drag.pointer_start).dot(drag.screen_axis) / drag.points_per_mm;
                        changed |= drag.parameter.set(d, drag.initial_value + delta as f64);
                    }
                }
                if response.drag_stopped() {
                    editor.drag = None;
                }
            }
            let value = parameter.value(d);
            let label = format!("{}  {:.2}{}", parameter.label(), value, parameter.unit());
            let bounds = tag(
                ui,
                rect,
                a.lerp(b, 0.5) + normal * 24.0,
                &label,
                theme::AQUA_BRIGHT,
            );
            if ui
                .interact(
                    bounds,
                    ui.id().with("dimension-label"),
                    egui::Sense::click(),
                )
                .clicked()
            {
                editor.sheet = Some(Sheet::Edit);
                editor.active_field = parameter.label().into();
            }
        }
        Mode::Surface => {
            if let Some(hit) = &editor.selection {
                let point = projection.at(hit.world);
                painter.circle_stroke(point, 13.0, egui::Stroke::new(2.0, theme::PINK));
                painter.circle_filled(point, 3.0, theme::PINK_BRIGHT);
                if let Some(entry) = selected_layer.and_then(|i| d.layers.layers.get(i)) {
                    tag(
                        ui,
                        rect,
                        point + egui::vec2(0.0, -30.0),
                        &entry.name,
                        theme::PINK_BRIGHT,
                    );
                }
            }
        }
        Mode::Stones => {
            let frames = ringdesign_core::stones::stone_frames(d);
            for (index, (stone, frame)) in frames.iter().enumerate() {
                let center = projection.at(frame.girdle.map(|x| x as f32));
                let (_, direction) = camera.ray(rect, center);
                if super::picking::dot(frame.normal, direction.map(f64::from)) > 0.12
                    || !rect.shrink(8.0).contains(center)
                {
                    continue;
                }
                let selected = editor.stone == Some(index);
                let color = if selected {
                    theme::PINK_BRIGHT
                } else {
                    theme::AQUA
                };
                let points: Vec<_> = (0..=32)
                    .map(|i| {
                        let angle = i as f64 * std::f64::consts::TAU / 32.0;
                        projection.at(std::array::from_fn(|axis| {
                            (frame.girdle[axis]
                                + frame.long[axis] * frame.semi.0 * angle.cos()
                                + frame.short[axis] * frame.semi.1 * angle.sin())
                                as f32
                        }))
                    })
                    .collect();
                painter.add(egui::Shape::line(
                    points,
                    egui::Stroke::new(if selected { 2.0 } else { 1.0 }, color),
                ));
                painter.circle_filled(center, if selected { 6.0 } else { 3.5 }, color);
                if selected {
                    tag(
                        ui,
                        rect,
                        center + egui::vec2(0.0, -30.0),
                        &format!("{}  {}", stone.label, stone.gem.display()),
                        color,
                    );
                }
                // Resolve overlaps by distance when tapping, instead of relying
                // on registration order of large finger targets.
            }
            if ui.input(|i| i.pointer.primary_clicked()) {
                if let Some(pos) = ui
                    .input(|i| i.pointer.interact_pos())
                    .filter(|p| rect.contains(*p))
                {
                    stone_pick = frames
                        .iter()
                        .enumerate()
                        .filter_map(|(index, (_, frame))| {
                            let center = projection.at(frame.girdle.map(|x| x as f32));
                            let (_, direction) = camera.ray(rect, center);
                            let distance = pos.distance(center);
                            (distance <= 24.0
                                && super::picking::dot(frame.normal, direction.map(f64::from))
                                    <= 0.12)
                                .then_some((index, distance))
                        })
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(i, _)| i);
                }
            }
        }
        Mode::Casting => {
            let setup = d
                .manufacturing
                .clone()
                .unwrap_or_else(|| ringdesign_core::manufacturing::Setup::from_design(d));
            let mut pull = setup.pull;
            let len = super::picking::dot(pull, pull).sqrt().max(1e-9);
            pull = pull.map(|v| v / len);
            let basis = if pull[2].abs() < 0.9 {
                [0.0, 0.0, 1.0]
            } else {
                [1.0, 0.0, 0.0]
            };
            let mut u = super::picking::cross(pull, basis);
            let len = super::picking::dot(u, u).sqrt().max(1e-9);
            u = u.map(|x| x / len);
            let v = super::picking::cross(pull, u);
            let extent = d.inner_radius_mm() + d.profile.thickness_mm + 3.0;
            let split = if setup.auto_parting && pull[2].abs() > 0.999 {
                parting_z * pull[2]
            } else {
                setup.parting_mm
            };
            let corners: Vec<_> = [
                (-1.0, -1.0),
                (1.0, -1.0),
                (1.0, 1.0),
                (-1.0, 1.0),
                (-1.0, -1.0),
            ]
            .iter()
            .map(|&(a, b)| {
                projection.at(std::array::from_fn(|axis| {
                    (pull[axis] * split + u[axis] * a * extent + v[axis] * b * extent) as f32
                }))
            })
            .collect();
            painter.add(egui::Shape::line(
                corners,
                egui::Stroke::new(1.0, theme::AQUA.gamma_multiply(0.55)),
            ));
            for sign in [-1.0, 1.0] {
                let start = projection.at(std::array::from_fn(|axis| (pull[axis] * split) as f32));
                let end = projection.at(std::array::from_fn(|axis| {
                    (pull[axis] * (split + sign * extent)) as f32
                }));
                painter.arrow(start, end - start, egui::Stroke::new(2.0, theme::AQUA));
            }
            tag(
                ui,
                rect,
                rect.center_top() + egui::vec2(0.0, 24.0),
                "Mould pull & parting guide",
                theme::AQUA_BRIGHT,
            );
            if let Some(hit) = &editor.selection {
                painter.circle_stroke(
                    projection.at(hit.world),
                    10.0,
                    egui::Stroke::new(2.0, theme::PINK),
                );
            }
        }
        _ => {}
    }
    if !ui.input(|i| i.pointer.primary_down()) {
        editor.drag = None;
    }
    (changed, stone_pick)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dimension_endpoints_match_the_source_size() {
        let mut d = RingDesign::default();
        for parameter in [
            Parameter::Bore,
            Parameter::Width,
            Parameter::Thickness,
            Parameter::HeadLength,
            Parameter::HeadRise,
        ] {
            let next = if parameter == Parameter::HeadRise {
                1.2
            } else {
                parameter.value(&d) + 0.3
            };
            parameter.set(&mut d, next);
            let (a, b) = dimension(&d, parameter);
            let distance = (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt();
            assert!(
                (distance as f64 - parameter.value(&d)).abs() < 1e-4,
                "{parameter:?}"
            );
        }
    }
}
