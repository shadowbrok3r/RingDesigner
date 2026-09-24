use super::{Editor, Parameter, ShapePart};
use egui_mobile::egui;
use ringdesign_core::field::{Layer, SeatStyle};
use ringdesign_core::{ProfileStyle, RingDesign, ShankKind};

#[derive(Clone, Copy)]
pub enum Action {
    Patterns,
    Layers,
    Stones,
    FrameHead,
    Advanced,
    Workshop,
    Findings,
    Report,
    AddStone,
    SketchFace,
    SketchSection,
}

#[derive(Default)]
pub struct Edit {
    pub changed: bool,
    pub view_changed: bool,
    pub action: Option<Action>,
}

fn compact_id() -> egui::Id {
    egui::Id::new("mobile-compact-fields")
}

/// A palette shows one parameter at a time. The remembered list is collected
/// from the same controls as the full inspector, so selection stays contextual.
pub fn begin_compact(ui: &mut egui::Ui, key: egui::Id, active: &mut String) {
    let fields = ui.data(|d| d.get_temp::<Vec<String>>(key).unwrap_or_default());
    if !fields.is_empty() {
        let response = egui::ComboBox::from_id_salt("parameter-picker")
            .selected_text(active.as_str())
            .width((ui.available_width() - 16.0).max(40.0))
            .truncate()
            .show_ui(ui, |ui| {
                for name in fields {
                    ui.selectable_value(active, name.clone(), name);
                }
            });
        super::layout::record(ui, "floating/parameter-picker", response.response.rect);
    }
    ui.data_mut(|d| d.insert_temp(compact_id(), Vec::<String>::new()));
}

pub fn end_compact(ui: &mut egui::Ui, key: egui::Id, active: &mut String) {
    let fields = ui.data_mut(|d| {
        d.remove_temp::<Vec<String>>(compact_id())
            .unwrap_or_default()
    });
    if !fields.is_empty() && !fields.contains(active) {
        *active = fields[0].clone();
        ui.ctx().request_repaint();
    }
    ui.data_mut(|d| d.insert_temp(key, fields));
}

pub fn is_compact(ui: &egui::Ui) -> bool {
    ui.data(|d| d.get_temp::<Vec<String>>(compact_id()).is_some())
}

/// Label and exact value share one line. Only the selected field expands its
/// slider and explanation; labels never compete with a fixed-width slider.
pub fn field(
    ui: &mut egui::Ui,
    active: &mut String,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    unit: &str,
    help: &str,
) -> bool {
    if is_compact(ui) {
        ui.data_mut(|d| {
            d.get_temp_mut_or_default::<Vec<String>>(compact_id())
                .push(label.into())
        });
        if active != label {
            return false;
        }
    }
    let mut changed = false;
    // Explicit identity survives keyboard-driven panel changes and scrolling.
    ui.scope_builder(
        egui::UiBuilder::new().id(ui.id().with(("mobile-field", label))),
        |ui| {
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 94.0).max(40.0);
                let response = ui.add_sized(
                    [width, 32.0],
                    egui::Button::new(egui::RichText::new(label).color(if active == label {
                        crate::theme::AQUA_BRIGHT
                    } else {
                        crate::theme::INK
                    }))
                    .frame(true)
                    .truncate(),
                );
                super::layout::record(ui, format!("field/{label}"), response.rect);
                if response.clicked() {
                    *active = label.into();
                }
                let value_response = ui.add_sized(
                    [86.0, 32.0],
                    egui::DragValue::new(value)
                        .range(range.clone())
                        .clamp_existing_to_range(false)
                        .speed(if unit.is_empty() { 0.01 } else { 0.02 })
                        .max_decimals(2)
                        .suffix(unit),
                );
                super::layout::record(ui, format!("value/{label}"), value_response.rect);
                if value_response.has_focus() {
                    value_response.scroll_to_me(Some(egui::Align::Center));
                }
                if value_response.changed() || value_response.clicked() {
                    *active = label.into();
                }
                changed |= value_response.changed();
            });
            if active == label {
                ui.spacing_mut().slider_width = (ui.available_width() - 4.0).max(40.0);
                changed |= ui
                    .add(
                        egui::Slider::new(value, range)
                            .clamping(egui::SliderClamping::Edits)
                            .show_value(false),
                    )
                    .changed();
                ui.label(
                    egui::RichText::new(help)
                        .size(12.0)
                        .color(crate::theme::INK_DIM),
                );
            }
        },
    );
    changed
}

