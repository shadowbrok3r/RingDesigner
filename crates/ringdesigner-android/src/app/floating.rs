//! Movable, contextual tools over the live ring. Reuses the inspector's edit paths.
use super::*;
use crate::editor::{
    self,
    workspace::{self, Palette},
};
use ringdesign_workbench::visual::Tool;

fn button(ui: &mut egui::Ui, label: &str, selected: bool) -> ringdesign_workbench::icons::Action {
    let r = ringdesign_workbench::icons::button(
        ui,
        ringdesign_workbench::icons::Icon::for_label(label),
        label,
        selected,
        egui::vec2(ui.available_width(), 26.0),
    );
    editor::layout::record(ui, format!("floating/{label}"), r.rect);
    r
}

impl RingApp {
    fn toggle_palette(&mut self, palette: Palette) {
        self.editor.palette = if self.editor.palette == Some(palette) {
            None
        } else {
            Some(palette)
        };
    }

    pub(super) fn floating_tools(
        &mut self,
        ctx: &egui::Context,
        viewport: egui::Rect,
        host: &Host,
    ) {
        self.editor.floating_rects.clear();
        self.editor.floating_dragging = false;
        self.editor.hold_before = false;
        if self.tab != Tab::Ring || viewport.width() < 160.0 || viewport.height() < 64.0 {
            return;
        }
        let bounds = crate::theme::content_bounds(ctx).shrink(4.0);
        let collapsed = self.editor.workspace.rail_collapsed;
        let mut rail_position = self.editor.workspace.rail_position;
        let rail = workspace::floating(
            ctx,
            "viewport-rail",
            "Tools",
            bounds,
            egui::vec2(
                92.0,
                if collapsed {
                    42.0
                } else {
                    bounds.height().min(350.0)
                },
            ),
            (viewport.min - bounds.min + egui::vec2(0.0, 8.0)).max(egui::Vec2::ZERO),
            &mut rail_position,
            Some(if collapsed { "+" } else { "−" }),
            |ui| {
                if collapsed {
                    return;
                }
                if button(ui, "Select", self.visual.tool == Tool::Select).clicked() {
                    self.visual.select(Tool::Select);
                }
                if button(ui, "Edit", self.editor.palette == Some(Palette::Properties)).clicked() {
                    self.toggle_palette(Palette::Properties);
                }
                let tools: &[(Tool, &str)] = match self.editor.mode {
                    Mode::Shape => &[(Tool::Section, "Section"), (Tool::Measure, "Measure")],
                    Mode::Surface => &[
                        (Tool::Paint, "Paint"),
                        (Tool::Stamp, "Stamp"),
                        (Tool::Path, "Path"),
                        (Tool::Transform, "Move"),
                    ],
                    Mode::Stones => &[(Tool::Clearance, "Spacing")],
                    Mode::Casting => &[(Tool::Mould, "Mould")],
                };
                for &(tool, label) in tools {
                    if button(ui, label, self.visual.tool == tool).clicked() {
                        self.visual.select(tool);
                        if self.editor.isolate && self.visual.is_painting() {
                            self.editor.isolate = false;
                            self.request_view_update();
                        }
                        self.editor.palette = Some(Palette::Properties);
                    }
                }
                if button(ui, "Layers", self.editor.palette == Some(Palette::Layers)).clicked() {
                    self.toggle_palette(Palette::Layers);
                }
                if button(ui, "More", self.editor.palette == Some(Palette::More)).clicked() {
                    self.toggle_palette(Palette::More);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    use ringdesign_workbench::icons::{self, Icon};
                    let before = ui
                        .add_enabled_ui(self.can_compare && self.visual.tool != Tool::Mould, |ui| {
                            icons::compact(ui, Icon::Before, false)
                        })
                        .inner;
                    editor::layout::record(ui, "floating/Before", before.rect);
                    self.editor.hold_before = before.is_pointer_button_down_on();
                    let guides = icons::compact(ui, Icon::Guides, self.editor.guides);
                    editor::layout::record(ui, "floating/Guides", guides.rect);
                    if guides.clicked() {
                        self.editor.guides = !self.editor.guides;
                        self.save_prefs();
                    }
                });
                if button(ui, "Panel", self.editor.sheet.is_some()).clicked() {
                    if self.editor.sheet.is_some() {
                        self.editor.sheet = None;
                    } else {
                        self.editor.sheet = Some(Sheet::Edit);
                        self.editor.workspace.inspector_fraction = [0.32, 0.36];
                    }
                    self.save_prefs();
                }
            },
        );
        self.editor.workspace.rail_position = rail_position;
        self.editor.floating_rects.push(rail.rect);
        self.editor.floating_dragging |= rail.dragging;
        if rail.close {
            self.editor.workspace.rail_collapsed = !collapsed;
            self.save_prefs();
        } else if rail.moved {
            self.save_prefs();
        }

