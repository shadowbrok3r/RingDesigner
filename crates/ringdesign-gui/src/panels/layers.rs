//! The layer stack: add, reorder, and edit every decorative element.

use egui_phosphor::regular as icon;
use ringdesign_core::curve::{CurveLayer, MAX_CURVE_POINTS, WireProfile};
use ringdesign_core::field::{
    Blend, BorderLayer, BorderProfile, DecalLayer, FieldContext, FluteProfile, FlutesLayer,
    GroupLayer, Layer, MAX_DECALS, MilgrainLayer, Remap, SIDE_FACE_MIN_DRAFT_DEG, SeatPadLayer,
    SeatRunLayer, SideFacePick, SignetLayer, SignetOutline, VGate, Window,
};
use ringdesign_core::tiling::TilingLayer;

use crate::app::RingDesignerApp;
use crate::pane::PaneKind;
use crate::theme;

/// A row button pressed during the list loop, applied once the loop is over.
enum Action {
    Select(usize),
    Move(usize, isize),
    MoveTo(usize, usize),
    Solo(usize),
    Duplicate(usize),
    Delete(usize),
}

/// A group-editor button, applied after the entry borrow ends.
enum GroupEdit {
    AdoptNext,
    MoveOut(usize),
}

pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    ui.add_space(2.0);
    add_menu(app, ui);
    pave_window(app, ui);
    ui.add_space(2.0);

    list(app, ui);

    let selected = app
        .selected_layer
        .filter(|&i| i < app.design.layers.layers.len());
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
        if ui
            .button(format!("{} Auto pavé…", icon::SPARKLE))
            .on_hover_text(
                "Pack an arc with stone seats — editable pads in a group, rows wrap-exact \
                 around the ring, gypsy mounds because those measure 0.000% on curved ground.",
            )
            .clicked()
        {
            app.pave_open = true;
            ui.close();
        }
        if ui.button(format!("{} Milgrain", icon::CIRCLES_THREE)).clicked() {
            app.add_layer("Milgrain", Layer::Milgrain(MilgrainLayer::default()));
            ui.close();
        }
        ui.menu_button(format!("{} Curve", icon::WAVE_SINE), |ui| {
            let ctx = app.design.field_context();
            // A wire lands on a side face when the profile has one: relief
            // there pulls straight out; across the crown it undercuts on its
            // crest-side flank.
            let side = ctx.side_faces_std().and_then(|sf| sf.wider());
            let mut place = |app: &mut RingDesignerApp, name: &str, mut l: CurveLayer| {
                if let Some((lo, hi)) = side {
                    l.retarget_v(0.5 * (lo + hi), (hi - lo) * 0.3);
                    app.add_layer(name, Layer::Curve(l));
                    if let Some(e) = app.design.layers.layers.last_mut() {
                        e.window.v_gate = VGate::SideFaces(SideFacePick::Wider);
                    }
                } else {
                    app.add_layer(name, Layer::Curve(l));
                }
            };
            if ui.button("S-scroll").clicked() {
                place(app, "S-scroll", CurveLayer::preset_scroll(&ctx));
                ui.close();
            }
            if ui.button("Running vine").clicked() {
                place(app, "Vine", CurveLayer::preset_vine(&ctx));
                ui.close();
            }
            if ui.button("Wavy rail").clicked() {
                place(app, "Wavy rail", CurveLayer::preset_wave_rail(&ctx));
                ui.close();
            }
        });
        ui.menu_button(format!("{} Flutes", icon::ROWS), |ui| {
            if ui
                .button("Reeded (raised)")
                .on_hover_text("Coin-edge reeding standing off the band")
                .clicked()
            {
                app.add_layer("Reeding", Layer::Flutes(FlutesLayer::default()));
                ui.close();
            }
            if ui
                .button("Fluted (carved)")
                .on_hover_text("Grooves cut into the band — the entry blends with Carve")
                .clicked()
            {
                app.add_layer("Flutes", Layer::Flutes(FlutesLayer::default()));
                if let Some(e) = app.design.layers.layers.last_mut() {
                    e.blend = Blend::Subtract;
                }
                ui.close();
            }
        });
        if ui
            .button(format!("{} Seat run", icon::CIRCLES_FOUR))
            .on_hover_text("A row of identical stone seats — eternity stock. Window it for a half row.")
            .clicked()
        {
            let ctx = app.design.field_context();
            let mut run = SeatRunLayer::default();
            run.seat.v_mm = ctx.crest_v_mm;
            run.solve_spacing(&ctx);
            app.add_layer("Eternity row", Layer::SeatRun(run));
            ui.close();
        }
        if ui
            .button(format!("{} Openwork", icon::EXCLUDE))
            .on_hover_text(
                "Carve the mask's ink toward a floor over the finger hole — the pierced                  look, with drafted walls. Lands on the wider side face.",
            )
            .clicked()
        {
            let ctx = app.design.field_context();
            let mut t = ringdesign_core::tiling::TilingLayer::default_for("Beads", &ctx);
            let on_side = t.fit_to_side_faces(&ctx, ringdesign_core::field::SIDE_FACE_MIN_DRAFT_DEG);
            t.repeats_around = 9;
            t.height_mm = 1.0;
            t.edge_mm = 0.35;
            t.feather_mm = 0.0;
            if let Some(src) = app.lib.get("Beads").cloned() {
                app.library_mut().insert(src.signed_distance_px());
            }
            let o = ringdesign_core::field::OpenworkLayer { tiling: t, depth_mm: 1.2, keep_mm: 0.8 };
            app.add_layer("Openwork", Layer::Openwork(o));
            if let Some(e) = app.design.layers.layers.last_mut() {
                e.blend = ringdesign_core::field::Blend::Add;
                if on_side {
                    e.window.v_gate = VGate::SideFaces(SideFacePick::Wider);
                }
            }
            ui.close();
        }
        if ui
            .button(format!("{} Channel set", icon::MINUS_SQUARE))
            .on_hover_text(
                "Two rails and a recessed channel on the wider side face — the one place \
                 a channel's walls are parallel to the pull. Wants a thick squared band; \
                 stones set at the bench.",
            )
            .clicked()
        {
            let gem = ringdesign_core::gem::Gem::calibrated(ringdesign_core::gem::GemCut::Round, 1.5);
            match ringdesign_core::pave::channel_set(&app.design, gem, 0.6) {
                Some(entry) => {
                    let name = entry.name.clone();
                    app.design.layers.layers.push(entry);
                    app.selected_layer = Some(app.design.layers.layers.len() - 1);
                    app.mark_dirty();
                    app.set_status(format!("Added {name}"));
                }
                None => app.set_status(
                    "No side face wide enough for stone + rails — square the sides and thicken the band (a 1.5 mm stone wants ~3 mm of face)",
                ),
            }
            ui.close();
        }
        if ui
            .button(format!("{} Halo", icon::CIRCLE))
            .on_hover_text(
                "A centre stone on a domed plate ringed by bead-set melee. Casts as a \
                 clean plate — the melee ring rides it as bench-set markers, because a \
                 ring of proud mounds is the two-flange valley. Wants a wide band.",
            )
            .clicked()
        {
            let spec = ringdesign_core::pave::HaloSpec::default();
            match ringdesign_core::pave::halo(&app.design, &spec) {
                Some((entry, n)) => {
                    let name = entry.name.clone();
                    app.design.layers.layers.push(entry);
                    app.selected_layer = Some(app.design.layers.layers.len() - 1);
                    app.mark_dirty();
                    app.set_status(format!("Added {name} ({n} accents)"));
                }
                None => app.set_status(
                    "The plate does not fit this band — widen it (a 5 mm centre with a melee ring wants a band ~11 mm wide)",
                ),
            }
            ui.close();
        }
        if ui.button(format!("{} Decals", icon::STAMP)).clicked() {
            let alpha = app.lib.names().first().cloned().unwrap_or_else(|| "Rope".to_string());
            app.add_layer("Decals", Layer::Decals(DecalLayer { alpha, ..Default::default() }));
            ui.close();
        }
        if ui
            .button(format!("{} Group", icon::FOLDERS))
            .on_hover_text(
                "A folder of layers composited together, then blended, windowed and masked \
                 as one. Adopt layers into it from its editor.",
            )
            .clicked()
        {
            app.add_layer("Group", Layer::Group(GroupLayer::default()));
            ui.close();
        }
        if ui
            .button(format!("{} Table pad", icon::SEAL))
            .on_hover_text(
                "A raised flat table sitting on the band. For a signet, shape the band itself \
                 instead — Design ▸ Shank ▸ Signet makes the head out of the ring.",
            )
            .clicked()
        {
            let signet = SignetLayer::fitted_to(&app.design.field_context());
            app.add_layer("Table pad", Layer::Signet(signet));
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
        Layer::Group(_) => icon::FOLDERS,
        Layer::Curve(_) => icon::WAVE_SINE,
        Layer::Flutes(_) => icon::ROWS,
        Layer::Decals(_) => icon::STAMP,
        Layer::SeatRun(_) => icon::CIRCLES_FOUR,
        Layer::Openwork(_) => icon::EXCLUDE,
    }
}

