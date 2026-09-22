//! Viewport-first phone shell. One inspector owns the available editor space.
use super::*;
use crate::editor::{
    self, Parameter, ShapePart,
    controls::{Action, Edit},
};
use ringdesign_workbench::visual::{Pointer as VisualPointer, Tool as VisualTool};

impl RingApp {
    pub(super) fn clear_viewport_selection(&mut self) {
        let isolated = self.editor.isolate;
        self.editor.reset_selection();
        self.probe_info = None;
        self.camera_turn = None;
        self.pane.focus = [0.0; 4];
        self.editor.palette = None;
        self.selected_layer = None;
        self.visual.select(VisualTool::Select);
        self.visual.selected_stone = None;
        if let Some(ed) = &mut self.graph.ed { ed.selected = None; }
        self.choose_node(None);
        if isolated { self.request_view_update(); }
        self.status = "Selection cleared — drag to orbit, pinch to zoom".into();
    }

    fn clear_opened_menus(&mut self, ctx: &egui::Context, view: egui::Rect, manual: bool) {
        let overlays: Vec<_> = ctx.memory(|m| {
            m.areas().visible_layer_ids().into_iter()
                .filter(|layer| layer.order == egui::Order::Foreground)
                .filter_map(|layer| m.area_rect(layer.id).map(|rect| (layer.id, rect)))
                .collect()
        });
        let mut observed = overlays.clone();
        // Semantic changes can reuse the same floating Area. Their bounds also
        // keep the short settle alive while the content-height inspector reflows.
        observed.push((egui::Id::new("viewport-layout"), view));
        if let Some(sheet) = self.editor.sheet {
            observed.push((egui::Id::new(("inspector-open", sheet as u8, self.editor.mode as u8)), view));
        }
        if let Some(palette) = self.editor.palette {
            observed.push((egui::Id::new(("palette-open", format!("{palette:?}"))), view));
        }
        if !self.editor.workspace.rail_collapsed {
            observed.push((egui::Id::new("rail-expanded"), view));
        }
        let ready = self.editor.menu_avoidance.observe(&observed, manual, self.preview_mesh.is_some());
        if self.editor.menu_avoidance.settling() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if !ready { return; }
        let Some(mesh) = &self.preview_mesh else { return; };
        let projector = self.pane.camera.projector(view);
        let mut ring = egui::Rect::NOTHING;
        for p in &mesh.vertices {
            ring.extend_with(projector.at([p.0, p.1, p.2]));
        }
        let obstacles: Vec<_> = overlays.iter().map(|(_, r)| *r).collect();
        let shift = editor::visibility::clearance(view.shrink(6.0), ring.expand(4.0), &obstacles);
        if shift.length() > 0.5 {
            self.pane.camera.pan_by(shift, view);
            self.editor.menu_avoidance.record_shift(shift);
            ctx.request_repaint();
        }
    }

    pub(super) fn capture_before(&mut self) {
        if self.history.is_pending() || self.fit_next || self.editor.isolate {
            return;
        }
        let Some(mesh) = &self.preview_mesh else {
            return;
        };
        let renderer = self
            .before_renderer
            .get_or_insert_with(|| Arc::new(Mutex::new(GpuMeshRenderer::default())));
        let Ok(mut renderer) = renderer.lock() else {
            return;
        };
        renderer.set_pending(GpuMeshRenderer::stage(
            mesh,
            self.cast.as_ref(),
            (
                self.design.inner_radius_mm(),
                self.design.draft.min_section_mm,
            ),
        ));
        renderer.set_pending_gems(self.preview_gems.clone());
        self.can_compare = true;
    }

    pub(super) fn choose_mode(&mut self, mode: Mode) {
        self.visual.select(VisualTool::Select);
        let was_isolated = self.editor.isolate;
        self.tab = Tab::Ring;
        self.editor.select_mode(mode);
        if mode == Mode::Casting {
            self.pane.shade = ShadeMode::Draft;
        } else {
            self.pane.shade = ShadeMode::Metal;
        }
        if was_isolated != self.editor.isolate {
            self.request_view_update();
        }
        self.save_prefs();
    }

    pub(super) fn mode_bar(&mut self, ui: &mut egui::Ui, host: &Host) {
        if self.tab == Tab::Graph || self.editor.sheet == Some(Sheet::Graph) && self.tab == Tab::Ring {
            use ringdesign_workbench::icons::{self, Icon};
            ui.horizontal(|ui| {
                if icons::button(ui, Icon::Shape, "Model", false, egui::vec2(0.,30.)).clicked() {
                    self.choose_mode(self.editor.mode);
                }
                ui.separator();
                ui.add(Icon::Graph.image(ui, 18.)); ui.strong("Graph workspace");
                crate::theme::up_menu(ui, "Workspace", |ui| {
                    for mode in Mode::ALL {
                        if ui.button(mode.label()).clicked() { self.choose_mode(mode); ui.close(); }
                    }
                });
            });
            return;
        }
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.spacing_mut().button_padding = egui::vec2(3.0, 4.0);
        ui.horizontal(|ui| {
            let width = editor::row_width(ui.available_width(), 5, 2.0);
            for mode in Mode::ALL {
                let selected = self.tab == Tab::Ring && self.editor.mode == mode;
                let response = ringdesign_workbench::icons::button(
                    ui,
                    ringdesign_workbench::icons::Icon::for_label(mode.label()),
                    mode.label(),
                    selected,
                    egui::vec2(width, 30.0),
                );
                editor::layout::record(ui, format!("mode/{}", mode.label()), response.rect);
                if response.clicked() {
                    self.choose_mode(mode);
                    host.haptic(Haptic::Light);
                }
            }
            crate::theme::menu_popup(
                ui,
                "Tools",
                Some(egui::vec2(width, 30.0)),
                egui::RectAlign::TOP_END,
                &[egui::RectAlign::BOTTOM_END],
                egui::PopupCloseBehavior::CloseOnClick,
                |ui| {
                    if ui.button("Construction guide").clicked() {
                        self.tab = Tab::Ring;
                        self.editor.sheet = Some(Sheet::Construction);
                        self.editor.guides = false;
                        self.editor.mode = Mode::Shape;
                        self.editor.isolate = false;
                        self.editor.hold_before = false;
                        self.visual.tool = VisualTool::Select;
                    }
                    if ui
                        .button("Play build reel")
                        .on_hover_text("Replays this design's construction on screen — bare stock, each layer in turn, the stones, their cutters ghosted, the cut, a closing spin — for recording. Tap the ring to stop.")
                        .clicked()
                    {
                        self.play_reel();
                        ui.close();
                    }
                    for (tab, title) in [
                        (Tab::Alphas, "Patterns & alphas"),
                        (Tab::Band, "Paint the band"),
                        (Tab::Tile, "Draw a repeating tile"),
                        (Tab::Graph, "Recipe graph"),
                        (Tab::Workshop, "CAD & mould workshop"),
                        (Tab::Files, "Files & exports"),
                        (Tab::Bench, "Performance bench"),
                    ] {
                        if ui.button(title).clicked() {
                            if tab == Tab::Graph {
                                self.open_graph_sheet();
                                continue;
                            }
                            if self.editor.isolate {
                                self.editor.isolate = false;
                                self.request_view_update();
                            }
                            self.tab = tab;
                            self.editor.hold_before = false;
                            if matches!(tab, Tab::Alphas | Tab::Band | Tab::Tile) {
                                self.editor.select_mode(Mode::Surface);
                                self.editor.selection = None;
                                self.editor.overlaps.clear();
                            }
                        }
                    }
                    ui.separator();
                    for (sheet, title) in [
                        (Sheet::Layers, "Design layers"),
                        (Sheet::Report, "Measurements & stone report"),
                        (Sheet::Timeline, "Edit history"),
                        (Sheet::Advanced, "All design controls"),
                    ] {
                        if ui.button(title).clicked() {
                            self.tab = Tab::Ring;
                            self.editor.sheet = Some(sheet);
                        }
                    }
                },
            );
        });
    }

