//! Base geometry: size, cross-section, shank, casting setup and export resolution.

use egui::RichText;
use egui_phosphor::regular as icon;
use ringdesign_core::field::SIDE_FACE_MIN_DRAFT_DEG;
use ringdesign_core::profile::{
    EDGE_FLANGE_T, MIN_EDGE_MM, ProfileStyle, SQUARED_SIDE_FILLET_MM, ShankKind, TOP_DEG,
};
use ringdesign_core::sizing::RingSize;

use ringdesign_core::refine::RefineParams;

use crate::app::RingDesignerApp;
use crate::theme;

pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    section(ui, format!("{} Ring", icon::CIRCLE_NOTCH), true, |ui| ring(app, ui));
    section(ui, format!("{} Profile", icon::CIRCLE_HALF), true, |ui| profile(app, ui));
    section(ui, format!("{} Shank", icon::WAVE_SINE), false, |ui| shank(app, ui));
    section(ui, format!("{} Casting", icon::HAMMER), false, |ui| casting(app, ui));
    section(ui, format!("{} Mesh", icon::TRIANGLE), false, |ui| mesh(app, ui));
}

fn section(ui: &mut egui::Ui, title: String, open: bool, add: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).strong())
        .default_open(open)
        .show(ui, add);
}

/// Wrapped small dim text.
fn hint(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(RichText::new(text.into()).small().color(theme::TEXT_DIM)).wrap());
}

// --- Ring ------------------------------------------------------------------

fn ring(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.add(
            egui::TextEdit::singleline(&mut app.design.name)
                .desired_width(f32::INFINITY)
                .hint_text("Untitled"),
        );
    });

    let mut size = app.design.size.0;
    let changed = ui
        .add(
            egui::Slider::new(&mut size, 3.0..=15.0)
                .step_by(0.25)
                .fixed_decimals(2)
                .text("Size"),
        )
        .changed();
    if changed {
        app.design.size = RingSize::new(size);
        app.mark_dirty();
    }

    let s = app.design.size;
    ui.horizontal(|ui| {
        ui.label(RichText::new(s.display()).color(theme::ACCENT).strong());
        ui.label(
            RichText::new(format!(
                "{} {:.2} mm bore • {:.2} mm circumference",
                icon::RULER,
                s.inner_diameter_mm(),
                s.inner_circumference_mm()
            ))
            .small()
            .color(theme::TEXT_DIM),
        );
    });
}

// --- Profile ---------------------------------------------------------------

