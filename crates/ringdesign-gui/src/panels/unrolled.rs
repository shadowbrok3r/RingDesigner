//! The unrolled band: a flat `u`/`v` canvas for laying tiles around the ring.
//!
//! Horizontal is arc distance around the ring, vertical is arc distance across
//! the cross-section. The composited height field is drawn underneath, so a
//! tile placed here is exactly where the metal lands.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use egui_phosphor::regular as icon;

use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::drawn::Stroke;
use ringdesign_core::field::{FieldContext, Layer, LayerStack, Uv};
use ringdesign_core::paint;
use ringdesign_core::profile::ProfileLoop;
use ringdesign_core::tiling::TilingLayer;

use crate::app::RingDesignerApp;
use crate::theme;

const PAD_L: f32 = 20.0;
const PAD_R: f32 = 14.0;
const PAD_T: f32 = 27.0;
const PAD_B: f32 = 22.0;

/// Cells drawn with the alpha thumbnail; the rest get outlines only.
const MAX_TEXTURED: usize = 320;
/// Cells drawn at all.
const MAX_OUTLINED: usize = 4000;
/// Scroll points per step of `repeats_around`.
const SCROLL_STEP: f32 = 26.0;
/// Upper bound on `repeats_around`, matching the layer editor's Around field.
const MAX_REPEATS: i64 = 400;
const KNOB_R: f32 = 15.0;
const HANDLE_GRAB_PX: f32 = 6.0;

/// Reach of the band-edge handles' snap onto a side-face boundary, mm.
const SNAP_MM: f64 = 0.2;

/// The pan/zoom window onto the unrolled band, in band mm.
#[derive(Clone, Copy, PartialEq)]
struct UnrolledView {
    zoom: f32,
    u0: f64,
    v0: f64,
}

