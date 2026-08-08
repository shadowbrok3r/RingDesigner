//! The layer stack: add, reorder, and edit every decorative element.

use egui_phosphor::regular as icon;
use ringdesign_core::field::{
    Blend, BorderLayer, BorderProfile, FieldContext, Layer, MilgrainLayer,
    SIDE_FACE_MIN_DRAFT_DEG, SeatPadLayer, SignetLayer, SignetOutline, Window,
};
use ringdesign_core::profile::{HEAD_SPAN_DEG, ShankKind};

/// Taper strength the head-shaping button applies.
const SIGNET_TAPER: f64 = 0.85;
/// Arc the head-shaping button gives the head, degrees.
const SIGNET_HEAD_ARC_DEG: f64 = HEAD_SPAN_DEG;
use ringdesign_core::tiling::TilingLayer;

use crate::app::RingDesignerApp;
use crate::pane::PaneKind;
use crate::theme;

/// A row button pressed during the list loop, applied once the loop is over.
enum Action {
    Select(usize),
    Move(usize, isize),
    Duplicate(usize),
    Delete(usize),
}

pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    ui.add_space(2.0);
    add_menu(app, ui);
    ui.add_space(2.0);

    list(app, ui);

    let selected = app.selected_layer.filter(|&i| i < app.design.layers.layers.len());
    match selected {
        Some(i) => {
            ui.add_space(4.0);
            ui.separator();
            editor(app, ui, i);
        }
        None if !app.design.layers.layers.is_empty() => {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Pick a layer to edit it.")
                    .small()
                    .color(theme::TEXT_DIM),
            );
        }
        None => {}
    }
    ui.add_space(6.0);
}

fn add_menu(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    ui.menu_button(format!("{} Add layer", icon::PLUS), |ui| {
        if ui.button(format!("{} Tiling", icon::GRID_FOUR)).clicked() {
            let ctx = app.design.field_context();
            let alpha = app
                .lib
                .names()
                .first()
                .cloned()
                .unwrap_or_else(|| "Rope".to_string());
            let layer = Layer::Tiling(TilingLayer::default_for(alpha.clone(), &ctx));
            app.add_layer(alpha, layer);
            ui.close();
        }
        if ui.button(format!("{} Border", icon::LINE_SEGMENTS)).clicked() {
            app.add_layer("Border", Layer::Border(BorderLayer::default()));
            ui.close();
        }
        if ui.button(format!("{} Gem Seat Pad", icon::DIAMOND)).clicked() {
            app.add_layer("Gem Seat Pad", Layer::SeatPad(SeatPadLayer::default()));
            ui.close();
        }
        if ui.button(format!("{} Milgrain", icon::CIRCLES_THREE)).clicked() {
            app.add_layer("Milgrain", Layer::Milgrain(MilgrainLayer::default()));
            ui.close();
        }
        if ui
            .button(format!("{} Signet", icon::SEAL))
            .on_hover_text("A raised flat table to hand-engrave.")
            .clicked()
        {
            let signet = SignetLayer::fitted_to(&app.design.field_context());
            app.add_layer("Signet", Layer::Signet(signet));
            ui.close();
        }
    });
}

fn kind_icon(layer: &Layer) -> &'static str {
    match layer {
        Layer::Tiling(_) => icon::GRID_FOUR,
        Layer::Border(_) => icon::LINE_SEGMENTS,
        Layer::SeatPad(_) => icon::DIAMOND,
        Layer::Milgrain(_) => icon::CIRCLES_THREE,
        Layer::Signet(_) => icon::SEAL,
    }
}

