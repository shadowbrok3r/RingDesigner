//! Forward Android's pen hover before egui hit testing, across the entire UI.
use egui_mobile::egui;

#[derive(Default)]
pub struct PenHover {
    hovering: bool,
    last_position: Option<egui::Pos2>,
}

impl PenHover {
    pub fn feed(&mut self, input: &mut egui::RawInput, hover: Option<(f32, f32)>, scale: f32) {
        let contact = input.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::PointerButton { pressed: true, .. }
                    | egui::Event::Touch {
                        phase: egui::TouchPhase::Start | egui::TouchPhase::Move,
                        ..
                    }
            )
        });
        let position = hover
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .filter(|_| scale.is_finite() && scale > 0.0 && !contact);
        if let Some((x, y)) = position {
            let next = egui::pos2(x / scale, y / scale);
            // egui restarts its tooltip timer on every PointerMoved, even if
            // the coordinates are identical. Coalesce steady pen samples.
            if self
                .last_position
                .is_none_or(|last| last.distance(next) > 0.5)
            {
                input.events.push(egui::Event::PointerMoved(next));
                self.last_position = Some(next);
            }
        } else if self.hovering
            && !contact
            && !input.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. }
                )
            })
        {
            input.events.push(egui::Event::PointerGone);
        }
        self.hovering = position.is_some();
        if !self.hovering {
            self.last_position = None;
        }
    }
}
impl egui::Plugin for PenHover {
    fn debug_name(&self) -> &'static str {
        "RingDesigner pen hover"
    }
    fn input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        #[cfg(target_os = "android")]
        let hover = {
            use android_activity::input::{PointerTool, pointer_probe};
            let p = pointer_probe();
            if matches!(p.tool, PointerTool::Stylus | PointerTool::Eraser) {
                p.hover
            } else {
                None
            }
        };
        #[cfg(not(target_os = "android"))]
        let hover = None;
        self.feed(input, hover, ctx.pixels_per_point());
        if self.hovering {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hover_uses_window_points_and_clears_on_exit() {
        let mut p = PenHover::default();
        let mut input = egui::RawInput::default();
        p.feed(&mut input, Some((875.0, 350.0)), 3.5);
        assert!(
            matches!(input.events.as_slice(),[egui::Event::PointerMoved(pos)] if *pos==egui::pos2(250.0,100.0))
        );
        input.events.clear();
        p.feed(&mut input, Some((875.0, 350.0)), 3.5);
        assert!(
            input.events.is_empty(),
            "stationary hover must not restart the hint timer"
        );
        p.feed(&mut input, None, 3.5);
        assert!(matches!(
            input.events.as_slice(),
            [egui::Event::PointerGone]
        ));
        input.events.clear();
        p.feed(&mut input, None, 3.5);
        assert!(input.events.is_empty());
    }
    #[test]
    fn stale_hover_does_not_override_contact_or_other_pointer() {
        let mut p = PenHover::default();
        let mut input = egui::RawInput::default();
        p.feed(&mut input, Some((50.0, 70.0)), 2.0);
        input.events.clear();
        input.events.push(egui::Event::PointerButton {
            pos: egui::pos2(5.0, 6.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
        p.feed(&mut input, Some((80.0, 90.0)), 2.0);
        assert_eq!(input.events.len(), 1);
        input.events.clear();
        p.feed(&mut input, Some((f32::NAN, 0.0)), 2.0);
        assert!(input.events.is_empty());
    }
}