fn profile(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    ui.horizontal_top(|ui| {
        preview(app, ui);
        ui.vertical(|ui| {
            let combo_w = (ui.available_width() - 6.0).clamp(96.0, 150.0);
            egui::ComboBox::from_id_salt("profile_style")
                .selected_text(app.design.profile.style.label())
                .width(combo_w)
                .show_ui(ui, |ui| {
                    for &style in ProfileStyle::ALL {
                        let selected = app.design.profile.style == style;
                        if ui
                            .selectable_label(selected, style.label())
                            .on_hover_text(style.casting_note())
                            .clicked()
                        {
                            app.design.profile.apply_style(style);
                            app.mark_dirty();
                        }
                    }
                });
            hint(ui, app.design.profile.style.casting_note());
        });
    });

    ui.add_space(2.0);

    let thickness = app.design.profile.thickness_mm;
    let mut changed = false;
    egui::Grid::new("profile_dims")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            let p = &mut app.design.profile;

            ui.label("Width");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut p.width_mm)
                        .speed(0.05)
                        .range(1.0..=20.0)
                        .suffix(" mm"),
                )
                .on_hover_text("Band width along the finger.")
                .changed();
            ui.end_row();

            ui.label("Thickness");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut p.thickness_mm)
                        .speed(0.02)
                        .range(0.6..=6.0)
                        .suffix(" mm"),
                )
                .on_hover_text("Metal at the crest, measured off the bore.")
                .changed();
            ui.end_row();

            ui.label("Crown");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut p.crown_mm)
                        .speed(0.02)
                        .range(0.0..=thickness)
                        .suffix(" mm"),
                )
                .on_hover_text("Drop from the crest down to the outer edge.")
                .changed();
            ui.end_row();

            ui.label("Edge round");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut p.edge_round_mm)
                        .speed(0.01)
                        .range(0.0..=(thickness * 0.45).max(0.05))
                        .suffix(" mm"),
                )
                .on_hover_text("Fillet where the side faces meet the outer surface.")
                .changed();
            ui.end_row();

            ui.label("Comfort fit");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut p.comfort_fit_mm)
                        .speed(0.01)
                        .range(0.0..=1.5)
                        .suffix(" mm"),
                )
                .on_hover_text("Dome inside the bore; size is measured at the contact band.")
                .changed();
            ui.end_row();

            ui.label("Side draft");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut p.side_draft_deg)
                        .speed(0.1)
                        .range(-5.0..=20.0)
                        .suffix("°"),
                )
                .on_hover_text("Positive narrows the band outward, adding draft to the side faces.")
                .changed();
            ui.end_row();

            ui.label("Crest bias");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut p.crest_bias)
                        .speed(0.01)
                        .range(-1.0..=1.0)
                        .fixed_decimals(2),
                )
                .on_hover_text("Moves the crest across the width; 0 is centred.")
                .changed();
            ui.end_row();
        });

    changed |= side_faces(app, ui);

    if app.design.profile.style == ProfileStyle::Custom {
        ui.add_space(2.0);
        hint(ui, "Superellipse exponents of d(x) = 1 - (1 - x^a)^(1/b).");
        egui::Grid::new("profile_shape")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                let p = &mut app.design.profile;

                ui.label("Shape a");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut p.shape_a)
                            .speed(0.02)
                            .range(0.25..=12.0)
                            .fixed_decimals(2),
                    )
                    .on_hover_text("Higher flattens the crown and sharpens the falloff at the edges.")
                    .changed();
                ui.end_row();

                ui.label("Shape b");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut p.shape_b)
                            .speed(0.02)
                            .range(0.25..=12.0)
                            .fixed_decimals(2),
                    )
                    .on_hover_text("Higher fills the crest out.")
                    .changed();
                ui.end_row();
            });
    }

    if changed {
        app.mark_dirty();
    }

    let p = &app.design.profile;
    let clamped = p.crown_mm > p.effective_crown_mm() + 1e-9;
    let edge = p.edge_thickness_mm();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Edge thickness").small().color(theme::TEXT_DIM));
        ui.label(
            RichText::new(format!("{edge:.2} mm"))
                .small()
                .strong()
                .color(if clamped { theme::WARN } else { theme::TEXT }),
        );
    });
    if clamped {
        ui.add(
            egui::Label::new(
                RichText::new(format!(
                    "{} Crown clamped to {:.2} mm: an edge under {:.2} mm will not fill in sand.",
                    icon::WARNING,
                    p.effective_crown_mm(),
                    MIN_EDGE_MM
                ))
                .small()
                .color(theme::WARN),
            )
            .wrap(),
        );
    }

    ui.add_space(2.0);
    let open = app.design.profile.flange.enabled;
    egui::CollapsingHeader::new(format!("{} Flat rim / flange", icon::DISC))
        .id_salt("profile_flange")
        .default_open(open)
        .show(ui, |ui| flange(app, ui));
}

