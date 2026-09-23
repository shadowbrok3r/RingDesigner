//! Measure on the phone's ring: a tap reads a vertex, an edge, a face, a stone, a pin or a point of the band, the readings drawn over the ring and said in a bar.
use egui::Pos2;
use egui_mobile::egui;
use ringdesign_core::interaction::pick::{Filter, Pick};
use ringdesign_workbench::{
    icons::Icon,
    touch,
    visual::measure::{Measure, Picked},
};

use super::{Cad, Request, View, bar, ctx};

/// The Measure tool's picks and how it reads the band.
#[derive(Clone, Debug, Default)]
pub struct Measuring {
    pub measure: Measure,
    /// Band points read where the finger lands, not snapped to the ring's features.
    pub free: bool,
    /// The last tap met nothing to measure.
    pub missed: bool,
}

/// What the Measure bar says under its readings when the last tap met nothing.
pub const MISSED: &str = "Nothing under the finger: tap on the metal";

/// `r` moved the least that puts it inside `bounds`, where it fits.
fn inside(r: egui::Rect, bounds: egui::Rect) -> egui::Rect {
    let dx = (bounds.left() - r.left()).max(0.0) + (bounds.right() - r.right()).min(0.0);
    let dy = (bounds.top() - r.top()).max(0.0) + (bounds.bottom() - r.bottom()).min(0.0);
    r.translate(egui::vec2(dx, dy))
}

/// What the Measure bar's buttons do, in their order.
const CLEAR: usize = 0;
const SNAP: usize = 1;
const DONE: usize = 2;

impl Cad {
    /// Reads what lies under a tap at `p` into the measurement and says what it reads; whether the tap was taken.
    pub(super) fn measure_tap(&mut self, v: &View, p: Pos2) -> bool {
        let picked = self.measure_pick(v, p);
        self.measuring.missed = picked.is_none();
        match picked {
            Some(picked) => {
                touch::measure::tap(&mut self.measuring.measure, picked);
                let said = touch::measure::said(&self.measuring.measure).join(" · ");
                self.status(said);
            }
            None => self.status("Nothing under the finger to measure"),
        }
        true
    }

    /// What a tap at `p` measures from: a pin in reach, else the finest thing under the finger, a band point snapped to the ring's features unless free.
    fn measure_pick(&mut self, v: &View, p: Pos2) -> Option<Picked> {
        let proj = v.camera.projector(v.rect);
        if let Some(pin) = touch::measure::pin_at(&v.design.pins, |w| proj.at(w.map(|x| x as f32)), p, touch::FINGER_PT) {
            return Some(pin);
        }
        let build = v.build?;
        let ray = |q: Pos2| v.ray(q);
        let (view, r) = ringdesign_workbench::hover::view_scale(p, &ray);
        let picks: Vec<Pick> = match &self.scene {
            Some(scene) => scene.pick(r, &view, touch::APERTURE_PT, Filter::default()),
            None => Self::band_pick(v, p).into_iter().collect(),
        };
        let free = self.measuring.free;
        let c = ctx!(self, v);
        let live = &mut self.live;
        touch::measure::reading(&picks, v.design, &build.0, |world| if free { None } else { live.snap_band_point(&c, build, world, view).map(|hit| (hit.world, hit.label)) })
    }