impl Default for UnrolledView {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            u0: 0.0,
            v0: 0.0,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Grab {
    #[default]
    None,
    Lattice,
    VLow,
    VHigh,
    Rotate,
    /// A handle on a non-tiling layer: rail and bead rows as v-lines, pads,
    /// stamps and signet plates as centres, a pad's rim as its radius.
    Other {
        layer: usize,
        handle: u8,
    },
}

/// Where a non-tiling layer can be grabbed on the canvas.
enum HShape {
    HLine(f32),
    Point(egui::Pos2),
    Ring(egui::Pos2, f32),
}

struct LHandle {
    layer: usize,
    handle: u8,
    shape: HShape,
}

impl HShape {
    fn dist(&self, p: egui::Pos2) -> f32 {
        match self {
            HShape::HLine(y) => (p.y - y).abs(),
            HShape::Point(c) => (p - *c).length(),
            HShape::Ring(c, r) => ((p - *c).length() - r).abs(),
        }
    }
}

/// Radius-drag handle ids start here; below are centres and v-lines.
const H_RADIUS: u8 = 1;
/// Decal instance k grabs as handle `H_DECAL + k`.
const H_DECAL: u8 = 8;
/// The selected layer's angular window: centre, both edges, both fades.
const H_WIN_C: u8 = 230;
const H_WIN_A: u8 = 231;
const H_WIN_B: u8 = 232;
const H_WIN_FA: u8 = 233;
const H_WIN_FB: u8 = 234;

/// Rendered height field, kept until the layer stack or the canvas changes.
#[derive(Clone)]
struct FieldCache {
    key: u64,
    tex: egui::TextureHandle,
    max_mm: f64,
}

pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    let ctx = app.design.field_context();
    let (rect, response) =
        ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme::VIEWPORT_BG);

    let too_small = rect.width() < PAD_L + PAD_R + 80.0 || rect.height() < PAD_T + PAD_B + 60.0;
    if ctx.circumference_mm <= 1e-6 || ctx.band_v_len_mm <= 1e-6 || too_small {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Not enough room to unroll the band",
            egui::FontId::proportional(13.0),
            theme::TEXT_DIM,
        );
        return;
    }

    let plot = egui::Rect::from_min_max(
        rect.min + egui::vec2(PAD_L, PAD_T),
        rect.max - egui::vec2(PAD_R, PAD_B),
    );

    // --- Pan/zoom: a window (u0, v0, spans) onto the band -------------------
    //
    // Everything keeps drawing through the same closures; zoom only changes
    // what they map, and the painter's clip does the cropping. `u` wraps —
    // the window can straddle the ring joint.
    let view_id = ui.id().with("unrolled_view");
    let mut view = ui
        .memory(|m| m.data.get_temp::<UnrolledView>(view_id))
        .unwrap_or_default();
    let circ = ctx.circumference_mm;
    let band = ctx.band_v_len_mm;
    view.zoom = view.zoom.clamp(1.0, 12.0);
    let u_span = circ / view.zoom as f64;
    let v_span = band / view.zoom as f64;
    view.v0 = view.v0.clamp(0.0, band - v_span);
    view.u0 = view.u0.rem_euclid(circ);
    let (u0, v0) = (view.u0, view.v0);

    let hover = response.hover_pos();
    if let Some(p) = hover {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.1 && ui.input(|i| i.modifiers.command || i.modifiers.ctrl) {
            // Anchor the mm under the pointer while zooming.
            let fx = ((p.x - plot.left()) / plot.width().max(1.0)).clamp(0.0, 1.0) as f64;
            let fy = ((p.y - plot.top()) / plot.height().max(1.0)).clamp(0.0, 1.0) as f64;
            let anchor_u = (u0 + fx * u_span).rem_euclid(circ);
            let anchor_v = v0 + fy * v_span;
            view.zoom = (view.zoom * (1.0 + scroll * 0.002)).clamp(1.0, 12.0);
            let nu = circ / view.zoom as f64;
            let nv = band / view.zoom as f64;
            view.u0 = (anchor_u - fx * nu).rem_euclid(circ);
            view.v0 = (anchor_v - fy * nv).clamp(0.0, band - nv);
        }
    }
    if response.dragged_by(egui::PointerButton::Secondary) {
        let d = response.drag_delta();
        view.u0 = (view.u0 - d.x as f64 * u_span / plot.width().max(1.0) as f64).rem_euclid(circ);
        view.v0 = (view.v0 - d.y as f64 * v_span / plot.height().max(1.0) as f64)
            .clamp(0.0, band - v_span);
    }
    ui.memory_mut(|m| m.data.insert_temp(view_id, view));
    let (u0, v0) = (view.u0, view.v0);
    let u_span = circ / view.zoom as f64;
    let v_span = band / view.zoom as f64;
    let full_w = plot.width() * view.zoom;
    let full_h = plot.height() * view.zoom;

    let x_of_u = |u: f64| plot.left() + ((u - u0).rem_euclid(circ) / circ) as f32 * full_w;
    let y_of_v = |v: f64| plot.top() + ((v - v0) / band) as f32 * full_h;
    let v_of_y = |y: f32| (y - plot.top()) as f64 / full_h.max(1.0) as f64 * band + v0;
    let mm_per_px_u = u_span / plot.width().max(1.0) as f64;
    let mm_per_px_v = v_span / plot.height().max(1.0) as f64;

    if view.zoom > 1.001 {
        let chip = egui::Rect::from_min_size(
            egui::pos2(plot.right() - 52.0, plot.top() + 4.0),
            egui::vec2(48.0, 18.0),
        );
        let r = ui.interact(chip, ui.id().with("unrolled_fit"), egui::Sense::click());
        painter.rect_filled(chip, 4.0, theme::PANEL.gamma_multiply(0.9));
        painter.text(
            chip.center(),
            egui::Align2::CENTER_CENTER,
            format!("{:.0}% ✖", view.zoom * 100.0),
            egui::FontId::proportional(10.0),
            if r.hovered() {
                theme::ACCENT
            } else {
                theme::TEXT_DIM
            },
        );
        if r.clicked() {
            ui.memory_mut(|m| m.data.insert_temp(view_id, UnrolledView::default()));
        }
        r.on_hover_text("Ctrl+scroll zooms, right-drag pans. Click to fit.");
    }

    // --- The selected tiling layer, worked on as a clone ---------------------

    let selected = app
        .selected_layer
        .filter(|&i| i < app.design.layers.layers.len());
    let mut tile: Option<TilingLayer> =
        selected.and_then(|i| match &app.design.layers.layers[i].layer {
            Layer::Tiling(t) => Some(t.clone()),
            _ => None,
        });
    let mut changed = false;

    // --- Handles for every other layer kind ---------------------------------
    //
    // Composing four or five layers by numeric entry is blind; these put a
    // grip on each one where it lands. Every enabled layer is grabbable, and
    // grabbing one selects it.
    let mut handles: Vec<LHandle> = Vec::new();
    for (i, entry) in app.design.layers.layers.iter().enumerate() {
        if !entry.enabled {
            continue;
        }
        let center = |theta: f64, v: f64| {
            egui::pos2(x_of_u(ctx.u_of_theta(theta.rem_euclid(360.0))), y_of_v(v))
        };
        match &entry.layer {
            Layer::Border(b) => {
                handles.push(LHandle {
                    layer: i,
                    handle: 0,
                    shape: HShape::HLine(y_of_v(b.v_mm)),
                });
            }
            Layer::Milgrain(m) => {
                handles.push(LHandle {
                    layer: i,
                    handle: 0,
                    shape: HShape::HLine(y_of_v(m.v_mm)),
                });
            }
            Layer::SeatRun(r) => {
                handles.push(LHandle {
                    layer: i,
                    handle: 0,
                    shape: HShape::HLine(y_of_v(r.seat.v_mm)),
                });
            }
            Layer::SeatPad(s) => {
                let c = center(s.theta_deg, s.v_mm);
                let kv = if s.metal_true { s.station_scale(&ctx).1.max(1e-6) } else { 1.0 };
                let r_px = (s.diameter_mm * 0.5 / kv / mm_per_px_v.max(1e-9)) as f32;
                handles.push(LHandle {
                    layer: i,
                    handle: 0,
                    shape: HShape::Point(c),
                });
                handles.push(LHandle {
                    layer: i,
                    handle: H_RADIUS,
                    shape: HShape::Ring(c, r_px.max(8.0)),
                });
            }
            Layer::Signet(sg) => {
                handles.push(LHandle {
                    layer: i,
                    handle: 0,
                    shape: HShape::Point(center(sg.theta_deg, sg.v_mm)),
                });
            }
            Layer::Decals(d) => {
                for (k, dec) in d.decals.iter().enumerate().take(120) {
                    handles.push(LHandle {
                        layer: i,
                        handle: H_DECAL + k.min(200) as u8,
                        shape: HShape::Point(center(dec.theta_deg, dec.v_mm)),
                    });
                }
            }
            _ => {}
        }
    }

    // The selected layer's angular window, as grips on a strip above the
    // band: centre moves it, the squares resize the span, the diamonds pull
    // the fades. Only when the window is on — a full-ring layer has nothing
    // to grab.
    if let Some(i) = selected
        && app
            .design
            .layers
            .layers
            .get(i)
            .is_some_and(|e| e.window.enabled)
    {
        let w = &app.design.layers.layers[i].window;
        let y = plot.top() + 14.0;
        let half = w.span_deg.max(0.0) * 0.5;
        let (a, b) = (w.theta_deg - half, w.theta_deg + half);
        let arc = |from: f64, to: f64, stroke: egui::Stroke| {
            let steps = 48;
            for k in 0..steps {
                let t0 = from + (to - from) * k as f64 / steps as f64;
                let t1 = from + (to - from) * (k + 1) as f64 / steps as f64;
                let p0 = egui::pos2(x_of_u(ctx.u_of_theta(t0.rem_euclid(360.0))), y);
                let p1 = egui::pos2(x_of_u(ctx.u_of_theta(t1.rem_euclid(360.0))), y);
                if (p1.x - p0.x).abs() < plot.width() * 0.5 {
                    painter.line_segment([p0, p1], stroke);
                }
            }
        };
        let tone = if w.invert { theme::WARN } else { theme::ACCENT };
        arc(a, b, egui::Stroke::new(3.0, tone.gamma_multiply(0.55)));
        arc(
            a - w.fade_deg,
            a,
            egui::Stroke::new(1.5, tone.gamma_multiply(0.3)),
        );
        arc(
            b,
            b + w.fade_deg,
            egui::Stroke::new(1.5, tone.gamma_multiply(0.3)),
        );

        let at = |deg: f64| egui::pos2(x_of_u(ctx.u_of_theta(deg.rem_euclid(360.0))), y);
        let centre = at(w.theta_deg);
        painter.circle_filled(centre, 5.0, tone);
        for (deg, h) in [(a, H_WIN_A), (b, H_WIN_B)] {
            let p = at(deg);
            painter.rect_filled(
                egui::Rect::from_center_size(p, egui::vec2(7.0, 7.0)),
                1.0,
                tone,
            );
            handles.push(LHandle {
                layer: i,
                handle: h,
                shape: HShape::Point(p),
            });
        }
        for (deg, h) in [(a - w.fade_deg, H_WIN_FA), (b + w.fade_deg, H_WIN_FB)] {
            let p = at(deg);
            painter.circle_stroke(p, 3.5, egui::Stroke::new(1.5, tone));
            handles.push(LHandle {
                layer: i,
                handle: h,
                shape: HShape::Point(p),
            });
        }
        handles.push(LHandle {
            layer: i,
            handle: H_WIN_C,
            shape: HShape::Point(centre),
        });
    }

    // --- Paint mode: the pointer is a brush, not a grab ----------------------

    let sheet_for_paint = egui::Rect::from_min_size(
        egui::pos2(
            plot.left() - (u0 / circ) as f32 * full_w,
            plot.top() - (v0 / band) as f32 * full_h,
        ),
        egui::vec2(full_w, full_h),
    );
    if app.band_paint {
        paint_interaction(app, ui, &response, sheet_for_paint, &ctx);
    }

    // --- Interaction ---------------------------------------------------------

    let knob_c = egui::pos2(plot.right() - 34.0, plot.bottom() - 34.0);
    let (mut v_lo, mut v_hi) = tile.as_ref().map_or((0.0, 0.0), |t| t.v_bounds());
    let grab_id = ui.id().with("unrolled_grab");
    let scroll_id = ui.id().with("unrolled_scroll");

    if response.drag_started() && !app.band_paint {
        let p = response.interact_pointer_pos().unwrap_or(plot.center());
        // Nearest layer handle in reach; a knob or band-edge grab on the
        // selected tiling still wins over a stray line underneath it.
        let nearest = handles
            .iter()
            .map(|h| (h, h.shape.dist(p)))
            .filter(|(_, d)| *d <= HANDLE_GRAB_PX + 2.0)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let grab = if tile.is_some() && (p - knob_c).length() <= KNOB_R + 5.0 {
            Grab::Rotate
        } else if tile.is_some() && (p.y - y_of_v(v_lo)).abs() <= HANDLE_GRAB_PX {
            Grab::VLow
        } else if tile.is_some() && (p.y - y_of_v(v_hi)).abs() <= HANDLE_GRAB_PX {
            Grab::VHigh
        } else if let Some((h, _)) = nearest {
            app.selected_layer = Some(h.layer);
            Grab::Other {
                layer: h.layer,
                handle: h.handle,
            }
        } else if tile.is_some() {
            Grab::Lattice
        } else {
            Grab::None
        };
        ui.memory_mut(|m| m.data.insert_temp(grab_id, grab));
    }
    if response.drag_stopped() {
        ui.memory_mut(|m| m.data.insert_temp(grab_id, Grab::None));
    }

    if let Some(t) = &mut tile {
        let grab = ui
            .memory(|m| m.data.get_temp::<Grab>(grab_id))
            .unwrap_or_default();
        let pointer = ui.input(|i| i.pointer.interact_pos());

        // Band-edge handles snap onto the side-face boundaries, so a drag
        // lands the tiling exactly on the castable runs.
        let snap_targets: Vec<f64> = ctx
            .side_faces_std()
            .map(|sf| {
                sf.low
                    .into_iter()
                    .chain(sf.high)
                    .flat_map(|(a, b)| [a, b])
                    .collect()
            })
            .unwrap_or_default();
        let snap = |v: f64| {
            snap_targets
                .iter()
                .copied()
                .filter(|s| (s - v).abs() <= SNAP_MM)
                .min_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs()))
                .unwrap_or(v)
        };

        if response.dragged() {
            match grab {
                Grab::Lattice => {
                    let d = response.drag_delta();
                    let (cw, ch) = t.cell_size(&ctx);
                    if d.x != 0.0 && cw > 1e-9 {
                        t.offset_u = (t.offset_u + d.x as f64 * mm_per_px_u / cw).rem_euclid(1.0);
                        changed = true;
                    }
                    if d.y != 0.0 && ch > 1e-9 {
                        t.offset_v = (t.offset_v + d.y as f64 * mm_per_px_v / ch).rem_euclid(1.0);
                        changed = true;
                    }
                }
                Grab::VLow => {
                    if let Some(p) = pointer {
                        v_lo = snap(v_of_y(p.y)).clamp(-0.25 * ctx.band_v_len_mm, v_hi - 0.2);
                        set_band(t, v_lo, v_hi);
                        changed = true;
                    }
                }
                Grab::VHigh => {
                    if let Some(p) = pointer {
                        v_hi = snap(v_of_y(p.y)).clamp(v_lo + 0.2, 1.25 * ctx.band_v_len_mm);
                        set_band(t, v_lo, v_hi);
                        changed = true;
                    }
                }
                Grab::Rotate => {
                    if let Some(p) = pointer {
                        let d = p - knob_c;
                        if d.length() > 3.0 {
                            t.rotation_deg = wrap_deg((d.y.atan2(d.x) as f64).to_degrees());
                            changed = true;
                        }
                    }
                }
                Grab::None | Grab::Other { .. } => {}
            }
        }

        if response.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            if (zoom - 1.0).abs() > 1e-4 {
                t.rotation_deg = wrap_deg(t.rotation_deg + (zoom - 1.0) as f64 * 240.0);
                changed = true;
            } else if scroll != 0.0 {
                let mut acc = ui
                    .memory(|m| m.data.get_temp::<f32>(scroll_id))
                    .unwrap_or(0.0)
                    + scroll;
                let steps = (acc / SCROLL_STEP).trunc() as i64;
                if steps != 0 {
                    acc -= steps as f32 * SCROLL_STEP;
                    t.repeats_around =
                        (t.repeats_around as i64 + steps).clamp(1, MAX_REPEATS) as u32;
                    changed = true;
                }
                ui.memory_mut(|m| m.data.insert_temp(scroll_id, acc));
            }
        }
    }

    // --- Dragging a non-tiling layer's handle --------------------------------

    {
        let grab = ui
            .memory(|m| m.data.get_temp::<Grab>(grab_id))
            .unwrap_or_default();
        let pointer = ui.input(|i| i.pointer.interact_pos());
        if let (Grab::Other { layer, handle }, true, Some(p)) = (grab, response.dragged(), pointer)
        {
            let snap_targets: Vec<f64> = ctx
                .side_faces_std()
                .map(|sf| {
                    sf.low
                        .into_iter()
                        .chain(sf.high)
                        .flat_map(|(a, b)| [a, b])
                        .collect()
                })
                .unwrap_or_default();
            let snap = |v: f64| {
                snap_targets
                    .iter()
                    .copied()
                    .filter(|s| (s - v).abs() <= SNAP_MM)
                    .min_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs()))
                    .unwrap_or(v)
            };
            let v_at = |y: f32| snap(v_of_y(y)).clamp(0.0, ctx.band_v_len_mm);
            let theta_at = |x: f32| {
                let u = (x - plot.left()) as f64 / full_w.max(1.0) as f64 * circ + u0;
                ctx.theta_of_u(u.rem_euclid(circ))
            };
            let mut moved = true;
            if let Some(e) = app.design.layers.layers.get_mut(layer) {
                if handle >= H_WIN_C {
                    let theta = theta_at(p.x);
                    let w = &mut e.window;
                    match handle {
                        H_WIN_C => w.theta_deg = theta,
                        H_WIN_A | H_WIN_B => {
                            // The dragged edge lands at the pointer; the far
                            // edge stays put.
                            let half = w.span_deg.max(0.0) * 0.5;
                            let (far, dir) = if handle == H_WIN_A {
                                (w.theta_deg + half, -1.0)
                            } else {
                                (w.theta_deg - half, 1.0)
                            };
                            let span = ringdesign_core::field::wrap_delta(far - theta, 360.0)
                                .abs()
                                .clamp(2.0, 358.0);
                            w.theta_deg = (far + dir * span * 0.5).rem_euclid(360.0);
                            w.span_deg = span;
                        }
                        H_WIN_FA | H_WIN_FB => {
                            let half = w.span_deg.max(0.0) * 0.5;
                            let edge = if handle == H_WIN_FA {
                                w.theta_deg - half
                            } else {
                                w.theta_deg + half
                            };
                            w.fade_deg = ringdesign_core::field::wrap_delta(theta - edge, 360.0)
                                .abs()
                                .clamp(0.0, 60.0);
                        }
                        _ => moved = false,
                    }
                } else {
                    match (&mut e.layer, handle) {
                        (Layer::Border(b), 0) => b.v_mm = v_at(p.y),
                        (Layer::Milgrain(m), 0) => m.v_mm = v_at(p.y),
                        (Layer::SeatRun(r), 0) => r.seat.v_mm = v_at(p.y),
                        (Layer::SeatPad(s), 0) => {
                            s.theta_deg = theta_at(p.x);
                            s.v_mm = v_at(p.y);
                        }
                        (Layer::SeatPad(s), H_RADIUS) => {
                            let c = egui::pos2(
                                x_of_u(ctx.u_of_theta(s.theta_deg.rem_euclid(360.0))),
                                y_of_v(s.v_mm),
                            );
                            let d = p - c;
                            // Metal-true: chart offsets scaled to metal mm.
                            let (ku, kv) =
                                if s.metal_true { s.station_scale(&ctx) } else { (1.0, 1.0) };
                            let mm = ((d.x as f64 * mm_per_px_u * ku).powi(2)
                                + (d.y as f64 * mm_per_px_v * kv).powi(2))
                            .sqrt();
                            s.diameter_mm = (mm * 2.0).clamp(0.5, 20.0);
                        }
                        (Layer::Signet(sg), 0) => {
                            sg.theta_deg = theta_at(p.x);
                            sg.v_mm = v_at(p.y);
                        }
                        (Layer::Decals(dl), h) if h >= H_DECAL => {
                            if let Some(dec) = dl.decals.get_mut((h - H_DECAL) as usize) {
                                dec.theta_deg = theta_at(p.x);
                                dec.v_mm = v_at(p.y);
                            } else {
                                moved = false;
                            }
                        }
                        _ => moved = false,
                    }
                }
            } else {
                moved = false;
            }
            if moved {
                app.mark_dirty();
            }
        }
    }

    // Write the edited clone back into the stack.
    if changed && let (Some(i), Some(t)) = (selected, tile.as_ref()) {
        if let Some(Layer::Tiling(dst)) = app.design.layers.layers.get_mut(i).map(|e| &mut e.layer)
        {
            *dst = t.clone();
        }
        app.mark_dirty();
    }

    // --- Composited height field --------------------------------------------

    // Half resolution while dragging.
    let quality = if response.dragged() { 2 } else { 1 };
    let tex_w = (plot.width() as usize / 16 * 16).clamp(128, 512) / quality;
    let tex_h = (plot.height() as usize / 8 * 8).clamp(48, 192) / quality;
    let cache_id = ui.id().with("unrolled_field");
    let key = field_key(app, &ctx, tex_w, tex_h);
    let cached = ui.memory(|m| m.data.get_temp::<FieldCache>(cache_id));
    let cache = match cached {
        Some(c) if c.key == key => c,
        _ => {
            let (image, max_mm) = field_image(&app.design.layers, &ctx, &app.lib, tex_w, tex_h);
            let tex = ui
                .ctx()
                .load_texture("unrolled-field", image, egui::TextureOptions::LINEAR);
            let fresh = FieldCache { key, tex, max_mm };
            ui.memory_mut(|m| m.data.insert_temp(cache_id, fresh.clone()));
            fresh
        }
    };

    let uv_full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    // The cache covers the whole band; the view shows a window of it. The
    // texture is drawn as the full virtual sheet (the painter clips), twice
    // when the window straddles the ring joint.
    let sheet = egui::Rect::from_min_size(
        egui::pos2(
            plot.left() - (u0 / circ) as f32 * full_w,
            plot.top() - (v0 / band) as f32 * full_h,
        ),
        egui::vec2(full_w, full_h),
    );
    painter.image(cache.tex.id(), sheet, uv_full, egui::Color32::WHITE);
    if view.zoom > 1.001 {
        painter.image(
            cache.tex.id(),
            sheet.translate(egui::vec2(full_w, 0.0)),
            uv_full,
            egui::Color32::WHITE,
        );
    }

    // --- Castability zones ---------------------------------------------------

    let (crest_lo, crest_hi) = crest_span(&app.design.reference_loop(), ctx.crest_v_mm);
    let crest_rect = egui::Rect::from_min_max(
        egui::pos2(plot.left(), y_of_v(crest_lo)),
        egui::pos2(plot.right(), y_of_v(crest_hi)),
    );
    let side_lo =
        egui::Rect::from_min_max(plot.left_top(), egui::pos2(plot.right(), y_of_v(crest_lo)));
    let side_hi = egui::Rect::from_min_max(
        egui::pos2(plot.left(), y_of_v(crest_hi)),
        plot.right_bottom(),
    );

    painter.rect_filled(side_lo, 0.0, theme::GOOD.gamma_multiply(0.07));
    painter.rect_filled(side_hi, 0.0, theme::GOOD.gamma_multiply(0.07));
    painter.rect_filled(crest_rect, 0.0, theme::INFO.gamma_multiply(0.10));

    let zone_stroke = egui::Stroke::new(1.0, theme::GRID);
    for y in [y_of_v(crest_lo), y_of_v(crest_hi)] {
        painter.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            zone_stroke,
        );
    }

    let small = egui::FontId::proportional(10.0);
    let zone_label = |v: f64, text: &str| {
        let y = y_of_v(v).clamp(plot.top() + 7.0, plot.bottom() - 7.0);
        // Indented clear of the "seam" label pinned to the top-left corner.
        painter.text(
            egui::pos2(plot.left() + 46.0, y),
            egui::Align2::LEFT_CENTER,
            text,
            small.clone(),
            theme::TEXT_DIM,
        );
    };
    zone_label(crest_lo * 0.5, "side face - free");
    zone_label((crest_hi + ctx.band_v_len_mm) * 0.5, "side face - free");
    zone_label(ctx.crest_v_mm, "crest - needs draft");

    // --- Ring angle ruler ----------------------------------------------------

    let tick_stroke = egui::Stroke::new(1.0, theme::GRID);
    for deg in (0..360).step_by(30) {
        let x = x_of_u(ctx.u_of_theta(deg as f64));
        let top = ringdesign_core::profile::TOP_DEG as i32 == deg;
        let color = if top { theme::ACCENT } else { theme::TEXT_DIM };
        painter.line_segment(
            [egui::pos2(x, plot.top() - 5.0), egui::pos2(x, plot.top())],
            tick_stroke,
        );
        if deg > 0 {
            painter.line_segment(
                [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
                egui::Stroke::new(1.0, theme::GRID.gamma_multiply(0.55)),
            );
        }
        let label = if top {
            format!("{deg} top")
        } else {
            deg.to_string()
        };
        painter.text(
            egui::pos2(
                x.clamp(rect.left() + 12.0, rect.right() - 12.0),
                plot.top() - 7.0,
            ),
            egui::Align2::CENTER_BOTTOM,
            label,
            small.clone(),
            color,
        );
    }

    // --- Tile lattice --------------------------------------------------------

    let mut cell_count = 0usize;
    let mut textured = 0usize;
    let mut capped = false;
    let mut alpha_missing = false;
    if let Some(t) = &tile {
        let thumb = app.thumbnail(ui.ctx(), &t.alpha);
        alpha_missing = thumb.is_none();
        let cells = t.cells(&ctx);
        cell_count = cells.len();

        let outline = egui::Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.30));
        let mut mesh = egui::Mesh::with_texture(thumb.unwrap_or_default());
        let tint = egui::Color32::from_white_alpha(120);

        for cell in cells.iter().take(MAX_OUTLINED) {
            let uvs = corner_uvs(cell.rot_deg, cell.mirror_u, cell.mirror_v);
            // A cell straddling the seam is drawn again on the far side.
            let mut shifts = vec![0.0f64];
            if cell.u0 < 0.0 {
                shifts.push(ctx.circumference_mm);
            }
            if cell.u1 > ctx.circumference_mm {
                shifts.push(-ctx.circumference_mm);
            }
            for shift in shifts {
                let r = egui::Rect::from_min_max(
                    egui::pos2(x_of_u(cell.u0 + shift), y_of_v(cell.v0)),
                    egui::pos2(x_of_u(cell.u1 + shift), y_of_v(cell.v1)),
                );
                if !r.intersects(plot) || r.width() < 0.5 || r.height() < 0.5 {
                    continue;
                }
                if thumb.is_some() {
                    if textured < MAX_TEXTURED {
                        push_quad(&mut mesh, r, uvs, tint);
                        textured += 1;
                    } else {
                        capped = true;
                    }
                }
                painter.rect_stroke(r, 0.0, outline, egui::StrokeKind::Inside);
            }
        }
        if !mesh.is_empty() {
            painter.add(egui::Shape::mesh(mesh));
        }
    }

    // --- Seam ----------------------------------------------------------------

    let seam_stroke = egui::Stroke::new(1.5, theme::ACCENT);
    for x in [plot.left(), plot.right()] {
        painter.extend(egui::Shape::dashed_line(
            &[egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
            seam_stroke,
            6.0,
            4.0,
        ));
    }
    painter.text(
        egui::pos2(plot.left() + 4.0, plot.top() + 3.0),
        egui::Align2::LEFT_TOP,
        "seam",
        small.clone(),
        theme::ACCENT,
    );
    painter.text(
        egui::pos2(plot.right() - 4.0, plot.top() + 3.0),
        egui::Align2::RIGHT_TOP,
        "seam",
        small.clone(),
        theme::ACCENT,
    );

    // --- Painted strokes and the brush cursor --------------------------------

    if app.band_paint {
        if let Some(d) = app
            .design
            .drawn
            .iter()
            .find(|d| d.name == paint::BAND_ALPHA)
        {
            draw_strokes(&painter, sheet_for_paint, d);
            if view.zoom > 1.001 {
                draw_strokes(
                    &painter,
                    sheet_for_paint.translate(egui::vec2(full_w, 0.0)),
                    d,
                );
            }
        }
        if let Some(p) = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| plot.contains(*p))
        {
            let v_mm = v_of_y(p.y).clamp(0.0, ctx.band_v_len_mm);
            let ceiling = paint::ceiling_mm(&ctx, v_mm);
            let r_px = app.brush_frac * plot.width();
            painter.circle_stroke(
                p,
                r_px.clamp(3.0, 200.0),
                egui::Stroke::new(
                    1.5,
                    if app.brush_erase {
                        theme::WARN
                    } else {
                        theme::ACCENT
                    },
                ),
            );
            let asked = paint::wanted_mm(1.0, app.brush_depth);
            painter.text(
                egui::pos2(p.x + r_px.clamp(3.0, 200.0) + 6.0, p.y),
                egui::Align2::LEFT_CENTER,
                if asked > ceiling + 1e-9 {
                    format!("{ceiling:.2} mm max here")
                } else {
                    format!("{asked:.2} mm")
                },
                egui::FontId::proportional(10.0),
                if asked > ceiling + 1e-9 {
                    theme::WARN
                } else {
                    theme::TEXT_DIM
                },
            );
        }
    }

    // --- Handles on the other layer kinds ------------------------------------
    //
    // Read fresh from the stack, so a handle tracks its layer within the
    // frame that dragged it.
    for (i, entry) in app.design.layers.layers.iter().enumerate() {
        if app.band_paint {
            break;
        }
        if !entry.enabled {
            continue;
        }
        let strong = Some(i) == app.selected_layer;
        let col = if strong {
            theme::ACCENT
        } else {
            theme::TEXT_DIM.gamma_multiply(0.75)
        };
        let stroke = egui::Stroke::new(if strong { 1.6 } else { 1.0 }, col);
        let center = |theta: f64, v: f64| {
            egui::pos2(x_of_u(ctx.u_of_theta(theta.rem_euclid(360.0))), y_of_v(v))
        };
        let vline = |painter: &egui::Painter, y: f32, label: &str| {
            painter.extend(egui::Shape::dashed_line(
                &[egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
                stroke,
                5.0,
                5.0,
            ));
            if strong {
                painter.text(
                    egui::pos2(plot.right() - 6.0, y - 3.0),
                    egui::Align2::RIGHT_BOTTOM,
                    label,
                    egui::FontId::proportional(10.0),
                    col,
                );
            }
        };
        let cross = |painter: &egui::Painter, c: egui::Pos2| {
            for (a, b) in [
                (egui::vec2(-6.0, 0.0), egui::vec2(6.0, 0.0)),
                (egui::vec2(0.0, -6.0), egui::vec2(0.0, 6.0)),
            ] {
                painter.line_segment([c + a, c + b], stroke);
            }
        };
        match &entry.layer {
            Layer::Border(b) => vline(&painter, y_of_v(b.v_mm), "border"),
            Layer::Milgrain(m) => vline(&painter, y_of_v(m.v_mm), "milgrain"),
            Layer::SeatRun(r) => vline(&painter, y_of_v(r.seat.v_mm), "seat run"),
            Layer::SeatPad(sp) => {
                let c = center(sp.theta_deg, sp.v_mm);
                cross(&painter, c);
                // Metal-true: the plan maps into the chart by the station's scale.
                let (ku, kv) = if sp.metal_true { sp.station_scale(&ctx) } else { (1.0, 1.0) };
                // The pad's own plan, not a circle: the outline drawn here is
                // the metal the seat actually raises.
                let pts: Vec<egui::Pos2> = (0..=64)
                    .map(|k| {
                        let t = k as f64 / 64.0 * std::f64::consts::TAU;
                        let (sn, cs) = t.sin_cos();
                        let r = ringdesign_core::field::superellipse_radius_mm(
                            cs,
                            sn,
                            sp.diameter_mm * 0.5 * sp.elong.max(1.0),
                            sp.diameter_mm * 0.5,
                            sp.plan_pow,
                        );
                        let (a, b) = (r * cs, r * sn);
                        let (s2, c2) = sp.rot_deg.to_radians().sin_cos();
                        let (uo, vo) = ((a * c2 - b * s2) / ku.max(1e-6), (a * s2 + b * c2) / kv.max(1e-6));
                        egui::pos2(
                            c.x + (uo / mm_per_px_u.max(1e-9)) as f32,
                            c.y + (vo / mm_per_px_v.max(1e-9)) as f32,
                        )
                    })
                    .collect();
                painter.add(egui::Shape::closed_line(pts, stroke));
            }
            Layer::Signet(sg) => cross(&painter, center(sg.theta_deg, sg.v_mm)),
            Layer::Decals(d) => {
                for dec in d.decals.iter().take(120) {
                    cross(&painter, center(dec.theta_deg, dec.v_mm));
                }
            }
            _ => {}
        }
    }

    // --- Band edge handles and the rotation knob -----------------------------

    if let Some(t) = &tile {
        let edge = egui::Stroke::new(1.4, theme::ACCENT.gamma_multiply(0.85));
        for y in [y_of_v(v_lo), y_of_v(v_hi)] {
            painter.line_segment(
                [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
                edge,
            );
            painter.rect_filled(
                egui::Rect::from_center_size(
                    egui::pos2(plot.left() + 42.0, y),
                    egui::vec2(26.0, 6.0),
                ),
                3.0,
                theme::ACCENT,
            );
        }
        draw_knob(&painter, knob_c, t.rotation_deg);
    }

    // --- Readout -------------------------------------------------------------

    match &tile {
        Some(t) => {
            let (cw, ch) = t.cell_size(&ctx);
            let mut lines = vec![
                (
                    format!("{} around • {} rows", t.repeats_around, t.rows.max(1)),
                    theme::TEXT,
                ),
                (format!("cell {cw:.2} x {ch:.2} mm"), theme::TEXT_DIM),
                (
                    format!(
                        "rotation {:.0} deg • relief {:.2} mm",
                        t.rotation_deg, t.height_mm
                    ),
                    theme::TEXT_DIM,
                ),
                (
                    format!("band {:.2} to {:.2} mm across", v_lo, v_hi),
                    theme::TEXT_DIM,
                ),
                (
                    format!("peak stack height {:.2} mm", cache.max_mm),
                    theme::TEXT_DIM,
                ),
            ];
            if alpha_missing {
                lines.push((format!("alpha \"{}\" is not loaded", t.alpha), theme::BAD));
            } else if cell_count > MAX_OUTLINED {
                lines.push((
                    format!("{cell_count} tiles • only the first {MAX_OUTLINED} are drawn"),
                    theme::WARN,
                ));
            } else if capped {
                lines.push((
                    format!("{cell_count} tiles • art on the first {MAX_TEXTURED}, outlines after"),
                    theme::WARN,
                ));
            }
            readout(&painter, plot, &lines);
        }
        None => {
            // Backed so it stays readable over the height field and zone labels.
            let galley = painter.layout_no_wrap(
                format!("{} Select a tiling layer to lay it out here", icon::STACK),
                egui::FontId::proportional(13.0),
                theme::TEXT_DIM,
            );
            let box_rect = egui::Align2::CENTER_CENTER
                .anchor_size(plot.center(), galley.size())
                .expand2(egui::vec2(10.0, 6.0));
            painter.rect_filled(box_rect, 4.0, theme::PANEL.gamma_multiply(0.82));
            painter.galley(
                box_rect.shrink2(egui::vec2(10.0, 6.0)).min,
                galley,
                theme::TEXT_DIM,
            );
        }
    }

    let hint = if app.band_paint {
        "Drag to paint metal • depth is capped by the local draft • strokes land on the \"band\" layer"
    } else if tile.is_some() {
        "Drag to shift the lattice • Scroll for repeats • Ctrl-scroll or the knob to rotate • Drag the band edges to resize"
    } else if !handles.is_empty() {
        "Drag a line, cross or rim to move that layer • grabbing one selects it"
    } else {
        "Add a layer in the Layers panel, then drag it into place here"
    };
    paint_bar(app, ui, rect);

    painter.text(
        egui::pos2(rect.left() + PAD_L, rect.bottom() - 4.0),
        egui::Align2::LEFT_BOTTOM,
        hint,
        egui::FontId::proportional(11.0),
        theme::TEXT_DIM,
    );
}

// --- Height field ----------------------------------------------------------

/// Cheap hash of everything the rendered field depends on.
fn field_key(app: &RingDesignerApp, ctx: &FieldContext, w: usize, h: usize) -> u64 {
    let mut hasher = DefaultHasher::new();
    serde_json::to_string(&app.design.layers)
        .unwrap_or_default()
        .hash(&mut hasher);
    for x in [ctx.circumference_mm, ctx.band_v_len_mm, ctx.crest_v_mm] {
        x.to_bits().hash(&mut hasher);
    }
    app.lib.len().hash(&mut hasher);
    for a in app.lib.iter() {
        a.name.hash(&mut hasher);
        a.width.hash(&mut hasher);
        a.height.hash(&mut hasher);
    }
    // A re-baked drawing keeps its name and size; the stroke tally is what
    // changes, and without it the field texture shows the previous stroke.
    for d in &app.design.drawn {
        d.name.hash(&mut hasher);
        d.strokes.len().hash(&mut hasher);
        d.strokes
            .iter()
            .map(|s| s.points.len())
            .sum::<usize>()
            .hash(&mut hasher);
    }
    w.hash(&mut hasher);
    h.hash(&mut hasher);
    hasher.finish()
}

/// Sample the whole stack over the unrolled band. Returns the image and the
/// largest absolute displacement found, in mm.
fn field_image(
    layers: &LayerStack,
    ctx: &FieldContext,
    lib: &AlphaLibrary,
    w: usize,
    h: usize,
) -> (egui::ColorImage, f64) {
    let mut heights = vec![0.0f64; w * h];
    let mut max_mm = 0.0f64;
    for j in 0..h {
        let v = (j as f64 + 0.5) / h as f64 * ctx.band_v_len_mm;
        for i in 0..w {
            let u = (i as f64 + 0.5) / w as f64 * ctx.circumference_mm;
            let x = layers.height(Uv { u, v }, ctx, lib);
            let x = if x.is_finite() { x } else { 0.0 };
            heights[j * w + i] = x;
            max_mm = max_mm.max(x.abs());
        }
    }
    let inv = if max_mm > 1e-9 { 1.0 / max_mm } else { 0.0 };
    let pixels = heights.iter().map(|&x| ramp(x * inv)).collect();
    (egui::ColorImage::new([w, h], pixels), max_mm)
}

/// Heat ramp over -1..1: cool where the stack carves, warm where it stands proud.
fn ramp(t: f64) -> egui::Color32 {
    const STOPS: [[f32; 3]; 5] = [
        [0.30, 0.44, 0.64],
        [0.14, 0.17, 0.23],
        [0.09, 0.10, 0.13],
        [0.55, 0.40, 0.22],
        [0.96, 0.87, 0.70],
    ];
    let p = ((t.clamp(-1.0, 1.0) + 1.0) * 2.0) as f32;
    let i = (p.floor() as usize).min(STOPS.len() - 2);
    let f = (p - i as f32).clamp(0.0, 1.0);
    let (a, b) = (STOPS[i], STOPS[i + 1]);
    let c = |k: usize| ((a[k] + (b[k] - a[k]) * f) * 255.0) as u8;
    egui::Color32::from_rgb(c(0), c(1), c(2))
}

// --- Zones -----------------------------------------------------------------

/// `v` span where the base surface stands nearer vertical than horizontal, so
/// relief there eats into the draft a +/-Z pull needs.
fn crest_span(loop_: &ProfileLoop, crest_v_mm: f64) -> (f64, f64) {
    let mut lo = crest_v_mm;
    let mut hi = crest_v_mm;
    for p in loop_.pts.iter().filter(|p| p.surface) {
        if p.nz.abs() < p.nr.abs() {
            lo = lo.min(p.v_mm);
            hi = hi.max(p.v_mm);
        }
    }
    (lo, hi)
}

// --- Tile drawing ----------------------------------------------------------

/// Cell-corner texture coordinates, clockwise from the top left, after the
/// cell's mirroring and rotation.
fn corner_uvs(rot_deg: f64, mirror_u: bool, mirror_v: bool) -> [egui::Pos2; 4] {
    let (s, c) = (-rot_deg.to_radians()).sin_cos();
    let mut out = [egui::Pos2::ZERO; 4];
    for (k, corner) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
        .iter()
        .enumerate()
    {
        let x = if mirror_u { 1.0 - corner.0 } else { corner.0 } - 0.5;
        let y = if mirror_v { 1.0 - corner.1 } else { corner.1 } - 0.5;
        out[k] = egui::pos2((x * c - y * s + 0.5) as f32, (x * s + y * c + 0.5) as f32);
    }
    out
}

fn push_quad(mesh: &mut egui::Mesh, r: egui::Rect, uvs: [egui::Pos2; 4], tint: egui::Color32) {
    let base = mesh.vertices.len() as u32;
    let corners = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
    ];
    for (pos, uv) in corners.into_iter().zip(uvs) {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv,
            color: tint,
        });
    }
    mesh.add_triangle(base, base + 1, base + 2);
    mesh.add_triangle(base, base + 2, base + 3);
}