/// The two faces square to the mould pull, and a control to square them up.
///
/// This is the only surface on the ring that holds relief deeper than a couple
/// of tenths, so it is worth reporting in millimetres rather than leaving the
/// user to infer it from side draft and edge round.
fn side_faces(app: &mut RingDesignerApp, ui: &mut egui::Ui) -> bool {
    let mut changed = false;
    let ctx = app.design.field_context();
    let faces = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG);

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let squared = app.design.profile.side_draft_deg == 0.0
            && app.design.profile.edge_round_mm <= SQUARED_SIDE_FILLET_MM;
        if ui
            .add_enabled(!squared, egui::Button::new(format!("{} Square the sides", icon::SQUARE_HALF)))
            .on_hover_text(
                "Drop the side draft to zero and shrink the edge fillet, so the two side \
                 faces sit flat against the mould pull and can carry deep ornament.",
            )
            .on_disabled_hover_text("The sides are already square to the pull.")
            .clicked()
        {
            app.design.profile.flatten_sides();
            changed = true;
        }
        let (text, colour) = match faces {
            Some(f) if f.is_even() => (
                format!("{:.2} mm on each edge", f.low_width().min(f.high_width())),
                theme::GOOD,
            ),
            Some(f) => (
                format!("{:.2} mm and {:.2} mm — uneven", f.low_width(), f.high_width()),
                theme::WARN,
            ),
            None => ("none — all dome".to_string(), theme::WARN),
        };
        ui.label(egui::RichText::new(text).small().color(colour));
    });

    let note = match faces {
        Some(_) => format!(
            "{} Ornament on a side face pulls straight out of the sand. The crest cannot \
             hold much past 0.15 mm.",
            icon::INFO
        ),
        None => format!(
            "{} A dome has no face square to the pull. Square the sides, pick a flatter \
             profile, or add an edge flange.",
            icon::INFO
        ),
    };
    hint(ui, &note);
    changed
}

// --- Flange ----------------------------------------------------------------

/// Dome crest as a fraction of the band width, ignoring any flange rim.
fn dome_crest_t(app: &RingDesignerApp) -> f64 {
    (0.5 + 0.5 * app.design.profile.crest_bias.clamp(-1.0, 1.0)).clamp(0.06, 0.94)
}

/// What the flange becomes at this position across the band.
fn flange_meaning(v_pos: f64) -> &'static str {
    if v_pos <= EDGE_FLANGE_T {
        "At the bottom edge: a widened side face, one broad flat annulus facing -Z."
    } else if v_pos >= 1.0 - EDGE_FLANGE_T {
        "At the top edge: a widened side face, one broad flat annulus facing +Z."
    } else {
        "Mid-band: a flange, a thin disc around the circumference with a flat face on each side."
    }
}

fn flange(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    let crest_t = dome_crest_t(app);
    let width = app.design.profile.width_mm;
    let thickness = app.design.profile.thickness_mm;
    let mut changed = false;

    {
        let f = &mut app.design.profile.flange;
        changed |= ui
            .checkbox(&mut f.enabled, "Flat annular face")
            .on_hover_text("A flat face square to the mould pull, the best surface on the ring for ornament.")
            .changed();

        let enabled = f.enabled;
        ui.add_enabled_ui(enabled, |ui| {
            changed |= ui
                .add(
                    egui::Slider::new(&mut f.v_pos, 0.0..=1.0)
                        .custom_formatter(|v, _| match v {
                            v if v <= EDGE_FLANGE_T => "bottom edge".to_string(),
                            v if v >= 1.0 - EDGE_FLANGE_T => "top edge".to_string(),
                            v if (v - 0.5).abs() < 0.005 => "middle".to_string(),
                            v => format!("{v:.2}"),
                        })
                        .text("Across"),
                )
                .on_hover_text("0 is the bottom band edge, 0.5 the middle, 1 the top band edge.")
                .changed();
            hint(ui, flange_meaning(f.v_pos));

            ui.add_space(2.0);
            egui::Grid::new("flange_dims")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Extent");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut f.extent_mm)
                                .speed(0.02)
                                .range(0.0..=width.max(thickness))
                                .suffix(" mm"),
                        )
                        .on_hover_text("Radial projection of the rim beyond the dome.")
                        .changed();
                    ui.end_row();

                    ui.label("Thickness");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut f.thickness_mm)
                                .speed(0.02)
                                .range(MIN_EDGE_MM..=(width * 0.8).max(MIN_EDGE_MM))
                                .suffix(" mm"),
                        )
                        .on_hover_text("Axial thickness of the flat disc.")
                        .changed();
                    ui.end_row();

                    ui.label("Edge round");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut f.edge_round_mm)
                                .speed(0.01)
                                .range(0.0..=2.0)
                                .suffix(" mm"),
                        )
                        .on_hover_text("Fillet where the flange meets the dome.")
                        .changed();
                    ui.end_row();
                });
        });
    }

    let f = app.design.profile.flange;
    if !f.is_castable_at(crest_t) {
        let snap = f.nearest_castable(crest_t);
        ui.add_space(2.0);
        ui.add(
            egui::Label::new(
                RichText::new(format!(
                    "{} Rim at {:.2}, crest at {:.2}. The mould parts at the rim, and the stretch \
                     of dome between the crest and the rim leans back under it — that undercuts \
                     and locks in the sand.",
                    icon::WARNING,
                    f.v_pos,
                    crest_t
                ))
                .small()
                .color(theme::WARN),
            )
            .wrap(),
        );
        if ui
            .button(format!("{} Snap to nearest castable", icon::MAGIC_WAND))
            .on_hover_text(format!("Moves the flange to {snap:.2}"))
            .clicked()
        {
            app.design.profile.flange.v_pos = snap;
            changed = true;
        }
    }

    if changed {
        app.mark_dirty();
    }
}