pub fn shape(ui: &mut egui::Ui, editor: &mut Editor, d: &mut RingDesign) -> Edit {
    if ui.rect_contains_pointer(ui.max_rect()) && ui.input(|i| i.pointer.primary_clicked()) {
        editor.handles_active = true;
    }
    let mut edit = Edit::default();
    edit.changed |= ringdesign_workbench::imported_base::ui(ui, d);
    if d.imported_base.is_some() { return edit; }
    ui.horizontal(|ui| {
        let width = super::row_width(ui.available_width(), 2, ui.spacing().item_spacing.x);
        for (part, title) in [
            (ShapePart::Band, "Band & fit"),
            (ShapePart::Head, "Signet face"),
        ] {
            if ui
                .add_sized(
                    [width, 32.0],
                    egui::Button::new(title).selected(editor.part == part),
                )
                .clicked()
            {
                editor.part = part;
                editor.parameter = if part == ShapePart::Head {
                    Parameter::HeadLength
                } else {
                    Parameter::Bore
                };
                editor.active_field = editor.parameter.label().into();
            }
        }
    });
    if editor.part == ShapePart::Head && d.shank.kind != ShankKind::Signet {
        ui.label("Add a signet face above the band, then shape it on the ring.");
        if ui.button("Make a signet").clicked() {
            d.shank.apply_signet(d.profile.width_mm.max(10.0));
            d.profile.width_mm = d.profile.width_mm.max(10.0);
            edit.changed = true;
            edit.action = Some(Action::FrameHead);
        }
        return edit;
    }
    if editor.part == ShapePart::Head {
        egui::ComboBox::from_id_salt("face-outline")
            .selected_text(d.shank.head.outline.label())
            .width(ui.available_width().min(240.0))
            .show_ui(ui, |ui| {
                for &outline in ringdesign_core::field::SignetOutline::ALL {
                    edit.changed |= ui
                        .selectable_value(&mut d.shank.head.outline, outline, outline.label())
                        .changed();
                }
            });
    }

    let params = if editor.part == ShapePart::Band {
        &Parameter::BAND
    } else {
        &Parameter::HEAD
    };
    for &parameter in params {
        let mut value = parameter.value(d);
        if field(
            ui,
            &mut editor.active_field,
            parameter.label(),
            &mut value,
            parameter.range(),
            parameter.unit(),
            parameter.help(),
        ) {
            edit.changed |= parameter.set(d, value);
        }
        if editor.active_field == parameter.label() {
            editor.parameter = parameter;
        }
    }
    if editor.part == ShapePart::Band && !is_compact(ui) {
        ui.collapsing("Profile & shank shape", |ui| {
            egui::ComboBox::from_id_salt("band-profile")
                .selected_text(d.profile.style.label())
                .width(160.0)
                .show_ui(ui, |ui| {
                    for &style in ProfileStyle::ALL {
                        if ui
                            .selectable_label(d.profile.style == style, style.label())
                            .clicked()
                        {
                            d.profile.apply_style(style);
                            edit.changed = true;
                        }
                    }
                });
            egui::ComboBox::from_id_salt("shank-style")
                .selected_text(format!("{:?}", d.shank.kind))
                .width(160.0)
                .show_ui(ui, |ui| {
                    for &kind in ShankKind::ALL {
                        if ui
                            .selectable_label(d.shank.kind == kind, format!("{kind:?}"))
                            .clicked()
                            && d.shank.kind != kind
                        {
                            d.shank.kind = kind;
                            if kind == ShankKind::Signet {
                                d.shank.apply_signet(d.profile.width_mm);
                            }
                            edit.changed = true;
                        }
                    }
                });
        });
    }
    if !is_compact(ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("More shape controls").clicked() {
                edit.action = Some(Action::Advanced);
            }
            if ui.button("Draw outline").clicked() {
                edit.action = Some(if editor.part == ShapePart::Head {
                    Action::SketchFace
                } else {
                    Action::SketchSection
                });
            }
        });
    }
    edit
}

