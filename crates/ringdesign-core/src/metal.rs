//! Casting metal densities and weight conversion.

use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Metal {
    pub name: &'static str,
    /// g/cm³.
    pub density: f64,
}

pub const METALS: &[Metal] = &[
    Metal { name: "Silver 925", density: 10.36 },
    Metal { name: "Bronze", density: 8.80 },
    Metal { name: "Brass", density: 8.55 },
    Metal { name: "Gold 10k", density: 11.55 },
    Metal { name: "Gold 14k", density: 13.07 },
    Metal { name: "Gold 18k", density: 15.58 },
    Metal { name: "Gold 22k", density: 17.80 },
    Metal { name: "Gold 24k", density: 19.32 },
    Metal { name: "Palladium 950", density: 12.00 },
    Metal { name: "Platinum 950", density: 21.45 },
];

const GRAMS_PER_DWT: f64 = 1.555_173_84;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct MetalWeight {
    pub metal: &'static str,
    pub grams: f64,
    pub dwt: f64,
}

/// Weight of a solid of the given volume in every listed metal.
pub fn metal_table(volume_mm3: f64) -> Vec<MetalWeight> {
    let cm3 = volume_mm3 / 1000.0;
    METALS
        .iter()
        .map(|m| {
            let grams = cm3 * m.density;
            MetalWeight { metal: m.name, grams, dwt: grams / GRAMS_PER_DWT }
        })
        .collect()
}
