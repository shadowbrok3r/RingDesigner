//! Cutters placed on the band and builders made under a stone, through the edit funnel.
use crate::app::RingDesignerApp;

/// Cutter `key` placed at `theta_deg`, `across_mm` on the band.
pub fn cut_here(_app: &mut RingDesignerApp, _theta_deg: f64, _across_mm: f64, _key: &'static str) {}

/// Builder `key` made under reference stone `stone`.
pub fn under_stone(_app: &mut RingDesignerApp, _stone: u64, _key: &'static str) {}
