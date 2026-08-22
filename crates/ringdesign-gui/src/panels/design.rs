//! Base geometry: size, cross-section, shank, casting setup and export resolution.

use egui::RichText;
use egui_phosphor::regular as icon;
use ringdesign_core::field::{SIDE_FACE_MIN_DRAFT_DEG, SignetOutline};
use ringdesign_core::profile::{
    EDGE_FLANGE_T, MIN_EDGE_MM, ProfileStyle, SQUARED_SIDE_FILLET_MM, ShankKind,
};
use ringdesign_core::sizing::RingSize;

use ringdesign_core::refine::RefineParams;

use crate::app::RingDesignerApp;
use crate::theme;

pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    section(ui, format!("{} Ring", icon::CIRCLE_NOTCH), true, |ui| {
        ring(app, ui)
    });
    section(ui, format!("{} Profile", icon::CIRCLE_HALF), true, |ui| {
        profile(app, ui)
    });
    section(ui, format!("{} Shank", icon::WAVE_SINE), false, |ui| {
        shank(app, ui)
    });
    section(ui, format!("{} Casting", icon::HAMMER), false, |ui| {
        casting(app, ui)
    });
    section(ui, format!("{} Mesh", icon::TRIANGLE), false, |ui| {
        mesh(app, ui)
    });
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
                    // Each entry shows the section it means — picking a
                    // cross-section is a visual act, not a vocabulary test.
                    for &style in ProfileStyle::ALL {
                        let selected = app.design.profile.style == style;
                        if style_row(ui, style, selected)
                            .on_hover_text(style.casting_note())
                            .clicked()
                        {
                            app.design.profile.apply_style(style);
                            app.mark_dirty();
                        }
                    }
                });
            hint(ui, app.design.profile.style.casting_note());

            // The user's own profile library — sections saved by name, the
            // CrossGems factory-folder idea. Applying one keeps this band's
            // width and thickness: a profile is a shape, never a size.
            if !app.saved_profiles.is_empty() {
                let combo_w = (ui.available_width() - 6.0).clamp(96.0, 150.0);
                egui::ComboBox::from_id_salt("saved_profile")
                    .selected_text("Saved…")
                    .width(combo_w)
                    .show_ui(ui, |ui| {
                        let mut apply: Option<ringdesign_core::BandProfile> = None;
                        for (name, p) in &app.saved_profiles {
                            if profile_row(ui, name, p, false).clicked() {
                                apply = Some(p.clone());
                            }
                        }
                        if let Some(p) = apply {
                            app.design.profile.apply_shape(&p);
                            app.mark_dirty();
                        }
                    });
            }
            ui.horizontal(|ui| {
                let edit = egui::TextEdit::singleline(&mut app.profile_save_name)
                    .hint_text("name")
                    .desired_width(84.0);
                ui.add(edit);
                if ui
                    .small_button("Save")
                    .on_hover_text("Save this cross-section to the profile library")
                    .clicked()
                {
                    match ringdesign_core::library::save_profile(
                        &app.profile_save_name,
                        &app.design.profile,
                    ) {
                        Ok(_) => {
                            app.saved_profiles = ringdesign_core::library::list_profiles();
                            app.set_status(format!(
                                "Saved profile \"{}\"",
                                app.profile_save_name.trim()
                            ));
                            app.profile_save_name.clear();
                        }
                        Err(e) => app.set_status(format!("Profile not saved: {e}")),
                    }
                }
            });
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
        changed |= crown(app, ui);
    }

    if changed {
        app.mark_dirty();
    }

    let p = &app.design.profile;
    let clamped = p.crown_mm > p.effective_crown_mm() + 1e-9;
    let edge = p.edge_thickness_mm();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Edge thickness")
                .small()
                .color(theme::TEXT_DIM),
        );
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

    ui.add_space(2.0);
    let open = app.design.profile.morph.is_some();
    egui::CollapsingHeader::new(format!("{} Morph toward the top", icon::ARROWS_LEFT_RIGHT))
        .id_salt("profile_morph")
        .default_open(open)
        .show(ui, |ui| morph(app, ui));
}