/// Cross-section plotted with the finger axis horizontal and the crest up.
fn preview(app: &RingDesignerApp, ui: &mut egui::Ui) {
    let (response, painter) = ui.allocate_painter(egui::vec2(120.0, 90.0), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 3.0, theme::BG);

    let loop_ = app.design.profile.sample(app.design.inner_radius_mm(), 160);
    if loop_.is_empty() {
        return;
    }

    let (z_lo, z_hi) = loop_.z_range();
    let (r_lo, r_hi) = loop_
        .pts
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p.r), hi.max(p.r)));
    let span_z = (z_hi - z_lo).max(1e-6);
    let span_r = (r_hi - r_lo).max(1e-6);

    let inner = rect.shrink(8.0);
    let scale = (inner.width() as f64 / span_z).min(inner.height() as f64 / span_r);
    let centre = inner.center();
    let z_mid = 0.5 * (z_lo + z_hi);
    let r_mid = 0.5 * (r_lo + r_hi);
    let to_screen = |r: f64, z: f64| {
        egui::pos2(
            centre.x + ((z - z_mid) * scale) as f32,
            centre.y - ((r - r_mid) * scale) as f32,
        )
    };

    let pts: Vec<egui::Pos2> = loop_.pts.iter().map(|p| to_screen(p.r, p.z)).collect();
    let stroke = egui::Stroke::new(1.2, theme::ACCENT);
    if is_convex(&pts) {
        painter.add(egui::Shape::convex_polygon(
            pts,
            theme::ACCENT.gamma_multiply(0.14),
            stroke,
        ));
    } else {
        painter.add(egui::Shape::closed_line(pts, stroke));
    }

    if let Some(crest) = loop_
        .pts
        .iter()
        .filter(|p| p.surface)
        .max_by(|a, b| a.r.total_cmp(&b.r))
    {
        painter.circle_filled(to_screen(crest.r, crest.z), 1.8, theme::ACCENT);
    }

    response.on_hover_text(format!(
        "Cross-section • {:.2} mm outside diameter",
        2.0 * loop_.crest_radius_mm
    ));
}

