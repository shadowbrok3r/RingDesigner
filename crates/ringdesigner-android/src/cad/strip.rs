//! The feature strip under the phone's ring: thumb-high chips, a long press for a chip's menu, every edit through the funnel.
use egui_mobile::egui;
use ringdesign_core::{
    cad::{Document, Evaluated, edit::CadEdit},
    sketch::Id,
};
use ringdesign_workbench::{
    icons::Icon,
    timeline::{self, Action},
    touch,
};

/// What the strip's gestures come to.
#[derive(Clone, Debug, PartialEq)]
pub enum Routed {
    /// Choose the part, as a tap on it would.
    Choose(Id),
    /// Open the feature's numbers, which on the phone live on the Workshop's CAD tab.
    Edit(Id),
    /// Edits for the funnel, in the order it must apply them.
    Commit(Vec<CadEdit>),
    /// Something to say instead: a refusal, or what the phone does not do yet.
    Say(String),
}

/// Draws the strip, its chips and their long-press menu a thumb high; what its gestures asked for.
pub fn show(ui: &mut egui::Ui, doc: &Document, evaluated: Option<&Evaluated>, selected: &[Id]) -> Vec<Action> {
    let chips = timeline::chips(doc, evaluated, selected);
    // Chips and their menu's rows stand a thumb high while the strip draws; the context's style is put back after.
    let before = ui.ctx().global_style().spacing.interact_size.y;
    ui.ctx().global_style_mut(|s| s.spacing.interact_size.y = touch::TARGET_PT);
    ui.spacing_mut().interact_size.y = touch::TARGET_PT;
    let actions = ui
        .horizontal(|ui| {
            ui.add(Icon::History.image(ui, 20.0)).on_hover_text("Feature timeline: tap a feature to choose it, hold it for its menu, drag it to reorder");
            egui::ScrollArea::horizontal().id_salt("phone-feature-strip").auto_shrink([false, true]).show(ui, |ui| timeline::show(ui, &chips, doc.through, false)).inner
        })
        .inner;
    ui.ctx().global_style_mut(|s| s.spacing.interact_size.y = before);
    actions
}

/// What the strip's actions come to: choices at once, one batch of edits for the funnel, the rest said.
pub fn route(doc: &Document, actions: Vec<Action>) -> Vec<Routed> {
    let mut out = Vec::new();
    let mut edits = Vec::new();
    for action in actions {
        match action {
            Action::Select(id) => out.push(Routed::Choose(id)),
            Action::Edit(id) => out.push(Routed::Edit(id)),
            Action::Isolate(_) => out.push(Routed::Say(super::menu::NO_ISOLATE.into())),
            edit => match timeline::edits(doc, &edit) {
                Ok(e) => edits.extend(e),
                Err(reason) => {
                    out.push(Routed::Say(reason));
                    return out;
                }
            },
        }
    }
    if !edits.is_empty() {
        out.push(Routed::Commit(edits));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::cad::{Component, EdgeRef, Feature, Operation};

    fn doc() -> Document {
        let mut d = Document::default();
        let part = |id: Id, name: &str, operation: Operation| Feature { id, name: name.into(), enabled: true, operation, component: Component::default() };
        d.append(part(1, "Procedural shank", Operation::Band)).unwrap();
        d.append(part(2, "Post", Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 })).unwrap();
        d.append(part(3, "Fillet", Operation::Fillet { source: 2, edges: vec![EdgeRef::bare(0)], radius_mm: 0.2 })).unwrap();
        d
    }

    #[test]
    fn the_strips_long_press_menu_suppresses_renames_rolls_back_and_deletes_through_one_batch() {
        let d = doc();
        let routed = route(&d, vec![Action::Select(2), Action::Enable(3, false), Action::Rename(2, "Pin".into()), Action::RollTo(Some(2))]);
        let Routed::Commit(edits) = routed.last().unwrap() else { panic!("{routed:?}") };
        assert_eq!(routed[0], Routed::Choose(2));
        assert_eq!(edits.iter().map(CadEdit::label).collect::<Vec<_>>(), ["Suppress #3", "Rename #2", "Roll back to #2"]);
        // Deleting what a later feature reads is refused by name, and nothing else in the batch lands.
        let refused = route(&d, vec![Action::Rename(2, "Pin".into()), Action::Delete(2)]);
        assert_eq!(refused, [Routed::Say("Delete Post: 1 feature depends on this: Fillet".into())]);
        let Routed::Commit(both) = &route(&d, vec![Action::DeleteWithDependents(2)])[0] else { panic!() };
        assert_eq!(both.iter().map(CadEdit::label).collect::<Vec<_>>(), ["Remove #3", "Remove #2"], "the reader goes first");
        assert_eq!(route(&d, vec![Action::Isolate(2), Action::Edit(2)]), [Routed::Say(crate::cad::menu::NO_ISOLATE.into()), Routed::Edit(2)]);
    }

    #[test]
    fn the_strip_draws_its_chips_a_thumb_high_and_leaves_the_style_as_it_found_it() {
        let ctx = egui::Context::default();
        let d = doc();
        let mut height = 0.0;
        let mut before = 0.0;
        let mut after = 0.0;
        let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420.0, 200.0))), ..Default::default() }, |ui| {
            before = ui.ctx().global_style().spacing.interact_size.y;
            let top = ui.cursor().top();
            show(ui, &d, None, &[2]);
            height = ui.cursor().top() - top;
            after = ui.ctx().global_style().spacing.interact_size.y;
        });
        out.textures_delta.clear();
        assert!(height >= touch::TARGET_PT - 4.0, "the strip stands {height} pt");
        assert_eq!(before, after);
    }
}