/// The crown blends toward a second style around the top of the ring —
/// D-shape at the palm easing to a flat crown under a setting.
fn morph(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    use ringdesign_core::profile::ProfileMorph;
    let mut changed = false;

    let mut on = app.design.profile.morph.is_some();
    if ui
        .checkbox(&mut on, "Blend to a second crown at the top")
        .on_hover_text(
            "A blend of two monotone crowns is monotone, so the base surface \
             stays castable at every angle in between.",
        )
        .changed()
    {
        app.design.profile.morph =
            on.then(|| ProfileMorph::from_style(ProfileStyle::Flat, &app.design.profile));
        changed = true;
    }

    // Remember which style seeded the target, for the combo label.
    let style_id = ui.make_persistent_id("morph_style");
    let profile_snapshot = app.design.profile;
    if let Some(m) = &mut app.design.profile.morph {
        let mut style: ProfileStyle = ui
            .memory(|mem| mem.data.get_temp(style_id))
            .unwrap_or(ProfileStyle::Flat);
        egui::ComboBox::from_id_salt("morph_target")
            .selected_text(style.label())
            .width(170.0)
            .show_ui(ui, |ui| {
                for &s in ProfileStyle::ALL {
                    if s == ProfileStyle::Custom {
                        continue;
                    }
                    if ui.selectable_value(&mut style, s, s.label()).clicked() {
                        *m = ProfileMorph {
                            focus: m.focus,
                            ..ProfileMorph::from_style(s, &profile_snapshot)
                        };
                        changed = true;
                    }
                }
            });
        ui.memory_mut(|mem| mem.data.insert_temp(style_id, style));

        changed |= ui
            .add(
                egui::Slider::new(&mut m.focus, 0.5..=6.0)
                    .fixed_decimals(1)
                    .text("Focus"),
            )
            .on_hover_text("How tightly the second crown hugs the top of the ring")
            .changed();
    }

    if changed {
        app.mark_dirty();
    }
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
            .add_enabled(
                !squared,
                egui::Button::new(format!("{} Square the sides", icon::SQUARE_HALF)),
            )
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
                format!(
                    "{:.2} mm and {:.2} mm — uneven",
                    f.low_width(),
                    f.high_width()
                ),
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
            .on_hover_text(
                "A flat face square to the mould pull, the best surface on the ring for ornament.",
            )
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
/// One row of the profile picker: the style's own section drawn small, then
/// its name. A row is one click target.
fn style_row(ui: &mut egui::Ui, style: ProfileStyle, selected: bool) -> egui::Response {
    let mut p = ringdesign_core::BandProfile::default();
    p.apply_style(style);
    profile_row(ui, style.label(), &p, selected)
}

/// A picker row for any profile — preset or saved: its section drawn small
/// at a normalized size, then the name.
fn profile_row(
    ui: &mut egui::Ui,
    label: &str,
    profile: &ringdesign_core::BandProfile,
    selected: bool,
) -> egui::Response {
    let desired = egui::vec2(ui.available_width().max(150.0), 30.0);
    let (response, painter) = ui.allocate_painter(desired, egui::Sense::click());
    let rect = response.rect;
    let bg = if selected {
        theme::ACCENT.gamma_multiply(0.28)
    } else if response.hovered() {
        theme::ACCENT.gamma_multiply(0.12)
    } else {
        theme::BG.gamma_multiply(0.0)
    };
    painter.rect_filled(rect, 3.0, bg);

    let thumb = egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 3.0), egui::vec2(48.0, 24.0));
    let mut p = profile.clone();
    p.width_mm = 4.0;
    p.thickness_mm = 2.0;
    let loop_ = p.sample(8.55, 96);
    if !loop_.is_empty() {
        let (z_lo, z_hi) = loop_.z_range();
        let (r_lo, r_hi) = loop_
            .pts
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), q| (lo.min(q.r), hi.max(q.r)));
        let scale = ((thumb.width() as f64 - 4.0) / (z_hi - z_lo).max(1e-6))
            .min((thumb.height() as f64 - 4.0) / (r_hi - r_lo).max(1e-6));
        let (zm, rm) = (0.5 * (z_lo + z_hi), 0.5 * (r_lo + r_hi));
        let c = thumb.center();
        let pts: Vec<egui::Pos2> = loop_
            .pts
            .iter()
            .map(|q| {
                egui::pos2(c.x + ((q.z - zm) * scale) as f32, c.y - ((q.r - rm) * scale) as f32)
            })
            .collect();
        painter.add(egui::Shape::closed_line(
            pts,
            egui::Stroke::new(1.0, if selected { theme::ACCENT } else { theme::ACCENT_DIM }),
        ));
    }

    painter.text(
        egui::pos2(thumb.right() + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        if selected { theme::TEXT } else { theme::TEXT_DIM },
    );
    response
}

