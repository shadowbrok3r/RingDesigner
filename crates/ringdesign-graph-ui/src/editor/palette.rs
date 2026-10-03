use super::*;
use std::collections::BTreeSet;

fn favorites_id() -> egui::Id {
    egui::Id::new("ringdesigner-favorite-nodes")
}

fn compatible(card: &NodeCard, snarl: &Snarl<NodeCard>, pins: &AnyPins<'_>) -> Option<usize> {
    match pins {
        AnyPins::Out(pins) => {
            let from = pins.first()?;
            let kind = snarl.get_node(from.node)?.pins_out.get(from.output)?.kind;
            card.pins_in
                .iter()
                .position(|p| p.kind.accepts(kind) || p.kind == ValueKind::List)
        }
        AnyPins::In(pins) => {
            let to = pins.first()?;
            let pin = snarl.get_node(to.node)?.pins_in.get(to.input)?;
            card.pins_out
                .iter()
                .position(|p| pin.kind.accepts(p.kind) || pin.kind == ValueKind::List)
        }
    }
}

impl Viewer<'_> {
    fn choice(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        snarl: &mut Snarl<NodeCard>,
        spec: &NodeSpec,
        pins: Option<&AnyPins<'_>>,
    ) {
        let mut favorites = ui.data_mut(|d| {
            d.get_persisted::<BTreeSet<String>>(favorites_id())
                .unwrap_or_default()
        });
        let favorite = favorites.contains(&spec.key);
        let mut chosen = false;
        ui.horizontal(|ui| {
            chosen = crate::marks::button(ui, crate::marks::of(spec), &spec.label)
                .on_hover_text(format!("{}\n{}", spec.key, spec.doc))
                .clicked();
            if ui
                .small_button(if favorite { "Unpin" } else { "Pin" })
                .on_hover_text("Keep frequently used nodes in Favorites")
                .clicked()
            {
                if favorite {
                    favorites.remove(&spec.key);
                } else {
                    favorites.insert(spec.key.clone());
                }
                ui.data_mut(|d| d.insert_persisted(favorites_id(), favorites));
            }
        });
        if !chosen {
            return;
        }
        let card = NodeCard::new_of(spec, self.reg);
        let link = pins.and_then(|p| compatible(&card, snarl, p));
        let node = snarl.insert_node(pos, card);
        match (pins, link) {
            (Some(AnyPins::Out(pins)), Some(input)) => {
                let from = OutPin {
                    id: pins[0],
                    remotes: vec![],
                };
                let to = InPin {
                    id: InPinId { node, input },
                    remotes: vec![],
                };
                self.connect(&from, &to, snarl);
            }
            (Some(AnyPins::In(pins)), Some(output)) => {
                let from = OutPin {
                    id: OutPinId { node, output },
                    remotes: vec![],
                };
                let to = InPin {
                    id: pins[0],
                    remotes: vec![],
                };
                self.connect(&from, &to, snarl);
            }
            _ => {}
        }
        self.search.clear();
        ui.close();
    }

    pub(super) fn palette(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        snarl: &mut Snarl<NodeCard>,
        pins: Option<&AnyPins<'_>>,
    ) {
        ui.set_min_width(260.0);
        ui.label(
            RichText::new(if pins.is_some() {
                "Add connected node"
            } else {
                "Add node"
            })
            .small()
            .weak(),
        );
        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search nodes…"));
        if pins.is_some() {
            ui.weak("Showing compatible pins; selection connects automatically.");
        }
        let reg = self.reg;
        let specs: Vec<_> = reg
            .list(self.mode)
            .into_iter()
            .filter(|spec| {
                pins.is_none_or(|pins| {
                    compatible(&NodeCard::new_of(spec, reg), snarl, pins).is_some()
                })
            })
            .collect();
        let needle = self.search.trim().to_lowercase();
        if !needle.is_empty() {
            let matches: Vec<_> = specs
                .iter()
                .filter(|s| {
                    s.key.to_lowercase().contains(&needle)
                        || s.label.to_lowercase().contains(&needle)
                })
                .collect();
            if matches.is_empty() {
                ui.weak("No matching nodes");
            }
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for spec in matches {
                        self.choice(pos, ui, snarl, spec, pins);
                    }
                });
            return;
        }
        let favorites = ui.data_mut(|d| {
            d.get_persisted::<BTreeSet<String>>(favorites_id())
                .unwrap_or_default()
        });
        let pinned: Vec<_> = specs
            .iter()
            .filter(|s| favorites.contains(&s.key))
            .collect();
        if !pinned.is_empty() {
            egui::containers::menu::SubMenuButton::new("Favorites").ui(ui, |ui| {
                for spec in pinned {
                    self.choice(pos, ui, snarl, spec, pins);
                }
            });
            ui.separator();
        }
        if specs.is_empty() {
            ui.weak("No compatible nodes in this graph mode");
        }
        for cat in Category::ALL {
            let in_cat: Vec<_> = specs.iter().filter(|s| s.category == *cat).collect();
            if in_cat.is_empty() {
                continue;
            }
            crate::marks::submenu(ui, *cat, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(400.0)
                    .show(ui, |ui| {
                        for spec in in_cat {
                            self.choice(pos, ui, snarl, spec, pins);
                        }
                    });
            });
        }
    }
}