fn list(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    let n = app.design.layers.layers.len();
    if n == 0 {
        ui.label(
            egui::RichText::new("Bare band — no decoration yet.")
                .small()
                .color(theme::TEXT_DIM),
        );
        return;
    }

    let mut action: Option<Action> = None;
    let mut dirty = false;

    for i in 0..n {
        let selected = app.selected_layer == Some(i);
        let fill = if selected {
            theme::ACCENT_DIM.gamma_multiply(0.35)
        } else {
            egui::Color32::TRANSPARENT
        };
        egui::Frame::NONE
            .fill(fill)
            .inner_margin(egui::Margin::symmetric(4, 1))
            .corner_radius(3.0)
            .show(ui, |ui| {
                let e = &mut app.design.layers.layers[i];
                let kind = e.layer.kind_label();
                let glyph = kind_icon(&e.layer);
                ui.horizontal(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(3.0, 2.0);
                    ui.spacing_mut().item_spacing.x = 3.0;

                    dirty |= ui
                        .checkbox(&mut e.enabled, "")
                        .on_hover_text("Include this layer in the build")
                        .changed();

                    let row_h = ui.spacing().interact_size.y;
                    let kind_w = 60.0;
                    let name_w = (ui.available_width() - 100.0 - kind_w).max(48.0);
                    let left = egui::Layout::left_to_right(egui::Align::Center);

                    ui.allocate_ui_with_layout(egui::vec2(name_w, row_h), left, |ui| {
                        let mut text = egui::RichText::new(format!("{glyph}  {}", e.name));
                        if selected {
                            text = text.color(theme::ACCENT);
                        } else if !e.enabled {
                            text = text.color(theme::TEXT_DIM);
                        }
                        let label = egui::Label::new(text)
                            .truncate()
                            .selectable(false)
                            .sense(egui::Sense::click());
                        if ui.add(label).clicked() {
                            action = Some(Action::Select(i));
                        }
                    });
                    ui.allocate_ui_with_layout(egui::vec2(kind_w, row_h), left, |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(kind).small().color(theme::TEXT_DIM),
                            )
                            .truncate()
                            .selectable(false),
                        );
                    });

                    let up = egui::Button::new(icon::ARROW_UP);
                    if ui
                        .add_enabled(i > 0, up)
                        .on_hover_text("Move up (composites earlier)")
                        .clicked()
                    {
                        action = Some(Action::Move(i, -1));
                    }
                    let down = egui::Button::new(icon::ARROW_DOWN);
                    if ui
                        .add_enabled(i + 1 < n, down)
                        .on_hover_text("Move down (composites later)")
                        .clicked()
                    {
                        action = Some(Action::Move(i, 1));
                    }
                    if ui
                        .button(icon::COPY)
                        .on_hover_text("Duplicate this layer")
                        .clicked()
                    {
                        action = Some(Action::Duplicate(i));
                    }
                    if ui
                        .button(icon::TRASH)
                        .on_hover_text("Delete this layer")
                        .clicked()
                    {
                        action = Some(Action::Delete(i));
                    }
                });
            });
    }

    match action {
        Some(Action::Select(i)) => app.selected_layer = Some(i),
        Some(Action::Move(i, d)) => app.move_layer(i, d),
        Some(Action::Duplicate(i)) => app.duplicate_layer(i),
        Some(Action::Delete(i)) => app.remove_layer(i),
        None => {}
    }
    if dirty {
        app.mark_dirty();
    }
}

// --- Editors ---------------------------------------------------------------