/// Editor for an openwork carve: the lattice controls are the tiling's own,
/// plus the floor.
fn openwork(
    ui: &mut egui::Ui,
    o: &mut ringdesign_core::field::OpenworkLayer,
    fctx: &FieldContext,
    names: &[String],
) -> bool {
    let mut c = false;
    ui.label(
        egui::RichText::new(
            "Ink is carved toward a floor over the finger hole. Walls ramp over the              mask's crisp-edge distance, so give the lattice an Edge value.",
        )
        .small()
        .color(theme::TEXT_DIM),
    );
    ui.add_space(2.0);
    c |= grid(ui, "openwork_floor", |ui| {
        let mut c = false;
        ui.label("Depth");
        c |= ui
            .add(
                egui::DragValue::new(&mut o.depth_mm)
                    .speed(0.01)
                    .range(0.1..=4.0)
                    .suffix(" mm"),
            )
            .on_hover_text("Deepest carve along the surface normal. Deep is safe on a side face; the floor over the bore still caps it everywhere.")
            .changed();
        ui.end_row();

        ui.label("Keep over bore");
        c |= ui
            .add(
                egui::DragValue::new(&mut o.keep_mm)
                    .speed(0.01)
                    .range(0.3..=2.5)
                    .suffix(" mm"),
            )
            .on_hover_text("Metal left standing under the deepest carve")
            .changed();
        ui.end_row();

        ui.label("Wall ramp");
        c |= ui
            .add(
                egui::DragValue::new(&mut o.tiling.edge_mm)
                    .speed(0.01)
                    .range(0.05..=1.5)
                    .suffix(" mm"),
            )
            .on_hover_text("Width of the drafted wall from rim to floor, mm-true at any tile size")
            .changed();
        ui.end_row();
        c
    });
    let mut no_bake = None;
    c |= tiling(ui, &mut o.tiling, fctx, names, None, &mut no_bake);
    c
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
    let dfm = ringdesign_core::dfm::findings(&app.design);

    for i in 0..n {
        let selected = app.selected_layer == Some(i);
        let fill = if selected {
            theme::ACCENT_DIM.gamma_multiply(0.35)
        } else {
            egui::Color32::TRANSPARENT
        };
        let row = egui::Frame::NONE
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

                    // Grip: drag a row anywhere in the stack.
                    ui.dnd_drag_source(egui::Id::new(("layer-drag", i)), i, |ui| {
                        ui.label(
                            egui::RichText::new(icon::DOTS_SIX_VERTICAL).color(theme::TEXT_DIM),
                        );
                    })
                    .response
                    .on_hover_text("Drag to reorder");

                    let solo = ui.input(|inp| inp.modifiers.alt);
                    let check = ui
                        .checkbox(&mut e.enabled, "")
                        .on_hover_text("Include this layer in the build. Alt-click: solo.");
                    if check.clicked() && solo {
                        // The plain toggle already flipped; solo overrides it.
                        e.enabled = !e.enabled;
                        action = Some(Action::Solo(i));
                    } else {
                        dirty |= check.changed();
                    }

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
                        if let Some(f) = dfm.iter().find(|f| f.layer == i) {
                            ui.label(egui::RichText::new(icon::WARNING).color(theme::WARN))
                                .on_hover_text(&f.message);
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
        // A dragged row dropped on this one lands at this position.
        let target = row.response;
        if let Some(from) = target.dnd_release_payload::<usize>() {
            if *from != i {
                action = Some(Action::MoveTo(*from, i));
            }
        } else if target.dnd_hover_payload::<usize>().is_some() {
            ui.painter().hline(
                target.rect.x_range(),
                target.rect.top(),
                egui::Stroke::new(2.0, theme::ACCENT),
            );
        }
    }

    match action {
        Some(Action::Select(i)) => app.selected_layer = Some(i),
        Some(Action::Move(i, d)) => app.move_layer(i, d),
        Some(Action::MoveTo(from, to)) => app.move_layer_to(from, to),
        Some(Action::Solo(i)) => app.solo_layer(i),
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
    let mut group_edit: Option<GroupEdit> = None;
    let mut bake_draft: Option<f64> = None;
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
                            c |= ui
                                .selectable_value(&mut entry.blend, b, b.label())
                                .clicked();
                        }
                    })
                    .response
                    .on_hover_text(
                        "How this layer stacks onto everything beneath it. Carve subtracts.",
                    );
                ui.end_row();

                if entry.blend.is_smooth() {
                    ui.label("Fillet");
                    c |= ui
                        .add(
                            egui::Slider::new(&mut entry.soft_mm, 0.0..=1.5)
                                .suffix(" mm")
                                .fixed_decimals(2),
                        )
                        .on_hover_text("Radius the crossing is melted over. 0 is a hard crease.")
                        .changed();
                    ui.end_row();
                }

                ui.label("Opacity");
                c |= ui
                    .add(egui::Slider::new(&mut entry.opacity, 0.0..=1.0).fixed_decimals(2))
                    .on_hover_text("Scales this layer's relief before it is blended")
                    .changed();
                ui.end_row();

                ui.label("Relief");
                let remap_label = match &entry.remap {
                    Remap::Off => "Plain",
                    Remap::Curve { .. } => "Curved",
                    Remap::Terrace { .. } => "Terraced",
                };
                egui::ComboBox::from_id_salt("layer_remap")
                    .selected_text(remap_label)
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(entry.remap.is_off(), "Plain").clicked() {
                            entry.remap = Remap::Off;
                            c = true;
                        }
                        if ui.selectable_label(false, "Cushion").clicked() {
                            entry.remap = Remap::cushion(0.35);
                            c = true;
                        }
                        if ui.selectable_label(false, "Chamfer").clicked() {
                            entry.remap = Remap::chamfer(0.35);
                            c = true;
                        }
                        if ui
                            .selectable_label(
                                matches!(entry.remap, Remap::Terrace { .. }),
                                "Terraced",
                            )
                            .clicked()
                        {
                            entry.remap = Remap::Terrace {
                                steps: 4,
                                span_mm: 0.35,
                                riser: 0.35,
                            };
                            c = true;
                        }
                    })
                    .response
                    .on_hover_text(
                        "Reshapes the relief profile: cushioned tops, chamfered take-offs, \
                         or stepped terraces with drafted risers.",
                    );
                ui.end_row();

                match &mut entry.remap {
                    Remap::Off => {}
                    Remap::Curve { span_mm, .. } => {
                        ui.label("Over");
                        c |= ui
                            .add(
                                egui::Slider::new(span_mm, 0.05..=2.0)
                                    .suffix(" mm")
                                    .fixed_decimals(2),
                            )
                            .on_hover_text("Relief height the curve is normalized over")
                            .changed();
                        ui.end_row();
                    }
                    Remap::Terrace {
                        steps,
                        span_mm,
                        riser,
                    } => {
                        ui.label("Steps");
                        c |= ui.add(egui::Slider::new(steps, 2..=12)).changed();
                        ui.end_row();
                        ui.label("Over");
                        c |= ui
                            .add(
                                egui::Slider::new(span_mm, 0.05..=2.0)
                                    .suffix(" mm")
                                    .fixed_decimals(2),
                            )
                            .changed();
                        ui.end_row();
                        ui.label("Riser");
                        c |= ui
                            .add(egui::Slider::new(riser, 0.15..=1.0).fixed_decimals(2))
                            .on_hover_text(
                                "Share of each tread spent rising. Low is crisper and \
                                 steeper; the floor keeps the risers drafted.",
                            )
                            .changed();
                        ui.end_row();
                    }
                }

                ui.label("Mask");
                let mask_label = entry.mask.as_deref().unwrap_or("None");
                egui::ComboBox::from_id_salt("layer_mask")
                    .selected_text(mask_label)
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        c |= ui.selectable_value(&mut entry.mask, None, "None").clicked();
                        for name in &names {
                            c |= ui
                                .selectable_value(&mut entry.mask, Some(name.clone()), name)
                                .clicked();
                        }
                    })
                    .response
                    .on_hover_text(
                        "Alpha multiplied into this layer's strength over the whole band — \
                         paint or import where the ornament goes.",
                    );
                ui.end_row();

                c
            });

            dirty |= window_controls(ui, &mut entry.window, &fctx);

            ui.add_space(4.0);
            dirty |= match &mut entry.layer {
                Layer::Tiling(t) => tiling(ui, t, &fctx, &names, thumb, &mut bake_draft),
                Layer::Border(b) => border(ui, b, &fctx),
                Layer::SeatPad(p) => seat_pad(ui, p, &fctx),
                Layer::Milgrain(m) => milgrain(ui, m, &fctx),
                Layer::Signet(s) => signet(ui, s, &fctx, &mut shape_head),
                Layer::Group(g) => group(ui, g, &fctx, &names, &mut group_edit),
                Layer::Curve(l) => curve_editor(ui, l, &fctx),
                Layer::Flutes(f) => flutes(ui, f),
                Layer::Decals(d) => decals(ui, d, &fctx, &names),
                Layer::SeatRun(r) => seat_run(ui, r, &fctx),
                Layer::Openwork(o) => openwork(ui, o, &fctx, &names),
            };
            dirty
        })
        .inner;
    if dirty
        && let Layer::Tiling(t) = &app.design.layers.layers[i].layer
        && t.edge_mm > 1e-9
        && let Some(src) = app.lib.get(&t.alpha).cloned()
    {
        // The crisp-edge mode reads the alpha's distance field; keep it
        // derived the moment the control moves.
        app.library_mut().insert(src.signed_distance_px());
    }
    if let Some(deg) = bake_draft {
        let baked = match &app.design.layers.layers[i].layer {
            Layer::Tiling(t) => app.lib.get(&t.alpha).map(|a| {
                let (cw, ch) = t.cell_size(&fctx);
                let mm_per_px = (cw / a.width.max(1) as f64, ch / a.height.max(1) as f64);
                a.draft_limited(deg, mm_per_px, t.height_mm.max(1e-6))
            }),
            _ => None,
        };
        if let Some(baked) = baked {
            let name = baked.name.clone();
            app.forget_thumbnail(&name);
            app.library_mut().insert(baked);
            if let Layer::Tiling(t) = &mut app.design.layers.layers[i].layer {
                t.alpha = name;
            }
            dirty = true;
        }
    }
    if let Some(edit) = group_edit {
        match edit {
            GroupEdit::AdoptNext => {
                if i + 1 < app.design.layers.layers.len() {
                    let child = app.design.layers.layers.remove(i + 1);
                    if let Layer::Group(g) = &mut app.design.layers.layers[i].layer {
                        g.stack.layers.push(child);
                    }
                }
            }
            GroupEdit::MoveOut(j) => {
                let child = match &mut app.design.layers.layers[i].layer {
                    Layer::Group(g) if j < g.stack.layers.len() => Some(g.stack.layers.remove(j)),
                    _ => None,
                };
                if let Some(child) = child {
                    app.design.layers.layers.insert(i + 1, child);
                }
            }
        }
        dirty = true;
    }
    // Turn the pad into the band. The head carries the same outline, length and
    // stand-off, so the shape does not change — it stops being something
    // standing on the ring and becomes the ring.
    if shape_head {
        if let Layer::Signet(s) = &app.design.layers.layers[i].layer {
            let (outline, length, rise) = (s.outline, s.length_mm, s.height_mm);
            let width = app.design.profile.width_mm.max(s.width_mm);
            app.design.profile.width_mm = width;
            app.design.shank.apply_signet(width);
            app.design.shank.head.outline = outline;
            app.design.shank.head.length_mm = length;
            app.design.shank.head.rise_mm = rise;
            app.design.layers.layers.remove(i);
            app.selected_layer = None;
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
    bake_draft: &mut Option<f64>,
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
            .add(
                egui::DragValue::new(&mut t.repeats_around)
                    .speed(0.15)
                    .range(1..=400),
            )
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
            .add(
                egui::DragValue::new(&mut t.contrast)
                    .speed(0.01)
                    .range(0.1..=4.0),
            )
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

        ui.label("Flow");
        ui.horizontal(|ui| {
            let mut on = t.warp.is_some();
            if ui
                .checkbox(&mut on, "warp")
                .on_hover_text(
                    "Bend the rows to follow a wave drawn around the ring. The guide is a                      point list; this builds a sine, and MCP or a later editor can author                      any shape into it.",
                )
                .changed()
            {
                t.warp = on.then(|| sine_warp(t.v_center_mm, 1.2, 2));
                c = true;
            }
            if let Some(w) = t.warp.as_mut() {
                let amp_id = egui::Id::new("warp_amp");
                let waves_id = egui::Id::new("warp_waves");
                let mut amp: f64 = ui.memory(|m| m.data.get_temp(amp_id)).unwrap_or(1.2);
                let mut waves: i32 = ui.memory(|m| m.data.get_temp(waves_id)).unwrap_or(2);
                let a = ui
                    .add(egui::DragValue::new(&mut amp).speed(0.02).range(0.1..=4.0).suffix(" mm"))
                    .on_hover_text("Guide amplitude")
                    .changed();
                let b = ui
                    .add(egui::DragValue::new(&mut waves).speed(0.1).range(1..=8))
                    .on_hover_text("Waves per revolution")
                    .changed();
                if a || b {
                    ui.memory_mut(|m| {
                        m.data.insert_temp(amp_id, amp);
                        m.data.insert_temp(waves_id, waves);
                    });
                    *w = sine_warp(t.v_center_mm, amp, waves.max(1) as usize);
                    c = true;
                }
                c |= ui
                    .add(
                        egui::DragValue::new(&mut w.falloff_mm)
                            .speed(0.05)
                            .range(0.5..=12.0)
                            .suffix(" mm"),
                    )
                    .on_hover_text("How far across the band the bend reaches")
                    .changed();
            }
        });
        ui.end_row();

        ui.label("Spiral / fold");
        ui.horizontal(|ui| {
            c |= ui
                .add(egui::DragValue::new(&mut t.shear).speed(0.02).range(-4.0..=4.0))
                .on_hover_text("Helix shear: cells of drift per band height — rows spiral. Always seamless.")
                .changed();
            let mut k = t.kfold as i32;
            if ui
                .add(egui::DragValue::new(&mut k).speed(0.1).range(0..=12).prefix("fold "))
                .on_hover_text("Kaleidoscope: mirror the pattern into 1/k wedges of the ring. 0 is off.")
                .changed()
            {
                t.kfold = k.max(0) as u32;
                c = true;
            }
        });
        ui.end_row();

        ui.label("Crisp edge");
        c |= ui
            .add(
                egui::DragValue::new(&mut t.edge_mm)
                    .speed(0.01)
                    .range(0.0..=1.5)
                    .suffix(" mm"),
            )
            .on_hover_text(
                "Rebuild height from the distance to the ink's edge: the bevel stays this \
                 many mm wide at any tile size. 0 keeps brightness-as-height. Best on \
                 masks with clean shapes; Remap still applies on top.",
            )
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
    ui.horizontal(|ui| {
        let deg_id = egui::Id::new("draft_bake_deg");
        let mut deg: f64 = ui.memory(|m| m.data.get_temp(deg_id)).unwrap_or(55.0);
        if ui
            .button(format!("{} Bake wall limit", icon::TRIANGLE))
            .on_hover_text(
                "Fair this alpha so no relief wall exceeds the angle at this cell size and \
                 height — a hard-edged import becomes mouldable. Saves a new tile and \
                 points the layer at it.",
            )
            .clicked()
        {
            *bake_draft = Some(deg);
        }
        ui.add(
            egui::DragValue::new(&mut deg)
                .range(15.0..=85.0)
                .suffix("°")
                .speed(1.0),
        )
        .on_hover_text("Steepest wall the baked relief may carry");
        ui.memory_mut(|m| m.data.insert_temp(deg_id, deg));
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
fn decals(ui: &mut egui::Ui, d: &mut DecalLayer, fctx: &FieldContext, names: &[String]) -> bool {
    let mut c = grid(ui, "decal_layer", |ui| {
        let mut c = false;

        ui.label("Alpha");
        egui::ComboBox::from_id_salt("decal_alpha")
            .selected_text(d.alpha.as_str())
            .width(150.0)
            .show_ui(ui, |ui| {
                for name in names {
                    c |= ui
                        .selectable_value(&mut d.alpha, name.clone(), name)
                        .clicked();
                }
            });
        ui.end_row();

        ui.label("Feather");
        c |= ui
            .add(
                egui::Slider::new(&mut d.feather_mm, 0.0..=1.5)
                    .suffix(" mm")
                    .fixed_decimals(2),
            )
            .on_hover_text("Fade inside each stamp's border, so no stamp ends in a wall")
            .changed();
        ui.end_row();

        ui.label("Invert");
        c |= ui.checkbox(&mut d.invert, "").changed();
        ui.end_row();

        c
    });

    let v_max = fctx.band_v_len_mm.max(0.5);
    let mut remove: Option<usize> = None;
    for (i, stamp) in d.decals.iter_mut().enumerate() {
        egui::CollapsingHeader::new(format!("Stamp {}", i + 1))
            .id_salt(("decal", i))
            .default_open(d_open(i))
            .show(ui, |ui| {
                c |= grid(ui, &format!("decal_grid_{i}"), |ui| {
                    let mut c = false;
                    ui.label("Around");
                    c |= ui
                        .add(egui::Slider::new(&mut stamp.theta_deg, 0.0..=360.0).suffix("°"))
                        .changed();
                    ui.end_row();
                    ui.label("Across");
                    c |= ui
                        .add(egui::Slider::new(&mut stamp.v_mm, 0.0..=v_max).suffix(" mm"))
                        .changed();
                    ui.end_row();
                    ui.label("Size");
                    c |= ui
                        .add(
                            egui::Slider::new(&mut stamp.size_mm, 0.5..=15.0)
                                .suffix(" mm")
                                .fixed_decimals(1),
                        )
                        .changed();
                    ui.end_row();
                    ui.label("Rotation");
                    c |= ui
                        .add(egui::Slider::new(&mut stamp.rotation_deg, -180.0..=180.0).suffix("°"))
                        .changed();
                    ui.end_row();
                    ui.label("Relief");
                    c |= ui
                        .add(
                            egui::Slider::new(&mut stamp.height_mm, 0.05..=1.2)
                                .suffix(" mm")
                                .fixed_decimals(2),
                        )
                        .changed();
                    ui.end_row();
                    ui.label("Flip");
                    ui.horizontal(|ui| {
                        c |= ui.checkbox(&mut stamp.flip, "").changed();
                        if ui.small_button(icon::TRASH).clicked() {
                            remove = Some(i);
                        }
                    });
                    ui.end_row();
                    c
                });
            });
    }
    if let Some(i) = remove
        && d.decals.len() > 1
    {
        d.decals.remove(i);
        c = true;
    }
    if d.decals.len() < MAX_DECALS && ui.button(format!("{} Add stamp", icon::PLUS)).clicked() {
        let mut stamp = d.decals.last().copied().unwrap_or_default();
        stamp.theta_deg = (stamp.theta_deg + 30.0).rem_euclid(360.0);
        d.decals.push(stamp);
        c = true;
    }
    c
}

/// Only the first stamp starts open, so a long list stays scannable.
/// A sine guide with `waves` periods: peaks and troughs alternating around
/// the ring, which Catmull-Rom rounds back into a smooth wave.
fn sine_warp(v_center: f64, amp: f64, waves: usize) -> ringdesign_core::tiling::WarpField {
    let n = (waves.clamp(1, 8)) * 2;
    let points = (0..n)
        .map(|k| {
            let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
            [k as f64 / n as f64, v_center + sign * amp]
        })
        .collect();
    ringdesign_core::tiling::WarpField {
        points,
        strength: 1.0,
        falloff_mm: 4.0,
    }
}

fn d_open(i: usize) -> bool {
    i == 0
}

fn flutes(ui: &mut egui::Ui, f: &mut FlutesLayer) -> bool {
    grid(ui, "flutes_layer", |ui| {
        let mut c = false;

        ui.label("Count");
        c |= ui
            .add(egui::Slider::new(&mut f.count, 8..=512))
            .on_hover_text("Flutes around the ring. Integer, so the pattern closes.")
            .changed();
        ui.end_row();

        ui.label("Profile");
        egui::ComboBox::from_id_salt("flute_profile")
            .selected_text(f.profile.label())
            .width(150.0)
            .show_ui(ui, |ui| {
                for &p in FluteProfile::ALL {
                    c |= ui.selectable_value(&mut f.profile, p, p.label()).clicked();
                }
            });
        ui.end_row();

        ui.label("Width");
        c |= ui
            .add(
                egui::Slider::new(&mut f.width_mm, 0.1..=2.0)
                    .suffix(" mm")
                    .fixed_decimals(2),
            )
            .changed();
        ui.end_row();

        ui.label("Depth");
        c |= ui
            .add(
                egui::Slider::new(&mut f.height_mm, 0.03..=0.6)
                    .suffix(" mm")
                    .fixed_decimals(2),
            )
            .on_hover_text("Standing height when blended Max, cut depth when blended Carve")
            .changed();
        ui.end_row();

        ui.label("Lean");
        c |= ui
            .add(egui::Slider::new(&mut f.lean, -6.0..=6.0).fixed_decimals(1))
            .on_hover_text("Cells of sideways drift across the band — diagonal reeding")
            .changed();
        ui.end_row();

        ui.label("Direction");
        c |= ui
            .checkbox(&mut f.along, "Along the ring (melon lobes)")
            .on_hover_text("Run the flutes around the ring, spaced across the band")
            .changed();
        ui.end_row();

        c
    })
}

fn curve_editor(ui: &mut egui::Ui, l: &mut CurveLayer, fctx: &FieldContext) -> bool {
    let mut c = grid(ui, "curve_layer", |ui| {
        let mut c = false;

        ui.label("Around");
        c |= ui
            .add(egui::Slider::new(&mut l.repeats_around, 1..=64))
            .on_hover_text("Instances of the drawn path around the ring. Integer, so it closes.")
            .changed();
        ui.end_row();

        ui.label("Width");
        c |= ui
            .add(
                egui::Slider::new(&mut l.width_mm, 0.2..=3.0)
                    .suffix(" mm")
                    .fixed_decimals(2),
            )
            .changed();
        ui.end_row();

        ui.label("Height");
        c |= ui
            .add(
                egui::Slider::new(&mut l.height_mm, 0.05..=1.2)
                    .suffix(" mm")
                    .fixed_decimals(2),
            )
            .changed();
        ui.end_row();

        ui.label("Wire");
        egui::ComboBox::from_id_salt("curve_profile")
            .selected_text(l.profile.label())
            .width(150.0)
            .show_ui(ui, |ui| {
                for &p in WireProfile::ALL {
                    c |= ui.selectable_value(&mut l.profile, p, p.label()).clicked();
                }
            });
        ui.end_row();

        ui.label("Ends");
        ui.horizontal(|ui| {
            c |= ui
                .checkbox(&mut l.closed, "Closed loop")
                .on_hover_text("Join the last point back to the first — a rail with no ends")
                .changed();
            c |= ui
                .checkbox(&mut l.mirror_v, "Mirror")
                .on_hover_text("Also place a copy mirrored about the middle of the band")
                .changed();
        });
        ui.end_row();

        if !l.closed {
            ui.label("Taper");
            c |= ui
                .add(egui::Slider::new(&mut l.taper, 0.0..=0.5).fixed_decimals(2))
                .on_hover_text("Fraction of each end the wire thins out over — calligraphic ends")
                .changed();
            ui.end_row();
        }

        c
    });

    // --- Path canvas: one instance's cell, x across, v up. ---
    ui.add_space(3.0);
    let id = ui.make_persistent_id("curve_layer_drag");
    let width = ui.available_width().clamp(160.0, 280.0);
    let (response, painter) =
        ui.allocate_painter(egui::vec2(width, 110.0), egui::Sense::click_and_drag());
    let rect = response.rect;
    painter.rect_filled(rect, 3.0, theme::BG);
    let plot = rect.shrink(8.0);
    let band = fctx.band_v_len_mm.max(1e-6);

    let to_screen = |x: f64, v: f64| {
        egui::pos2(
            plot.left() + x as f32 * plot.width(),
            plot.bottom() - (v / band) as f32 * plot.height(),
        )
    };
    let to_path = |p: egui::Pos2| {
        (
            (((p.x - plot.left()) / plot.width().max(1.0)) as f64).clamp(0.0, 1.0),
            ((plot.bottom() - p.y) / plot.height().max(1.0)).clamp(0.0, 1.0) as f64 * band,
        )
    };

    for t in [0.0, 0.5, 1.0] {
        let stroke = egui::Stroke::new(1.0, theme::GRID);
        painter.line_segment([to_screen(t, 0.0), to_screen(t, band)], stroke);
    }
    // The crest line, so the drawing reads against the band.
    let crest = to_screen(0.0, fctx.crest_v_mm).y;
    painter.line_segment(
        [
            egui::pos2(plot.left(), crest),
            egui::pos2(plot.right(), crest),
        ],
        egui::Stroke::new(1.0, theme::ACCENT_DIM.gamma_multiply(0.5)),
    );

    let nearest = |p: egui::Pos2, pts: &[[f64; 2]]| {
        pts.iter()
            .enumerate()
            .map(|(i, q)| (i, to_screen(q[0], q[1]).distance(p)))
            .filter(|(_, d)| *d <= GRAB_PX)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };

    let mut drag: Option<usize> = ui.memory(|m| m.data.get_temp(id)).flatten();
    if response.drag_started() {
        drag = response
            .interact_pointer_pos()
            .and_then(|p| nearest(p, &l.points));
        ui.memory_mut(|m| m.data.insert_temp(id, drag));
    }
    if response.drag_stopped() {
        ui.memory_mut(|m| m.data.insert_temp(id, Option::<usize>::None));
    }
    if response.dragged()
        && let (Some(i), Some(p)) = (drag, response.interact_pointer_pos())
        && i < l.points.len()
    {
        let (x, v) = to_path(p);
        l.points[i] = [x, v];
        c = true;
    }
    if response.double_clicked()
        && l.points.len() < MAX_CURVE_POINTS
        && let Some(p) = response.interact_pointer_pos()
    {
        let (x, v) = to_path(p);
        // After the nearest existing point, so the path is not re-threaded.
        let after = l
            .points
            .iter()
            .enumerate()
            .min_by(|a, b| {
                let da = (a.1[0] - x).abs();
                let db = (b.1[0] - x).abs();
                da.total_cmp(&db)
            })
            .map(|(i, q)| if x >= q[0] { i + 1 } else { i })
            .unwrap_or(l.points.len());
        l.points.insert(after.min(l.points.len()), [x, v]);
        c = true;
    }
    if response.secondary_clicked()
        && l.points.len() > 2
        && let Some(p) = response.interact_pointer_pos()
        && let Some(i) = nearest(p, &l.points)
    {
        l.points.remove(i);
        c = true;
    }

    let line: Vec<egui::Pos2> = l
        .sample_path(16)
        .into_iter()
        .map(|q| to_screen(q[0], q[1]))
        .collect();
    if line.len() >= 2 {
        painter.add(egui::Shape::line(
            line,
            egui::Stroke::new(1.6, theme::ACCENT),
        ));
    }
    for q in &l.points {
        painter.circle_filled(to_screen(q[0], q[1]), 3.2, theme::ACCENT);
    }

    response.on_hover_text(
        "One instance of the path: left edge meets the previous copy, right edge the next. \
         Drag points; double-click adds, right-click removes.",
    );
    c
}

const GRAB_PX: f32 = 9.0;

fn group(
    ui: &mut egui::Ui,
    g: &mut GroupLayer,
    fctx: &FieldContext,
    names: &[String],
    pending: &mut Option<GroupEdit>,
) -> bool {
    let mut c = false;
    if g.stack.layers.is_empty() {
        ui.label(
            egui::RichText::new("Empty group — adopt the layer below to start.")
                .small()
                .color(theme::TEXT_DIM),
        );
    }

    let mut delete: Option<usize> = None;
    for j in 0..g.stack.layers.len() {
        let ce = &mut g.stack.layers[j];
        ui.horizontal(|ui| {
            c |= ui
                .checkbox(&mut ce.enabled, "")
                .on_hover_text("Include in the group's composite")
                .changed();
            ui.label(format!("{} {}", kind_icon(&ce.layer), ce.name));
            if ui
                .small_button(icon::SIGN_OUT)
                .on_hover_text("Move out of the group, back into the stack")
                .clicked()
            {
                *pending = Some(GroupEdit::MoveOut(j));
            }
            if ui
                .small_button(icon::TRASH)
                .on_hover_text("Delete")
                .clicked()
            {
                delete = Some(j);
            }
        });
        egui::CollapsingHeader::new("Edit")
            .id_salt(("group_child", j))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Blend");
                    egui::ComboBox::from_id_salt(("group_child_blend", j))
                        .selected_text(ce.blend.label())
                        .show_ui(ui, |ui| {
                            for &b in Blend::ALL {
                                c |= ui.selectable_value(&mut ce.blend, b, b.label()).clicked();
                            }
                        });
                    ui.label("Opacity");
                    c |= ui
                        .add(egui::Slider::new(&mut ce.opacity, 0.0..=1.0).fixed_decimals(2))
                        .changed();
                });
                let mut dummy = false;
                c |= match &mut ce.layer {
                    Layer::Tiling(t) => {
                        let mut no_bake = None;
                        tiling(ui, t, fctx, names, None, &mut no_bake)
                    }
                    Layer::Border(b) => border(ui, b, fctx),
                    Layer::SeatPad(p) => seat_pad(ui, p, fctx),
                    Layer::Milgrain(m) => milgrain(ui, m, fctx),
                    Layer::Signet(s) => signet(ui, s, fctx, &mut dummy),
                    Layer::Curve(l) => curve_editor(ui, l, fctx),
                    Layer::Flutes(f) => flutes(ui, f),
                    Layer::Decals(d) => decals(ui, d, fctx, names),
                    Layer::SeatRun(r) => seat_run(ui, r, fctx),
                    Layer::Openwork(o) => openwork(ui, o, fctx, names),
                    Layer::Group(_) => {
                        ui.label(
                            egui::RichText::new("Nested groups edit one level at a time.")
                                .small()
                                .color(theme::TEXT_DIM),
                        );
                        false
                    }
                };
            });
    }
    if let Some(j) = delete {
        g.stack.layers.remove(j);
        c = true;
    }

    ui.add_space(3.0);
    if ui
        .button(format!("{} Adopt the layer below", icon::DOWNLOAD_SIMPLE))
        .on_hover_text("Move the next layer in the stack into this group")
        .clicked()
    {
        *pending = Some(GroupEdit::AdoptNext);
    }
    c
}

