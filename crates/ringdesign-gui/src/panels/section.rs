//! Cross-section view: the slice at one ring angle, with per-segment draft.

use egui::{Align2, Color32, FontId, Pos2, RichText, Stroke, Vec2, pos2, vec2};
use egui_phosphor::regular as icon;
use ringdesign_core::castability::{FaceClass, Section, SectionPoint};

use crate::app::RingDesignerApp;
use crate::theme;

const CLASSES: [FaceClass; 4] = [
    FaceClass::Good,
    FaceClass::Marginal,
    FaceClass::Vertical,
    FaceClass::Undercut,
];

/// Ring angles worth a one-click jump, degrees.
const QUICK_ANGLES: [(&str, f64); 4] = [
    ("Top", 90.0),
    ("Side", 0.0),
    ("Bottom", 270.0),
    ("Shoulder", 45.0),
];

/// Canvas toggles, held in egui temp memory rather than on the app.
#[derive(Clone, Copy)]
struct ViewOpts {
    ticks: bool,
    fill: bool,
}

impl Default for ViewOpts {
    fn default() -> Self {
        Self {
            ticks: true,
            fill: true,
        }
    }
}

pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize) {
    let opts_id = egui::Id::new(("section_view_opts", pane));

    egui::Panel::top(egui::Id::new(("section_controls", pane)))
        .frame(strip_frame())
        .show(ui, |ui| controls(app, ui, pane, opts_id));

    egui::Panel::bottom(egui::Id::new(("section_readout", pane)))
        .frame(strip_frame())
        .show(ui, |ui| readout(app, ui, pane));

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::VIEWPORT_BG))
        .show(ui, |ui| canvas(app, ui, pane, opts_id));
}

fn strip_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(theme::PANEL)
        .inner_margin(egui::Margin::symmetric(10, 6))
}

// --- Controls --------------------------------------------------------------

/// Width below which the strip keeps only the angle slider. A split pane is
/// half the window, and the full row overruns itself there.
const WIDE_CONTROLS_MM: f32 = 720.0;
/// Width below which even the quick-angle buttons come off.
const MEDIUM_CONTROLS_MM: f32 = 430.0;

fn controls(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize, opts_id: egui::Id) {
    let width = ui.available_width();
    ui.horizontal(|ui| {
        let mut theta = app.panes[pane].section_theta_deg;
        let slider_w = if width < MEDIUM_CONTROLS_MM {
            width - 24.0
        } else {
            210.0
        };
        let slider = ui.add_sized(
            [slider_w.max(90.0), ui.spacing().interact_size.y],
            egui::Slider::new(&mut theta, 0.0..=360.0)
                .text(if width < MEDIUM_CONTROLS_MM {
                    ""
                } else {
                    "Ring angle"
                })
                .suffix("°")
                .fixed_decimals(1),
        );
        if slider.changed() {
            app.panes[pane].section_theta_deg = theta;
            app.refresh_section(pane);
        }

        if width >= MEDIUM_CONTROLS_MM {
            for (label, deg) in QUICK_ANGLES {
                let at = (theta - deg).abs() < 0.05;
                if ui.selectable_label(at, label).clicked() {
                    app.panes[pane].section_theta_deg = deg;
                    app.refresh_section(pane);
                }
            }
        }

        if width >= WIDE_CONTROLS_MM {
            ui.label(
                RichText::new(format!("{} 90° is the top of the ring", icon::INFO))
                    .small()
                    .color(theme::TEXT_DIM),
            );
        }

        if width < MEDIUM_CONTROLS_MM {
            return;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut opts = ui.memory_mut(|m| *m.data.get_temp_mut_or_default::<ViewOpts>(opts_id));
            let changed = ui.checkbox(&mut opts.fill, "Fill").changed()
                | ui.checkbox(&mut opts.ticks, "Draft ticks").changed();
            if changed {
                ui.memory_mut(|m| m.data.insert_temp(opts_id, opts));
            }

            if width < WIDE_CONTROLS_MM {
                return;
            }
            ui.separator();

            let draft = app.design.draft;
            let (parting_z, mode) = match (draft.auto_parting, app.panes[pane].section.as_ref()) {
                (true, Some(s)) => (s.parting_z_mm, "auto"),
                (true, None) => (draft.parting_z_mm, "auto"),
                (false, _) => (draft.parting_z_mm, "fixed"),
            };
            ui.label(
                RichText::new(format!(
                    "{} parting z {:+.2} mm ({mode}) • min draft {:.1}°",
                    icon::RULER,
                    parting_z,
                    draft.min_draft_deg
                ))
                .color(theme::TEXT_DIM),
            );
        });
    });
}