fn editor(app: &mut RingDesignerApp, ui: &mut egui::Ui, i: usize) {
    let names = app.lib.names();
    let fctx = app.design.field_context();
    let alpha_name = match &app.design.layers.layers[i].layer {
        Layer::Tiling(t) => Some(t.alpha.clone()),
        _ => None,
    };
    let thumb = alpha_name
        .as_deref()
        .and_then(|name| app.thumbnail(ui.ctx(), name));

    let mut shape_head = false;
    let entry = &mut app.design.layers.layers[i];
    let mut dirty = ui
        .scope(|ui| {
            let mut dirty = false;
            ui.spacing_mut().slider_width = 104.0;
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "{} {}",
                    kind_icon(&entry.layer),
                    entry.layer.kind_label()
                ))
                .strong(),
            );

            dirty |= grid(ui, "layer_common", |ui| {
                let mut c = false;

                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut entry.name).desired_width(f32::INFINITY));
                ui.end_row();

                ui.label("Blend");
                egui::ComboBox::from_id_salt("layer_blend")
                    .selected_text(entry.blend.label())
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        for &b in Blend::ALL {
                            c |= ui.selectable_value(&mut entry.blend, b, b.label()).clicked();
                        }
                    })
                    .response
                    .on_hover_text(
                        "How this layer stacks onto everything beneath it. Carve subtracts.",
                    );
                ui.end_row();

                ui.label("Opacity");
                c |= ui
                    .add(egui::Slider::new(&mut entry.opacity, 0.0..=1.0).fixed_decimals(2))
                    .on_hover_text("Scales this layer's relief before it is blended")
                    .changed();
                ui.end_row();

                c
            });

            dirty |= window_controls(ui, &mut entry.window);

            ui.add_space(4.0);
            dirty |= match &mut entry.layer {
                Layer::Tiling(t) => tiling(ui, t, &fctx, &names, thumb),
                Layer::Border(b) => border(ui, b, &fctx),
                Layer::SeatPad(p) => seat_pad(ui, p, &fctx),
                Layer::Milgrain(m) => milgrain(ui, m, &fctx),
                Layer::Signet(s) => signet(ui, s, &fctx, &mut shape_head),
            };
            dirty
        })
        .inner;
    if shape_head {
        if let Layer::Signet(s) = &mut app.design.layers.layers[i].layer {
            let outline = s.outline;
            s.fill_head(&fctx);
            app.design.shank.kind = ShankKind::Signet;
            app.design.shank.amount = SIGNET_TAPER;
            app.design.shank.head_span_deg = SIGNET_HEAD_ARC_DEG;
            app.design.shank.head_shape_a = outline.exponent();
        }
        dirty = true;
    }
    if dirty {
        app.mark_dirty();
    }
}