fn window_controls(ui: &mut egui::Ui, w: &mut Window, fctx: &FieldContext) -> bool {
    let mut c = v_gate_controls(ui, w, fctx);
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        c |= ui
            .checkbox(&mut w.enabled, "Limit to an arc")
            .on_hover_text("Confine this layer to part of the ring instead of all the way round")
            .changed();
        if w.enabled {
            c |= ui
                .checkbox(&mut w.invert, "Outside")
                .on_hover_text(
                    "Keep the layer everywhere but the arc — use it to clear a signet head",
                )
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

/// Cross-band gate: hold the layer to a `v` strip, or to the side faces.
fn v_gate_controls(ui: &mut egui::Ui, w: &mut Window, fctx: &FieldContext) -> bool {
    let mut c = false;
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        let mut on = !w.v_gate.is_off();
        if ui
            .checkbox(&mut on, "Limit across the band")
            .on_hover_text("Confine this layer to a strip across the section")
            .changed()
        {
            w.v_gate = if on {
                VGate::Band {
                    center_mm: fctx.crest_v_mm,
                    span_mm: (fctx.band_v_len_mm * 0.4).max(0.5),
                    fade_mm: 0.4,
                }
            } else {
                VGate::Off
            };
            c = true;
        }
    });
    let side_label = |p: SideFacePick| match p {
        SideFacePick::Low => "Low edge",
        SideFacePick::High => "High edge",
        SideFacePick::Wider => "Wider face",
        SideFacePick::Both => "Both faces",
    };
    match &mut w.v_gate {
        VGate::Off => {}
        VGate::Band {
            center_mm,
            span_mm,
            fade_mm,
        } => {
            c |= grid(ui, "layer_v_gate", |ui| {
                let mut c = false;
                let v_max = fctx.band_v_len_mm.max(0.5);

                ui.label("Centre v");
                c |= ui
                    .add(egui::Slider::new(center_mm, 0.0..=v_max).suffix(" mm"))
                    .changed();
                ui.end_row();

                ui.label("Span");
                c |= ui
                    .add(egui::Slider::new(span_mm, 0.0..=v_max).suffix(" mm"))
                    .changed();
                ui.end_row();

                ui.label("Fade");
                c |= ui
                    .add(egui::Slider::new(fade_mm, 0.0..=2.0).suffix(" mm"))
                    .on_hover_text("A hard edge raises a wall the mould has to clear")
                    .changed();
                ui.end_row();

                c
            });
        }
        VGate::SideFaces(pick) => {
            c |= grid(ui, "layer_v_gate_sf", |ui| {
                let mut c = false;
                ui.label("Faces");
                egui::ComboBox::from_id_salt("v_gate_pick")
                    .selected_text(side_label(*pick))
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        for p in [
                            SideFacePick::Wider,
                            SideFacePick::Low,
                            SideFacePick::High,
                            SideFacePick::Both,
                        ] {
                            c |= ui.selectable_value(pick, p, side_label(p)).clicked();
                        }
                    });
                ui.end_row();
                c
            });
        }
    }
    // Switch between the two gate kinds under one combo.
    if let VGate::Band { .. } = w.v_gate {
        ui.horizontal(|ui| {
            if ui
                .small_button("Snap to side faces")
                .on_hover_text(
                    "Track the faces square to the mould pull instead of a fixed strip. \
                     Relief there pulls straight out, whatever the profile becomes.",
                )
                .clicked()
            {
                w.v_gate = VGate::SideFaces(SideFacePick::Wider);
                c = true;
            }
        });
    } else if let VGate::SideFaces(_) = w.v_gate {
        if fctx.side_faces_std().is_none() {
            ui.label(
                egui::RichText::new(format!(
                    "{} This profile has no side faces — the layer passes nothing. \
                     Square the sides in Design ▸ Profile.",
                    icon::WARNING
                ))
                .small()
                .color(theme::WARN),
            );
        }
        ui.horizontal(|ui| {
            if ui.small_button("Use a fixed strip instead").clicked() {
                w.v_gate = VGate::Band {
                    center_mm: fctx.crest_v_mm,
                    span_mm: (fctx.band_v_len_mm * 0.4).max(0.5),
                    fade_mm: 0.4,
                };
                c = true;
            }
        });
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
            .add_enabled(
                enabled,
                egui::Button::new(format!("{} Fit to sides", icon::ARROWS_OUT_LINE_VERTICAL)),
            )
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
                .add(
                    egui::DragValue::new(&mut b.rope_twists)
                        .speed(0.3)
                        .range(1..=400),
                )
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

fn seat_run(ui: &mut egui::Ui, r: &mut SeatRunLayer, fctx: &FieldContext) -> bool {
    use ringdesign_core::gem::{Gem, GemCut};
    let mut c = grid(ui, "seat_run_grid", |ui| {
        let mut c = false;

        ui.label("Stone");
        ui.horizontal(|ui| {
            let mut gem = r.gem;
            let mut picked = false;
            egui::ComboBox::from_id_salt("run_cut")
                .selected_text(gem.cut.label())
                .width(120.0)
                .show_ui(ui, |ui| {
                    for &cut in GemCut::ALL {
                        if ui.selectable_label(gem.cut == cut, cut.label()).clicked() {
                            gem = Gem::calibrated(cut, gem.w_mm);
                            picked = true;
                        }
                    }
                });
            egui::ComboBox::from_id_salt("run_size")
                .selected_text(format!("{:.1} mm", gem.w_mm))
                .width(72.0)
                .show_ui(ui, |ui| {
                    for &w in gem.cut.calibrated_mm() {
                        if ui
                            .selectable_label((gem.w_mm - w).abs() < 0.01, format!("{w:.1} mm"))
                            .clicked()
                        {
                            gem = Gem::calibrated(gem.cut, w);
                            picked = true;
                        }
                    }
                });
            if picked {
                r.gem = gem;
                r.solve_spacing(fctx);
                c = true;
            }
        });
        ui.end_row();

        ui.label("Count");
        ui.horizontal(|ui| {
            c |= ui.add(egui::Slider::new(&mut r.count, 3..=120)).changed();
            if ui
                .small_button("Solve")
                .on_hover_text("Most stones of this size and bridge that fit the ring")
                .clicked()
            {
                r.solve_spacing(fctx);
                c = true;
            }
        });
        ui.end_row();

        ui.label("Bridge");
        c |= ui
            .add(
                egui::DragValue::new(&mut r.bridge_mm)
                    .speed(0.01)
                    .range(0.1..=2.0)
                    .suffix(" mm"),
            )
            .on_hover_text("Metal wanted between neighbouring stones when solving")
            .changed();
        ui.end_row();

        ui.label("Across");
        c |= ui
            .add(
                egui::DragValue::new(&mut r.seat.v_mm)
                    .speed(0.02)
                    .range(0.0..=fctx.band_v_len_mm.max(0.5))
                    .suffix(" mm"),
            )
            .changed();
        ui.end_row();

        ui.label("Height");
        c |= ui
            .add(
                egui::DragValue::new(&mut r.seat.height_mm)
                    .speed(0.01)
                    .range(0.0..=3.0)
                    .suffix(" mm"),
            )
            .changed();
        ui.end_row();

        ui.label("Graduate");
        c |= ui
            .add(egui::Slider::new(&mut r.taper, 0.0..=0.85).fixed_decimals(2))
            .on_hover_text(
                "Stones shrink toward the far side of the ring — 0.4 is the classic \
                 graduated eternity. Seats scale with their stones and the report \
                 sums the graded carats.",
            )
            .changed();
        ui.end_row();

        if r.taper > 0.0 {
            ui.label("Largest at");
            c |= ui
                .add(
                    egui::DragValue::new(&mut r.taper_theta_deg)
                        .speed(1.0)
                        .range(0.0..=360.0)
                        .suffix("°"),
                )
                .on_hover_text("Ring angle of the largest stone; 90° is the top.")
                .changed();
            ui.end_row();
        }

        c
    });

    let bridge = r.bridge_at(fctx);
    let total: f64 = r.gem.carats() * r.count as f64;
    let colour = if bridge < 0.2 {
        theme::WARN
    } else {
        theme::TEXT_DIM
    };
    ui.label(
        egui::RichText::new(format!(
            "{} {} stones • {:.2} ct total • {:.2} mm bridge",
            icon::DIAMOND,
            r.count,
            total,
            bridge
        ))
        .small()
        .color(colour),
    );
    if bridge < 0.0 {
        ui.label(
            egui::RichText::new(format!(
                "{} Seats overlap — fewer stones or smaller.",
                icon::WARNING
            ))
            .small()
            .color(theme::BAD),
        );
    }
    c |= false;
    c
}

fn seat_pad(ui: &mut egui::Ui, p: &mut SeatPadLayer, fctx: &FieldContext) -> bool {
    use ringdesign_core::field::SeatStyle;
    use ringdesign_core::gem::{Gem, GemCut};
    let v_max = fctx.band_v_len_mm.max(0.5);
    let c = grid(ui, "seat_pad_grid", |ui| {
        let mut c = false;

        ui.label("Style");
        egui::ComboBox::from_id_salt("seat_style")
            .selected_text(p.style.label())
            .width(150.0)
            .show_ui(ui, |ui| {
                for &st in SeatStyle::ALL {
                    c |= ui.selectable_value(&mut p.style, st, st.label()).clicked();
                }
            });
        ui.end_row();

        ui.label("Stone");
        ui.horizontal(|ui| {
            let mut gem = p.gem.unwrap_or_default();
            let mut picked = false;
            egui::ComboBox::from_id_salt("seat_cut")
                .selected_text(gem.cut.label())
                .width(120.0)
                .show_ui(ui, |ui| {
                    for &cut in GemCut::ALL {
                        if ui.selectable_label(gem.cut == cut, cut.label()).clicked() {
                            gem = Gem::calibrated(cut, gem.w_mm);
                            picked = true;
                        }
                    }
                });
            egui::ComboBox::from_id_salt("seat_size")
                .selected_text(format!("{:.1} mm", gem.w_mm))
                .width(72.0)
                .show_ui(ui, |ui| {
                    for &w in gem.cut.calibrated_mm() {
                        if ui
                            .selectable_label((gem.w_mm - w).abs() < 0.01, format!("{w:.1} mm"))
                            .clicked()
                        {
                            gem = Gem::calibrated(gem.cut, w);
                            picked = true;
                        }
                    }
                });
            if picked {
                p.fit_stone(gem);
                c = true;
            }
        });
        ui.end_row();

        if p.style == SeatStyle::Bezel {
            ui.label("Wall");
            c |= ui
                .add(
                    egui::DragValue::new(&mut p.bezel_wall_mm)
                        .speed(0.01)
                        .range(0.2..=1.5)
                        .suffix(" mm"),
                )
                .on_hover_text("Collar thickness burnished over the girdle at the bench")
                .changed();
            ui.end_row();

            ui.label("Recess");
            c |= ui
                .add(
                    egui::DragValue::new(&mut p.recess_mm)
                        .speed(0.01)
                        .range(0.0..=2.0)
                        .suffix(" mm"),
                )
                .on_hover_text("Pocket depth below the rim")
                .changed();
            ui.end_row();
        }

        ui.label("Bur dimple");
        c |= ui
            .add(
                egui::DragValue::new(&mut p.dimple_mm)
                    .speed(0.01)
                    .range(0.0..=3.0)
                    .suffix(" mm"),
            )
            .on_hover_text("A shallow centre dimple cast into the seat, so the setting bur starts true. 0 is none.")
            .changed();
        ui.end_row();

        ui.label("Prongs");
        ui.horizontal(|ui| {
            c |= ui
                .add(egui::Slider::new(&mut p.prongs, 0..=8))
                .on_hover_text(
                    "Drafted cone stock on the seat circle, notched and shaped at the bench",
                )
                .changed();
            if p.prongs > 0 {
                c |= ui
                    .add(
                        egui::DragValue::new(&mut p.prong_mm)
                            .speed(0.01)
                            .range(0.2..=2.0)
                            .suffix(" mm"),
                    )
                    .changed();
            }
        });
        ui.end_row();

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
            .add(
                egui::DragValue::new(&mut m.beads_around)
                    .speed(0.5)
                    .range(3..=800),
            )
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
            "This is a pad standing on the band. A signet is not — the head is the band's own \
             swell, and its outline is the band's silhouette.",
        )
        .small()
        .color(theme::TEXT_DIM),
    );
    if ui
        .button(format!("{} Make this the band", icon::WAVE_SINE))
        .on_hover_text(
            "Moves this shape into the ring itself: sets the shank to Signet with the same \
             outline, length and stand-off, and removes the pad.",
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

// --- Auto pavé ---------------------------------------------------------------

fn pave_window(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    use ringdesign_core::gem::{Gem, GemCut};
    use ringdesign_core::pave::{self, PaveRegion};

    if !app.pave_open {
        return;
    }
    let mut open = app.pave_open;
    let mut generate = false;
    egui::Window::new(format!("{} Auto pavé", icon::SPARKLE))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ui.ctx(), |ui| {
            let spec = &mut app.pave_spec;
            ui.horizontal(|ui| {
                ui.label("Stone");
                let mut w = spec.gem.w_mm;
                if ui
                    .add(
                        egui::Slider::new(&mut w, 0.8..=4.0)
                            .suffix(" mm")
                            .fixed_decimals(2),
                    )
                    .changed()
                {
                    spec.gem = Gem::calibrated(GemCut::Round, w);
                }
                ui.label(
                    egui::RichText::new(format!("{:.3} ct each", spec.gem.carats()))
                        .small()
                        .color(theme::TEXT_DIM),
                );
            });
            ui.add(
                egui::Slider::new(&mut spec.bridge_mm, 0.2..=1.2)
                    .suffix(" mm")
                    .fixed_decimals(2)
                    .text("Bridge"),
            );

            let mut full = spec.span_deg >= 360.0;
            ui.horizontal(|ui| {
                ui.checkbox(&mut full, "Full ring");
                if full {
                    spec.span_deg = 360.0;
                } else {
                    if spec.span_deg >= 360.0 {
                        spec.span_deg = 120.0;
                    }
                    ui.add(egui::Slider::new(&mut spec.span_deg, 20.0..=300.0).suffix("°"));
                    ui.add(
                        egui::DragValue::new(&mut spec.theta_deg)
                            .speed(1.0)
                            .range(0.0..=360.0)
                            .suffix("° at"),
                    );
                }
            });

            let mut on_side = matches!(spec.region, PaveRegion::SideFace(_));
            ui.horizontal(|ui| {
                ui.label("Region");
                if ui.selectable_label(on_side, "Side face").clicked() {
                    on_side = true;
                }
                if ui.selectable_label(!on_side, "Crown band").clicked() {
                    on_side = false;
                }
            });
            let ctx = app.design.field_context();
            match (&mut spec.region, on_side) {
                (r @ PaveRegion::VBand { .. }, true) => {
                    *r = PaveRegion::SideFace(Default::default());
                }
                (r @ PaveRegion::SideFace(_), false) => {
                    *r = PaveRegion::VBand {
                        center_mm: ctx.crest_v_mm,
                        width_mm: (ctx.band_v_len_mm * 0.3).max(2.0),
                    };
                }
                _ => {}
            }
            if let PaveRegion::VBand {
                center_mm,
                width_mm,
            } = &mut spec.region
            {
                ui.add(
                    egui::Slider::new(center_mm, 0.0..=ctx.band_v_len_mm)
                        .suffix(" mm")
                        .text("Centre v"),
                );
                ui.add(
                    egui::Slider::new(width_mm, 1.0..=ctx.band_v_len_mm)
                        .suffix(" mm")
                        .text("Width"),
                );
            } else {
                ui.label(
                    egui::RichText::new(
                        "The wider side face, resolved from the profile when generated.",
                    )
                    .small()
                    .color(theme::TEXT_DIM),
                );
            }
            ui.checkbox(&mut spec.stagger, "Stagger rows (hex packing)");

            ui.add_space(4.0);
            match pave::fill(&app.design, &app.pave_spec) {
                Some((_, out)) => {
                    ui.label(format!(
                        "{} stones in {} rows • {:.2} ct total",
                        out.seats,
                        out.rows,
                        app.pave_spec.gem.carats() * out.seats as f64
                    ));
                    if let Some(n) = &out.note {
                        ui.label(egui::RichText::new(n).small().color(theme::WARN));
                    }
                    if ui.button(format!("{} Generate", icon::SPARKLE)).clicked() {
                        generate = true;
                    }
                }
                None => {
                    ui.label(
                        egui::RichText::new(
                            "Nothing fits — no side face on this profile, or the stone is \
                             wider than the region.",
                        )
                        .color(theme::WARN),
                    );
                }
            }
        });
    if generate {
        if let Some((entry, out)) = ringdesign_core::pave::fill(&app.design, &app.pave_spec) {
            app.design.layers.layers.push(entry);
            app.selected_layer = Some(app.design.layers.layers.len() - 1);
            app.set_status(format!("Pavé: {} stones in {} rows", out.seats, out.rows));
            app.mark_dirty();
        }
        open = false;
    }
    app.pave_open = open;
}