/// Degrees folded into -180..=180.
fn wrap_deg(deg: f64) -> f64 {
    (deg + 180.0).rem_euclid(360.0) - 180.0
}

fn set_band(t: &mut TilingLayer, lo: f64, hi: f64) {
    t.v_center_mm = (lo + hi) * 0.5;
    t.v_span_mm = (hi - lo).max(0.2);
}

fn draw_knob(painter: &egui::Painter, c: egui::Pos2, rot_deg: f64) {
    painter.circle_filled(c, KNOB_R, theme::PANEL.gamma_multiply(0.90));
    painter.circle_stroke(c, KNOB_R, egui::Stroke::new(1.0, theme::ACCENT_DIM));
    let (s, cs) = rot_deg.to_radians().sin_cos();
    let tip = c + egui::vec2(cs as f32, s as f32) * (KNOB_R - 3.0);
    painter.line_segment([c, tip], egui::Stroke::new(2.0, theme::ACCENT));
    painter.circle_filled(c, 2.0, theme::ACCENT);
    painter.text(
        egui::pos2(c.x, c.y + KNOB_R + 3.0),
        egui::Align2::CENTER_TOP,
        "rotate",
        egui::FontId::proportional(10.0),
        theme::TEXT_DIM,
    );
}

/// Floating stats panel in the bottom-left of the canvas.
fn readout(painter: &egui::Painter, plot: egui::Rect, lines: &[(String, egui::Color32)]) {
    if lines.is_empty() {
        return;
    }
    let font = egui::FontId::proportional(11.0);
    let galleys: Vec<_> = lines
        .iter()
        .map(|(text, color)| painter.layout_no_wrap(text.clone(), font.clone(), *color))
        .collect();

    let line_h = 15.0f32;
    let pad = egui::vec2(9.0, 7.0);
    let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let size = egui::vec2(width, line_h * lines.len() as f32) + pad * 2.0;
    let panel = egui::Rect::from_min_size(
        egui::pos2(plot.left() + 8.0, plot.bottom() - 8.0 - size.y),
        size,
    );

    painter.rect_filled(panel, 5.0, theme::PANEL.gamma_multiply(0.90));
    painter.rect_stroke(
        panel,
        5.0,
        egui::Stroke::new(1.0, theme::GRID),
        egui::StrokeKind::Inside,
    );
    for (i, (galley, (_, color))) in galleys.into_iter().zip(lines).enumerate() {
        let y = panel.top() + pad.y + i as f32 * line_h;
        painter.galley(egui::pos2(panel.left() + pad.x, y), galley, *color);
    }
}