fn tiling(
    ui: &mut egui::Ui,
    t: &mut TilingLayer,
    fctx: &FieldContext,
    names: &[String],
    thumb: Option<egui::TextureId>,
) -> bool {
    let mut c = false;
    let v_max = fctx.band_v_len_mm.max(0.5);

    ui.horizontal(|ui| {
        if let Some(id) = thumb {
            let tex = egui::load::SizedTexture::new(id, egui::vec2(48.0, 48.0));
            ui.add(egui::Image::from_texture(tex).corner_radius(3.0));
        }
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Alpha").small().color(theme::TEXT_DIM));
            let width = (ui.available_width() - 4.0).clamp(90.0, 190.0);
            egui::ComboBox::from_id_salt("tiling_alpha")
                .selected_text(t.alpha.clone())
                .width(width)
                .show_ui(ui, |ui| {
                    for name in names {
                        c |= ui
                            .selectable_value(&mut t.alpha, name.clone(), name.as_str())
                            .clicked();
                    }
                });
        });
    });

    ui.add_space(3.0);
    c |= side_face_fit(ui, t, fctx);

    ui.add_space(3.0);
    c |= grid(ui, "tiling_lattice", |ui| {
        let mut c = false;

        ui.label("Around");
        c |= ui
            .add(egui::DragValue::new(&mut t.repeats_around).speed(0.15).range(1..=400))
            .on_hover_text(
                "Tiles around the circumference. A whole count is what makes the pattern \
                 close on itself with no seam at the joint.",
            )
            .changed();
        ui.end_row();

        ui.label("Rows");
        c |= ui
            .add(egui::DragValue::new(&mut t.rows).speed(0.05).range(1..=32))
            .on_hover_text("Tile rows stacked across the band")
            .changed();
        ui.end_row();

        ui.label("Centre");
        c |= ui
            .add(
                egui::DragValue::new(&mut t.v_center_mm)
                    .speed(0.02)
                    .range((-0.25 * v_max)..=(1.25 * v_max))
                    .suffix(" mm"),
            )
            .on_hover_text(format!(
                "Where the tiled band sits across the section. 0 is one bore edge, \
                 {:.2} mm the other, {:.2} mm the outer crest.",
                v_max, fctx.crest_v_mm
            ))
            .changed();
        ui.end_row();

        ui.label("Span");
        c |= ui
            .add(
                egui::DragValue::new(&mut t.v_span_mm)
                    .speed(0.02)
                    .range(0.05..=(1.5 * v_max))
                    .suffix(" mm"),
            )
            .on_hover_text("How far across the section the tiling reaches")
            .changed();
        ui.end_row();

        ui.label("Rotation");
        c |= ui
            .add(egui::Slider::new(&mut t.rotation_deg, -180.0..=180.0).suffix("°"))
            .on_hover_text("Turns the alpha inside its cell")
            .changed();
        ui.end_row();

        ui.label("Offset");
        ui.horizontal(|ui| {
            c |= ui
                .add(
                    egui::DragValue::new(&mut t.offset_u)
                        .speed(0.004)
                        .range(0.0..=1.0)
                        .prefix("u ")
                        .fixed_decimals(3),
                )
                .on_hover_text("Shifts the lattice around the ring, in cells")
                .changed();
            c |= ui
                .add(
                    egui::DragValue::new(&mut t.offset_v)
                        .speed(0.004)
                        .range(0.0..=1.0)
                        .prefix("v ")
                        .fixed_decimals(3),
                )
                .on_hover_text("Shifts the lattice across the band, in cells")
                .changed();
        });
        ui.end_row();

        ui.label("Relief");
        c |= ui
            .add(
                egui::DragValue::new(&mut t.height_mm)
                    .speed(0.005)
                    .range(0.0..=2.0)
                    .suffix(" mm"),
            )
            .on_hover_text("Metal raised where the alpha is white")
            .changed();
        ui.end_row();

        ui.label("Gap");
        c |= ui
            .add(
                egui::DragValue::new(&mut t.gap_mm)
                    .speed(0.01)
                    .range(0.0..=5.0)
                    .suffix(" mm"),
            )
            .on_hover_text("Flat land left between neighbouring tiles")
            .changed();
        ui.end_row();

        ui.label("Stagger");
        c |= ui
            .add(egui::Slider::new(&mut t.stagger, 0.0..=1.0).fixed_decimals(2))
            .on_hover_text("Brick-style shift applied per row, in cells")
            .changed();
        ui.end_row();

        ui.label("Flip alternate");
        ui.horizontal(|ui| {
            c |= ui
                .checkbox(&mut t.mirror_alternate_u, "Around")
                .on_hover_text("Mirror every other column")
                .changed();
            c |= ui
                .checkbox(&mut t.mirror_alternate_v, "Across")
                .on_hover_text("Mirror every other row")
                .changed();
        });
        ui.end_row();

        ui.label("Both sides");
        c |= ui
            .checkbox(&mut t.mirror_v, "Mirror across the band")
            .on_hover_text(
                "Repeat this band mirrored about the middle of the section, so one layer \
                 covers both side faces.",
            )
            .changed();
        ui.end_row();

        c
    });

    ui.add_space(3.0);
    ui.label(
        egui::RichText::new("Response")
            .small()
            .color(theme::TEXT_DIM),
    );
    c |= grid(ui, "tiling_response", |ui| {
        let mut c = false;

        ui.label("Contrast");
        c |= ui
            .add(egui::DragValue::new(&mut t.contrast).speed(0.01).range(0.1..=4.0))
            .on_hover_text("Gamma on the alpha. Above 1 deepens, below 1 flattens.")
            .changed();
        ui.end_row();

        ui.label("Bias");
        c |= ui
            .add(egui::Slider::new(&mut t.bias, -1.0..=1.0).fixed_decimals(2))
            .on_hover_text("Lifts or drops the alpha before it is shaped")
            .changed();
        ui.end_row();

        ui.label("Feather");
        c |= ui
            .add(
                egui::DragValue::new(&mut t.feather_mm)
                    .speed(0.01)
                    .range(0.0..=3.0)
                    .suffix(" mm"),
            )
            .on_hover_text("Fades the tiling out over this distance at the band edges")
            .changed();
        ui.end_row();

        ui.label("");
        ui.horizontal(|ui| {
            c |= ui
                .checkbox(&mut t.invert, "Invert")
                .on_hover_text("Swap raised and recessed")
                .changed();
            c |= ui
                .checkbox(&mut t.continuous, "Continuous")
                .on_hover_text("Sample the alpha wrapped, so a seamless source flows across cells")
                .changed();
        });
        ui.end_row();

        c
    });

    ui.add_space(3.0);
    ui.label(
        egui::RichText::new(format!(
            "{} The {} tab drags cells and orientation directly.",
            icon::INFO,
            PaneKind::Unrolled.label()
        ))
        .small()
        .color(theme::TEXT_DIM),
    );

    c
}

