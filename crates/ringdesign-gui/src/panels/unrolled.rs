//! The unrolled band: a flat `u`/`v` canvas for laying tiles around the ring.
//!
//! Horizontal is arc distance around the ring, vertical is arc distance across
//! the cross-section. The composited height field is drawn underneath, so a
//! tile placed here is exactly where the metal lands.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use egui_phosphor::regular as icon;

use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::field::{FieldContext, Layer, LayerStack, Uv};
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

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Grab {
    #[default]
    None,
    Lattice,
    VLow,
    VHigh,
    Rotate,
}

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
    let x_of_u = |u: f64| plot.left() + (u / ctx.circumference_mm) as f32 * plot.width();
    let y_of_v = |v: f64| plot.top() + (v / ctx.band_v_len_mm) as f32 * plot.height();
    let v_of_y = |y: f32| (y - plot.top()) as f64 / plot.height().max(1.0) as f64 * ctx.band_v_len_mm;
    let mm_per_px_u = ctx.circumference_mm / plot.width().max(1.0) as f64;
    let mm_per_px_v = ctx.band_v_len_mm / plot.height().max(1.0) as f64;

    // --- The selected tiling layer, worked on as a clone ---------------------

    let selected = app
        .selected_layer
        .filter(|&i| i < app.design.layers.layers.len());
    let mut tile: Option<TilingLayer> = selected.and_then(|i| {
        match &app.design.layers.layers[i].layer {
            Layer::Tiling(t) => Some(t.clone()),
            _ => None,
        }
    });
    let mut changed = false;

    // --- Interaction ---------------------------------------------------------

    let knob_c = egui::pos2(plot.right() - 34.0, plot.bottom() - 34.0);
    let (mut v_lo, mut v_hi) = tile.as_ref().map_or((0.0, 0.0), |t| t.v_bounds());
    let grab_id = ui.id().with("unrolled_grab");
    let scroll_id = ui.id().with("unrolled_scroll");

    if response.drag_started() {
        let p = response.interact_pointer_pos().unwrap_or(plot.center());
        let grab = if tile.is_none() {
            Grab::None
        } else if (p - knob_c).length() <= KNOB_R + 5.0 {
            Grab::Rotate
        } else if (p.y - y_of_v(v_lo)).abs() <= HANDLE_GRAB_PX {
            Grab::VLow
        } else if (p.y - y_of_v(v_hi)).abs() <= HANDLE_GRAB_PX {
            Grab::VHigh
        } else {
            Grab::Lattice
        };
        ui.memory_mut(|m| m.data.insert_temp(grab_id, grab));
    }
    if response.drag_stopped() {
        ui.memory_mut(|m| m.data.insert_temp(grab_id, Grab::None));
    }

    if let Some(t) = &mut tile {
        let grab = ui.memory(|m| m.data.get_temp::<Grab>(grab_id)).unwrap_or_default();
        let pointer = ui.input(|i| i.pointer.interact_pos());

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
                        v_lo = v_of_y(p.y).clamp(-0.25 * ctx.band_v_len_mm, v_hi - 0.2);
                        set_band(t, v_lo, v_hi);
                        changed = true;
                    }
                }
                Grab::VHigh => {
                    if let Some(p) = pointer {
                        v_hi = v_of_y(p.y).clamp(v_lo + 0.2, 1.25 * ctx.band_v_len_mm);
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
                Grab::None => {}
            }
        }

        if response.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            if (zoom - 1.0).abs() > 1e-4 {
                t.rotation_deg = wrap_deg(t.rotation_deg + (zoom - 1.0) as f64 * 240.0);
                changed = true;
            } else if scroll != 0.0 {
                let mut acc =
                    ui.memory(|m| m.data.get_temp::<f32>(scroll_id)).unwrap_or(0.0) + scroll;
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

    // Write the edited clone back into the stack.
    if changed && let (Some(i), Some(t)) = (selected, tile.as_ref()) {
        if let Some(Layer::Tiling(dst)) = app.design.layers.layers.get_mut(i).map(|e| &mut e.layer) {
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
    painter.image(cache.tex.id(), plot, uv_full, egui::Color32::WHITE);

    // --- Castability zones ---------------------------------------------------

    let (crest_lo, crest_hi) = crest_span(&app.design.reference_loop(), ctx.crest_v_mm);
    let crest_rect = egui::Rect::from_min_max(
        egui::pos2(plot.left(), y_of_v(crest_lo)),
        egui::pos2(plot.right(), y_of_v(crest_hi)),
    );
    let side_lo = egui::Rect::from_min_max(plot.left_top(), egui::pos2(plot.right(), y_of_v(crest_lo)));
    let side_hi =
        egui::Rect::from_min_max(egui::pos2(plot.left(), y_of_v(crest_hi)), plot.right_bottom());

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
        let label = if top { format!("{deg} top") } else { deg.to_string() };
        painter.text(
            egui::pos2(x.clamp(rect.left() + 12.0, rect.right() - 12.0), plot.top() - 7.0),
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
            painter.galley(box_rect.shrink2(egui::vec2(10.0, 6.0)).min, galley, theme::TEXT_DIM);
        }
    }

    let hint = if tile.is_some() {
        "Drag to shift the lattice • Scroll for repeats • Ctrl-scroll or the knob to rotate • Drag the band edges to resize"
    } else {
        "Add a tiling layer in the Layers panel, then drag it into place here"
    };
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
    let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
    for (pos, uv) in corners.into_iter().zip(uvs) {
        mesh.vertices.push(egui::epaint::Vertex { pos, uv, color: tint });
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