fn preview(app: &RingDesignerApp, ui: &mut egui::Ui) {
    let (response, painter) = ui.allocate_painter(egui::vec2(120.0, 90.0), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 3.0, theme::BG);

    let loop_ = app.design.profile.sample(app.design.inner_radius_mm(), 160);
    if loop_.is_empty() {
        return;
    }

    let (z_lo, z_hi) = loop_.z_range();
    let (r_lo, r_hi) = loop_.pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.r), hi.max(p.r))
    });
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

    let was = app.design.shank.kind;
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
    // Picking Signet from a standing start would otherwise land on whatever the
    // last style used, which is a head nobody chose.
    if app.design.shank.kind == ShankKind::Signet && was != ShankKind::Signet {
        let width = app.design.profile.width_mm;
        app.design.shank.apply_signet(width);
    }
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

    if matches!(app.design.shank.kind, ShankKind::Wave | ShankKind::Twist) {
        changed |= ui
            .add(egui::Slider::new(&mut app.design.shank.waves, 1..=6).text("Waves"))
            .on_hover_text("Waves per revolution. Integer, so the band closes on itself.")
            .changed();
    }

    if app.design.shank.kind == ShankKind::Signet {
        changed |= signet_head(app, ui);
    }

    if app.design.shank.kind == ShankKind::Keyframes {
        changed |= shank_keys(app, ui);
    }

    if changed {
        app.mark_dirty();
    }
}

/// Authored stations for the keyframed shank: a row per key, blended
/// smoothly and periodically by the modulation.
fn shank_keys(app: &mut RingDesignerApp, ui: &mut egui::Ui) -> bool {
    use ringdesign_core::profile::ShankKey;
    let mut changed = false;
    let keys = &mut app.design.shank.keys;
    let mut remove: Option<usize> = None;
    for (i, k) in keys.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.push_id(i, |ui| {
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut k.theta_deg)
                            .speed(0.5)
                            .range(0.0..=360.0)
                            .suffix(" deg"),
                    )
                    .on_hover_text("Where round the ring; 90 is the top")
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut k.width_scale)
                            .speed(0.01)
                            .range(0.3..=3.0)
                            .prefix("w "),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut k.thickness_scale)
                            .speed(0.01)
                            .range(0.3..=3.0)
                            .prefix("t "),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut k.crown_scale)
                            .speed(0.01)
                            .range(0.0..=2.5)
                            .prefix("c "),
                    )
                    .changed();
                if ui.small_button(icon::TRASH).clicked() {
                    remove = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove {
        keys.remove(i);
        changed = true;
    }
    if keys.len() < 16 && ui.button(format!("{} Add station", icon::PLUS)).clicked() {
        // New stations land on whichever preset angle is still free.
        let presets = [90.0, 270.0, 0.0, 180.0, 45.0, 135.0, 225.0, 315.0];
        let taken: Vec<f64> = keys.iter().map(|k| k.theta_deg).collect();
        let theta = presets
            .iter()
            .copied()
            .find(|p| taken.iter().all(|t| (t - p).abs() > 1.0))
            .unwrap_or(90.0);
        keys.push(ShankKey {
            theta_deg: theta,
            ..ShankKey::default()
        });
        changed = true;
    }
    hint(
        ui,
        "Width, thickness and crown at each station, blended smoothly around the ring.",
    );
    changed
}

