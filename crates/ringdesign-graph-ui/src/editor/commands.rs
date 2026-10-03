use super::*;
use ringdesign_graph::fragment::Fragment;

fn clipboard_id() -> egui::Id {
    egui::Id::new("ringdesigner-node-clipboard")
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
            egui_snarl::ui::set_selected_nodes(
                snarl,
                ctx,
                ids.iter()
                    .filter_map(|id| self.ids.to_snarl.get(id).copied()),
            );
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
        if keyboard {
            ui.input_mut(|i| {
                duplicate = i.consume_key(egui::Modifiers::COMMAND, egui::Key::D);
                all = i.consume_key(egui::Modifiers::COMMAND, egui::Key::A);
                delete = i.consume_key(egui::Modifiers::NONE, egui::Key::Delete);
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
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(self.editable, |ui| {
                ui.menu_button("Selection", |ui| {
                    ui.weak("Shift-click titles to select several nodes.");
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
                    ui.weak("Frames organize nodes without changing their wires.");
                    ui.text_edit_singleline(&mut self.group_name);
                    if ui
                        .add_enabled(
                            !ids.is_empty() && !self.group_name.trim().is_empty(),
                            egui::Button::new("Group selected nodes"),
                        )
                        .clicked()
                    {
                        self.graph.groups.push(NodeGroup {
                            name: self.group_name.trim().to_string(),
                            nodes: ids.clone(),
                        });
                        self.revision += 1;
                        changed = true;
                        ui.close();
                    }
                    let mut selected = None;
                    let mut remove = None;
                    for (index, group) in self.graph.groups.iter_mut().enumerate() {
                        ui.push_id(index, |ui| {
                            ui.horizontal(|ui| {
                                if ui.text_edit_singleline(&mut group.name).changed() {
                                    changed = true;
                                }
                                if ui.small_button("Select group").clicked() {
                                    selected = Some(group.nodes.clone());
                                }
                                if ui.small_button("Remove frame").clicked() {
                                    remove = Some(index);
                                }
                            });
                        });
                    }
                    if let Some(index) = remove {
                        self.graph.groups.remove(index);
                        changed = true;
                    }
                    if let Some(ids) = selected {
                        self.select(ui.ctx(), &ids);
                        ui.close();
                    }
                });
            });
            ui.weak("Right-click to add • Drag a wire to empty space to connect a new node");
        });
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
