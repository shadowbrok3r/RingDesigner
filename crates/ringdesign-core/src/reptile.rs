//! Sculpted reptile relief, shared by the tile library and collection author.
//! Coordinates are cell units; heights are 0..1. There is no baked lighting.

use crate::field::smoothstep;
use std::f64::consts::TAU;

/// Imbricated, keeled lanceolate scales. Every second transverse row is offset
/// half a scale. Integer along periods and even across periods close exactly.
pub fn snake(along: f64, across: f64) -> f64 {
    let mut h: f64 = 0.0;
    let row = across.round() as i64;
    for j in row - 1..=row + 1 {
        let stagger = 0.5 * j.rem_euclid(2) as f64;
        let col = (along - stagger).round();
        for i in [-1.0, 0.0, 1.0] {
            let x = along - col - stagger - i;
            let y = across - j as f64;
            // A broad heel tapering to a pointed free edge, with a soft bevel.
            let q = (x.abs() / 0.70).powf(1.25) + (y.abs() / 0.55).powf(1.40);
            let edge = 1.0 - smoothstep(0.79, 1.0, q);
            let plate = 0.51 + 0.24 * smoothstep(-0.55, 0.48, x);
            let keel = 0.22 * (-((y / 0.14).powi(2))).exp()
                * (1.0 - smoothstep(0.23, 0.65, x.abs()));
            h = h.max(edge * (plate + keel));
        }
    }
    h.clamp(0.0, 1.0)
}

/// Broad snake belly plates. `across` is -1..1 across a ventral plate;
/// the bowed joints continue into the small flank scales in the collection.
pub fn ventral(along: f64, across: f64) -> f64 {
    let t = (along + 0.13 * across * across).rem_euclid(1.0);
    let joint = smoothstep(0.045, 0.17, t) * (1.0 - smoothstep(0.86, 0.985, t));
    let bow = 0.78 + 0.16 * (std::f64::consts::PI * t).sin();
    joint * bow * (1.0 - 0.10 * across.abs().min(1.0).powi(4))
}

/// Rectangular crocodilian osteoderms with a central keel and shallow grain.
/// The common joint grid joins cleanly when adjacent columns change size.
pub fn crocodile(along: f64, across: f64) -> f64 {
    let x = along.rem_euclid(1.0) - 0.5;
    let y = across.rem_euclid(1.0) - 0.5;
    let q = ((x / 0.48).abs().powi(6) + (y / 0.47).abs().powi(6)).powf(1.0 / 6.0);
    let edge = 1.0 - smoothstep(0.76, 1.0, q);
    let crown = 0.57 + 0.17 * (1.0 - (x / 0.5).powi(2)).max(0.0);
    let keel = 0.23 * (1.0 - smoothstep(0.035, 0.22, y.abs()))
        * (1.0 - smoothstep(0.20, 0.45, x.abs()));
    let grain = 0.055 * (TAU * (y * 5.0 + 0.11 * (TAU * x).sin())).cos()
        * (1.0 - smoothstep(0.55, 0.85, q));
    (edge * (crown + keel + grain)).clamp(0.0, 1.0)
}

/// Six-sided shield scales with broad centres, drafted rims and recessed joints.
pub fn shields(along: f64, across: f64) -> f64 {
    let row = across.round() as i64;
    let mut h: f64 = 0.0;
    for j in row - 1..=row + 1 {
        let stagger = 0.5 * j.rem_euclid(2) as f64;
        let x = (along - stagger + 0.5).rem_euclid(1.0) - 0.5;
        let y = (across - j as f64).abs();
        let q = (x.abs() * 2.0).max(x.abs() + 1.5 * y);
        let edge = 1.0 - smoothstep(0.84, 0.99, q);
        let rim = smoothstep(0.57, 0.73, q) * (1.0 - smoothstep(0.79, 0.94, q));
        h = h.max(edge * (0.58 + 0.14 * (1.0 - q).max(0.0)) + 0.22 * rim);
    }
    h.clamp(0.0, 1.0)
}

/// Unit-square library tiles. Each includes enough cells to show the stagger.
pub fn snake_tile(x: f64, y: f64) -> f64 { snake(x * 4.0, y * 4.0) }
pub fn ventral_tile(x: f64, y: f64) -> f64 {
    ventral(x * 4.0, (y * TAU).cos())
}
pub fn crocodile_tile(x: f64, y: f64) -> f64 { crocodile(x * 3.0, y * 3.0) }
pub fn shield_tile(x: f64, y: f64) -> f64 { shields(x * 4.0, y * 4.0) }