pub fn surface(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    d: &mut RingDesign,
    selected: &mut Option<usize>,
) -> Edit {
    let mut edit = Edit::default();
    ui.horizontal_wrapped(|ui| {
        if !is_compact(ui) || selected.is_none() {
            if ui.button("Choose pattern").clicked() {
                edit.action = Some(Action::Patterns);
            }
            if ui.button("Layer list").clicked() {
                edit.action = Some(Action::Layers);
            }
        }
        if ui
            .add_enabled(
                selected.is_some(),
                egui::Button::new("Isolate").selected(editor.isolate),
            )
            .clicked()
        {
            editor.isolate = !editor.isolate;
            edit.view_changed = true;
        }
    });
    if selected.is_none() {
        ui.label("Tap an ornament on the ring to edit it, or choose a pattern to add one.");
        return edit;
    }
    if editor.isolate {
        ui.label(
            egui::RichText::new(
                "Preview: this layer over the bare ring. Saved geometry stays complete.",
            )
            .small()
            .color(crate::theme::AQUA_BRIGHT),
        );
    }
    if editor.overlaps.len() > 1 {
        egui::ComboBox::from_id_salt("overlapping-layers")
            .selected_text("Layers at this point")
            .show_ui(ui, |ui| {
                for &i in &editor.overlaps {
                    if let Some(entry) = d.layers.layers.get(i) {
                        if ui
                            .selectable_value(selected, Some(i), &entry.name)
                            .changed()
                            && editor.isolate
                        {
                            edit.view_changed = true;
                        }
                    }
                }
            });
    }
    let index = selected.unwrap();
    let ctx = d.field_context();
    let Some(entry) = d.layers.layers.get_mut(index) else {
        *selected = None;
        return edit;
    };
    if !is_compact(ui) {
        ui.label(
            egui::RichText::new(&entry.name)
                .strong()
                .color(crate::theme::PINK_BRIGHT),
        );
    }
    let active = &mut editor.active_field;
    match &mut entry.layer {
        Layer::Tiling(t) => {
            edit.changed |= field(
                ui,
                active,
                "Relief height",
                &mut t.height_mm,
                0.0..=2.5,
                " mm",
                "Maximum height added by the light parts of this pattern. Check Casting after increasing it.",
            );
            let mut repeats = t.repeats_around as f64;
            if field(
                ui,
                active,
                "Copies around ring",
                &mut repeats,
                1.0..=200.0,
                "",
                "More copies make each tile smaller. Whole repeats keep the pattern seamless.",
            ) {
                t.repeats_around = repeats.round().max(1.0) as u32;
                edit.changed = true;
            }
            edit.changed |= field(
                ui,
                active,
                "Tile rotation",
                &mut t.rotation_deg,
                -180.0..=180.0,
                "°",
                "Turns each pattern inside its tile without rotating the ring.",
            );
            edit.changed |= field(
                ui,
                active,
                "Across-band position",
                &mut t.v_center_mm,
                0.0..=ctx.band_v_len_mm,
                " mm",
                "Moves the pattern strip across the ring's outer surface, from one inner edge to the other.",
            );
            edit.changed |= field(
                ui,
                active,
                "Pattern strip width",
                &mut t.v_span_mm,
                0.1..=ctx.band_v_len_mm,
                " mm",
                "How much of the curved outer surface this pattern covers.",
            );
            edit.changed |= field(
                ui,
                active,
                "Around-ring offset",
                &mut t.offset_u,
                -1.0..=1.0,
                "",
                "Slides the pattern around the ring by a fraction of one tile.",
            );
        }
        Layer::Border(b) => {
            edit.changed |= field(
                ui,
                active,
                "Relief height",
                &mut b.height_mm,
                0.0..=2.5,
                " mm",
                "How far this border stands above the band's surface.",
            );
            edit.changed |= field(
                ui,
                active,
                "Border width",
                &mut b.width_mm,
                0.1..=ctx.band_v_len_mm.max(4.0),
                " mm",
                "Width of the raised rail measured across the surface.",
            );
            edit.changed |= field(
                ui,
                active,
                "Across-band position",
                &mut b.v_mm,
                0.0..=ctx.band_v_len_mm,
                " mm",
                "Moves the border across the curved surface of the band.",
            );
            edit.changed |= ui
                .checkbox(&mut b.mirror, "Matching border on other edge")
                .changed();
        }
        Layer::Milgrain(m) => {
            edit.changed |= field(
                ui,
                active,
                "Bead diameter",
                &mut m.bead_diameter_mm,
                0.15..=2.0,
                " mm",
                "Diameter of each decorative bead; very small beads may not resolve in sand.",
            );
            edit.changed |= field(
                ui,
                active,
                "Relief height",
                &mut m.height_mm,
                0.0..=2.0,
                " mm",
                "Height of the beads above the surface.",
            );
            edit.changed |= field(
                ui,
                active,
                "Across-band position",
                &mut m.v_mm,
                0.0..=ctx.band_v_len_mm,
                " mm",
                "Moves the bead row across the band.",
            );
        }
        Layer::SeatPad(_) | Layer::SeatRun(_) => {
            if ui.button("Edit this setting in Stones").clicked() {
                edit.action = Some(Action::Stones);
            }
        }
        _ => {
            ui.label("This layer's placement and blend can be edited below. Its detailed source remains available in the layer and Workshop tools.");
        }
    }
    edit.changed |= field(
        ui,
        active,
        "Layer strength",
        &mut entry.opacity,
        0.0..=1.0,
        "",
        "Scales this layer's contribution to the actual ring geometry; zero removes its effect.",
    );
    ui.collapsing("Placement window & blend", |ui| {
        edit.changed |= crate::layers::window_controls(ui, index, &mut entry.window, &ctx);
        egui::ComboBox::from_id_salt("surface-blend")
            .selected_text(entry.blend.label())
            .show_ui(ui, |ui| {
                for &blend in ringdesign_core::field::Blend::ALL {
                    edit.changed |= ui
                        .selectable_value(&mut entry.blend, blend, blend.label())
                        .changed();
                }
            });
        edit.changed |= ui
            .checkbox(&mut entry.enabled, "Include layer in design")
            .changed();
    });
    edit
}