/// The head is the band, so it lives here with the rest of the base geometry.
/// There is nothing on top of anything: the outline is the band's own plan
/// silhouette and the table is its crest.
fn signet_head(app: &mut RingDesignerApp, ui: &mut egui::Ui) -> bool {
    let mut changed = false;
    let band_width = app.design.profile.width_mm;
    let shank = &mut app.design.shank;

    ui.horizontal(|ui| {
        ui.label("Face");
        let before = shank.head.outline;
        // A custom face shows its imported name, not the bare "Custom".
        let shown = shank
            .custom_outline(before)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| before.label().to_string());
        let mut adopt: Option<ringdesign_core::CustomOutline> = None;
        egui::ComboBox::from_id_salt("signet_outline")
            .selected_text(shown)
            .width(140.0)
            .show_ui(ui, |ui| {
                for &o in SignetOutline::ALL {
                    changed |= ui
                        .selectable_value(&mut shank.head.outline, o, o.label())
                        .clicked();
                }
                // Plans already on this design, then the import library.
                for (i, c) in shank.custom_outlines.iter().enumerate() {
                    let o = SignetOutline::Custom(i as u8);
                    changed |= ui
                        .selectable_value(&mut shank.head.outline, o, &c.name)
                        .clicked();
                }
                let on_design: Vec<String> =
                    shank.custom_outlines.iter().map(|c| c.name.clone()).collect();
                let library: Vec<_> = ringdesign_core::library::list_outlines()
                    .into_iter()
                    .filter(|c| !on_design.contains(&c.name))
                    .collect();
                if !library.is_empty() {
                    ui.separator();
                    ui.label(
                        egui::RichText::new("From the outline library")
                            .small()
                            .color(theme::TEXT_DIM),
                    );
                    for c in library {
                        if ui.selectable_label(false, &c.name).clicked() {
                            adopt = Some(c);
                        }
                    }
                }
            });
        if let Some(c) = adopt {
            // Copied into the design, so the file stays self-contained.
            shank.head.outline = shank.adopt_outline(c);
            changed = true;
        }
        // A new shape wants its own proportions; the length is right there to
        // override if the ring wants a long cushion rather than a square one.
        // Deeply lobed imports also default onto the cut dome, where the
        // lobes read in the arris instead of corrugating the flank — the
        // slider below overrides it either way.
        if shank.head.outline != before {
            let aspect = shank.outline_aspect(shank.head.outline);
            shank.head.length_mm = (band_width.max(1.0) * aspect).clamp(2.0, 40.0);
            shank.head.dome = shank.suggest_dome(shank.head.outline);
            changed = true;
        }
    });
    let head = &mut app.design.shank.head;

    changed |= ui
        .add(
            egui::Slider::new(&mut head.length_mm, 3.0..=30.0)
                .fixed_decimals(1)
                .suffix(" mm")
                .text("Face length"),
        )
        .on_hover_text("Extent of the face around the ring. Across the band it is the Width.")
        .changed();

    changed |= ui
        .add(
            egui::Slider::new(&mut head.rise_mm, 0.0..=4.0)
                .fixed_decimals(2)
                .suffix(" mm")
                .text("Rise"),
        )
        .on_hover_text("How far the middle of the table stands above the band's crest.")
        .changed();

    changed |= ui
        .add(
            egui::Slider::new(&mut head.body_fair, 0.0..=1.0)
                .fixed_decimals(2)
                .text("Body fairing"),
        )
        .on_hover_text(
            "How far the body under the table rounds away from the face's outline. 0 extrudes \
             the face straight down to the finger, so a heart's dimple runs the whole depth of \
             the ring; 1 leaves the shape on the table and fairs everything beneath it.",
        )
        .changed();

    let auto = head.swell_deg.is_none();
    ui.horizontal(|ui| {
        let mut on = auto;
        if ui
            .checkbox(&mut on, "Auto")
            .on_hover_text("Take the swell from the head's own size, which is where it comes from.")
            .changed()
        {
            head.swell_deg = (!on).then_some(head.swell_deg.unwrap_or(90.0));
            changed = true;
        }
        let mut deg = head.swell_deg.unwrap_or(90.0);
        if ui
            .add_enabled(
                !on,
                egui::Slider::new(&mut deg, 20.0..=160.0)
                    .fixed_decimals(0)
                    .suffix("°")
                    .text("Swell"),
            )
            .on_hover_text(
                "Arc the band's width takes to come back to the shank. This is what a signet \
                 reads as from the side, and it runs two and a half times as far as the face.",
            )
            .changed()
        {
            head.swell_deg = Some(deg);
            changed = true;
        }
    });

    changed |= ui
        .add(
            egui::Slider::new(&mut head.shoulder_deg, 8.0..=80.0)
                .fixed_decimals(0)
                .suffix("°")
                .text("Shoulder"),
        )
        .on_hover_text("Arc the crest takes to fall from the head back to the shank.")
        .changed();

    changed |= ui
        .add(
            egui::Slider::new(&mut head.table_flat, 0.0..=1.0)
                .fixed_decimals(2)
                .text("Table flat"),
        )
        .on_hover_text(
            "1 is a true plane to engrave. Below that the head keeps the profile's crown.",
        )
        .changed();

    changed |= ui
        .add(
            egui::Slider::new(&mut head.dome, 0.0..=1.0)
                .fixed_decimals(2)
                .text("Cut dome"),
        )
        .on_hover_text(
            "1 cuts the face from a swollen dome: the band's plan ignores the \
             outline, the flank rounds as one dome, and the facet is where the \
             table plane slices it — no pinched corners, no prism walls. The \
             facet is exactly that cut, so concave outlines (heart, shield) \
             soften; keep those at 0.",
        )
        .changed();

    changed |= ui
        .add(
            egui::Slider::new(&mut head.loft, 0.0..=1.0)
                .fixed_decimals(2)
                .text("Lofted body"),
        )
        .on_hover_text(
            "1 builds the head the way the factory presets do: one loose loft from the \
             table's rim, through a body outline three millimetres under it, down to the \
             ring's equator silhouette. The flank bulges a few tenths under the table and \
             curls back toward the finger, and the shoulder is one smooth sheet to the \
             shank. 0 keeps the reference prism and its swell.",
        )
        .changed();
    if head.loft > 0.0 {
        changed |= ui
            .add(
                egui::Slider::new(&mut head.loft_frontal_mm, 0.0..=8.0)
                    .fixed_decimals(1)
                    .suffix(" mm")
                    .text("Body growth along"),
            )
            .on_hover_text(
                "How much wider than the table the body outline is along the ring, 3 mm \
                 under the table. A control row of the loft, so it shows as a bulge of a \
                 few tenths rather than a shelf.",
            )
            .changed();
        changed |= ui
            .add(
                egui::Slider::new(&mut head.loft_lateral_mm, 0.0..=8.0)
                    .fixed_decimals(1)
                    .suffix(" mm")
                    .text("Body growth across"),
            )
            .on_hover_text("The same growth across the band.")
            .changed();
    }

    changed |= ui
        .add(
            egui::Slider::new(&mut head.table_dome_mm, 0.0..=3.0)
                .fixed_decimals(2)
                .suffix(" mm")
                .text("Cab dome"),
        )
        .on_hover_text(
            "Dome standing on the table's centre. On a prism or cut-dome head a \
             parabolic cab; on a lofted head the factory presets' smooth table — the \
             loft starts at an apex this high and passes a 0.6-scaled outline at that \
             height, so a lobed plan reads as a lobed dome. A domed table also has \
             real draft everywhere a flat one has none.",
        )
        .changed();

    changed |= ui
        .add(
            egui::Slider::new(&mut head.rim_round_mm, 0.0..=1.5)
                .fixed_decimals(2)
                .suffix(" mm")
                .text("Rim round"),
        )
        .on_hover_text(
            "Rounding between the table and the head's walls — how hard the face \
             outline reads. The reference signets round theirs about 0.6 mm; the \
             outline is the one edge a signet has.",
        )
        .changed();

    changed |= ui
        .add(
            egui::Slider::new(&mut head.theta_deg, 0.0..=360.0)
                .suffix("°")
                .text("Around"),
        )
        .on_hover_text("Where the head sits round the ring. 90° is the top.")
        .changed();

    // --- Toi et moi: a second head on the same band -------------------------
    {
        let extras = &mut app.design.shank.extra_heads;
        let mut two = !extras.is_empty();
        if ui
            .checkbox(&mut two, "Second head (toi et moi)")
            .on_hover_text(
                "Two faces on one band. Each keeps its own plate; the swells union \
                 between them with a fillet, never a crease.",
            )
            .changed()
        {
            if two {
                let mut h = ringdesign_core::profile::SignetHead {
                    outline: ringdesign_core::field::SignetOutline::Round,
                    theta_deg: app.design.shank.head.theta_deg + 48.0,
                    ..Default::default()
                };
                h.fit_length_to(app.design.profile.width_mm * 0.8);
                extras.push(h);
            } else {
                extras.clear();
            }
            changed = true;
        }
        if let Some(h2) = app.design.shank.extra_heads.first_mut() {
            ui.indent("second_head", |ui| {
                let before = h2.outline;
                egui::ComboBox::from_id_salt("second_head_outline")
                    .selected_text(h2.outline.label())
                    .show_ui(ui, |ui| {
                        for &o in ringdesign_core::field::SignetOutline::ALL {
                            changed |= ui.selectable_value(&mut h2.outline, o, o.label()).clicked();
                        }
                    });
                if h2.outline != before {
                    h2.fit_length_to(h2.length_mm / before.head_aspect().max(0.1));
                }
                changed |= ui
                    .add(
                        egui::Slider::new(&mut h2.length_mm, 3.0..=30.0)
                            .fixed_decimals(1)
                            .suffix(" mm")
                            .text("Face length"),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::Slider::new(&mut h2.theta_deg, 0.0..=360.0)
                            .suffix("°")
                            .text("Around"),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::Slider::new(&mut h2.rise_mm, 0.0..=4.0)
                            .fixed_decimals(2)
                            .suffix(" mm")
                            .text("Rise"),
                    )
                    .changed();
            });
        }
    }
    let head = &mut app.design.shank.head;
    let _ = head;

    let inner_r = app.design.inner_radius_mm();
    let crest_r = app.design.reference_loop().crest_radius_mm;
    let sh = app.design.shank.clone();
    // Read behind the head, wherever the head happens to sit.
    let back = sh.head.theta_deg + 180.0;
    let shank_mm = app.design.profile.width_mm * sh.signet_width_frac(back, inner_r, crest_r, &app.design.profile);
    let corner = (crest_r + sh.head.rise_mm).hypot(sh.head.length_mm * 0.5) - crest_r;
    hint(
        ui,
        format!(
            "Face {:.1} x {:.1} mm on a {shank_mm:.1} mm shank. Width is the face across the \
             band, so set it to the head. The table's corners stand {corner:.2} mm proud of the \
             band where its middle stands {:.2} mm — that is what a plane does over a curve, and \
             it is the chunk a signet reads as.",
            sh.head.length_mm, app.design.profile.width_mm, sh.head.rise_mm,
        ),
    );

    if ui
        .button(format!("{} Square the head's sides", icon::SQUARE))
        .on_hover_text(
            "Drops the side draft and shrinks the edge fillet, which is what turns the head's \
             flanks into faces square to the mould pull — the best surface on the ring for \
             ornament.",
        )
        .clicked()
    {
        app.design.profile.flatten_sides();
        changed = true;
    }

    changed
}

