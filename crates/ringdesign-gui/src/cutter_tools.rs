//! Cutters placed on the band and builders made under a stone, through the edit funnel.
use crate::app::RingDesignerApp;
use ringdesign_core::interaction::pick::Entity;
use ringdesign_workbench::viewport::{Sel, cutters, selection::Mods};

/// Cutter `key` placed at `theta_deg`, `across_mm` on the band, or where the right-click landed on it.
pub fn cut_here(app: &mut RingDesignerApp, theta_deg: f64, across_mm: f64, key: &'static str) {
    let hit = app.selection.under.as_ref().filter(|p| p.entity == Entity::Band).map(|p| (p.world, p.normal));
    match cutters::cut_here(&app.design, theta_deg, across_mm, hit, key) {
        Ok((edits, id)) => {
            if crate::cad_edit::apply(app, &edits).is_ok() {
                app.selection.click(Some(Sel::Part(id)), Mods::default());
            }
        }
        Err(why) => app.set_status(why),
    }
}

/// Builder `key` made under reference stone `stone`.
pub fn under_stone(app: &mut RingDesignerApp, stone: u64, key: &'static str) {
    let built = app.build.as_ref().and_then(|b| b.parts.evaluated.as_ref());
    match cutters::under_stone(&app.design, built, stone, key) {
        Ok((edits, id)) => {
            if crate::cad_edit::apply(app, &edits).is_ok() {
                app.selection.click(Some(Sel::Part(id)), Mods::default());
            }
        }
        Err(why) => app.set_status(why),
    }
}