// --- Measurements ----------------------------------------------------------

fn readout(app: &RingDesignerApp, ui: &mut egui::Ui, pane: usize) {
    let Some(s) = app.panes[pane].section.as_ref() else {
        ui.label(
            RichText::new("No measurements")
                .small()
                .color(theme::TEXT_DIM),
        );
        return;
    };

    let thin = s.min_wall_mm < app.design.draft.min_section_mm;
    let thickness = s.max_r - s.min_r;
    let width = s.max_z - s.min_z;

    ui.horizontal_wrapped(|ui| {
        field(
            ui,
            "Min wall",
            &format!("{:.2} mm", s.min_wall_mm),
            if thin { theme::BAD } else { theme::GOOD },
        );
        if thin {
            ui.label(
                RichText::new(format!(
                    "{} under {:.2} mm minimum section",
                    icon::WARNING,
                    app.design.draft.min_section_mm
                ))
                .small()
                .color(theme::BAD),
            );
        }
        ui.separator();
        field(
            ui,
            "Undercuts",
            &format!("{}", s.undercut_count),
            if s.undercut_count > 0 {
                theme::BAD
            } else {
                theme::GOOD
            },
        );
        ui.separator();
        field(ui, "Thickness", &format!("{thickness:.2} mm"), theme::TEXT);
        field(ui, "Width", &format!("{width:.2} mm"), theme::TEXT);
        ui.separator();
        field(ui, "Bore r", &format!("{:.2} mm", s.min_r), theme::TEXT_DIM);
        field(
            ui,
            "Outer r",
            &format!("{:.2} mm", s.max_r),
            theme::TEXT_DIM,
        );
        field(
            ui,
            "z range",
            &format!("{:+.2} to {:+.2} mm", s.min_z, s.max_z),
            theme::TEXT_DIM,
        );
    });
}

fn field(ui: &mut egui::Ui, label: &str, value: &str, color: Color32) {
    ui.label(RichText::new(label).small().color(theme::TEXT_DIM));
    ui.label(RichText::new(value).color(color).strong());
}

// --- Canvas ----------------------------------------------------------------