/// True when every turn around the loop has the same sign.
fn is_convex(pts: &[egui::Pos2]) -> bool {
    let n = pts.len();
    if n < 3 {
        return false;
    }
    let mut sign = 0i32;
    for i in 0..n {
        let (a, b, c) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
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

// --- Shank -----------------------------------------------------------------

fn shank(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    let mut changed = false;

    egui::ComboBox::from_id_salt("shank_kind")
        .selected_text(app.design.shank.kind.label())
        .width(180.0)
        .show_ui(ui, |ui| {
            for &kind in ShankKind::ALL {
                changed |= ui
                    .selectable_value(&mut app.design.shank.kind, kind, kind.label())
                    .on_hover_text(kind.description())
                    .changed();
            }
        });
    hint(ui, app.design.shank.kind.description());

    let uniform = app.design.shank.kind == ShankKind::Uniform;
    changed |= ui
        .add_enabled(
            !uniform,
            egui::Slider::new(&mut app.design.shank.amount, 0.0..=1.0)
                .fixed_decimals(2)
                .text("Amount"),
        )
        .changed();

    if app.design.shank.kind == ShankKind::Signet {
        let sh = &mut app.design.shank;
        changed |= ui
            .add(egui::Slider::new(&mut sh.head_span_deg, 40.0..=220.0).suffix("°").text("Head arc"))
            .on_hover_text("How far round the ring the head reaches before it is shank again.")
            .changed();
        changed |= ui
            .add(
                egui::Slider::new(&mut sh.head_shape_a, 1.5..=10.0)
                    .fixed_decimals(1)
                    .text("Head shape"),
            )
            .on_hover_text("Fullness of the head outline: 2 oval, 4 cushion, 8 rectangle.")
            .changed();
        let shank_mm = app.design.profile.width_mm * app.design.shank.signet_width_frac(TOP_DEG + 180.0);
        hint(
            ui,
            format!(
                "Head {:.1} mm wide, shank {shank_mm:.1} mm. Width is the head; the taper makes \
                 the rest.",
                app.design.profile.width_mm
            ),
        );
    }

    if changed {
        app.mark_dirty();
    }
}

// --- Casting ---------------------------------------------------------------

fn casting(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    let mut changed = false;
    let d = &mut app.design.draft;

    changed |= ui
        .checkbox(&mut d.auto_parting, "Auto parting plane")
        .on_hover_text("Part the mould at the widest silhouette of the ring.")
        .changed();

    ui.horizontal(|ui| {
        ui.label("Parting Z");
        changed |= ui
            .add_enabled(
                !d.auto_parting,
                egui::DragValue::new(&mut d.parting_z_mm)
                    .speed(0.02)
                    .range(-10.0..=10.0)
                    .suffix(" mm"),
            )
            .changed();
    });

    changed |= ui
        .add(
            egui::Slider::new(&mut d.min_draft_deg, 1.0..=10.0)
                .fixed_decimals(1)
                .suffix("°")
                .text("Min draft"),
        )
        .changed();
    hint(ui, "3° is a normal minimum for sand; below it a wall drags on the way out.");

    changed |= ui
        .add(
            egui::Slider::new(&mut d.min_section_mm, 0.3..=2.0)
                .fixed_decimals(2)
                .suffix(" mm")
                .text("Min section"),
        )
        .changed();
    hint(ui, "Thinnest section the metal will reliably fill.");

    if changed {
        app.mark_dirty();
    }
}

// --- Mesh ------------------------------------------------------------------

fn mesh(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    crate::panels::quality_picker(ui, "export_quality", &mut app.export_params);

    match app.export_params.refine {
        Some(r) => {
            ui.add_space(4.0);
            let mut tol = r.tolerance_mm;
            if ui
                .add(
                    egui::Slider::new(&mut tol, 0.004..=0.2)
                        .logarithmic(true)
                        .fixed_decimals(3)
                        .suffix(" mm")
                        .text("Tolerance"),
                )
                .changed()
            {
                app.export_params.refine = Some(RefineParams { tolerance_mm: tol, ..r });
            }
            hint(
                ui,
                "Furthest the mesh may sit from the surface the design describes. The triangle \
                 count falls out of it rather than being chosen.",
            );
        }
        None => {
            egui::Grid::new("export_res")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Around");
                    ui.add(
                        egui::DragValue::new(&mut app.export_params.theta_steps)
                            .speed(8.0)
                            .range(64..=4096),
                    )
                    .on_hover_text("Sweep steps around the ring.");
                    ui.end_row();

                    ui.label("Across");
                    ui.add(
                        egui::DragValue::new(&mut app.export_params.profile_steps)
                            .speed(4.0)
                            .range(32..=1024),
                    )
                    .on_hover_text("Vertices around the cross-section.");
                    ui.end_row();
                });

            ui.label(
                RichText::new(format!(
                    "{} ~{} triangles on export",
                    icon::TRIANGLE,
                    thousands(app.export_params.triangle_estimate())
                ))
                .small()
                .color(theme::TEXT_DIM),
            );
        }
    }
    hint(ui, "Preview quality is on the toolbar.");
}

/// Group digits in threes.
fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}
