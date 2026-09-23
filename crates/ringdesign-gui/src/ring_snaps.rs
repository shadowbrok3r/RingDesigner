//! Pins on the ring, which the snaps and Measure read from, and the Measure tool in the Ring viewport.
use egui::{Pos2, Rect};
use ringdesign_core::interaction::pick::{Entity, Filter};
use ringdesign_workbench::viewport::pins::{self, Pin};
use ringdesign_workbench::visual::measure::{self, Picked};

use crate::app::RingDesignerApp;
use crate::camera::Projector;
use crate::theme;
use crate::viewport::APERTURE_PX;

/// A pin's marker radius on screen.
const PIN_PX: f32 = 4.0;

/// Drops a pin where the band was clicked.
pub fn pin_here(app: &mut RingDesignerApp, _pane: usize, world: [f64; 3]) {
    let pin = Pin::at(world, pins::next_number(app.pins()));
    let said = format!("{} at {:.1}° · {:.2} mm across — the snaps and Measure read it", pin.name, pin.theta_deg, pin.across_mm);
    app.pins_mut().push(pin);
    app.set_status(said);
}

/// Takes every pin off the ring.
pub fn clear_pins(app: &mut RingDesignerApp) {
    let n = app.pins().len();
    app.pins_mut().clear();
    app.set_status(if n == 0 { "No pins to clear".to_string() } else { format!("{n} pin{} cleared", if n == 1 { "" } else { "s" }) });
}

/// The pins, each a marker and its name, on a Ring viewport.
pub fn draw_pins(app: &RingDesignerApp, painter: &egui::Painter, proj: &Projector, rect: Rect) {
    for pin in app.pins() {
        let p = proj.at(pin.world.map(|v| v as f32));
        if !rect.contains(p) {
            continue;
        }
        let stroke = egui::Stroke::new(1.5, theme::ACCENT);
        painter.line_segment([p, p + egui::vec2(0.0, -12.0)], stroke);
        painter.circle(p + egui::vec2(0.0, -12.0 - PIN_PX), PIN_PX, theme::ACCENT, egui::Stroke::new(1.0, egui::Color32::from_black_alpha(200)));
        painter.circle_filled(p, 1.8, theme::ACCENT);
        painter.text(p + egui::vec2(PIN_PX + 3.0, -12.0 - PIN_PX), egui::Align2::LEFT_CENTER, &pin.name, egui::FontId::proportional(11.0), theme::ACCENT);
    }
}

/// What a Measure pick at `pos` reads: a pin in reach, else the best pick, a band point snapped to the ring's features unless `free`.
fn pick(app: &mut RingDesignerApp, pane: usize, rect: Rect, pos: Pos2, free: bool) -> Option<Picked> {
    let camera = app.panes[pane].camera;
    let proj = camera.projector(rect);
    let near = app.pins().iter().map(|p| (p, proj.at(p.world.map(|v| v as f32)).distance(pos))).filter(|(_, d)| *d <= APERTURE_PX).min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((pin, _)) = near {
        return Some(Picked::Point { at: pin.world, label: pin.name.clone() });
    }
    let (scene, build) = (app.pick_scene.clone()?, app.build.clone()?);
    let ray = |p: Pos2| camera.ray(rect, p);
    let (view, _) = ringdesign_workbench::hover::view_scale(pos, &ray);
    let first = ringdesign_workbench::hover::pick_at(&scene, pos, &ray, APERTURE_PX, Filter::default()).into_iter().next()?;
    if first.entity == Entity::Band
        && !free
        && let Some(hit) = crate::command::snap_band_point(app, &build, first.world, view)
    {
        return Some(Picked::Point { at: hit.world, label: hit.label });
    }
    measure::picked(&first, &app.design, &build)
}

/// Measure on the active Ring viewport: a click picks, Shift chains a third, and each reading is drawn and labelled for a reader.
pub fn measure(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize, rect: Rect, response: &egui::Response, painter: &egui::Painter, proj: &Projector) {
    let (shift, free) = ui.input(|i| (i.modifiers.shift, i.modifiers.command));
    // A Shift-click chains; a Shift-drag pans.
    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos().filter(|p| rect.contains(*p))
    {
        app.active_pane = pane;
        match pick(app, pane, rect, pos, free) {
            Some(p) => {
                app.visual.measurement.add(p, shift);
                let said = app.visual.measurement.readings().iter().map(|r| r.line()).collect::<Vec<_>>().join(" · ");
                let picked = app.visual.measurement.picks.last().map(|p| p.label().to_owned()).unwrap_or_default();
                app.set_status(if said.is_empty() { format!("Measure from {picked}: pick the second") } else { said });
            }
            None => app.set_status("Nothing under the pointer to measure"),
        }
    }
    // The pick under the pointer, before it is taken.
    if let Some(pos) = response.hover_pos().filter(|p| rect.contains(*p) && !response.dragged()) {
        if let Some(p) = pick(app, pane, rect, pos, free) {
            let at = proj.at(p.at().map(|v| v as f32));
            painter.circle_stroke(at, 5.0, egui::Stroke::new(1.6, ringdesign_workbench::hover::AQUA));
            ringdesign_workbench::hover::caption(painter, rect.with_min_x(rect.left() + crate::command::RAIL_W), p.label(), ringdesign_workbench::hover::AQUA);
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    let screen = |w: [f64; 3]| proj.at(w.map(|v| v as f32));
    for p in &app.visual.measurement.picks {
        painter.circle_filled(screen(p.at()), 4.0, theme::ACCENT);
    }
    let accessible = ui.ctx().accesskit_node_builder(ui.id(), |_| ()).is_some();
    let readings = app.visual.measurement.readings();
    for (i, r) in readings.iter().enumerate() {
        let (a, b) = (screen(r.from), screen(r.to));
        let at = if r.what == "Corner" {
            app.visual.measurement.picks.get(1).map_or(a.lerp(b, 0.5), |m| screen(m.at())) + egui::vec2(10.0, 10.0)
        } else {
            painter.line_segment([a, b], egui::Stroke::new(2.0, theme::ACCENT));
            for end in [a, b] {
                painter.circle_stroke(end, 3.0, egui::Stroke::new(1.2, theme::ACCENT));
            }
            a.lerp(b, 0.5)
        };
        let text = r.line();
        let galley = painter.layout_no_wrap(text.clone(), egui::FontId::proportional(12.0), theme::TEXT);
        let tag = Rect::from_min_size(at + egui::vec2(6.0, -galley.size().y - 4.0), galley.size());
        painter.rect_filled(tag.expand2(egui::vec2(5.0, 3.0)), 3.0, egui::Color32::from_black_alpha(200));
        painter.galley(tag.min, galley, theme::TEXT);
        if accessible {
            let node = ui.interact(tag, ui.id().with(("measure-reading", pane, i)), egui::Sense::hover());
            node.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, format!("Measure: {text}")));
        }
    }
}
