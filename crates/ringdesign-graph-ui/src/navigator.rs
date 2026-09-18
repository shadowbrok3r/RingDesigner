//! Stepping through a graph without finding each node on the canvas.
//!
//! One row: previous, a jump list over the walk, next, and the chosen node's
//! inputs and outputs as menus. Every move goes through [`Editor::focus`],
//! so the canvas follows and the host sees the selection change on the
//! editor's next response — which is what lets a host show, node by node,
//! what each one does to the ring.

use egui::containers::menu::MenuButton;
use egui::{Button, Ui};
use ringdesign_graph::graph::NodeId;

use crate::editor::Editor;

const STEP_W: f32 = 34.0;
const SIDE_W: f32 = 62.0;
const LIST_H: f32 = 300.0;

fn menu_list(ui: &mut Ui, rows: impl FnOnce(&mut Ui)) {
    ui.set_min_width(220.0);
    egui::ScrollArea::vertical().max_height(LIST_H).show(ui, rows);
}

/// What the row did, and where its controls landed, for a host that maps
/// its own chrome (touch-target audits, scripted review).
#[derive(Clone, Debug, Default)]
pub struct NavResponse {
    /// The node the reader moved to, if they moved.
    pub moved: Option<NodeId>,
    /// `(name, rect)` of each control, in the order drawn.
    pub controls: Vec<(&'static str, egui::Rect)>,
}

/// Draws the row.
pub fn navigator(ed: &mut Editor, ui: &mut Ui) -> NavResponse {
    let order = ed.walk_order();
    let at = ed.selected.and_then(|s| order.iter().position(|id| *id == s));
    let (ins, outs) = ed.selected.map(|s| ed.neighbours(s)).unwrap_or_default();
    let mut go: Option<NodeId> = None;
    let mut controls: Vec<(&'static str, egui::Rect)> = Vec::new();
    ui.scope(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        ui.horizontal(|ui| {
            let h = ui.spacing().interact_size.y;
            let step = |ui: &mut Ui, text: &str, tip: &str| ui.add_enabled(!order.is_empty(), Button::new(text).min_size(egui::vec2(STEP_W, h))).on_hover_text(tip);
            let prev = step(ui, "<", "Previous node, toward the sources");
            controls.push(("previous", prev.rect));
            if prev.clicked() {
                go = ed.step(-1);
            }
            let gaps = ui.spacing().item_spacing.x * 4.0;
            let width = (ui.available_width() - STEP_W - 2.0 * SIDE_W - gaps).max(90.0);
            let label = match at {
                Some(i) => format!("{}/{}  {}", i + 1, order.len(), ed.title_of(order[i])),
                None => format!("{} nodes - choose one", order.len()),
            };
            let jump = ui.allocate_ui(egui::vec2(width, h), |ui| {
                ui.set_max_width(width);
                MenuButton::from_button(Button::new(label).truncate().min_size(egui::vec2(width, h))).ui(ui, |ui| {
                    menu_list(ui, |ui| {
                        for (i, id) in order.iter().enumerate() {
                            if ui.selectable_label(at == Some(i), format!("{}  {}", i + 1, ed.title_of(*id))).clicked() {
                                go = Some(*id);
                                ui.close();
                            }
                        }
                    });
                });
            });
            controls.push(("jump", jump.response.rect));
            let next = step(ui, ">", "Next node, toward the output");
            controls.push(("next", next.rect));
            if next.clicked() {
                go = ed.step(1);
            }
            for (title, name, arrow, wires, tip) in [("In", "inputs", "<-", &ins, "Go to a node wired into this one"), ("Out", "outputs", "->", &outs, "Go to a node this one feeds")] {
                let button = Button::new(format!("{title} {}", wires.len())).min_size(egui::vec2(SIDE_W, h));
                let side = ui.add_enabled_ui(!wires.is_empty(), |ui| {
                    MenuButton::from_button(button).ui(ui, |ui| {
                        menu_list(ui, |ui| {
                            for (pin, id) in wires.iter() {
                                if ui.button(format!("{pin}  {arrow}  {}", ed.title_of(*id))).on_hover_text(tip).clicked() {
                                    go = Some(*id);
                                    ui.close();
                                }
                            }
                        });
                    });
                });
                controls.push((name, side.response.rect));
            }
        });
    });
    if let Some(id) = go {
        ed.focus(id);
    }
    NavResponse { moved: go, controls }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_graph::registry::Registry;

    #[test]
    fn the_row_draws_and_names_the_chosen_node() {
        let reg = Registry::builtin();
        let g = ringdesign_graph::templates::graph("Braided band").expect("bundled");
        let mut ed = Editor::new(g, &reg);
        let first = ed.step(1).expect("a graph with nodes steps");
        let title = ed.title_of(first);
        let mut seen = NavResponse::default();
        let mut harness = egui_kittest::Harness::new_ui(|ui| {
            seen = navigator(&mut ed, ui);
        });
        harness.set_size(egui::vec2(420.0, 60.0));
        harness.run();
        use egui_kittest::kittest::Queryable;
        assert!(harness.query_by_label_contains(&title).is_some(), "{title} not shown");
        assert!(harness.query_by_label("<").is_some() && harness.query_by_label(">").is_some());
        assert!(harness.query_by_label_contains("Out").is_some());
        drop(harness);
        let names: Vec<&str> = seen.controls.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, ["previous", "jump", "next", "inputs", "outputs"]);
        let row = seen.controls.iter().fold(egui::Rect::NOTHING, |r, (_, c)| r.union(*c));
        assert!(row.width() <= 420.0, "the row fits a phone: {}", row.width());
        assert!(seen.controls.iter().all(|(n, c)| c.width() >= 30.0 && c.height() >= 16.0 || panic!("{n} is {c:?}")));
    }
}
