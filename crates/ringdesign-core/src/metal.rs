//! Casting metal densities, solidification shrinkage, and weight conversion.

use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Metal {
    pub name: &'static str,
    /// g/cm³.
    pub density: f64,
    /// Linear solidification + cooling shrinkage, percent — the
    /// patternmaker's allowance. A pattern is cut oversize by `1/(1-s)` so
    /// the cast cools to nominal.
    pub shrink_pct: f64,
}

pub const METALS: &[Metal] = &[
    Metal { name: "Silver 925", density: 10.36, shrink_pct: 1.9 },
    Metal { name: "Bronze", density: 8.80, shrink_pct: 1.6 },
    Metal { name: "Brass", density: 8.55, shrink_pct: 1.5 },
    Metal { name: "Gold 10k", density: 11.55, shrink_pct: 1.3 },
    Metal { name: "Gold 14k", density: 13.07, shrink_pct: 1.3 },
    Metal { name: "Gold 18k", density: 15.58, shrink_pct: 1.3 },
    Metal { name: "Gold 22k", density: 17.80, shrink_pct: 1.4 },
    Metal { name: "Gold 24k", density: 19.32, shrink_pct: 1.4 },
    Metal { name: "Palladium 950", density: 12.00, shrink_pct: 1.7 },
    Metal { name: "Platinum 950", density: 21.45, shrink_pct: 1.0 },
];

/// The listed metal by (case-insensitive) name prefix: "sterling" and
/// "silver" both find Silver 925; "14k" finds Gold 14k.
pub fn find(name: &str) -> Option<&'static Metal> {
    let q = name.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    let alias = match q.as_str() {
        "sterling" => "silver 925",
        _ => &q,
    };
    METALS.iter().find(|m| {
        let n = m.name.to_lowercase();
        n == alias || n.starts_with(alias) || n.split_whitespace().any(|w| w == alias)
    })
}

/// Scale a pattern is cut at so the cast shrinks to nominal: `1/(1-s)`.
pub fn pattern_scale(shrink_pct: f64) -> f64 {
    1.0 / (1.0 - (shrink_pct / 100.0).clamp(0.0, 0.2))
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrink_lookup_and_scale_behave() {
        let m = find("sterling").expect("sterling resolves");
        assert_eq!(m.name, "Silver 925");
        assert_eq!(find("14k").unwrap().name, "Gold 14k");
        assert_eq!(find("platinum").unwrap().name, "Platinum 950");
        assert!(find("unobtanium").is_none());
        // A 1.9% shrink pattern is cut ~1.94% oversize, and every listed
        // metal's allowance is a sane linear percent.
        assert!((pattern_scale(1.9) - 1.0194).abs() < 1e-3);
        for m in METALS {
            assert!(m.shrink_pct > 0.5 && m.shrink_pct < 3.0, "{}", m.name);
            assert!(pattern_scale(m.shrink_pct) > 1.0);
        }
    }
}