fn canvas(app: &RingDesignerApp, ui: &mut egui::Ui, pane: usize, opts_id: egui::Id) {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme::VIEWPORT_BG);

    let section = match app.panes[pane].section.as_ref() {
        Some(s) if s.points.len() >= 3 => s,
        _ => {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "No slice yet — build the ring to cut a cross-section",
                FontId::proportional(13.0),
                theme::TEXT_DIM,
            );
            return;
        }
    };

    let inner = rect.shrink2(vec2(66.0, 42.0));
    if inner.width() < 8.0 || inner.height() < 8.0 {
        return;
    }

    let lo_r = section.min_r;
    let hi_r = section.max_r.max(lo_r + 0.01);
    let lo_z = section.min_z.min(section.parting_z_mm);
    let hi_z = section.max_z.max(section.parting_z_mm).max(lo_z + 0.01);
    let r_mid = (lo_r + hi_r) * 0.5;
    let z_mid = (lo_z + hi_z) * 0.5;
    let scale = (f64::from(inner.width()) / (hi_r - lo_r))
        .min(f64::from(inner.height()) / (hi_z - lo_z)) as f32;
    let centre = inner.center();

    // r runs horizontally, z vertically with screen y inverted.
    let map = |r: f64, z: f64| {
        pos2(
            centre.x + (r - r_mid) as f32 * scale,
            centre.y - (z - z_mid) as f32 * scale,
        )
    };

    let pts: Vec<Pos2> = section.points.iter().map(|p| map(p.r, p.z)).collect();
    let n = pts.len();

    // Stone silhouettes: any seat whose arc covers this slice gets its
    // stone drawn to scale — girdle on the pad, pavilion into the metal —
    // so depth against the bore is visible where it matters.
    {
        let surface: Vec<&ringdesign_core::castability::SectionPoint> =
            section.points.iter().filter(|p| p.surface).collect();
        let total: f64 = surface
            .windows(2)
            .map(|w| ((w[1].r - w[0].r).powi(2) + (w[1].z - w[0].z).powi(2)).sqrt())
            .sum();
        let at_v = |v_mm: f64| -> Option<(f64, f64, f64, f64)> {
            if surface.len() < 2 || total <= 1e-9 {
                return None;
            }
            let ctx = app.design.field_context();
            let target = v_mm / ctx.band_v_len_mm.max(1e-9) * total;
            let mut acc = 0.0;
            for w in surface.windows(2) {
                let seg = ((w[1].r - w[0].r).powi(2) + (w[1].z - w[0].z).powi(2)).sqrt();
                if acc + seg >= target {
                    let f = ((target - acc) / seg.max(1e-12)).clamp(0.0, 1.0);
                    let r = w[0].r + (w[1].r - w[0].r) * f;
                    let z = w[0].z + (w[1].z - w[0].z) * f;
                    let nr = w[0].nr + (w[1].nr - w[0].nr) * f;
                    let nz = w[0].nz + (w[1].nz - w[0].nz) * f;
                    return Some((r, z, nr, nz));
                }
                acc += seg;
            }
            None
        };
        let mut draw_stone = |seat: &ringdesign_core::field::SeatPadLayer| {
            let Some((r, z, nr, nz)) = at_v(seat.v_mm) else { return };
            let gem = seat
                .gem
                .unwrap_or_else(|| ringdesign_core::gem::Gem::calibrated(
                    ringdesign_core::gem::GemCut::Round,
                    seat.suggested_stone_mm(),
                ));
            let half = gem.w_mm * 0.5;
            let depth = gem.pavilion_mm();
            let len = (nr * nr + nz * nz).sqrt().max(1e-9);
            let (nr, nz) = (nr / len, nz / len);
            let (tr, tz) = (-nz, nr);
            // Girdle sits on the pad top; pavilion dives along the normal.
            let top = (r + nr * seat.height_mm, z + nz * seat.height_mm);
            let ga = map(top.0 + tr * half, top.1 + tz * half);
            let gb = map(top.0 - tr * half, top.1 - tz * half);
            let culet = map(top.0 - nr * depth, top.1 - nz * depth);
            let stroke = egui::Stroke::new(1.2, theme::ACCENT_DIM);
            painter.line_segment([ga, gb], stroke);
            painter.line_segment([ga, culet], stroke);
            painter.line_segment([gb, culet], stroke);
        };
        for e in app.design.layers.layers.iter().filter(|e| e.enabled) {
            match &e.layer {
                ringdesign_core::field::Layer::SeatPad(p) => {
                    let arc = (p.diameter_mm * 0.5 + p.blend_mm)
                        / (section.max_r.max(1e-9))
                        * 180.0
                        / std::f64::consts::PI;
                    let d = ringdesign_core::field::wrap_delta(
                        section.theta_deg - p.theta_deg,
                        360.0,
                    )
                    .abs();
                    if d <= arc.max(2.0) {
                        draw_stone(p);
                    }
                }
                ringdesign_core::field::Layer::SeatRun(run) => {
                    // A run has a seat at every station; the slice always
                    // sits within half a pitch of one.
                    draw_stone(&run.seat);
                }
                _ => {}
            }
        }
    }

    // The pinned comparison's slice, dashed behind the live one — cut from
    // the pinned design at this angle, so it needs no stored build.
    if let Some(pinned) = app.pinned.as_ref() {
        let ghost =
            ringdesign_core::castability::section_at(pinned, &app.lib, section.theta_deg, 160);
        let gpts: Vec<Pos2> = ghost.points.iter().map(|p| map(p.r, p.z)).collect();
        for k in 0..gpts.len() {
            let (a, b) = (gpts[k], gpts[(k + 1) % gpts.len()]);
            if k % 2 == 0 {
                painter.line_segment([a, b], egui::Stroke::new(1.0, theme::TEXT_DIM));
            }
        }
    }
    let opts = ui.memory_mut(|m| *m.data.get_temp_mut_or_default::<ViewOpts>(opts_id));

    if opts.fill && is_convex(&pts) {
        let [mr, mg, mb] =
            crate::viewport::FINISHES[app.finish.min(crate::viewport::FINISHES.len() - 1)].rgb;
        let tint = Color32::from_rgba_unmultiplied(
            (mr * 255.0) as u8,
            (mg * 255.0) as u8,
            (mb * 255.0) as u8,
            26,
        );
        painter.add(egui::Shape::convex_polygon(pts.clone(), tint, Stroke::NONE));
    }

    draw_axis(
        &painter,
        rect,
        section,
        map(0.0, z_mid).x,
        map(section.min_r, z_mid).x,
    );
    draw_parting(&painter, rect, section, map(r_mid, section.parting_z_mm).y);

    if opts.ticks {
        let step = (n / 44).max(1);
        for i in (0..n).step_by(step) {
            let p = &section.points[i];
            let Some(dir) = normal_dir(p) else { continue };
            painter.line_segment(
                [pts[i], pts[i] + dir * 11.0],
                Stroke::new(1.2, theme::class_color(p.class).gamma_multiply(0.85)),
            );
        }
    }

    for i in 0..n {
        let p = &section.points[i];
        let color = if p.surface {
            theme::class_color(p.class)
        } else {
            theme::class_color(p.class).gamma_multiply(0.55)
        };
        painter.line_segment([pts[i], pts[(i + 1) % n]], Stroke::new(2.0, color));
    }

    draw_undercuts(&painter, section, &pts);
    draw_legend(&painter, rect, section);

    painter.text(
        rect.right_bottom() - vec2(12.0, 9.0),
        Align2::RIGHT_BOTTOM,
        format!(
            "Slice at {:.1}° • hover a wall for its draft",
            section.theta_deg
        ),
        FontId::proportional(11.0),
        theme::TEXT_DIM,
    );

    let mut pick: Option<(usize, Pos2)> = None;
    if let Some(cursor) = response.hover_pos() {
        let mut best = 16.0f32;
        for i in 0..n {
            let (d, at) = nearest_on_segment(cursor, pts[i], pts[(i + 1) % n]);
            if d < best {
                best = d;
                pick = Some((i, at));
            }
        }
    }
    if let Some((i, at)) = pick {
        let p = section.points[i];
        painter.circle_stroke(at, 5.0, Stroke::new(1.4, theme::ACCENT));
        response.on_hover_ui(|ui| {
            let color = theme::class_color(p.class);
            ui.label(
                RichText::new(format!("{:+.1}° draft", p.draft_deg))
                    .color(color)
                    .strong(),
            );
            ui.label(RichText::new(p.class.label()).color(color));
            ui.label(
                RichText::new(format!("r {:.3} mm • z {:+.3} mm", p.r, p.z))
                    .small()
                    .color(theme::TEXT_DIM),
            );
            ui.label(
                RichText::new(if p.surface {
                    "Displaceable surface"
                } else {
                    "Bore wall"
                })
                .small()
                .color(theme::TEXT_DIM),
            );
            if p.class == FaceClass::Undercut {
                ui.label(RichText::new("Locks in the sand").small().color(theme::BAD));
            }
        });
    }
}

