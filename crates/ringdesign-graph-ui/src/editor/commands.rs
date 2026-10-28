use super::*;
use ringdesign_graph::fragment::Fragment;

fn clipboard_id() -> egui::Id {
    egui::Id::unique("ringdesigner-node-clipboard")
}

impl Editor {
    fn selection(&self, ctx: &egui::Context) -> Vec<NodeId> {
        let ids = self.selected_nodes(ctx);
        if ids.is_empty() {
            self.selected
                .filter(|id| self.graph.contains(*id))
                .into_iter()
                .collect()
        } else {
            ids
        }
    }

    fn select(&mut self, ctx: &egui::Context, ids: &[NodeId]) {
        self.selected = ids.first().copied();
        if let Some(snarl) = self.snarl_id {
            let shown: BTreeSet<SnarlId> = ids.iter().filter_map(|id| self.view_node_of(*id)).filter_map(|v| self.ids.to_snarl.get(&v).copied()).collect();
            egui_snarl::ui::set_selected_nodes(snarl, ctx, shown);
        }
    }

    fn paste_fragment(
        &mut self,
        ui: &Ui,
        reg: &Registry,
        fragment: &Fragment,
        at: [f32; 2],
    ) -> bool {
        let mut graph = self.graph.clone();
        match fragment.paste(&mut graph, at, reg) {
            Ok(ids) => {
                // Pasted inside a group, the nodes join it.
                if let Some(group) = self.view.entered.and_then(|i| graph.groups.get_mut(i)) {
                    group.nodes.extend(ids.iter().copied());
                }
                self.set_graph(graph, reg);
                self.select(ui.ctx(), &ids);
                self.command_error = None;
                true
            }
            Err(e) => {
                self.command_error = Some(e.to_string());
                false
            }
        }
    }

