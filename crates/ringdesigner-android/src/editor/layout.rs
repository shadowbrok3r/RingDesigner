//! Optional bounds overlay and a structured report for emulator interaction tests.
use egui_mobile::egui;

#[derive(Clone)]
struct Bounds {
    label: String,
    rect: egui::Rect,
    clip: egui::Rect,
}
fn id() -> egui::Id {
    egui::Id::new("mobile-layout-bounds")
}

pub fn begin(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(id(), Vec::<Bounds>::new()));
}
pub fn record(ui: &egui::Ui, label: impl Into<String>, rect: egui::Rect) {
    let clip = ui.clip_rect();
    if !clip.intersects(rect) {
        return;
    }
    ui.ctx().data_mut(|d| {
        d.get_temp_mut_or_default::<Vec<Bounds>>(id()).push(Bounds {
            label: label.into(),
            rect,
            clip,
        })
    });
}

fn array(r: egui::Rect) -> [f32; 4] {
    [r.left(), r.top(), r.right(), r.bottom()]
}
pub fn report(ctx: &egui::Context, safe: egui::Rect) -> serde_json::Value {
    let bounds = ctx.data(|d| d.get_temp::<Vec<Bounds>>(id()).unwrap_or_default());
    let mut overflow = Vec::new();
    let controls:Vec<_>=bounds.iter().map(|b| {
        // Vertical clipping inside a scroll area is expected. Horizontal loss
        // of controls and panels outside the phone's safe area are failures.
        let clip=b.clip.intersect(safe);
        let outside=b.rect.left()<clip.left()-1.5 || b.rect.right()>clip.right()+1.5;
        if outside {overflow.push(b.label.clone());}
        serde_json::json!({"label":b.label,"rect":array(b.rect),"clip":array(clip),"horizontal_overflow":outside})
    }).collect();
    serde_json::json!({"safe_rect":array(safe),"controls":controls,"overflow":overflow})
}

pub fn draw(ui: &egui::Ui, safe: egui::Rect) {
    let bounds = ui
        .ctx()
        .data(|d| d.get_temp::<Vec<Bounds>>(id()).unwrap_or_default());
    ui.painter().rect_stroke(
        safe.shrink(1.0),
        0.0,
        egui::Stroke::new(1.0, egui::Color32::LIGHT_BLUE),
        egui::StrokeKind::Inside,
    );
    for b in bounds {
        let clip = b.clip.intersect(safe);
        let outside = b.rect.left() < clip.left() - 1.5 || b.rect.right() > clip.right() + 1.5;
        let color = if outside {
            egui::Color32::RED
        } else {
            egui::Color32::from_rgba_unmultiplied(43, 226, 214, 90)
        };
        ui.painter().rect_stroke(
            b.rect.intersect(clip),
            2.0,
            egui::Stroke::new(1.0, color),
            egui::StrokeKind::Inside,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{Editor, Mode, controls};
    #[test]
    fn primary_inspectors_fit_narrow_scrolling_panels() {
        for width in [280.0, 320.0, 411.0] {
            for mode in Mode::ALL {
                let ctx = egui::Context::default();
                crate::theme::apply(&ctx);
                let mut editor = Editor::default();
                editor.mode = mode;
                let mut d = ringdesign_core::RingDesign::default();
                let lib_ctx = d.field_context();
                let t = ringdesign_core::tiling::TilingLayer::default_for("test", &lib_ctx);
                d.layers
                    .layers
                    .push(ringdesign_core::field::LayerEntry::new(
                        "An unusually long ornament name that must stay inside the phone",
                        ringdesign_core::field::Layer::Tiling(t),
                    ));
                let mut selected = Some(0);
                if mode == Mode::Stones {
                    let mut seat = ringdesign_core::field::SeatPadLayer::default();
                    seat.fit_stone(ringdesign_core::gem::Gem::calibrated(
                        ringdesign_core::gem::GemCut::Round,
                        4.0,
                    ));
                    d.layers.layers[0].layer = ringdesign_core::field::Layer::SeatPad(seat);
                    editor.active_field = "Stone width".into();
                }
                // Merely displaying an imported design must not clamp its values.
                d.profile.width_mm = 25.0;
                let before = serde_json::to_value(&d).unwrap();
                let safe = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 300.0));
                for _ in 0..2 {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(safe),
                            ..Default::default()
                        },
                        |root| {
                            begin(root.ctx());
                            egui::CentralPanel::default().show(root, |ui| {
                                egui::ScrollArea::vertical()
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| match mode {
                                        Mode::Shape => {
                                            controls::shape(ui, &mut editor, &mut d);
                                        }
                                        Mode::Surface => {
                                            controls::surface(
                                                ui,
                                                &mut editor,
                                                &mut d,
                                                &mut selected,
                                            );
                                        }
                                        Mode::Stones => {
                                            controls::stones(
                                                ui,
                                                &mut editor,
                                                &mut d,
                                                &mut selected,
                                            );
                                        }
                                        Mode::Casting => {
                                            controls::casting(ui, &mut editor, &mut d);
                                        }
                                    });
                            });
                        },
                    );
                    output.textures_delta.clear();
                }
                let data = report(&ctx, safe);
                assert!(
                    data["overflow"].as_array().unwrap().is_empty(),
                    "{width} {mode:?}: {data}"
                );
                assert_eq!(
                    before,
                    serde_json::to_value(&d).unwrap(),
                    "{mode:?} changed source without input"
                );
            }
        }
    }

    #[test]
    fn numeric_focus_survives_chrome_changes() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut active = String::new();
        let mut value = 17.3;
        let mut target = egui::Pos2::ZERO;
        let mut focus = None;
        for step in 0..5 {
            let mut events = Vec::new();
            if step == 1 || step == 2 {
                events.push(egui::Event::PointerMoved(target));
                events.push(egui::Event::PointerButton {
                    pos: target,
                    button: egui::PointerButton::Primary,
                    pressed: step == 1,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(320.0, 400.0),
                    )),
                    events,
                    ..Default::default()
                },
                |root| {
                    begin(root.ctx());
                    egui::CentralPanel::default().show(root, |ui| {
                        if step >= 3 {
                            ui.label("Keyboard changes the surrounding chrome");
                        }
                        controls::field(
                            ui,
                            &mut active,
                            "Finger opening",
                            &mut value,
                            13.0..=25.0,
                            " mm",
                            "Fit diameter",
                        );
                    });
                },
            );
            output.textures_delta.clear();
            if step == 0 {
                let b = ctx.data(|d| d.get_temp::<Vec<Bounds>>(id()).unwrap());
                target = b
                    .iter()
                    .find(|b| b.label == "value/Finger opening")
                    .unwrap()
                    .rect
                    .center();
            }
            if step == 2 {
                focus = ctx.memory(|m| m.focused());
                assert!(focus.is_some());
            }
            if step >= 3 {
                assert_eq!(
                    focus,
                    ctx.memory(|m| m.focused()),
                    "Keyboard layout lost numeric focus"
                );
            }
        }
    }
}