/// Limit a layer to part of the ring, so ornament can flank a signet head
/// instead of running over it.
fn window_controls(ui: &mut egui::Ui, w: &mut Window) -> bool {
    let mut c = false;
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        c |= ui
            .checkbox(&mut w.enabled, "Limit to an arc")
            .on_hover_text("Confine this layer to part of the ring instead of all the way round")
            .changed();
        if w.enabled {
            c |= ui
                .checkbox(&mut w.invert, "Outside")
                .on_hover_text("Keep the layer everywhere but the arc — use it to clear a signet head")
                .changed();
        }
    });
    if !w.enabled {
        return c;
    }
    c |= grid(ui, "layer_window", |ui| {
        let mut c = false;

        ui.label("Centre");
        c |= ui
            .add(egui::Slider::new(&mut w.theta_deg, 0.0..=360.0).suffix("°"))
            .on_hover_text("Ring angle the arc is centred on. 90 is the top.")
            .changed();
        ui.end_row();

        ui.label("Span");
        c |= ui
            .add(egui::Slider::new(&mut w.span_deg, 0.0..=360.0).suffix("°"))
            .on_hover_text("Arc held at full strength")
            .changed();
        ui.end_row();

        ui.label("Fade");
        c |= ui
            .add(egui::Slider::new(&mut w.fade_deg, 0.0..=90.0).suffix("°"))
            .on_hover_text(
                "Falloff at each end. A hard edge raises a wall the mould has to clear, \
                 so leave some fade.",
            )
            .changed();
        ui.end_row();

        c
    });
    if w.fade_deg < 1.0 && w.span_deg > 0.0 {
        ui.label(
            egui::RichText::new(format!(
                "{} No fade leaves a vertical wall at each end of the arc.",
                icon::WARNING
            ))
            .small()
            .color(theme::WARN),
        );
    }
    c
}

/// Snap the tiling onto the faces square to the mould pull, which are the only
/// ones that hold relief deeper than a few tenths.
fn side_face_fit(ui: &mut egui::Ui, t: &mut TilingLayer, fctx: &FieldContext) -> bool {
    let mut c = false;
    let faces = fctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG);
    ui.horizontal(|ui| {
        let enabled = faces.is_some();
        if ui
            .add_enabled(enabled, egui::Button::new(format!("{} Fit to sides", icon::ARROWS_OUT_LINE_VERTICAL)))
            .on_hover_text(
                "Sit the tiling on the band's side faces with square, unstretched cells. \
                 Relief there pulls straight out of the sand.",
            )
            .on_disabled_hover_text(
                "This profile has no face square to the mould pull. Square the sides on the \
                 Design tab, or add an edge flange.",
            )
            .clicked()
        {
            c |= t.fit_to_side_faces(fctx, SIDE_FACE_MIN_DRAFT_DEG);
        }
        if ui
            .button(format!("{} Square cells", icon::SQUARE))
            .on_hover_text("Set the tile count so each cell is as wide as it is tall")
            .clicked()
        {
            t.repeats_around = t.repeats_for_square_cells(fctx);
            c = true;
        }
    });
    let (msg, colour) = match faces {
        Some(f) if f.is_even() => (
            format!(
                "{} Side faces {:.2} mm each edge, {:.0} deg draft or better.",
                icon::CHECK_CIRCLE,
                f.low_width().min(f.high_width()),
                SIDE_FACE_MIN_DRAFT_DEG
            ),
            theme::GOOD,
        ),
        Some(f) => (
            format!(
                "{} Side faces are uneven: {:.2} mm and {:.2} mm. Fitting takes the wider one only.",
                icon::WARNING,
                f.low_width(),
                f.high_width()
            ),
            theme::WARN,
        ),
        None => (
            format!(
                "{} All dome, no side face. Relief here undercuts above about 0.15 mm.",
                icon::WARNING
            ),
            theme::WARN,
        ),
    };
    ui.label(egui::RichText::new(msg).small().color(colour));
    c
}