// --- Casting ---------------------------------------------------------------

fn casting(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    use ringdesign_core::castability::CastProcess;
    let mut changed = false;
    let d = &mut app.design.draft;

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Process").color(crate::theme::TEXT_DIM));
        for &p in CastProcess::ALL {
            if ui
                .selectable_label(d.process == p, p.label())
                .on_hover_text(match p {
                    CastProcess::SandTwoPart => {
                        "Two-part sand: the verdict enforces the +/-Z pull — undercuts \
                         and drag gate castability."
                    }
                    CastProcess::LostWax => {
                        "Lost wax: the investment burns out of any surface, so the pull \
                         statistics report but never gate. Fill and detail still do. \
                         Generators switch to their free-form variants."
                    }
                })
                .clicked()
                && d.process != p
            {
                p.apply(d);
                changed = true;
            }
        }
    });

    if d.process == CastProcess::SandTwoPart {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Sand").color(crate::theme::TEXT_DIM));
            for p in ringdesign_core::castability::SandProcess::ALL {
                if ui
                    .button(p.label())
                    .on_hover_text("Set min draft, section and detail to this sand's numbers.")
                    .clicked()
                {
                    p.apply(d);
                    changed = true;
                }
            }
        });
    }

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
    hint(
        ui,
        "3° is a normal minimum for sand; below it a wall drags on the way out.",
    );

    changed |= ui
        .add(
            egui::Slider::new(&mut d.min_section_mm, 0.3..=2.0)
                .fixed_decimals(2)
                .suffix(" mm")
                .text("Min section"),
        )
        .changed();
    hint(ui, "Thinnest section the metal will reliably fill.");

    changed |= ui
        .add(
            egui::Slider::new(&mut d.min_detail_mm, 0.1..=1.0)
                .fixed_decimals(2)
                .suffix(" mm")
                .text("Min detail"),
        )
        .changed();
    hint(
        ui,
        "Smallest feature the sand reproduces; finer beads and cells cast as mush.",
    );

    if changed {
        app.mark_dirty();
    }
}