    pub(super) fn tools_ui(&mut self, reg: &Registry, ui: &mut Ui) -> bool {
        let ids = self.selection(ui.ctx());
        let mut copy = false;
        let mut paste = false;
        let mut duplicate = false;
        let mut delete = false;
        let mut all = false;
        let mut changed = false;
        let mut pasted_text = None;
        let mut group: Option<bool> = None;
        let mut auto = false;
        let mut fold: Option<bool> = None;
        let canvas = ui.available_rect_before_wrap();
        let pointer = ui.input(|i| i.pointer.hover_pos());
        let keyboard = self.editable
            && !ui.ctx().egui_wants_keyboard_input()
            && pointer.is_some_and(|p| {
                canvas.contains(p)
                    && ui
                        .ctx()
                        .layer_id_at(p)
                        .is_none_or(|layer| layer == ui.layer_id())
            })
            && !egui::Popup::is_any_open(ui.ctx());
        let mut leave = false;
        let mut help = false;
        if keyboard {
            ui.input_mut(|i| {
                duplicate = i.consume_key(egui::Modifiers::COMMAND, egui::Key::D);
                all = i.consume_key(egui::Modifiers::COMMAND, egui::Key::A);
                delete = i.consume_key(egui::Modifiers::NONE, egui::Key::Delete);
                leave = self.view.entered.is_some() && self.palette.is_none() && i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
                help = i.consume_key(egui::Modifiers::NONE, egui::Key::F1);
                i.events.retain(|event| match event {
                    egui::Event::Copy => {
                        copy = true;
                        false
                    }
                    egui::Event::Paste(text) => {
                        pasted_text = Some(text.clone());
                        false
                    }
                    _ => true,
                });
            });
        }
        let mut action: Option<super::groups::Action> = None;
        if let Some(group) = self.entered_group() {
            let (name, count) = (group.name.clone(), group.nodes.len());
            egui::Frame::new().fill(crate::style::AQUA.gamma_multiply(0.12)).corner_radius(4.0).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Back to the whole graph").on_hover_text("Leave this group (Esc)").clicked() {
                        action = Some(super::groups::Action::Leave);
                    }
                    ui.label(RichText::new(format!("Inside the group {name}")).strong().color(crate::style::AQUA));
                    ui.weak(format!("{count} nodes. The Group inputs and Group outputs nodes stand for the wires that cross its edge."));
                    if let Some(i) = self.view.entered {
                        if ui.small_button("Show its nodes in the whole graph").clicked() {
                            action = Some(super::groups::Action::Expand(i));
                        }
                    }
                });
            });
        }
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(self.editable, |ui| {
                ui.menu_button("Selection", |ui| {
                    ui.weak("Drag on empty canvas to box-select; Shift adds, Ctrl removes.");
                    copy |= ui
                        .add_enabled(!ids.is_empty(), egui::Button::new("Copy nodes  Ctrl+C"))
                        .clicked();
                    paste |= ui
                        .add_enabled(
                            ui.data(|d| d.get_temp::<Fragment>(clipboard_id()).is_some()),
                            egui::Button::new("Paste nodes  Ctrl+V"),
                        )
                        .clicked();
                    duplicate |= ui
                        .add_enabled(
                            !ids.is_empty(),
                            egui::Button::new("Duplicate selection  Ctrl+D"),
                        )
                        .clicked();
                    delete |= ui
                        .add_enabled(
                            !ids.is_empty(),
                            egui::Button::new("Delete selection  Delete"),
                        )
                        .clicked();
                    all |= ui.button("Select all  Ctrl+A").clicked();
                    if copy || paste || duplicate || delete || all {
                        ui.close();
                    }
                });
                ui.menu_button("Groups", |ui| {
                    ui.weak("A group folds nodes into one box you can open. Evaluation never depends on groups.");
                    ui.text_edit_singleline(&mut self.group_name).accessible_name("New group name");
                    ui.horizontal(|ui| {
                        let can = !ids.is_empty() && !self.group_name.trim().is_empty();
                        if ui.add_enabled(can, egui::Button::new("Group selected nodes")).on_hover_text("Fold the selection into one group node").clicked() {
                            group = Some(true);
                            ui.close();
                        }
                        if ui.add_enabled(can, egui::Button::new("Frame them")).on_hover_text("Draw a named frame round the selection, nodes left in view").clicked() {
                            group = Some(false);
                            ui.close();
                        }
                    });
                    if ui.button("Group every layer automatically").on_hover_text("One group per layer, stamp list, part chain and settings block not grouped yet").clicked() {
                        auto = true;
                        ui.close();
                    }
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!self.graph.groups.is_empty(), egui::Button::new("Fold all")).clicked() {
                            fold = Some(true);
                            ui.close();
                        }
                        if ui.add_enabled(!self.graph.groups.is_empty(), egui::Button::new("Unfold all")).clicked() {
                            fold = Some(false);
                            ui.close();
                        }
                    });
                    if !self.graph.groups.is_empty() {
                        ui.separator();
                    }
                    let mut selected = None;
                    egui::ScrollArea::vertical().id_salt("group-list").max_height(320.0).show(ui, |ui| {
                        for (index, group) in self.graph.groups.iter_mut().enumerate() {
                            ui.push_id(index, |ui| {
                                ui.horizontal(|ui| {
                                    if ui.add(egui::TextEdit::singleline(&mut group.name).desired_width(140.0)).on_hover_text("Group name").changed() {
                                        changed = true;
                                    }
                                    ui.weak(format!("{}", group.nodes.len()));
                                    if ui.small_button("Open").clicked() {
                                        action = Some(super::groups::Action::Enter(index));
                                        ui.close();
                                    }
                                    let fold_label = if group.collapsed { "Unfold" } else { "Fold" };
                                    if ui.small_button(fold_label).clicked() {
                                        action = Some(if group.collapsed { super::groups::Action::Expand(index) } else { super::groups::Action::Collapse(index) });
                                        ui.close();
                                    }
                                    if ui.small_button("Select").clicked() {
                                        selected = Some(group.nodes.clone());
                                    }
                                    if ui.small_button("Ungroup").clicked() {
                                        action = Some(super::groups::Action::Ungroup(index));
                                        ui.close();
                                    }
                                });
                            });
                        }
                    });
                    if let Some(ids) = selected {
                        self.select(ui.ctx(), &ids);
                        ui.close();
                    }
                });
            });
            let hint = if ui.input(|i| i.has_touch_screen()) { "Long-press to add a node • Double-tap a group to open it • Pinch to zoom" } else { "Type to add a node • Drag selects • Right-drag pans • Wheel zooms • F1 help" };
            ui.weak(hint).on_hover_text("Type a node's name over the canvas, or right-click, to add one. Drag on empty canvas to box-select (Shift adds, Ctrl removes); right- or middle-drag pans; the wheel zooms (hold Shift to pan instead). Double-click a group to open it, Esc to come back out. F1 over a node opens its help.");
        });
        if let Some(collapsed) = group {
            let name = self.group_name.trim().to_string();
            changed |= self.group_selection(&ids, &name, collapsed);
        }
        if auto {
            changed |= self.auto_group();
        }
        if let Some(collapsed) = fold {
            changed |= self.fold_all(collapsed);
        }
        if leave {
            action = Some(super::groups::Action::Leave);
        }
        if let Some(a) = action {
            changed |= self.apply_group_action(a, &ui.ctx().clone(), reg);
        }
        if help {
            if let Some(kind) = self.selected.and_then(|id| self.graph.node(id)).map(|n| n.kind.clone()) {
                self.help = Some(kind);
            }
        }
        if all {
            let ids = self.graph.nodes.iter().map(|n| n.id).collect::<Vec<_>>();
            self.select(ui.ctx(), &ids);
        }
        if copy && !ids.is_empty() {
            let fragment = Fragment::capture(&self.graph, &ids);
            if let Ok(text) = serde_json::to_string(&fragment) {
                ui.ctx().copy_text(text);
            }
            ui.data_mut(|d| d.insert_temp(clipboard_id(), fragment));
        }
        let at = self
            .transform
            .map_or(egui::Pos2::ZERO, |t| t.inverse() * canvas.center());
        if duplicate && !ids.is_empty() {
            let fragment = Fragment::capture(&self.graph, &ids);
            let origin = fragment.nodes.iter().fold([f32::INFINITY; 2], |a, n| {
                [a[0].min(n.pos[0]), a[1].min(n.pos[1])]
            });
            changed |=
                self.paste_fragment(ui, reg, &fragment, [origin[0] + 60.0, origin[1] + 60.0]);
        } else if paste {
            if let Some(fragment) = ui.data(|d| d.get_temp::<Fragment>(clipboard_id())) {
                changed |= self.paste_fragment(ui, reg, &fragment, [at.x, at.y]);
            }
        } else if let Some(text) = pasted_text {
            if text.len() <= 32 * 1024 * 1024 {
                match serde_json::from_str::<Fragment>(&text) {
                    Ok(fragment) => {
                        changed |= self.paste_fragment(ui, reg, &fragment, [at.x, at.y])
                    }
                    Err(_) => {
                        self.command_error =
                            Some("Clipboard does not contain RingDesigner nodes".into())
                    }
                }
            } else {
                self.command_error = Some("Clipboard is too large (32 MB maximum)".into());
            }
        }
        if delete && !ids.is_empty() {
            let mut graph = self.graph.clone();
            for id in ids {
                let _ = graph.remove(id);
            }
            self.set_graph(graph, reg);
            self.select(ui.ctx(), &[]);
            changed = true;
        }
        if let Some(error) = &self.command_error {
            ui.horizontal(|ui| {
                ui.colored_label(crate::style::ERROR, error);
            });
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{Harness, kittest::Queryable};

    #[test]
    fn a_node_duplicated_inside_a_group_stays_in_it() {
        let reg = Registry::builtin();
        let mut ed = Editor::new(ringdesign_graph::templates::graph("Gypsy trio").unwrap(), &reg);
        ed.enter_group(0);
        let gem = ed.graph().groups[0].nodes.iter().copied().find(|id| ed.graph().node(*id).unwrap().kind == "gem").unwrap();
        ed.selected = Some(gem);
        let before = ed.graph().groups[0].nodes.len();
        let mut h = Harness::builder().with_size([1100., 700.]).build_ui_state(
            |ui, ed: &mut Editor| {
                ed.show(&reg, ui, "group-paste");
            },
            ed,
        );
        h.run();
        h.hover_at(egui::pos2(700., 450.));
        h.run();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::D);
        h.run();
        let ed = h.state();
        assert_eq!(ed.graph().groups[0].nodes.len(), before + 1, "the copy joined the group in hand");
        assert!(ed.view().graph.nodes.iter().filter(|n| n.kind == "gem").count() == 2, "and shows inside it");
    }

    #[test]
    fn menu_paste_selects_the_new_nodes_and_delete_removes_only_that_selection() {
        let reg = Registry::builtin();
        let mut graph = Graph::default();
        let a = graph.add("math.add").unwrap();
        let b = graph.add("math.mul").unwrap();
        graph.connect(a, "out", b, "a").unwrap();
        graph.node_mut(b).unwrap().pos = [260., 0.];
        let mut h = Harness::builder().with_size([900., 600.]).build_ui_state(
            |ui, ed| {
                ed.show(&reg, ui, "commands-test");
            },
            Editor::new(graph, &reg),
        );
        h.run();
        for action in [
            "Select all  Ctrl+A",
            "Copy nodes  Ctrl+C",
            "Paste nodes  Ctrl+V",
        ] {
            h.get_by_label("Selection").click();
            h.run();
            h.get_by_label(action).click();
            h.run();
        }
        assert_eq!(h.state().graph.nodes.len(), 4);
        assert_eq!(h.state().graph.wires.len(), 2);
        let selected = h.state().selected_nodes(&h.ctx);
        assert_eq!(selected.len(), 2);
        assert!(selected.iter().all(|id| ![a, b].contains(id)));
        h.get_by_label("Groups").click();
        h.run();
        h.get_by_label("Group selected nodes").click();
        h.run();
        assert_eq!(h.state().graph.groups.len(), 1);
        h.hover_at(egui::pos2(700., 450.));
        h.run();
        h.key_press(egui::Key::Delete);
        h.run();
        assert_eq!(
            h.state()
                .graph
                .nodes
                .iter()
                .map(|n| n.id)
                .collect::<Vec<_>>(),
            [a, b]
        );
        assert_eq!(h.state().graph.wires.len(), 1);
        assert!(h.state().graph.groups.is_empty());
    }
}