    /// The picks as dots, and every reading as a dimension line with its value, over the ring.
    pub(super) fn draw_measure(&self, painter: &egui::Painter, v: &View) {
        let proj = v.camera.projector(v.rect);
        let screen = |w: [f64; 3]| proj.at(w.map(|x| x as f32));
        let accent = crate::theme::PINK_BRIGHT;
        let m = &self.measuring.measure;
        for p in &m.picks {
            painter.circle(screen(p.at()), 5.0, accent, egui::Stroke::new(1.5, egui::Color32::BLACK));
        }
        for r in m.readings() {
            let (a, b) = (screen(r.from), screen(r.to));
            let at = if r.what == "Corner" {
                m.picks.get(1).map_or(a.lerp(b, 0.5), |mid| screen(mid.at())) + egui::vec2(12.0, 12.0)
            } else {
                painter.line_segment([a, b], egui::Stroke::new(2.5, accent));
                for end in [a, b] {
                    painter.circle_stroke(end, 4.0, egui::Stroke::new(1.5, accent));
                }
                a.lerp(b, 0.5)
            };
            let galley = painter.layout_no_wrap(r.line(), egui::FontId::proportional(13.0), crate::theme::INK);
            let tag = inside(egui::Rect::from_min_size(at + egui::vec2(8.0, -galley.size().y - 6.0), galley.size()), v.rect.shrink(10.0));
            painter.rect_filled(tag.expand2(egui::vec2(6.0, 4.0)), 4.0, egui::Color32::from_black_alpha(210));
            painter.galley(tag.min, galley, crate::theme::INK);
        }
    }

    /// The Measure bar at the ring's foot: what the taps read, Clear, Snap and Done.
    pub(super) fn measure_bar(&mut self, ctx: &egui::Context, v: &View) {
        let mut lines = vec![(touch::measure::said(&self.measuring.measure).join("\n"), crate::theme::INK)];
        if self.measuring.measure.picks.len() < 2 {
            lines.insert(0, ("Measure: a third tap chains on and reads the corner".to_string(), crate::theme::AQUA));
        }
        if self.measuring.missed {
            lines.push((MISSED.to_string(), crate::theme::PINK_BRIGHT));
        }
        let buttons = [
            bar::Button { icon: Icon::Delete, label: "Clear", checked: false, enabled: !self.measuring.measure.picks.is_empty() },
            bar::Button { icon: Icon::Guides, label: "Snap", checked: !self.measuring.free, enabled: true },
            bar::Button { icon: Icon::Check, label: "Done", checked: false, enabled: true },
        ];
        match bar::show(ctx, v.rect, v.covered, &lines, &buttons) {
            Some(CLEAR) => {
                self.measuring.measure.clear();
                self.measuring.missed = false;
                self.status("Measurement cleared");
            }
            Some(SNAP) => {
                self.measuring.free = !self.measuring.free;
                self.status(if self.measuring.free { "Band points read where the finger lands" } else { "Band points snap to the ring's features" });
            }
            Some(DONE) => {
                self.measuring.measure.clear();
                self.measuring.missed = false;
                self.requests.push(Request::EndMeasure);
            }
            _ => {}
        }
    }

    /// The Measure tool's controls in a panel: how to use it, the picks, the readings, Snap and Clear.
    pub fn measure_controls(&mut self, ui: &mut egui::Ui) {
        ui.label("Tap a vertex, an edge, a face, a stone, a pin or a point of the band. A second tap reads the distance; a third chains on and reads the corner; a fourth starts again.");
        let m = &self.measuring.measure;
        for (i, p) in m.picks.iter().enumerate() {
            ui.small(format!("{}. {}", i + 1, p.label()));
        }
        for r in m.readings() {
            ui.colored_label(crate::theme::AQUA, r.line());
        }
        let mut snap = !self.measuring.free;
        if ui.checkbox(&mut snap, "Snap band points to the ring's features").changed() {
            self.measuring.free = !snap;
        }
        if ui.add_enabled(!self.measuring.measure.picks.is_empty(), egui::Button::new("Clear measurement")).clicked() {
            self.measuring.measure.clear();
        }
        ui.small("Straight-line distances on the ring as built; not arc length round the band.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Rect, pos2, vec2};

    #[test]
    fn a_value_tag_past_the_views_edge_moves_back_inside_it_and_one_inside_stays() {
        let view = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 600.0));
        let past = Rect::from_min_size(pos2(360.0, -8.0), vec2(90.0, 16.0));
        assert_eq!(inside(past, view), Rect::from_min_size(pos2(310.0, 0.0), vec2(90.0, 16.0)));
        let within = Rect::from_min_size(pos2(100.0, 200.0), vec2(90.0, 16.0));
        assert_eq!(inside(within, view), within);
    }
}