fn border(ui: &mut egui::Ui, b: &mut BorderLayer, fctx: &FieldContext) -> bool {
    let v_max = fctx.band_v_len_mm.max(0.5);
    let c = grid(ui, "border_grid", |ui| {
        let mut c = false;

        ui.label("Across");
        c |= ui
            .add(
                egui::DragValue::new(&mut b.v_mm)
                    .speed(0.02)
                    .range(0.0..=v_max)
                    .suffix(" mm"),
            )
            .on_hover_text(format!(
                "Centre of the rail across the section. 0 is a bore edge, {:.2} mm the crest.",
                fctx.crest_v_mm
            ))
            .changed();
        ui.end_row();

        ui.label("Width");
        c |= ui
            .add(
                egui::DragValue::new(&mut b.width_mm)
                    .speed(0.01)
                    .range(0.05..=6.0)
                    .suffix(" mm"),
            )
            .changed();
        ui.end_row();

        ui.label("Height");
        c |= ui
            .add(
                egui::DragValue::new(&mut b.height_mm)
                    .speed(0.005)
                    .range(0.0..=2.0)
                    .suffix(" mm"),
            )
            .changed();
        ui.end_row();

        ui.label("Profile");
        egui::ComboBox::from_id_salt("border_profile")
            .selected_text(b.profile.label())
            .width(150.0)
            .show_ui(ui, |ui| {
                for &p in BorderProfile::ALL {
                    c |= ui.selectable_value(&mut b.profile, p, p.label()).clicked();
                }
            });
        ui.end_row();

        if b.profile == BorderProfile::Rope {
            ui.label("Twists");
            c |= ui
                .add(egui::DragValue::new(&mut b.rope_twists).speed(0.3).range(1..=400))
                .on_hover_text("Twists per revolution. A whole count keeps the rope seamless.")
                .changed();
            ui.end_row();
        }

        ui.label("");
        c |= ui
            .checkbox(&mut b.mirror, "Mirror to the far side")
            .on_hover_text(format!(
                "Also runs a rail at {:.2} mm",
                (v_max - b.v_mm).max(0.0)
            ))
            .changed();
        ui.end_row();

        c
    });

    if b.profile == BorderProfile::Knife {
        ui.label(
            egui::RichText::new(format!(
                "{} A knife rail comes to a feather edge — it may not fill.",
                icon::WARNING
            ))
            .small()
            .color(theme::WARN),
        );
    }
    c
}