pub fn stones(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    d: &mut RingDesign,
    selected: &mut Option<usize>,
) -> Edit {
    let mut edit = Edit::default();
    if !is_compact(ui) || selected.is_none() {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Add a stone setting").clicked() {
                edit.action = Some(Action::AddStone);
            }
            if ui.button("Stone report").clicked() {
                edit.action = Some(Action::Report);
            }
        });
    }
    let Some(index) = *selected else {
        ui.label("Tap a stone marker on the ring, or add a setting at the top of the band.");
        return edit;
    };
    let ctx = d.field_context();
    let path = if editor.stone_path.first() == Some(&index) {
        editor.stone_path.clone()
    } else {
        vec![index]
    };
    if let Some(generator) = super::picking::live_ancestor(&d.layers, &path) {
        ui.label("This stone is generated by a live group. Make the group manual to keep individual stone edits when the band changes.");
        if ui.button("Make group manual").clicked() {
            if let Some(entry) = super::picking::entry_mut(&mut d.layers, &generator) {
                if let Layer::Group(group) = &mut entry.layer {
                    group.recipe = None;
                    edit.changed = true;
                }
            }
        }
        return edit;
    }
    let Some(entry) = super::picking::entry_mut(&mut d.layers, &path) else {
        return edit;
    };
    if !is_compact(ui) {
        ui.label(
            egui::RichText::new(&entry.name)
                .strong()
                .color(crate::theme::PINK_BRIGHT),
        );
    }
    let active = &mut editor.active_field;
    fn seat_ui(
        ui: &mut egui::Ui,
        active: &mut String,
        seat: &mut ringdesign_core::field::SeatPadLayer,
        v_max: f64,
        position: bool,
    ) -> bool {
        let mut changed = false;
        let compact = is_compact(ui);
        let stone_options = |ui: &mut egui::Ui, seat: &mut ringdesign_core::field::SeatPadLayer| {
            let Some(mut gem) = seat.gem else {
                return false;
            };
            let mut gem_changed = false;
            // Fixed-width combos in a wrapped row can still grow a narrow Area.
            // Stack the two selectors when the palette cannot fit both.
            let choices = |ui: &mut egui::Ui| {
                egui::ComboBox::from_id_salt("stone-cut")
                    .selected_text(gem.cut.label())
                    .width(130.0)
                    .show_ui(ui, |ui| {
                        for &cut in ringdesign_core::gem::GemCut::ALL {
                            if ui.selectable_label(gem.cut == cut, cut.label()).clicked()
                                && gem.cut != cut
                            {
                                let form = gem.form;
                                gem = ringdesign_core::gem::Gem::calibrated(cut, gem.w_mm);
                                gem.form = form;
                                gem_changed = true;
                            }
                        }
                    });
                egui::ComboBox::from_id_salt("stone-form")
                    .selected_text(gem.form.label())
                    .width(105.0)
                    .show_ui(ui, |ui| {
                        for &form in ringdesign_core::gem::GemForm::ALL {
                            gem_changed |= ui
                                .selectable_value(&mut gem.form, form, form.label())
                                .changed();
                        }
                    });
            };
            if ui.available_width() < 260.0 {
                ui.vertical(choices);
            } else {
                ui.horizontal_wrapped(choices);
            }
            if gem_changed {
                seat.fit_stone(gem);
            }
            gem_changed
        };
        if !compact {
            changed |= stone_options(ui, seat);
        }
        if let Some(mut gem) = seat.gem {
            let mut width = gem.w_mm;
            if field(
                ui,
                active,
                "Stone width",
                &mut width,
                0.8..=16.0,
                " mm",
                "Changes the stone and refits its seat. Check clearance before manufacturing.",
            ) {
                let ratio = width / gem.w_mm.max(0.1);
                gem.w_mm = width;
                gem.l_mm *= ratio;
                seat.fit_stone(gem);
                changed = true;
            }
            if !compact {
                ui.label(
                    egui::RichText::new(format!("{} · {:.2} ct", gem.display(), gem.carats()))
                        .small(),
                );
            }
        }
        let mut setting_options = |ui: &mut egui::Ui| {
            if compact {
                changed |= stone_options(ui, seat);
            }
            egui::ComboBox::from_id_salt("setting-style")
                .selected_text(seat.style.label())
                .width(ui.available_width().min(150.0))
                .show_ui(ui, |ui| {
                    for &style in SeatStyle::ALL {
                        if ui
                            .selectable_value(&mut seat.style, style, style.label())
                            .changed()
                        {
                            if let Some(g) = seat.gem {
                                seat.fit_stone(g);
                            }
                            changed = true;
                        }
                    }
                });
            // The pre-made part the seat carries: resolved into the ring by boolean, sized to the stone.
            egui::ComboBox::from_id_salt("setting-solid")
                .selected_text(seat.solid.label())
                .width(ui.available_width().min(150.0))
                .show_ui(ui, |ui| {
                    for &kind in ringdesign_core::setting::SolidKind::ALL {
                        changed |= ui.selectable_value(&mut seat.solid, kind, kind.label()).changed();
                    }
                });
            if !seat.solid.is_none() {
                changed |= ui
                    .checkbox(&mut seat.through, "Drill through")
                    .on_hover_text("Carries the seat's pilot through to the finger, where the seat faces out from the bore.")
                    .changed();
            }
        };
        if compact {
            ui.collapsing("Stone & setting", setting_options);
        } else {
            setting_options(ui);
        }
        if position {
            changed |= field(
                ui,
                active,
                "Around-ring position",
                &mut seat.theta_deg,
                0.0..=360.0,
                "°",
                "Moves this setting around the ring. 90° is the signet's usual top.",
            );
        }
        changed |= field(
            ui,
            active,
            "Across-band position",
            &mut seat.v_mm,
            0.0..=v_max,
            " mm",
            "Moves the setting across the band's curved outer surface.",
        );
        changed |= field(
            ui,
            active,
            "Setting height",
            &mut seat.height_mm,
            0.1..=6.0,
            " mm",
            "Raises or lowers the supporting metal under the stone; check pavilion clearance.",
        );
        changed
    }
    match &mut entry.layer {
        Layer::SeatPad(seat) => edit.changed |= seat_ui(ui, active, seat, ctx.band_v_len_mm, true),
        Layer::SeatRun(run) => {
            ui.label(
                "This stone belongs to a repeated row. These controls change the shared setting.",
            );
            let mut seat = run.seat;
            seat.gem = Some(run.gem);
            if seat_ui(ui, active, &mut seat, ctx.band_v_len_mm, false) {
                let resize = seat.gem.is_some_and(|g| g != run.gem);
                run.seat = seat;
                if let Some(g) = seat.gem {
                    run.gem = g;
                }
                if resize {
                    run.solve_spacing(&ctx);
                }
                edit.changed = true;
            }
            ui.label(format!("{} seats in this row", run.count));
        }
        _ => {
            ui.label("This setting is inside a grouped layer. Open the layer source for its component controls.");
            if ui.button("Open layer list").clicked() {
                edit.action = Some(Action::Layers);
            }
        }
    }
    edit
}