/// Dashed line at the parting height plus the two pull directions.
fn draw_parting(painter: &egui::Painter, rect: egui::Rect, section: &Section, y: f32) {
    painter.extend(egui::Shape::dashed_line(
        &[pos2(rect.left() + 10.0, y), pos2(rect.right() - 10.0, y)],
        Stroke::new(1.4, theme::ACCENT_DIM),
        7.0,
        5.0,
    ));
    painter.text(
        pos2(rect.right() - 12.0, y - 4.0),
        Align2::RIGHT_BOTTOM,
        format!("parting plane z = {:+.2} mm", section.parting_z_mm),
        FontId::proportional(11.0),
        theme::ACCENT,
    );

    let x = rect.left() + 26.0;
    let stroke = Stroke::new(1.5, theme::INFO);
    let font = FontId::proportional(11.0);
    arrow(painter, pos2(x, y - 8.0), pos2(x, y - 36.0), stroke);
    painter.text(
        pos2(x + 8.0, y - 24.0),
        Align2::LEFT_CENTER,
        "cope pulls +Z",
        font.clone(),
        theme::INFO,
    );
    arrow(painter, pos2(x, y + 8.0), pos2(x, y + 36.0), stroke);
    painter.text(
        pos2(x + 8.0, y + 24.0),
        Align2::LEFT_CENTER,
        "drag pulls -Z",
        font,
        theme::INFO,
    );
}