        let Some(palette) = self.editor.palette else {
            return;
        };
        let title = match palette {
            Palette::Properties => {
                if self.visual.tool == Tool::Select {
                    self.editor.mode.label()
                } else {
                    self.visual.tool.label()
                }
            }
            Palette::Layers => "Design layers",
            Palette::More => "Toolbox",
            Palette::Details(sheet) => sheet.label(),
        };
        let mut palette_position = self.editor.workspace.palette_position;
        let size = egui::vec2(
            if palette == Palette::Properties && self.visual.is_painting() {
                (bounds.width() - 100.0).clamp(150.0, 180.0)
            } else { (bounds.width() - 100.0).clamp(200.0, 250.0) },
            bounds.height().min(if palette == Palette::Properties {
                360.0
            } else {
                265.0
            }),
        );
        let panel = workspace::floating(
            ctx,
            "viewport-palette",
            title,
            bounds,
            size,
            egui::vec2(100.0, (bounds.height() - size.y - 8.0).max(0.0)),
            &mut palette_position,
            Some("×"),
            |ui| {
                // Names may be long; retain the full name in its tooltip.
                if palette == Palette::Properties
                    && self.visual.tool == Tool::Select
                    && matches!(self.editor.mode, Mode::Surface | Mode::Stones)
                {
                    if let Some(entry) = self
                        .selected_layer
                        .and_then(|i| self.design.layers.layers.get(i))
                    {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&entry.name)
                                    .small()
                                    .color(crate::theme::PINK_BRIGHT),
                            )
                            .truncate(),
                        )
                        .on_hover_text(&entry.name);
                    }
                }
                match palette {
                    Palette::Properties => {
                        let key = egui::Id::new((
                            "palette-fields",
                            self.editor.mode as u8,
                            self.editor.part as u8,
                            self.visual.tool as u8,
                            self.selected_layer,
                        ));
                        editor::controls::begin_compact(ui, key, &mut self.editor.active_field);
                        self.inspector_content(ui, Sheet::Edit, host);
                        editor::controls::end_compact(ui, key, &mut self.editor.active_field);
                    }
                    Palette::Layers => self.floating_layers(ui),
                    Palette::More => self.floating_more(ui, host),
                    Palette::Details(sheet) => self.inspector_content(ui, sheet, host),
                }
            },
        );
        self.editor.workspace.palette_position = palette_position;
        self.editor.floating_rects.push(panel.rect);
        self.editor.floating_dragging |= panel.dragging;
        if panel.close {
            self.editor.palette = None;
        }
        if panel.moved {
            self.save_prefs();
        }
        if self.editor.reset_workspace {
            self.editor.reset_workspace = false;
            self.editor.workspace = crate::prefs::Workspace::default();
            self.editor.palette = None;
            self.editor.sheet = Some(Sheet::Edit);
            self.save_prefs();
        }
    }

    fn floating_layers(&mut self, ui: &mut egui::Ui) {
        if self.driven_banner(ui) {
            return;
        }
        if let Some(note) = crate::layers::add_menu(ui, &mut self.design, &mut self.selected_layer)
        {
            self.status = note;
            self.mark_dirty();
        }
        ui.small("Tap a layer to edit. The checkbox includes it in the design.");
        let mut pick = None;
        let mut changed = false;
        for (i, entry) in self.design.layers.layers.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                ui.horizontal(|ui| {
                    changed |= ui.checkbox(&mut entry.enabled, "").changed();
                    let r = ui.add_sized(
                        [ui.available_width(), 32.0],
                        egui::Button::new(&entry.name)
                            .selected(self.selected_layer == Some(i))
                            .truncate(),
                    );
                    editor::layout::record(ui, format!("floating/layer/{i}"), r.rect);
                    if r.on_hover_text(format!("{} — {}", entry.name, entry.layer.kind_label()))
                        .clicked()
                    {
                        pick = Some(i);
                    }
                });
            });
        }
        if changed {
            self.mark_dirty();
        }
        if let Some(i) = pick {
            let mode = if matches!(
                self.design.layers.layers[i].layer,
                Layer::SeatPad(_) | Layer::SeatRun(_)
            ) {
                Mode::Stones
            } else {
                Mode::Surface
            };
            self.choose_mode(mode);
            self.selected_layer = Some(i);
            self.editor.stone_path = vec![i];
            self.editor.overlaps.clear();
            self.editor.active_field = if mode == Mode::Stones {
                "Stone width"
            } else {
                "Relief height"
            }
            .into();
            self.editor.palette = Some(Palette::Properties);
        }
        if button(ui, "Full layer controls", false).clicked() {
            self.editor.palette = Some(Palette::Details(Sheet::Layers));
        }
    }

    fn floating_more(&mut self, ui: &mut egui::Ui, host: &Host) {
        if button(ui, "Jewelry workflow", false).clicked() {
            self.editor.palette = Some(Palette::Details(Sheet::Workflow));
        }
        ui.collapsing("Shape & construction", |ui| {
            for (part, label) in [
                (editor::ShapePart::Band, "Band & fit"),
                (editor::ShapePart::Head, "Signet face"),
            ] {
                if button(ui, label, false).clicked() {
                    self.choose_mode(Mode::Shape);
                    self.editor.part = part;
                    self.editor.palette = Some(Palette::Properties);
                }
            }
            for (sheet, label) in [
                (Sheet::Construction, "Construction guide"),
                (Sheet::Advanced, "All shape controls"),
            ] {
                if button(ui, label, false).clicked() {
                    self.editor.palette = Some(Palette::Details(sheet));
                }
            }
        });
        ui.collapsing("Surface & stones", |ui| {
            for (tool, label) in [
                (Tool::Paint, "Paint on ring"),
                (Tool::Stamp, "Place an alpha"),
                (Tool::Path, "Draw a surface path"),
                (Tool::Transform, "Move ornament"),
            ] {
                if button(ui, label, false).clicked() {
                    self.choose_mode(Mode::Surface);
                    self.visual.select(tool);
                    self.editor.palette = Some(Palette::Properties);
                }
            }
            if button(ui, "Add stone setting", false).clicked() {
                self.choose_mode(Mode::Stones);
                self.apply_editor_edit(
                    editor::controls::Edit {
                        action: Some(editor::controls::Action::AddStone),
                        ..Default::default()
                    },
                    host,
                );
                self.editor.palette = Some(Palette::Properties);
            }
            for (tab, label) in [
                (Tab::Alphas, "Pattern library"),
                (Tab::Band, "Unrolled band painting"),
                (Tab::Tile, "Draw a repeating tile"),
            ] {
                if button(ui, label, false).clicked() {
                    self.choose_mode(Mode::Surface);
                    self.tab = tab;
                    self.editor.palette = None;
                }
            }
        });
        ui.collapsing("Inspect & export", |ui| {
            for (sheet, label) in [
                (Sheet::Findings, "Casting findings"),
                (Sheet::Report, "Measurements"),
                (Sheet::Timeline, "Edit history"),
            ] {
                if button(ui, label, false).clicked() {
                    self.editor.palette = Some(Palette::Details(sheet));
                }
            }
            for (tab, label) in [
                (Tab::Workshop, "CAD & mould workshop"),
                (Tab::Graph, "Recipe graph"),
                (Tab::Files, "Files & exports"),
            ] {
                if button(ui, label, false).clicked() {
                    self.tab = tab;
                    self.editor.palette = None;
                }
            }
        });
        ui.collapsing("Camera & display", |ui| {
            self.zoom_controls(ui);
            if self.design.shank.kind == ringdesign_core::ShankKind::Signet {
                if button(ui, "Signet face", false).clicked() {
                    self.frame_head(false);
                }
                if button(ui, "Signet 3/4", false).clicked() {
                    self.frame_head(true);
                }
            }
            for &view in crate::camera::StandardView::ALL {
                if button(ui, view.label(), false).clicked() {
                    self.pane.camera.set_view(view);
                    self.pane.actual_size = false;
                }
            }
            for &shade in ShadeMode::ALL {
                if button(ui, shade.label(), self.pane.shade == shade).clicked() {
                    self.pane.shade = shade;
                }
            }
        });
        ui.separator();
        if button(ui, "Reset workspace layout", false).clicked() {
            self.editor.reset_workspace = true;
        }
    }
}