pub fn casting(ui: &mut egui::Ui, editor: &mut Editor, d: &mut RingDesign) -> Edit {
    let mut edit = Edit::default();
    use ringdesign_core::castability::CastProcess;
    ui.horizontal(|ui| {
        let width = super::row_width(ui.available_width(), 2, ui.spacing().item_spacing.x);
        for (process, label) in [
            (CastProcess::SandTwoPart, "Sand casting"),
            (CastProcess::LostWax, "Investment"),
        ] {
            if ui
                .add_sized(
                    [width, 32.0],
                    egui::Button::new(label).selected(d.draft.process == process),
                )
                .clicked()
                && d.draft.process != process
            {
                crate::casting::set_process(&mut d.draft, process);
                if let Some(setup) = d.manufacturing.as_mut() {
                    setup.recipe.process = process;
                    setup.recipe.sand = (process == CastProcess::SandTwoPart)
                        .then_some(crate::casting::DEFAULT_SAND);
                    setup.recipe.name = format!(
                        "{} / {}",
                        if process == CastProcess::SandTwoPart {
                            crate::casting::DEFAULT_SAND.label()
                        } else {
                            "Investment"
                        },
                        setup.recipe.alloy
                    );
                    setup.recipe.min_draft_deg = d.draft.min_draft_deg;
                    setup.recipe.min_section_mm = d.draft.min_section_mm;
                    setup.recipe.min_detail_mm = d.draft.min_detail_mm;
                }
                edit.changed = true;
            }
        }
    });
    ui.label(if d.draft.process == CastProcess::SandTwoPart {
        "The pattern must withdraw without tearing the sand. Use the guides to understand the pull, then review the findings."
    } else { "Undercuts are allowed in investment casting; wall thickness and fine detail still need checking." });
    ui.horizontal_wrapped(|ui| {
        if ui.button("Findings").clicked() {
            edit.action = Some(Action::Findings);
        }
        if ui.button("Mould setup & repairs").clicked() {
            edit.action = Some(Action::Workshop);
        }
    });
    ui.label(
        egui::RichText::new(format!(
            "Detail {:.2} mm · wall {:.2} mm · draft {:.1}°",
            d.draft.min_detail_mm, d.draft.min_section_mm, d.draft.min_draft_deg
        ))
        .small(),
    );
    if let Some(hit) = &editor.selection {
        ui.separator();
        ui.label(format!("Picked surface: {:.1}° around ring", hit.theta_deg));
        ui.label(format!(
            "Radial metal {:.2} mm · relief {:+.2} mm",
            hit.radial_wall_mm, hit.relief_mm
        ));
        ui.label(
            egui::RichText::new("Radial metal is a local screen, not a minimum-wall guarantee.")
                .small()
                .weak(),
        );
    }
    edit
}