fn seat_pad(ui: &mut egui::Ui, p: &mut SeatPadLayer, fctx: &FieldContext) -> bool {
    let v_max = fctx.band_v_len_mm.max(0.5);
    let c = grid(ui, "seat_pad_grid", |ui| {
        let mut c = false;

        ui.label("Around");
        c |= ui
            .add(egui::Slider::new(&mut p.theta_deg, 0.0..=360.0).suffix("°"))
            .on_hover_text("Where the pad sits round the ring. 90° is the top.")
            .changed();
        ui.end_row();

        ui.label("Across");
        c |= ui
            .add(
                egui::DragValue::new(&mut p.v_mm)
                    .speed(0.02)
                    .range(0.0..=v_max)
                    .suffix(" mm"),
            )
            .on_hover_text(format!(
                "Position across the section. {:.2} mm is the outer crest.",
                fctx.crest_v_mm
            ))
            .changed();
        ui.end_row();

        ui.label("Diameter");
        c |= ui
            .add(
                egui::DragValue::new(&mut p.diameter_mm)
                    .speed(0.02)
                    .range(0.5..=20.0)
                    .suffix(" mm"),
            )
            .changed();
        ui.end_row();

        ui.label("Height");
        c |= ui
            .add(
                egui::DragValue::new(&mut p.height_mm)
                    .speed(0.01)
                    .range(0.0..=5.0)
                    .suffix(" mm"),
            )
            .on_hover_text("Stock proud of the band for cutting the seat into")
            .changed();
        ui.end_row();

        ui.label("Crown");
        c |= ui
            .add(egui::Slider::new(&mut p.crown, 0.0..=1.0).fixed_decimals(2))
            .on_hover_text("0 is a flat-topped boss, 1 a full dome")
            .changed();
        ui.end_row();

        ui.label("Blend");
        c |= ui
            .add(
                egui::DragValue::new(&mut p.blend_mm)
                    .speed(0.01)
                    .range(0.0..=4.0)
                    .suffix(" mm"),
            )
            .on_hover_text("Skirt fairing the pad down into the band")
            .changed();
        ui.end_row();

        c
    });

    ui.label(
        egui::RichText::new(format!(
            "{} Seats a stone up to {:.2} mm",
            icon::DIAMOND,
            p.suggested_stone_mm()
        ))
        .small()
        .color(theme::INFO),
    );

    if p.crown < 0.15 && p.blend_mm < 0.05 {
        ui.label(
            egui::RichText::new(format!(
                "{} Flat top with no skirt is a straight wall — it will lock in the sand. \
                 Raise the crown or widen the blend.",
                icon::WARNING
            ))
            .small()
            .color(theme::WARN),
        );
    }

    c
}

fn milgrain(ui: &mut egui::Ui, m: &mut MilgrainLayer, fctx: &FieldContext) -> bool {
    let v_max = fctx.band_v_len_mm.max(0.5);
    let c = grid(ui, "milgrain_grid", |ui| {
        let mut c = false;

        ui.label("Across");
        c |= ui
            .add(
                egui::DragValue::new(&mut m.v_mm)
                    .speed(0.02)
                    .range(0.0..=v_max)
                    .suffix(" mm"),
            )
            .on_hover_text(format!(
                "Centre of the bead run. {:.2} mm is the outer crest.",
                fctx.crest_v_mm
            ))
            .changed();
        ui.end_row();

        ui.label("Bead");
        c |= ui
            .add(
                egui::DragValue::new(&mut m.bead_diameter_mm)
                    .speed(0.005)
                    .range(0.05..=2.0)
                    .suffix(" mm"),
            )
            .changed();
        ui.end_row();

        ui.label("Count");
        c |= ui
            .add(egui::DragValue::new(&mut m.beads_around).speed(0.5).range(3..=800))
            .on_hover_text("Beads around the ring. A whole count closes the run on itself.")
            .changed();
        ui.end_row();

        ui.label("Height");
        c |= ui
            .add(
                egui::DragValue::new(&mut m.height_mm)
                    .speed(0.005)
                    .range(0.0..=1.5)
                    .suffix(" mm"),
            )
            .changed();
        ui.end_row();

        ui.label("");
        c |= ui
            .checkbox(&mut m.mirror, "Mirror to the far side")
            .on_hover_text(format!(
                "Also runs beads at {:.2} mm",
                (v_max - m.v_mm).max(0.0)
            ))
            .changed();
        ui.end_row();

        c
    });

    let pitch = fctx.circumference_mm / m.beads_around.max(1) as f64;
    ui.label(
        egui::RichText::new(format!(
            "{:.3} mm pitch, {:.2} mm between beads",
            pitch,
            (pitch - m.bead_diameter_mm).max(0.0)
        ))
        .small()
        .color(theme::TEXT_DIM),
    );

    c
}