// --- Paint-on-band -----------------------------------------------------------

/// The brush: strokes into the design's "band" drawing, depth capped by the
/// local draft exactly like the Android pen. Desktop pointers rarely report
/// force, so a press is a full press and the depth slider is the hand.
fn paint_interaction(
    app: &mut RingDesignerApp,
    ui: &mut egui::Ui,
    response: &egui::Response,
    plot: egui::Rect,
    ctx: &FieldContext,
) {
    if response.drag_started() {
        let idx = paint::ensure_band_layer(&mut app.design);
        app.design.drawn[idx].strokes.push(Stroke::new(
            app.brush_frac,
            app.brush_soft,
            app.brush_erase,
        ));
    }
    if response.dragged() {
        let (Some(p), Some(d)) = (
            response.interact_pointer_pos(),
            app.design
                .drawn
                .iter_mut()
                .find(|d| d.name == paint::BAND_ALPHA),
        ) else {
            return;
        };
        let force = ui.input(|i| {
            i.events.iter().rev().find_map(|e| match e {
                egui::Event::Touch { force, .. } => *force,
                _ => None,
            })
        });
        let pressure = force.unwrap_or(1.0);
        let nx = ((p.x - plot.left()) / plot.width().max(1.0)).rem_euclid(1.0);
        let ny = ((p.y - plot.top()) / plot.height().max(1.0)).clamp(0.0, 1.0);
        let v_mm = ny as f64 * ctx.band_v_len_mm;
        let b = paint::bite(ctx, v_mm, pressure, app.brush_depth);
        if let Some(s) = d.strokes.last_mut() {
            s.push(nx, ny, b.alpha_value());
        }
    }
    if response.drag_stopped() {
        let Some(pos) = app
            .design
            .drawn
            .iter()
            .position(|d| d.name == paint::BAND_ALPHA)
        else {
            return;
        };
        if app.design.drawn[pos]
            .strokes
            .last()
            .is_some_and(|s| s.is_empty())
        {
            app.design.drawn[pos].strokes.pop();
            return;
        }
        bake_band(app);
    }
}