/// The finger axis when it is in frame, otherwise the bore wall and its distance.
fn draw_axis(
    painter: &egui::Painter,
    rect: egui::Rect,
    section: &Section,
    axis_x: f32,
    bore_x: f32,
) {
    let top = rect.top() + 8.0;
    let bottom = rect.bottom() - 8.0;
    if axis_x > rect.left() + 4.0 {
        painter.extend(egui::Shape::dashed_line(
            &[pos2(axis_x, top), pos2(axis_x, bottom)],
            Stroke::new(1.0, theme::GRID),
            4.0,
            4.0,
        ));
        painter.text(
            pos2(axis_x + 5.0, top),
            Align2::LEFT_TOP,
            "ring axis, r = 0",
            FontId::proportional(11.0),
            theme::TEXT_DIM,
        );
        return;
    }

    painter.line_segment(
        [pos2(bore_x, top), pos2(bore_x, bottom)],
        Stroke::new(1.0, theme::GRID),
    );
    let y = rect.top() + 20.0;
    arrow(
        painter,
        pos2(bore_x - 8.0, y),
        pos2(bore_x - 30.0, y),
        Stroke::new(1.2, theme::TEXT_DIM),
    );
    painter.text(
        pos2(bore_x + 5.0, y),
        Align2::LEFT_CENTER,
        format!("bore wall • axis {:.2} mm this way", section.min_r),
        FontId::proportional(11.0),
        theme::TEXT_DIM,
    );
}

/// Thick overdraw and a label on every run of undercut segments.
fn draw_undercuts(painter: &egui::Painter, section: &Section, pts: &[Pos2]) {
    let n = pts.len();
    let font = FontId::proportional(11.0);
    for (labelled, (start, len)) in undercut_runs(&section.points).into_iter().enumerate() {
        for k in 0..len {
            let i = (start + k) % n;
            painter.line_segment([pts[i], pts[(i + 1) % n]], Stroke::new(3.6, theme::BAD));
        }
        let mid = (start + len / 2) % n;
        painter.circle_filled(pts[mid], 3.0, theme::BAD);
        if labelled >= 4 {
            continue;
        }
        let p = &section.points[mid];
        let dir = normal_dir(p).unwrap_or(vec2(1.0, 0.0));
        let align = if dir.x >= 0.0 {
            Align2::LEFT_CENTER
        } else {
            Align2::RIGHT_CENTER
        };
        painter.text(
            pts[mid] + dir * 14.0,
            align,
            format!("{} undercut {:+.1}°", icon::WARNING, p.draft_deg),
            font.clone(),
            theme::BAD,
        );
    }
}