/// Signet editor; sets `cathedral` when the shank button is pressed.
fn signet(
    ui: &mut egui::Ui,
    s: &mut SignetLayer,
    fctx: &FieldContext,
    shape_head: &mut bool,
) -> bool {
    let v_max = fctx.band_v_len_mm.max(0.5);
    let c = grid(ui, "signet_grid", |ui| {
        let mut c = false;

        ui.label("Around");
        c |= ui
            .add(egui::Slider::new(&mut s.theta_deg, 0.0..=360.0).suffix("°"))
            .on_hover_text("Where the table sits round the ring. 90° is the top.")
            .changed();
        ui.end_row();

        ui.label("Across");
        c |= ui
            .add(
                egui::DragValue::new(&mut s.v_mm)
                    .speed(0.02)
                    .range(0.0..=v_max)
                    .suffix(" mm"),
            )
            .on_hover_text(format!(
                "Position across the section. {:.2} mm is the outer crest.",
                fctx.crest_v_mm
            ))
            .changed();
        ui.end_row();

        ui.label("Outline");
        egui::ComboBox::from_id_salt("signet_outline")
            .selected_text(s.outline.label())
            .width(150.0)
            .show_ui(ui, |ui| {
                for &o in SignetOutline::ALL {
                    c |= ui.selectable_value(&mut s.outline, o, o.label()).clicked();
                }
            });
        ui.end_row();

        ui.label("Length");
        c |= ui
            .add(
                egui::DragValue::new(&mut s.length_mm)
                    .speed(0.05)
                    .range(1.0..=40.0)
                    .suffix(" mm"),
            )
            .on_hover_text("Extent around the ring")
            .changed();
        ui.end_row();

        ui.label("Width");
        c |= ui
            .add(
                egui::DragValue::new(&mut s.width_mm)
                    .speed(0.05)
                    .range(1.0..=40.0)
                    .suffix(" mm"),
            )
            .on_hover_text("Extent across the band")
            .changed();
        ui.end_row();

        ui.label("Height");
        c |= ui
            .add(
                egui::DragValue::new(&mut s.height_mm)
                    .speed(0.02)
                    .range(0.0..=8.0)
                    .suffix(" mm"),
            )
            .on_hover_text("Table above the band")
            .changed();
        ui.end_row();

        ui.label("Top flat");
        c |= ui
            .add(egui::Slider::new(&mut s.top_flat, 0.0..=1.0).fixed_decimals(2))
            .on_hover_text("Fraction of the face left dead flat for the graver; the rest rolls off")
            .changed();
        ui.end_row();

        ui.label("Shoulder");
        c |= ui
            .add(
                egui::DragValue::new(&mut s.shoulder_mm)
                    .speed(0.02)
                    .range(0.0..=8.0)
                    .suffix(" mm"),
            )
            .on_hover_text("Fairing from the table down into the band")
            .changed();
        ui.end_row();

        ui.label("Rotation");
        c |= ui
            .add(egui::Slider::new(&mut s.rotation_deg, -180.0..=180.0).suffix("°"))
            .on_hover_text("Turns the outline within the band")
            .changed();
        ui.end_row();

        c
    });

    ui.label(
        egui::RichText::new(format!(
            "{} {:.1} mm² to engrave",
            icon::PENCIL,
            s.engraving_area_mm2()
        ))
        .small()
        .color(theme::INFO),
    );

    if s.shoulder_mm < 0.4 {
        ui.label(
            egui::RichText::new(format!(
                "{} A shoulder under 0.4 mm stands the table on a near-vertical wall — it will \
                 drag in the sand. Widen the shoulder.",
                icon::WARNING
            ))
            .small()
            .color(theme::WARN),
        );
    }

    ui.add_space(3.0);
    ui.label(
        egui::RichText::new(
            "A signet is a narrow shank swelling into a broad head — the band's own width \
             is the head outline.",
        )
        .small()
        .color(theme::TEXT_DIM),
    );
    if ui
        .button(format!("{} Shape the head", icon::WAVE_SINE))
        .on_hover_text(
            "Sets the shank to Signet, matches the head outline to this table's shape, and \
             grows the table to fill it.",
        )
        .clicked()
    {
        *shape_head = true;
    }

    c
}

/// Two-column parameter grid; the closure reports whether anything changed.
fn grid(ui: &mut egui::Ui, id: &str, add: impl FnOnce(&mut egui::Ui) -> bool) -> bool {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, add)
        .inner
}