/// Re-bake the band drawing into the shared library and refresh what shows it.
/// On stroke end, not per sample: `Arc::make_mut` deep-copies the library.
fn bake_band(app: &mut RingDesignerApp) {
    let Some(d) = app
        .design
        .drawn
        .iter()
        .find(|d| d.name == paint::BAND_ALPHA)
    else {
        return;
    };
    let baked = d.rasterize();
    std::sync::Arc::make_mut(&mut app.lib).insert(baked);
    app.forget_thumbnail(paint::BAND_ALPHA);
    app.mark_dirty();
}

/// Vector overlay of the strokes: the crisp version of what the coarse field
/// texture shows underneath.
fn draw_strokes(painter: &egui::Painter, plot: egui::Rect, d: &ringdesign_core::drawn::DrawnAlpha) {
    for s in &d.strokes {
        if s.points.len() < 2 {
            continue;
        }
        let w = (s.radius * plot.width() * 2.0).clamp(1.0, 200.0);
        let color = if s.erase {
            theme::WARN.gamma_multiply(0.5)
        } else {
            theme::ACCENT.gamma_multiply(0.55)
        };
        let pts: Vec<egui::Pos2> = s
            .points
            .iter()
            .map(|p| {
                egui::pos2(
                    plot.left() + p[0] * plot.width(),
                    plot.top() + p[1] * plot.height(),
                )
            })
            .collect();
        painter.add(egui::Shape::line(pts, egui::Stroke::new(w, color)));
    }
}