    pub(super) fn zoom_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (label, factor) in [("Zoom out", 1.0 / 1.2), ("Zoom in", 1.2)] {
                let response = ringdesign_workbench::icons::button(
                    ui,
                    ringdesign_workbench::icons::Icon::for_label(label),
                    label,
                    false,
                    egui::vec2(0.0, 26.0),
                );
                editor::layout::record(ui, &format!("camera/{label}"), response.rect);
                if response.clicked() {
                    self.pane.camera.zoom_by_factor(factor);
                    self.pane.actual_size = false;
                }
            }
        });
    }

    fn view_menu(&mut self, ui: &mut egui::Ui) {
        if self.tab == Tab::Graph {
            crate::theme::up_menu(ui, "View", |ui| {
                if ui.button("Fit graph").clicked() { if let Some(ed) = &mut self.graph.ed { ed.fit(); } }
                if ui.button("Show ring alongside graph").clicked() {
                    self.editor.workspace.graph_fullscreen = false;
                    self.open_graph_sheet();
                }
            });
            return;
        }
        crate::theme::up_menu(ui, "View", |ui| {
            if self.design.shank.kind == ringdesign_core::ShankKind::Signet {
                if ui.button("Signet face").clicked() {
                    self.frame_head(false);
                }
                if ui.button("Signet 3/4").clicked() {
                    self.frame_head(true);
                }
            }
            for view in crate::camera::StandardView::ALL {
                if ui.button(view.label()).clicked() {
                    self.pane.camera.set_view(*view);
                    self.pane.actual_size = false;
                }
            }
            if ui.button("Fit ring").clicked() {
                if let Some(mesh) = &self.preview_mesh {
                    self.pane.camera.fit(mesh.bounds());
                }
                self.pane.camera.zoom = 1.0;
                self.pane.camera.pan = [0.0; 2];
                self.pane.actual_size = false;
            }
            ui.separator();
            ui.menu_button("Zoom", |ui| self.zoom_controls(ui));
            ui.menu_button("Mesh detail", |ui| {
                ui.set_max_width(215.0);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                for quality in crate::ring::PreviewQuality::ALL {
                    if ui
                        .selectable_value(&mut self.preview_quality, quality, quality.label())
                        .changed()
                    {
                        self.request_view_update();
                        self.save_prefs();
                    }
                }
                ui.label("Refines after editing stops. Showcase uses more memory.");
            });
            ui.menu_button("Metal & polish", |ui| {
                let mut changed = false;
                for (i, (name, _)) in ringdesign_core::render::METAL_FINISHES.iter().enumerate() {
                    changed |= ui
                        .selectable_value(&mut self.pane.finish, i, *name)
                        .changed();
                }
                ui.separator();
                for (i, (name, _)) in ringdesign_core::render::POLISHES.iter().enumerate() {
                    changed |= ui
                        .selectable_value(&mut self.pane.polish, i, *name)
                        .changed();
                }
                if changed {
                    self.save_prefs();
                }
            });
            for &mode in ShadeMode::ALL {
                if ui
                    .selectable_label(self.pane.shade == mode, mode.label())
                    .clicked()
                {
                    self.pane.shade = mode;
                }
            }
            ui.checkbox(&mut self.pane.wireframe, "Mesh edges");
            if ui.checkbox(&mut self.show_gems, "Show stones").changed() {
                self.request_view_update();
            }
            if ui
                .checkbox(&mut self.cuts.live, "Live cuts")
                .on_hover_text("Resolve made settings — burs, heads, collets — into the ring as you edit. Off, the ring shows its cast stock alone, and builds faster.")
                .changed()
            {
                self.request_view_update();
            }
            if ui
                .checkbox(&mut self.cuts.ghost, "Show cutters")
                .on_hover_text("Draw each seat's cutter over the ring as a ghost: what the boolean takes away, where it stands.")
                .changed()
            {
                self.request_view_update();
            }
            if ui
                .checkbox(&mut self.as_cast, "Soften to sand detail")
                .changed()
            {
                self.request_view_update();
            }
            if self.px_per_mm.is_some() {
                ui.checkbox(&mut self.pane.actual_size, "Actual size (1:1)");
            }
            ui.checkbox(&mut self.editor.help, "Show editing hints");
            if ui.button("Reset workspace layout").clicked() {
                self.editor.workspace = crate::prefs::Workspace::default();
                self.editor.palette = None;
                self.editor.sheet = Some(Sheet::Edit);
                self.save_prefs();
            }
            if ui
                .checkbox(&mut self.editor.debug_layout, "Layout bounds (debug)")
                .changed()
            {
                self.save_prefs();
            }
        });
    }

    pub(super) fn studio_ui(&mut self, ui: &mut egui::Ui, host: &Host) {
        crate::theme::ambience(ui.ctx());
        if !egui::Popup::is_any_open(ui.ctx()) && !ringdesign_graph_ui::alpha_picker::is_open(ui.ctx()) && !ringdesign_workbench::feedback::is_open(ui.ctx()) && !ui.ctx().egui_wants_keyboard_input() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.clear_viewport_selection();
        }
        let safe = ui.available_rect_before_wrap();
        crate::theme::set_content_bounds(ui.ctx(), safe);
        editor::layout::begin(ui.ctx());
        ui.set_max_width(safe.width());
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        ui.spacing_mut().button_padding = egui::vec2(4.0, 2.0);
        ui.spacing_mut().interact_size.y = crate::theme::MENU_ROW_H;
        ui.spacing_mut().slider_width = (safe.width() - 170.0).clamp(70.0, 200.0);
        let typing = host.keyboard_height() > 1.0;
        if !typing {
            egui::Panel::bottom(egui::Id::new("studio-navigation"))
                .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(6, 3)))
                .show(ui, |ui| self.nav_bar(ui, host));
        } else {
            // Panel::show consumes one parent auto ID. Keep the following
            // inputs stable when Android opens/closes the keyboard.
            ui.skip_ahead_auto_ids(1);
        }
        egui::Panel::top(egui::Id::new("studio-header"))
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 2)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    self.header_file_menu(ui, host);
                    let title = if self.tab == Tab::Ring {
                        self.design.name.as_str()
                    } else {
                        self.tab.label()
                    };
                    let width = (ui.available_width() - 188.0).max(30.0);
                    ui.add_sized(
                        [width, 32.0],
                        egui::Label::new(egui::RichText::new(title).strong().size(14.0)).truncate(),
                    );
                    self.undo_row(ui);
                    use ringdesign_workbench::icons::{self, Icon};
                    let guide =
                        icons::compact(ui, Icon::Guide, self.editor.sheet == Some(Sheet::Workflow));
                    editor::layout::record(ui, "header/Guide", guide.rect);
                    if guide.clicked() {
                        self.tab = Tab::Ring;
                        self.editor.sheet = Some(Sheet::Workflow);
                        self.editor.workspace.inspector_fraction = [0.42, 0.40];
                        self.editor.palette = None;
                        self.save_prefs();
                    }
                    let panel_available = self.tab == Tab::Ring || self.tab == Tab::Graph && self.graph.ed.as_ref().is_some_and(|ed| !ed.graph().exposed.is_empty());
                    let panel = ui.add_enabled_ui(panel_available, |ui| icons::compact(ui, Icon::Panel, self.editor.sheet.is_some())).inner;
                    panel.response.clone().on_disabled_hover_text("This graph has no exposed parameters");
                    editor::layout::record(ui, "header/Panel", panel.rect);
                    if panel.clicked() && self.tab == Tab::Graph {
                        self.graph.parameters = !self.graph.parameters;
                    } else if panel.clicked() {
                        if self.editor.sheet.is_some()
                            && self
                                .editor
                                .workspace
                                .inspector_fraction
                                .iter()
                                .all(|v| *v >= 0.2)
                        {
                            self.editor.sheet = None;
                        } else {
                            self.editor.sheet = Some(Sheet::Edit);
                            self.editor.workspace.inspector_fraction = [0.32, 0.36];
                        }
                        self.save_prefs();
                    }
                    ui.add_enabled_ui(matches!(self.tab, Tab::Ring | Tab::Graph | Tab::Band | Tab::Tile | Tab::Alphas), |ui| self.view_menu(ui))
                        .response.on_disabled_hover_text("View controls are available in visual workspaces");
                });
            });
        if self.tab == Tab::Ring && self.sketch_mode.is_none() {
            if let Some(sheet) = self.editor.sheet {
                let available = ui.available_size();
                let landscape = safe.width() > safe.height() * 1.15;
                let graph = sheet == Sheet::Graph;
                let fraction = if graph {
                    self.editor.workspace.graph_fraction[landscape as usize]
                } else {
                    self.editor.workspace.inspector_fraction[landscape as usize]
                };
                let extent = if landscape || graph {
                    editor::workspace::inspector_extent(available, landscape, fraction)
                } else {
                    editor::workspace::content_height(available.y, self.editor.inspector_height)
                };
                if extent >= 42.0 {
                    let panel = if landscape {
                        egui::Panel::right(egui::Id::new("studio-inspector-side"))
                    } else {
                        egui::Panel::bottom(egui::Id::new("studio-inspector-bottom"))
                    };
                    panel
                        .exact_size(extent)
                        .resizable(false)
                        .frame(
                            egui::Frame::new()
                                .fill(egui::Color32::from_rgb(20, 20, 25))
                                .stroke(egui::Stroke::new(1.5, if graph { crate::theme::PINK_BRIGHT } else { crate::theme::AQUA.gamma_multiply(0.55) }))
                                .inner_margin(6),
                        )
                        .show(ui, |ui| {
                            ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
                            editor::layout::record(ui, "inspector", ui.max_rect());
                            let share = if graph {
                                &mut self.editor.workspace.graph_fraction[landscape as usize]
                            } else {
                                &mut self.editor.workspace.inspector_fraction[landscape as usize]
                            };
                            if (landscape || graph)
                                && editor::workspace::splitter(ui, share, available, landscape)
                            {
                                self.save_prefs();
                            }
                            if graph {
                                self.graph_panel(ui, host, true);
                                return;
                            }
                            let wanted = self.inspector(ui, sheet, host, landscape) + 12.0;
                            if !landscape {
                                let next = editor::workspace::content_height(available.y, Some(wanted));
                                self.editor.inspector_height = Some(wanted);
                                if (next - extent).abs() > 0.5 {
                                    ui.ctx().request_repaint();
                                }
                            }
                        });
                }
            }
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(4))
            .show(ui, |ui| {
                if self.sketch_mode.is_some() {
                    self.sketch_pad(ui, host);
                    return;
                }
                if matches!(self.tab, Tab::Band | Tab::Tile | Tab::Alphas)
                    && (ui.available_height() > 350.0 || ui.available_width() > 600.0)
                {
                    let wide = ui.available_width() > ui.available_height() * 1.15;
                    let (panel, extent) = if wide {
                        (
                            egui::Panel::left(egui::Id::new("tool-live-preview-side")),
                            ui.available_width() * 0.42,
                        )
                    } else {
                        (
                            egui::Panel::top(egui::Id::new("tool-live-preview")),
                            (ui.available_height() * 0.34).clamp(155.0, 270.0),
                        )
                    };
                    panel
                        .exact_size(extent)
                        .resizable(false)
                        .frame(egui::Frame::new())
                        .show(ui, |ui| self.visual_ring(ui, host));
                }
                match self.tab {
                    Tab::Ring => self.ring_tab(ui, host),
                    Tab::Workshop => self.workshop_tab(ui, host),
                    Tab::Band => self.paint_tab(ui, host, Domain::Band),
                    Tab::Tile => self.paint_tab(ui, host, Domain::Tile),
                    Tab::Graph => self.graph_tab(ui, host),
                    Tab::Alphas => self.alphas_tab(ui, host),
                    Tab::Files => self.files_tab(ui, host),
                    Tab::Bench => self.bench_tab(ui, host),
                }
                editor::layout::record(ui, "tool/content", ui.min_rect());
            });
        ringdesign_graph_ui::alpha_picker::show_in(ui.ctx(), crate::theme::content_bounds(ui.ctx()));
        if let Some(url) = ringdesign_workbench::feedback::show(ui.ctx(), env!("CARGO_PKG_VERSION"), self.editor.mode.label()) { host.open_url(url); }
        if self.editor.debug_layout {
            editor::layout::draw(ui, safe);
            let mut data = editor::layout::report(ui.ctx(), safe);
            data["mode"] = serde_json::json!(self.editor.mode.label());
            data["process_id"] = serde_json::json!(std::process::id());
            data["mesh"] = serde_json::json!({
                "quality": self.preview_quality,
                "triangles": self.preview_mesh.as_ref().map(|m| m.faces.len()),
                "building": self.preview_in_flight || self.dirty_at.is_some(),
                "generation": self.generation,
                "finish": self.pane.finish,
                "polish": self.pane.polish,
            });
            data["pointer"] = serde_json::json!({"hover":ui.input(|i|i.pointer.hover_pos()).map(|p|[p.x,p.y]),"down":ui.input(|i|i.pointer.any_down()),"pixels_per_point":ui.ctx().pixels_per_point()});
            data["pen"] = serde_json::json!({"tool":format!("{:?}",self.probe.tool),"hover":self.probe.hover});
            data["history"] = serde_json::json!({
                "pending": self.history.is_pending(),
                "undo": self.history.can_undo(),
                "redo": self.history.can_redo(),
                "layers": self.design.layers.layers.len(),
            });
            data["workspace"] = serde_json::json!(self.editor.workspace);
            data["palette"] = serde_json::json!(format!("{:?}", self.editor.palette));
            data["navigation"] = serde_json::to_value(self.pane.navigation).unwrap_or_default();
            data["selection"] = serde_json::json!({"layer":self.selected_layer,"node":self.graph.shown,"hit":self.editor.selection.is_some(),"stone":self.editor.stone,"handles":self.editor.handles_active,"cutters":self.cuts.ghost,"tab":self.tab.label(),"sheet":format!("{:?}",self.editor.sheet)});
            data["visual"] = serde_json::json!({"tool":format!("{:?}",self.visual.tool),"path_points":self.visual.path.curve.points.len(),"navigating":self.visual.navigating()});
            data["camera"] = serde_json::json!({"yaw":self.pane.camera.yaw,"pitch":self.pane.camera.pitch,"zoom":self.pane.camera.zoom,"pan":self.pane.camera.pan});
            data["menu_avoidance"] = serde_json::json!({"shifts":self.editor.menu_avoidance.shifts,"last_delta":self.editor.menu_avoidance.last_delta});
            let text = data.to_string();
            if text != self.editor.last_layout {
                log::info!("mobile-layout {text}");
                if let Some(root) = &self.data_root {
                    let _ = std::fs::write(root.join("layout-debug.json"), &text);
                }
                self.editor.last_layout = text;
            }
        }
    }

    fn inspector(&mut self, ui: &mut egui::Ui, sheet: Sheet, host: &Host, landscape: bool) -> f32 {
        let top = ui.cursor().top();
        let title = if sheet == Sheet::Edit {
            self.editor.mode.label()
        } else {
            sheet.label()
        };
        ui.horizontal(|ui| {
            ui.add_sized(
                [(ui.available_width() - if landscape { 70.0 } else { 36.0 }).max(30.0), 26.0],
                egui::Label::new(egui::RichText::new(title).strong()).truncate(),
            );
            use ringdesign_workbench::icons::{self, Icon};
            if landscape {
                let expand = icons::compact(ui, Icon::Expand, false);
                editor::layout::record(ui, "inspector/expand", expand.rect);
                if expand.clicked() {
                    self.editor.workspace.inspector_fraction[1] = 0.4;
                    self.save_prefs();
                }
            }
            if icons::compact(ui, Icon::Close, false).clicked() {
                self.editor.sheet = None;
                self.save_prefs();
            }
        });
        let width = ui.available_width();
        ui.spacing_mut().slider_width = (width - 150.0).clamp(60.0, 170.0);
        let scroll = crate::theme::scroll_vertical()
            .id_salt(("inspector-scroll", sheet as u8, self.editor.mode as u8))
            .auto_shrink([false, true])
            .min_scrolled_height(1.0)
            .max_height(ui.available_height().max(1.0))
            .max_width(width)
            .show(ui, |ui| {
                let inner_width = ui.available_width();
                ui.set_max_width(inner_width);
                self.inspector_content(ui, sheet, host);
                editor::layout::record(ui, "inspector/content", ui.min_rect());
            });
        scroll.inner_rect.top() - top + scroll.content_size.y
    }

    pub(super) fn inspector_content(&mut self, ui: &mut egui::Ui, sheet: Sheet, host: &Host) {
        match sheet {
            Sheet::Workflow => {
                if let Some(action) = ringdesign_workbench::workflow::show(ui) {
                    use ringdesign_workbench::workflow::Action as Next;
                    self.editor.palette = None;
                    self.editor.sheet = Some(Sheet::Edit);
                    match action {
                        Next::Fit => {
                            self.choose_mode(Mode::Shape);
                            self.editor.part = ShapePart::Band;
                            self.editor.parameter = Parameter::Bore;
                        }
                        Next::Shape => {
                            self.choose_mode(Mode::Shape);
                            self.editor.part = ShapePart::Head;
                        }
                        Next::Tool(tool) => {
                            self.choose_mode(if tool == VisualTool::Clearance {
                                Mode::Stones
                            } else if tool == VisualTool::Section {
                                Mode::Shape
                            } else {
                                Mode::Surface
                            });
                            self.visual.select(tool);
                        }
                        Next::Stones => self.choose_mode(Mode::Stones),
                        Next::Checks => {
                            self.choose_mode(Mode::Casting);
                            self.editor.sheet = Some(Sheet::Findings);
                        }
                        Next::Export => self.tab = Tab::Files,
                    }
                    self.save_prefs();
                }
            }
            Sheet::Construction => {
                let before = self.design.clone();
                let event = self.construction.ui(ui, &mut self.design);
                if event.changed {
                    self.history.commit(&before);
                    self.history.commit(&self.design);
                    self.design.unpack_embedded(Arc::make_mut(&mut self.lib));
                    self.design.bake_all(Arc::make_mut(&mut self.lib));
                    self.thumbs.clear();
                    self.editor.reset_selection();
                    self.selected_layer = None;
                    self.fit_next = true;
                    self.mark_dirty();
                }
                if let Some(view) = event.view {
                    use ringdesign_workbench::construction::View;
                    self.pane.camera.yaw = self.design.shank.head.theta_deg.to_radians() as f32;
                    self.pane.camera.pitch = match view {
                        View::Seal => 0.,
                        View::ThreeQuarter => -0.72,
                        View::Cheek => -1.30,
                        View::Bore => -std::f32::consts::FRAC_PI_2 + 0.001,
                    };
                    if matches!(view, View::ThreeQuarter) {
                        self.pane.camera.yaw -= 0.48;
                    }
                    self.pane.camera.pan = [0.; 2];
                    self.pane.camera.zoom = 1.23;
                    self.pane.shade = ShadeMode::Metal;
                    self.pane.actual_size = false;
                }
            }
            Sheet::Edit => {
                if self.driven_banner(ui) {
                    return;
                }
                if let Some(info) = &self.probe_info {
                    ui.label(
                        egui::RichText::new(info)
                            .small()
                            .color(crate::theme::AQUA_BRIGHT),
                    );
                    if ui.small_button("Dismiss surface reading").clicked() {
                        self.probe_info = None;
                    }
                }
                let choices: &[VisualTool] = match self.editor.mode {
                    Mode::Shape => &[VisualTool::Select, VisualTool::Section, VisualTool::Measure],
                    Mode::Surface => &[
                        VisualTool::Select,
                        VisualTool::Paint,
                        VisualTool::Stamp,
                        VisualTool::Path,
                        VisualTool::Transform,
                    ],
                    Mode::Stones => &[VisualTool::Select, VisualTool::Clearance],
                    Mode::Casting => &[VisualTool::Select, VisualTool::Mould],
                };
                if !editor::controls::is_compact(ui) {
                    self.visual.chooser(ui, choices);
                }
                if self.visual.tool != VisualTool::Select {
                    if self.editor.isolate && self.visual.is_painting() {
                        self.editor.isolate = false;
                        self.request_view_update();
                    }
                    self.visual.controls(ui, &self.design, &self.lib);
                    if let Some(layer) = self.visual.apply_controls(&mut self.design) {
                        self.selected_layer = Some(layer);
                        self.mark_dirty();
                    }
                    if self.visual.tool != VisualTool::Clearance {
                        return;
                    }
                    ui.separator();
                }
                let edit = match self.editor.mode {
                    Mode::Shape => editor::controls::shape(ui, &mut self.editor, &mut self.design),
                    Mode::Surface => editor::controls::surface(
                        ui,
                        &mut self.editor,
                        &mut self.design,
                        &mut self.selected_layer,
                    ),
                    Mode::Stones => editor::controls::stones(
                        ui,
                        &mut self.editor,
                        &mut self.design,
                        &mut self.selected_layer,
                    ),
                    Mode::Casting => {
                        ui.horizontal_wrapped(|ui| {
                            for (mode, label) in [
                                (ShadeMode::Draft, "Axial draft"),
                                (ShadeMode::Wall, "Wall"),
                                (ShadeMode::Halves, "Pull sides"),
                            ] {
                                if ui
                                    .selectable_label(self.pane.shade == mode, label)
                                    .clicked()
                                {
                                    self.pane.shade = mode;
                                }
                            }
                        });
                        match self.pane.shade {
                            ShadeMode::Draft => {
                                ui.horizontal_wrapped(|ui| {
                                    use ringdesign_core::castability::FaceClass;
                                    for (class, label) in [
                                        (FaceClass::Good, "Good draft"),
                                        (FaceClass::Marginal, "Low draft"),
                                        (FaceClass::Vertical, "Vertical"),
                                        (FaceClass::Undercut, "Undercut"),
                                    ] {
                                        let rgb = class.rgb().map(|c| (c * 255.0) as u8);
                                        ui.label(egui::RichText::new(label).small().color(
                                            egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
                                        ));
                                    }
                                });
                            }
                            ShadeMode::Wall => {
                                ui.label(egui::RichText::new(format!("Red: ≤ {:.2} mm · amber/green/blue: thicker metal · grey: bore",self.design.draft.min_section_mm)).small());
                            }
                            ShadeMode::Halves => {
                                ui.label(egui::RichText::new("Blue: +Z-facing · gold: −Z-facing · yellow: vertical walls").small());
                            }
                            _ => {}
                        }
                        if self.editor.check_pending {
                            ui.colored_label(
                                crate::theme::AQUA_BRIGHT,
                                "Checking the changed design…",
                            );
                        } else if let Some(report) = &self.field {
                            let (color, text) = field_chip(report, self.design.draft.process);
                            ui.colored_label(color, text);
                        }
                        ui.label(egui::RichText::new("Nominal ring, ±Z colours. Workshop checks the prepared pattern and chosen pull.").small().weak());
                        editor::controls::casting(ui, &mut self.editor, &mut self.design)
                    }
                };
                self.apply_editor_edit(edit, host);
            }
            Sheet::Layers => {
                let previous = self.selected_layer;
                if let Some(note) =
                    crate::layers::add_menu(ui, &mut self.design, &mut self.selected_layer)
                {
                    self.status = note;
                    self.mark_dirty();
                }
                let ctx = self.design.field_context();
                if crate::layers::sheet(
                    ui,
                    &mut self.design.layers,
                    &ctx,
                    &self.dfm,
                    &mut self.selected_layer,
                ) {
                    self.editor.stone_path.clear();
                    self.mark_dirty();
                }
                if previous != self.selected_layer {
                    self.editor.overlaps.clear();
                    self.editor.stone_path.clear();
                    if self.editor.isolate {
                        self.editor.isolate = self.selected_layer.is_some();
                        self.request_view_update();
                    }
                }
            }
            Sheet::Advanced => self.design_tab(ui),
            Sheet::Findings => {
                if self.editor.check_pending {
                    ui.label("Findings below belong to the last completed check.");
                }
                if self.dfm_pending {
                    ui.label("Checking fine detail in the background…");
                } else if self.dfm.is_empty() {
                    ui.label("No fine-detail findings in the last completed check.");
                } else {
                    self.dfm_sheet(ui);
                }
                if let Some(report) = &self.field {
                    for note in &report.notes {
                        ui.label(note);
                    }
                }
                if ui.button("Detailed mould analysis & repairs").clicked() {
                    self.tab = Tab::Workshop;
                }
            }
            Sheet::Report => {
                let mut close = false;
                crate::report::sheet(
                    ui,
                    self.report.as_ref(),
                    self.stones.as_ref(),
                    &self.design.size.display(),
                    &mut close,
                );
                if close {
                    self.editor.sheet = None;
                }
            }
            Sheet::Timeline => self.timeline_sheet(ui),
            Sheet::Graph => self.graph_panel(ui, host, true),
        }
    }

    pub(super) fn apply_editor_edit(&mut self, edit: Edit, host: &Host) {
        if edit.changed {
            self.mark_dirty();
        }
        if edit.view_changed {
            self.request_view_update();
        }
        let Some(action) = edit.action else {
            return;
        };
        match action {
            Action::Patterns => {
                self.tab = Tab::Alphas;
                self.editor.mode = Mode::Surface;
            }
            Action::Layers => self.open_editor_sheet(Sheet::Layers),
            Action::Stones => self.choose_mode(Mode::Stones),
            Action::FrameHead => self.frame_head(true),
            Action::Advanced => self.open_editor_sheet(Sheet::Advanced),
            Action::Workshop => self.tab = Tab::Workshop,
            Action::Findings => self.open_editor_sheet(Sheet::Findings),
            Action::Report => self.open_editor_sheet(Sheet::Report),
            Action::AddStone => {
                let ctx = self.design.field_context();
                let mut seat = ringdesign_core::field::SeatPadLayer::default();
                seat.theta_deg = self
                    .editor
                    .selection
                    .as_ref()
                    .map(|h| h.theta_deg)
                    .unwrap_or(ringdesign_core::profile::TOP_DEG);
                seat.v_mm = self
                    .editor
                    .selection
                    .as_ref()
                    .map(|h| h.v_mm)
                    .unwrap_or(ctx.crest_v_mm);
                seat.style = self.stone.style;
                seat.fit_stone(self.stone.gem());
                crate::layers::add_layer(
                    &mut self.design.layers,
                    &mut self.selected_layer,
                    "Stone setting",
                    Layer::SeatPad(seat),
                );
                self.editor.stone_path = self.selected_layer.into_iter().collect();
                self.editor.active_field = "Stone width".into();
                self.mark_dirty();
                host.haptic(Haptic::Light);
            }
            Action::SketchFace => {
                self.sketch_mode = Some(crate::sketch::Mode::Face);
                self.sketch.clear();
            }
            Action::SketchSection => {
                self.sketch_mode = Some(crate::sketch::Mode::Section);
                self.sketch.clear();
            }
        }
    }

    pub(super) fn request_view_update(&mut self) {
        self.can_compare = false;
        self.live_requested = true;
        self.dirty_at = Some(Instant::now());
    }

    fn open_editor_sheet(&mut self, sheet: Sheet) {
        if self.editor.palette.is_some() {
            self.editor.palette = Some(editor::workspace::Palette::Details(sheet));
        } else {
            self.editor.sheet = Some(sheet);
        }
    }

    pub(super) fn frame_head(&mut self, angled: bool) {
        self.pane.camera.yaw = self.design.shank.head.theta_deg.to_radians() as f32
            - if angled {
                std::f32::consts::FRAC_PI_8
            } else {
                0.0
            };
        self.pane.camera.pitch = if angled { -0.55 } else { 0.0 };
        self.pane.camera.pan = [0.0; 2];
        self.pane.actual_size = false;
    }

    pub(super) fn visual_ring(&mut self, ui: &mut egui::Ui, host: &Host) {
        self.visual.poll();
        let mould_active = self.visual.tool == VisualTool::Mould && self.visual.study.is_some();
        if mould_active {
            if let Some(study) = &self.visual.study {
                if self.mould_serial != self.visual.study_serial {
                    if let Ok(mut r) = self.mould_renderer.lock() {
                        r.set_pending(GpuMeshRenderer::stage(
                            &study.pattern,
                            None,
                            (
                                self.design.inner_radius_mm() * study.scale,
                                self.design.draft.min_section_mm,
                            ),
                        ));
                        r.set_pending_gems(Vec::new());
                        r.set_pending_ghost(Vec::new());
                    }
                    self.mould_serial = self.visual.study_serial;
                }
                if self.mould_camera.is_none() {
                    self.mould_camera = Some(self.pane.camera);
                    self.pane.camera.zoom = 1.0;
                    let bounds = study.pattern.bounds().map(|(a, b)| {
                        let p = study.report.frame.z.map(|v| v.abs() as f32 * 16.0);
                        (
                            ringdesign_core::mesh::Vec3(a.0 - p[0], a.1 - p[1], a.2 - p[2]),
                            ringdesign_core::mesh::Vec3(b.0 + p[0], b.1 + p[1], b.2 + p[2]),
                        )
                    });
                    self.pane.camera.fit(bounds);
                }
            }
        } else if let Some(camera) = self.mould_camera.take() {
            self.pane.camera = camera;
        }
        self.pane.clip_plane = self.visual.clip();
        if self.visual.wants_repaint() {
            ui.ctx().request_repaint();
        }
        egui::Panel::bottom(egui::Id::new("viewport-selection-bar"))
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(4,3)).stroke(egui::Stroke::new(1., crate::theme::AQUA.gamma_multiply(0.45))))
            .show(ui, |ui| {
                use ringdesign_workbench::icons::{self, Icon};
                ui.horizontal(|ui| {
                    let selected = self.editor.selection.is_some() || self.selected_layer.is_some() || self.graph.shown.is_some() || self.visual.tool != VisualTool::Select || self.editor.mode == Mode::Shape && self.editor.guides && self.editor.handles_active;
                    let clear = ui.add_enabled_ui(selected, |ui| icons::button(ui, Icon::Close, "Clear", false, egui::vec2(62.,28.))).inner;
                    editor::layout::record(ui, "viewport/Clear", clear.rect);
                    if clear.clicked() { self.clear_viewport_selection(); }
                    if self.cuts.ghost {
                        let cutters = icons::button(ui, Icon::Cutters, "Cutters", true, egui::vec2(0.,28.));
                        editor::layout::record(ui, "viewport/Cutters", cutters.rect);
                        if cutters.clicked() { self.cuts.ghost = false; self.request_view_update(); self.save_prefs(); }
                    }
                    ui.add(Icon::Select.image(ui, 16.));
                    ui.add(egui::Label::new(egui::RichText::new("Drag to orbit · pinch to zoom").small().color(crate::theme::INK_DIM)).truncate());
                });
            });
        let rect = ui.available_rect_before_wrap();
        self.floating_tools(ui.ctx(), rect, host);
        if self.tab == Tab::Band && !self.pane.navigation.locked
            && !ui.input(|i|i.pointer.hover_pos()).is_some_and(|p|rect.contains(p)) {
            let cam=&mut self.pane.camera;
            if let Some(pose)=ringdesign_workbench::paint_preview::follow(ui,cam.pose(),cam.target,cam.half_extent()*cam.zoom/1.15) {
                cam.set_pose(pose); self.camera_turn=None;
            }
        }
        let nav = ringdesign_workbench::navigation::show(
            ui, rect, ui.id().with("phone-view"), &mut self.pane.navigation,
            [self.pane.camera.yaw, self.pane.camera.pitch, self.pane.camera.roll], self.design.shank.head.theta_deg as f32,
        );
        for (name, r) in &nav.controls { editor::layout::record(ui, format!("navigator/{name}"), *r); }
        if let Some(action) = nav.action {
            let angles = action.apply([self.pane.camera.yaw, self.pane.camera.pitch, self.pane.camera.roll], self.design.shank.head.theta_deg as f32);
            if action.recentres() {
                // A view from the cube eases in, as a chosen node's does.
                let from = self.pane.camera.pose();
                let to = crate::focus::Pose { yaw: angles[0], pitch: angles[1], roll: angles[2], pan: [0.0; 2], ..from };
                self.camera_turn = Some(crate::focus::Turn::new(from, to));
            } else {
                self.camera_turn = None;
                self.pane.camera.yaw = angles[0];
                self.pane.camera.pitch = angles[1];
                self.pane.camera.roll = angles[2];
            }
            self.pane.actual_size = false;
            ui.ctx().request_repaint();
        }
        if nav.changed { self.save_prefs(); }
        let pointer = ui.input(|i| i.pointer.press_origin().or(i.pointer.interact_pos()));
        let explicit_navigation = crate::ring::pinch_in(ui, rect).is_some_and(|m| m.num_touches >= 2)
            || crate::paint::barrel(self.probe.buttons).is_some();
        let floating_blocked = self.editor.floating_dragging
            || pointer.is_some_and(|p| nav.rect.contains(p) || self.editor.floating_rects.iter().any(|r| r.contains(p)))
            || pointer.is_some_and(|p| {
                ui.ctx().layer_id_at(p).is_some_and(|layer| layer != ui.layer_id())
            });
        let manual_navigation = explicit_navigation || nav.action.is_some()
            || ui.input(|i| i.pointer.any_down())
                && !floating_blocked && pointer.is_some_and(|p| rect.contains(p));
        if manual_navigation && nav.action.is_none() {
            self.camera_turn = None;
        }
        self.reel_caption = if self.reel.is_some() {
            let tapped = pointer.is_some_and(|p| rect.contains(p)) && ui.input(|i| i.pointer.primary_clicked()) && !floating_blocked;
            self.advance_reel(ui.ctx(), tapped)
        } else {
            None
        };
        self.advance_camera_turn(ui.ctx());
        self.clear_opened_menus(ui.ctx(), rect, manual_navigation);
        let accepted = crate::paint::accepts(crate::paint::Tool::from_code(self.probe.tool), self.visual.stylus_only);
        let camera = self.pane.camera;
        let projector = camera.projector(rect);
        let visual_blocked = self.preview_mesh.as_ref().is_some_and(|mesh| {
            self.visual.route_pointer(ui, rect, mesh,
                |p| projector.at(p.map(|v| v as f32)), |p| camera.ray(rect, p),
                explicit_navigation, accepted && !floating_blocked)
        });
        let navigating = explicit_navigation || self.visual.navigating();
        let blocked = floating_blocked
            || visual_blocked
            || (self.visual.tool == VisualTool::Select
                && editor::overlay::blocks_orbit(
                    &self.editor,
                    &self.design,
                    &self.pane.camera,
                    rect,
                    pointer,
                ));
        let renderer = if mould_active {
            &self.mould_renderer
        } else if self.editor.hold_before {
            self.before_renderer.as_ref().unwrap_or(&self.renderer)
        } else {
            &self.renderer
        };
        let previous_shade = self.pane.shade;
        if mould_active {
            self.pane.shade = ShadeMode::Metal;
        }
        let view = self.pane.ui(ui, renderer, self.px_per_mm, blocked);
        self.pane.shade = previous_shade;
        editor::layout::record(ui, "viewport", view.rect);
        let can_edit = self.design.graph.is_none() && !self.editor.hold_before;
        let parting = self
            .field
            .as_ref()
            .map(|f| f.parting_z_mm)
            .unwrap_or(self.design.draft.parting_z_mm);
        let graph_sheet = self.editor.sheet == Some(Sheet::Graph);
        let (changed, stone_pick) = if !floating_blocked
            && !graph_sheet
            && matches!(self.visual.tool, VisualTool::Select | VisualTool::Clearance)
        {
            editor::overlay::draw(
                ui,
                view.rect,
                &self.pane.camera,
                &mut self.editor,
                &mut self.design,
                self.selected_layer,
                parting,
                can_edit,
            )
        } else {
            (false, None)
        };
        if changed {
            self.mark_dirty();
        }
        if let Some(stone) = stone_pick {
            self.editor.stone = Some(stone);
            self.editor.stone_path = editor::picking::stone_paths(&self.design)
                .get(stone)
                .cloned()
                .unwrap_or_default();
            self.selected_layer = self.editor.stone_path.first().copied();
            if self.editor.sheet.is_some() {
                self.editor.sheet = Some(Sheet::Edit);
            }
            self.editor.active_field = "Stone width".into();
        } else if view.response.clicked()
            && !blocked
            && !self.editor.hold_before
            && matches!(self.visual.tool, VisualTool::Select | VisualTool::Clearance)
        {
            if let (Some(mesh), Some(pos)) =
                (&self.preview_mesh, view.response.interact_pointer_pos())
            {
                let (origin, direction) = self.pane.camera.ray(view.rect, pos);
                let hit = editor::picking::hit(&self.design, &self.lib, mesh, origin, direction);
                if hit.is_none() { self.clear_viewport_selection(); }
                if let (true, Some(hit)) = (graph_sheet, &hit) {
                    // Under the graph a tap asks which node made this metal.
                    let node = crate::focus::layer_behind(&self.design, &self.lib, hit)
                        .and_then(|layer| self.graph.node_for_layer(layer));
                    match (node, &mut self.graph.ed) {
                        (Some(node), Some(ed)) => {
                            ed.focus(node);
                            host.haptic(Haptic::Selection);
                        }
                        _ => self.status = "bare band here: no layer's node to open".into(),
                    }
                } else if let Some(hit) = hit {
                    match self.editor.mode {
                        Mode::Shape => {
                            let delta = ringdesign_core::field::wrap_delta(
                                hit.theta_deg - self.design.shank.head.theta_deg,
                                360.0,
                            )
                            .abs();
                            self.editor.part = if self.design.shank.kind
                                == ringdesign_core::ShankKind::Signet
                                && delta < 45.0
                            {
                                ShapePart::Head
                            } else {
                                ShapePart::Band
                            };
                            self.editor.parameter = if self.editor.part == ShapePart::Head {
                                Parameter::HeadLength
                            } else {
                                Parameter::Width
                            };
                            self.editor.active_field = self.editor.parameter.label().into();
                        }
                        Mode::Surface => {
                            self.editor.overlaps =
                                editor::picking::layers_at(&self.design, &self.lib, &hit);
                            if self.editor.isolate {
                                self.editor
                                    .overlaps
                                    .retain(|&i| Some(i) == self.selected_layer);
                            }
                            if !self.editor.isolate {
                                self.selected_layer = self.editor.overlaps.first().copied();
                            }
                            self.editor.active_field = "Relief height".into();
                        }
                        _ => {}
                    }
                    self.editor.handles_active = true;
                    self.editor.selection = Some(hit);
                    if self.editor.sheet.is_some() {
                        self.editor.sheet = Some(Sheet::Edit);
                    }
                }
            }
        }
        if self.tab == Tab::Band {
            let project=self.pane.camera.projector(view.rect);
            ringdesign_workbench::paint_preview::draw(ui,view.rect,|p|project.at(p));
        }
        if !floating_blocked && self.visual.tool == VisualTool::Select {
            if let Some(mesh) = &self.preview_mesh {
                let camera = self.pane.camera; let project = camera.projector(view.rect);
                ringdesign_workbench::hover::show(ui, view.rect, &view.response, &self.design, &self.lib, mesh,
                    |p| camera.ray(view.rect,p), |p| project.at(p), |hit| {
                        if graph_sheet { crate::focus::layer_behind(&self.design,&self.lib,hit).and_then(|i| self.graph.node_for_layer(i)).map(|_| "Graph feature".into()) }
                        else if self.editor.mode == Mode::Surface { editor::picking::layers_at(&self.design,&self.lib,hit).first().filter(|&&i| !self.editor.isolate || Some(i) == self.selected_layer).map(|&i| self.design.layers.layers[i].name.clone()) }
                        else if self.editor.mode == Mode::Shape { Some(if self.design.shank.kind == ringdesign_core::ShankKind::Signet && ringdesign_core::field::wrap_delta(hit.theta_deg-self.design.shank.head.theta_deg,360.).abs()<45. { "Signet head" } else { "Band" }.into()) }
                        else { None }
                    });
            }
        }
        if let Some((origin, direction)) = view.probe {
            if self.visual.tool == VisualTool::Select && !blocked && !graph_sheet {
                self.probe(origin, direction);
            }
        }
        if !self.editor.hold_before && !floating_blocked {
            if let Some(mesh) = self.preview_mesh.clone() {
                let camera = self.pane.camera;
                let project = camera.projector(view.rect);
                let pressure = ui
                    .input(|i| {
                        i.events.iter().rev().find_map(|event| match event {
                            egui::Event::Touch { force, .. } => *force,
                            _ => None,
                        })
                    })
                    .unwrap_or(1.0);
                let accepted = crate::paint::accepts(
                    crate::paint::Tool::from_code(self.probe.tool),
                    self.visual.stylus_only,
                );
                let edit = self.visual.draw(
                    ui,
                    view.rect,
                    &view.response,
                    &mut self.design,
                    &self.lib,
                    &mesh,
                    |p| project.at(p.map(|v| v as f32)),
                    |p| camera.ray(view.rect, p),
                    VisualPointer {
                        pressure,
                        tilt: [self.probe.tilt, self.probe.azimuth],
                        accepted,
                        navigating,
                    },
                );
                if let Some(index) = edit.drawing {
                    self.bake(index);
                }
                if let Some(layer) = edit.layer {
                    self.selected_layer = Some(layer);
                }
                if edit.changed() {
                    self.mark_dirty();
                    // Completed viewport gestures are already discrete edits.
                    // Record them now so Undo is ready as soon as they finish.
                    self.history.commit(&self.design);
                    ui.ctx().request_repaint();
                }
            }
        }
        if self.pane.navigation.magnifier && !floating_blocked && !navigating && !self.editor.hold_before {
            if let Some((contact, reach)) = self.visual.placement_focus(ui, view.rect) {
                let mut obstacles = self.editor.floating_rects.clone(); obstacles.push(nav.rect);
                let lens = ringdesign_workbench::loupe::show(ui, view.rect, contact, reach, &obstacles);
                editor::layout::record(ui, "viewport/magnifier", lens);
            }
        }
        let node_words = if graph_sheet { self.node_words() } else { None };
        let state = if let Some(words) = &node_words {
            words.clone()
        } else if self.editor.hold_before {
            "Before latest edit — release to return".to_string()
        } else if self.editor.isolate {
            "Isolated layer preview · full design preserved".to_string()
        } else if self.editor.check_pending {
            "Updating shape / checking…".to_string()
        } else if self.dfm_pending {
            "Checking fine detail…".to_string()
        } else {
            format!(
                "{}   ·   {:.2} mm opening   ·   {} layers",
                self.design.size.display(),
                self.design.inner_radius_mm() * 2.0,
                self.design.layers.layers.len()
            )
        };
        editor::overlay::tag(
            ui,
            view.rect,
            view.rect.center_bottom() - egui::vec2(0.0, 19.0),
            &state,
            if node_words.is_some() { crate::theme::PINK_BRIGHT } else { crate::theme::INK_DIM },
        );
        if let Some(caption) = &self.reel_caption {
            let at = view.rect.center_bottom() - egui::vec2(0.0, 74.0);
            let galley = ui.painter().layout(caption.clone(), egui::FontId::proportional(19.0), crate::theme::INK, view.rect.width() - 48.0);
            let plate = egui::Rect::from_center_size(at, galley.size() + egui::vec2(28.0, 18.0));
            ui.painter().rect_filled(plate, 12.0, egui::Color32::from_rgba_unmultiplied(8, 8, 12, 214));
            ui.painter().rect_stroke(plate, 12.0, egui::Stroke::new(1.0, crate::theme::PINK_BRIGHT), egui::StrokeKind::Inside);
            ui.painter().galley(plate.center() - galley.size() * 0.5, galley, crate::theme::INK);
        }
        if self.editor.help {
            editor::overlay::tag(
                ui,
                view.rect,
                view.rect.center_top() + egui::vec2(0.0, 35.0),
                self.editor.mode.hint(),
                crate::theme::INK,
            );
        }
    }
}