/// Face-class key with the segment count of each class in this slice.
fn draw_legend(painter: &egui::Painter, rect: egui::Rect, section: &Section) {
    let mut counts = [0usize; 4];
    for p in &section.points {
        counts[class_index(p.class)] += 1;
    }

    let font = FontId::proportional(11.0);
    let galleys: Vec<_> = CLASSES
        .iter()
        .enumerate()
        .map(|(i, c)| {
            painter.layout_no_wrap(
                format!("{} • {}", c.label(), counts[i]),
                font.clone(),
                theme::TEXT,
            )
        })
        .collect();

    let swatch = 9.0f32;
    let line_h = 16.0f32;
    let pad = vec2(9.0, 7.0);
    let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max) + swatch + 7.0;
    let size = vec2(width, line_h * CLASSES.len() as f32) + pad * 2.0;
    let at = pos2(rect.left() + 12.0, rect.bottom() - 12.0 - size.y);
    let panel = egui::Rect::from_min_size(at, size);

    painter.rect_filled(panel, 5.0, theme::PANEL.gamma_multiply(0.88));
    painter.rect_stroke(
        panel,
        5.0,
        Stroke::new(1.0, theme::GRID),
        egui::StrokeKind::Inside,
    );

    for (i, galley) in galleys.into_iter().enumerate() {
        let y = at.y + pad.y + i as f32 * line_h;
        painter.rect_filled(
            egui::Rect::from_min_size(
                pos2(at.x + pad.x, y + (line_h - swatch) * 0.5),
                Vec2::splat(swatch),
            ),
            2.0,
            theme::class_color(CLASSES[i]),
        );
        let ty = y + (line_h - galley.size().y) * 0.5;
        painter.galley(pos2(at.x + pad.x + swatch + 7.0, ty), galley, theme::TEXT);
    }
}

// --- Geometry helpers ------------------------------------------------------

/// Shaft and two barbs.
fn arrow(painter: &egui::Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    let span = to - from;
    if span.length() < 1.0 {
        return;
    }
    let dir = span.normalized();
    let side = vec2(-dir.y, dir.x);
    painter.line_segment([from, to], stroke);
    painter.line_segment([to, to - dir * 6.0 + side * 3.5], stroke);
    painter.line_segment([to, to - dir * 6.0 - side * 3.5], stroke);
}

/// Unit screen direction of a point's outward normal, y inverted.
fn normal_dir(p: &SectionPoint) -> Option<Vec2> {
    let d = vec2(p.nr as f32, -(p.nz as f32));
    (d.length() > 1e-4).then(|| d.normalized())
}

fn nearest_on_segment(p: Pos2, a: Pos2, b: Pos2) -> (f32, Pos2) {
    let ab = b - a;
    let len2 = ab.length_sq();
    let t = if len2 <= f32::EPSILON {
        0.0
    } else {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    };
    let at = a + ab * t;
    ((p - at).length(), at)
}

fn is_convex(pts: &[Pos2]) -> bool {
    let n = pts.len();
    if n < 3 {
        return false;
    }
    let mut sign = 0i32;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let c = pts[(i + 2) % n];
        let cross = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
        if cross.abs() < 1e-5 {
            continue;
        }
        let s = if cross > 0.0 { 1 } else { -1 };
        if sign == 0 {
            sign = s;
        } else if sign != s {
            return false;
        }
    }
    true
}

/// Contiguous runs of undercut segments as (start index, length), wrapping.
fn undercut_runs(points: &[SectionPoint]) -> Vec<(usize, usize)> {
    let n = points.len();
    let is_cut = |i: usize| points[i].class == FaceClass::Undercut;
    let Some(origin) = (0..n).find(|&i| !is_cut(i)) else {
        return vec![(0, n)];
    };

    let mut runs = Vec::new();
    let mut i = 0;
    while i < n {
        if is_cut((origin + i) % n) {
            let mut len = 1;
            while i + len < n && is_cut((origin + i + len) % n) {
                len += 1;
            }
            runs.push(((origin + i) % n, len));
            i += len;
        } else {
            i += 1;
        }
    }
    runs
}

fn class_index(c: FaceClass) -> usize {
    match c {
        FaceClass::Good => 0,
        FaceClass::Marginal => 1,
        FaceClass::Vertical => 2,
        FaceClass::Undercut => 3,
    }
}