/// The floating brush bar: mode toggle always, controls while painting.
fn paint_bar(app: &mut RingDesignerApp, ui: &mut egui::Ui, rect: egui::Rect) {
    egui::Area::new(ui.id().with("band_paint_bar"))
        .fixed_pos(rect.left_top() + egui::vec2(8.0, 24.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::NONE
                .fill(theme::PANEL.gamma_multiply(0.92))
                .corner_radius(5.0)
                .inner_margin(egui::Margin::symmetric(7, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let label = if app.band_paint {
                            format!("{} Done", icon::CHECK)
                        } else {
                            format!("{} Paint", icon::PAINT_BRUSH)
                        };
                        if ui
                            .button(label)
                            .on_hover_text(
                                "Paint metal straight onto the band. Pressure-aware on a pen; the \
                                 depth slider is the hand on a mouse. Strokes travel in the design \
                                 and open on the phone as the same layer.",
                            )
                            .clicked()
                        {
                            app.band_paint = !app.band_paint;
                        }
                        if app.band_paint {
                            ui.label(egui::RichText::new("size").small().color(theme::TEXT_DIM));
                            ui.add(
                                egui::Slider::new(&mut app.brush_frac, 0.002..=0.06)
                                    .show_value(false),
                            );
                            ui.label(egui::RichText::new("depth").small().color(theme::TEXT_DIM));
                            ui.add(
                                egui::Slider::new(&mut app.brush_depth, 0.05..=1.0)
                                    .show_value(false),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "{:.2} mm",
                                    paint::wanted_mm(1.0, app.brush_depth)
                                ))
                                .small(),
                            );
                            ui.label(egui::RichText::new("soft").small().color(theme::TEXT_DIM));
                            ui.add(
                                egui::Slider::new(&mut app.brush_soft, 0.0..=1.0).show_value(false),
                            );
                            ui.toggle_value(&mut app.brush_erase, format!("{}", icon::ERASER))
                                .on_hover_text("Erase instead of adding");
                            let has_strokes = app
                                .design
                                .drawn
                                .iter()
                                .find(|d| d.name == paint::BAND_ALPHA)
                                .is_some_and(|d| !d.strokes.is_empty());
                            if ui
                                .add_enabled(
                                    has_strokes,
                                    egui::Button::new(format!("{}", icon::ARROW_COUNTER_CLOCKWISE)),
                                )
                                .on_hover_text("Undo the last stroke")
                                .clicked()
                            {
                                if let Some(d) = app
                                    .design
                                    .drawn
                                    .iter_mut()
                                    .find(|d| d.name == paint::BAND_ALPHA)
                                {
                                    d.strokes.pop();
                                }
                                bake_band(app);
                            }
                        }
                    });
                });
        });
}