// --- Custom crown ----------------------------------------------------------

/// Points the "draw it" button seeds the curve with.
const SEED_POINTS: usize = 7;
/// Grab radius for a control point, pixels.
const GRAB_PX: f32 = 9.0;

/// The crown of a custom profile: either superellipse exponents, or a curve
/// drawn by hand.
fn crown(app: &mut RingDesignerApp, ui: &mut egui::Ui) -> bool {
    let mut changed = false;
    let drawn = app.design.profile.drop_curve.is_active();

    ui.horizontal(|ui| {
        if ui
            .selectable_label(!drawn, "Exponents")
            .on_hover_text("Superellipse d(x) = 1 - (1 - x^a)^(1/b).")
            .clicked()
            && drawn
        {
            app.design.profile.clear_drop_curve();
            changed = true;
        }
        if ui
            .selectable_label(drawn, format!("{} Draw the crown", icon::PENCIL_SIMPLE))
            .on_hover_text("Shape the drop from crest to edge by hand.")
            .clicked()
            && !drawn
        {
            app.design.profile.adopt_drop_curve(SEED_POINTS);
            changed = true;
        }
    });

    ui.add_space(3.0);
    if !app.design.profile.drop_curve.is_active() {
        return changed | exponents(app, ui);
    }
    changed | curve_canvas(app, ui)
}

fn exponents(app: &mut RingDesignerApp, ui: &mut egui::Ui) -> bool {
    let mut changed = false;
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
    changed
}

/// Draggable control points over the drop from crest to edge.
///
/// `x` runs left to right from the crest to the band edge and `d` runs top to
/// bottom from no drop to the full crown, so the picture is the shape the metal
/// takes — falling away to the right, as the section does.
fn curve_canvas(app: &mut RingDesignerApp, ui: &mut egui::Ui) -> bool {
    let mut changed = false;
    let id = ui.make_persistent_id("drop_curve_drag");
    let width = ui.available_width().clamp(160.0, 260.0);
    let (response, painter) =
        ui.allocate_painter(egui::vec2(width, 132.0), egui::Sense::click_and_drag());
    let rect = response.rect;
    painter.rect_filled(rect, 3.0, theme::BG);
    let plot = rect.shrink(9.0);

    let to_screen = |x: f64, d: f64| {
        egui::pos2(
            plot.left() + x as f32 * plot.width(),
            plot.top() + d as f32 * plot.height(),
        )
    };
    let to_curve = |p: egui::Pos2| {
        (
            ((p.x - plot.left()) / plot.width().max(1.0)).clamp(0.0, 1.0) as f64,
            ((p.y - plot.top()) / plot.height().max(1.0)).clamp(0.0, 1.0) as f64,
        )
    };

    for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let stroke = egui::Stroke::new(1.0, theme::GRID);
        painter.line_segment([to_screen(t, 0.0), to_screen(t, 1.0)], stroke);
        painter.line_segment([to_screen(0.0, t), to_screen(1.0, t)], stroke);
    }

    // --- Drag, add, remove ---
    let mut drag: Option<usize> = ui.memory(|m| m.data.get_temp(id)).flatten();
    let curve = app.design.profile.drop_curve;
    if response.drag_started() {
        drag = response.interact_pointer_pos().and_then(|p| {
            curve
                .points()
                .iter()
                .enumerate()
                .map(|(i, q)| (i, to_screen(q[0], q[1]).distance(p)))
                .filter(|(_, d)| *d <= GRAB_PX)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
        });
        ui.memory_mut(|m| m.data.insert_temp(id, drag));
    }
    if response.drag_stopped() {
        ui.memory_mut(|m| m.data.insert_temp(id, Option::<usize>::None));
    }
    if response.dragged() {
        if let (Some(i), Some(p)) = (drag, response.interact_pointer_pos()) {
            let (x, d) = to_curve(p);
            app.design.profile.drop_curve.set(i, x, d);
            changed = true;
        }
    }
    if response.double_clicked() {
        if let Some(p) = response.interact_pointer_pos() {
            let (x, d) = to_curve(p);
            app.design.profile.drop_curve.insert(x, d);
            changed = true;
        }
    }
    if response.secondary_clicked() {
        if let Some(p) = response.interact_pointer_pos() {
            let hit = curve
                .points()
                .iter()
                .enumerate()
                .map(|(i, q)| (i, to_screen(q[0], q[1]).distance(p)))
                .filter(|(_, d)| *d <= GRAB_PX)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((i, _)) = hit {
                app.design.profile.drop_curve.remove(i);
                changed = true;
            }
        }
    }

    // --- The curve as the sweep will read it ---
    let curve = app.design.profile.drop_curve;
    let line: Vec<egui::Pos2> = (0..=96)
        .map(|i| {
            let x = i as f64 / 96.0;
            to_screen(x, curve.eval(x))
        })
        .collect();
    let reversed = curve.worst_reversal() > 1e-6;
    let colour = if reversed { theme::BAD } else { theme::ACCENT };
    painter.add(egui::Shape::line(line, egui::Stroke::new(1.6, colour)));

    for (i, q) in curve.points().iter().enumerate() {
        let at = to_screen(q[0], q[1]);
        let end = i == 0 || i + 1 == curve.len();
        painter.circle_filled(at, if end { 2.6 } else { 3.4 }, colour);
        if !end {
            painter.circle_stroke(at, 3.4, egui::Stroke::new(1.0, theme::BG));
        }
    }

    let label = |at: egui::Pos2, text: &str, align| {
        painter.text(
            at,
            align,
            text,
            egui::FontId::proportional(9.0),
            theme::TEXT_DIM,
        )
    };
    label(
        rect.left_top() + egui::vec2(3.0, 1.0),
        "crest",
        egui::Align2::LEFT_TOP,
    );
    label(
        rect.right_bottom() - egui::vec2(3.0, 1.0),
        "edge",
        egui::Align2::RIGHT_BOTTOM,
    );

    response.on_hover_text(
        "Drag a point to reshape the crown. Double-click to add one, right-click to remove.",
    );

    let mut monotone = app.design.profile.drop_curve.monotone;
    if ui
        .checkbox(&mut monotone, "Cannot undercut")
        .on_hover_text(
            "Hold the drop so it never falls back. This is what makes any shape you can draw \
             castable; clear it only to model an undercut deliberately.",
        )
        .changed()
    {
        app.design.profile.drop_curve.monotone = monotone;
        // Re-running the constraint is what actually straightens an already
        // reversed curve; setting the flag alone leaves it as drawn.
        app.design.profile.drop_curve.sanitize();
        changed = true;
    }

    if reversed {
        ui.label(
            RichText::new(format!(
                "{} Leans back {:.0}% of the crown — this will lock in the sand.",
                icon::WARNING,
                curve.worst_reversal() * 100.0
            ))
            .small()
            .color(theme::BAD),
        );
    }

    changed
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
                app.export_params.refine = Some(RefineParams {
                    tolerance_mm: tol,
                    ..r
                });
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
