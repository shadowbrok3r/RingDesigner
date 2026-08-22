//! Gem stock: cuts, proportions, carat weights, calibrated sizes.
//!
//! The same shape of module as [`crate::metal`]: small tables of real-world
//! numbers every stone feature reads — seat sizing, the report's carat totals,
//! section silhouettes, viewport previews. Nothing here is cast; stones are
//! set at the bench, and what the ring carries is the *stock* for them.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GemCut {
    Round,
    Oval,
    Cushion,
    Princess,
    Emerald,
    Baguette,
    Pear,
    Marquise,
    Trillion,
    Heart,
    Radiant,
    Asscher,
    Hexagon,
    HalfMoon,
}

impl GemCut {
    pub const ALL: &'static [GemCut] = &[
        GemCut::Round,
        GemCut::Oval,
        GemCut::Cushion,
        GemCut::Princess,
        GemCut::Emerald,
        GemCut::Baguette,
        GemCut::Pear,
        GemCut::Marquise,
        GemCut::Trillion,
        GemCut::Heart,
        GemCut::Radiant,
        GemCut::Asscher,
        GemCut::Hexagon,
        GemCut::HalfMoon,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GemCut::Round => "Round brilliant",
            GemCut::Oval => "Oval",
            GemCut::Cushion => "Cushion",
            GemCut::Princess => "Princess",
            GemCut::Emerald => "Emerald",
            GemCut::Baguette => "Baguette",
            GemCut::Pear => "Pear",
            GemCut::Marquise => "Marquise",
            GemCut::Trillion => "Trillion",
            GemCut::Heart => "Heart",
            GemCut::Radiant => "Radiant",
            GemCut::Asscher => "Asscher",
            GemCut::Hexagon => "Hexagon",
            GemCut::HalfMoon => "Half moon",
        }
    }

    /// Length over width of the standard proportion.
    ///
    /// Cross-checked against the CrossGems proportion switches (their Y is
    /// this ratio; tools/harvest/reports/CrossGems-Proportion.md): Oval and Pear adopt
    /// their 1.6, Emerald their 1.5, Hexagon their 2/sqrt(3). Marquise stays
    /// 2.0 and Baguette 2.0 — the trade's own classic makes — where their UI
    /// defaults say 1.7 and 1.6.
    pub fn aspect(self) -> f64 {
        match self {
            GemCut::Round
            | GemCut::Cushion
            | GemCut::Princess
            | GemCut::Trillion
            | GemCut::Heart
            | GemCut::Radiant
            | GemCut::Asscher => 1.0,
            GemCut::Oval => 1.6,
            GemCut::Emerald => 1.5,
            GemCut::Baguette => 2.0,
            GemCut::Pear => 1.6,
            GemCut::Marquise => 2.0,
            GemCut::Hexagon => 1.154,
            GemCut::HalfMoon => 1.7,
        }
    }

    /// Total depth as a share of the width, table to culet, standard make.
    ///
    /// Same cross-check: Round 0.62 (the classic 61.5% total depth),
    /// Cushion 0.62, Princess 0.78 (a deep square), Emerald 0.60, Radiant
    /// 0.65, Octagon-family 0.6 all match the CrossGems switches. Trillion
    /// stays 0.40 — trillions are cut shallow (32–44%) and their 0.53 is an
    /// outlier — and Half Moon uses the trade's ~0.65 of width.
    pub fn depth_frac(self) -> f64 {
        match self {
            GemCut::Round => 0.62,
            GemCut::Oval => 0.60,
            GemCut::Cushion => 0.62,
            GemCut::Princess => 0.78,
            GemCut::Emerald => 0.60,
            GemCut::Baguette => 0.55,
            GemCut::Pear => 0.60,
            GemCut::Marquise => 0.60,
            GemCut::Trillion => 0.40,
            GemCut::Heart => 0.60,
            GemCut::Radiant => 0.65,
            GemCut::Asscher => 0.62,
            GemCut::Hexagon => 0.60,
            GemCut::HalfMoon => 0.65,
        }
    }

    /// Carats of a diamond-density stone at `w x l` mm and standard depth.
    ///
    /// The classic estimator family: length x width x depth (mm) x a
    /// per-shape packing factor — a 6.5 mm round at 62% depth is 1.04 ct.
    pub fn carats(self, w_mm: f64, l_mm: f64) -> f64 {
        let factor = match self {
            GemCut::Round => 0.0061,
            GemCut::Oval => 0.0062,
            GemCut::Cushion => 0.00815,
            GemCut::Princess => 0.0083,
            GemCut::Emerald => 0.0080,
            GemCut::Baguette => 0.00700,
            // Pear and marquise carry the sibling mandrel crate's factors,
            // calibrated against a MatrixGold stone report; the rest are
            // the trade's textbook figures.
            GemCut::Pear => 0.00527,
            GemCut::Marquise => 0.00565,
            GemCut::Trillion => 0.0057,
            GemCut::Heart => 0.0059,
            GemCut::Radiant => 0.0081,
            GemCut::Asscher => 0.0080,
            GemCut::Hexagon => 0.0065,
            GemCut::HalfMoon => 0.0057,
        };
        let depth_mm = w_mm * self.depth_frac();
        (w_mm * l_mm * depth_mm * factor).max(0.0)
    }

    /// Superellipse exponent of the girdle in plan: 2 is an ellipse, higher
    /// squares the corners toward a rectangle, lower points the ends.
    ///
    /// One table, read by both the seat stock in [`crate::field::SeatPadLayer`]
    /// and the viewport's faceted preview, so the drawn stone and the metal
    /// cut for it are the same outline. Every value is at least 1, which
    /// keeps the plan convex — and a convex plan is star-shaped about the
    /// seat centre, which is what lets a mound built on it stay a monotone
    /// drop from a single crest.
    pub fn plan_pow(self) -> f64 {
        match self {
            GemCut::Round | GemCut::Oval | GemCut::Pear | GemCut::Heart => 2.0,
            GemCut::Cushion | GemCut::Trillion | GemCut::Hexagon | GemCut::HalfMoon => 3.2,
            GemCut::Princess
            | GemCut::Emerald
            | GemCut::Baguette
            | GemCut::Radiant
            | GemCut::Asscher => 6.0,
            GemCut::Marquise => 1.5,
        }
    }

    /// Calibrated stock widths, mm — the sizes a supplier actually stocks.
    pub fn calibrated_mm(self) -> &'static [f64] {
        match self {
            GemCut::Round => &[
                1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5,
                6.0, 6.5, 7.0, 8.0,
            ],
            GemCut::Baguette => &[1.5, 2.0, 2.5, 3.0],
            GemCut::HalfMoon => &[3.0, 4.0, 5.0, 6.0],
            _ => &[3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        }
    }
}

/// How the stone is made below its girdle — which is what decides how much
/// metal a seat has to swallow.
///
/// From the CrossGems gem-info `ObjectType` key (`0 = Gem, 1 = Cabochon`)
/// and their separate `Cabochons.GetProportion` table: a cabochon is a
/// different stone from a faceted one of the same footprint, not a finish
/// on it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GemForm {
    /// Crown, girdle and a pavilion diving to a culet.
    #[default]
    Faceted,
    /// A domed cabochon on a flat back. Nothing below the girdle, so it
    /// wants a bed rather than a hole — the easiest stone a cast band can
    /// carry, and the one a gypsy setting was invented for.
    Cabochon,
}

impl GemForm {
    pub const ALL: &'static [GemForm] = &[GemForm::Faceted, GemForm::Cabochon];

    pub fn label(self) -> &'static str {
        match self {
            GemForm::Faceted => "Faceted",
            GemForm::Cabochon => "Cabochon",
        }
    }
}

/// A cabochon's plan is fatter than the faceted make of the same name: their
/// cabochon Marquise and Pear are both 1.25 where the faceted cuts run 1.7
/// and 1.6. Read as a ceiling, which reproduces their whole table.
const CABOCHON_MAX_ASPECT: f64 = 1.25;

/// A medium dome, as a share of the width — the common make, and shallower
/// than a faceted stone of the same footprint because there is no pavilion
/// under it. (Their table's 0.6 is height over *length*, which on their
/// elongated cabs reads as a high dome; this is the trade's ordinary one.)
const CABOCHON_DOME_FRAC: f64 = 0.45;

/// One stone: a cut at a physical size, in one of the two makes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gem {
    pub cut: GemCut,
    /// Width (the short axis), mm.
    pub w_mm: f64,
    /// Length along the ring, mm. Equal to width for the square cuts.
    pub l_mm: f64,
    #[serde(default)]
    pub form: GemForm,
}

impl Default for Gem {
    fn default() -> Self {
        Gem::calibrated(GemCut::Round, 3.0)
    }
}

impl Gem {
    /// A stone of standard proportions at a stock width.
    pub fn calibrated(cut: GemCut, w_mm: f64) -> Self {
        Self { cut, w_mm, l_mm: w_mm * cut.aspect(), form: GemForm::Faceted }
    }

    /// A cabochon of the same cut, at its own fatter proportions.
    pub fn cabochon(cut: GemCut, w_mm: f64) -> Self {
        Self {
            cut,
            w_mm,
            l_mm: w_mm * cut.aspect().min(CABOCHON_MAX_ASPECT),
            form: GemForm::Cabochon,
        }
    }

    pub fn carats(&self) -> f64 {
        match self.form {
            GemForm::Faceted => self.cut.carats(self.w_mm, self.l_mm),
            // Half an ellipsoid at diamond density: volume (pi/6)·L·W·H mm³,
            // and a carat is 0.2 g of a 3.52 g/cm³ stone, so 0.0176 ct/mm³.
            GemForm::Cabochon => {
                let v = std::f64::consts::PI / 6.0 * self.l_mm * self.w_mm * self.depth_mm();
                (v * 0.0176).max(0.0)
            }
        }
    }

    /// Depth from table to culet, mm — the dome's own height for a cabochon.
    pub fn depth_mm(&self) -> f64 {
        match self.form {
            GemForm::Faceted => self.w_mm * self.cut.depth_frac(),
            GemForm::Cabochon => self.w_mm * CABOCHON_DOME_FRAC,
        }
    }

    /// Depth of the pavilion below the girdle, mm — what a seat must swallow.
    /// The crown above the girdle is roughly a third of the depth.
    ///
    /// A cabochon has no pavilion: it is flat-backed, so all it asks of the
    /// metal is a bed to sit flat on. Reading the faceted 0.65 of depth
    /// there refused a 6 mm cab on a 2 mm band — it wanted 2.42 mm of metal
    /// under a stone that needs none.
    pub fn pavilion_mm(&self) -> f64 {
        match self.form {
            GemForm::Faceted => self.depth_mm() * 0.65,
            GemForm::Cabochon => BED_CLEARANCE_MM,
        }
    }

    pub fn display(&self) -> String {
        let name = match self.form {
            GemForm::Faceted => self.cut.label().to_string(),
            GemForm::Cabochon => format!("{} cabochon", self.cut.label()),
        };
        if (self.l_mm - self.w_mm).abs() < 0.05 {
            format!("{:.1} mm {} ({:.2} ct)", self.w_mm, name, self.carats())
        } else {
            format!("{:.1}x{:.1} mm {} ({:.2} ct)", self.l_mm, self.w_mm, name, self.carats())
        }
    }
}

/// Metal a flat-backed stone still wants under it, mm — the setter's bed,
/// not a pavilion.
pub const BED_CLEARANCE_MM: f64 = 0.1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_carat_tables_hit_the_industry_anchors() {
        // The anchors everyone sizes against: a 6.5 mm round is one carat, a
        // 5.0 mm round is half, melee at 1.3 mm is a point.
        let one = GemCut::Round.carats(6.5, 6.5);
        assert!((one - 1.0).abs() < 0.08, "6.5 mm round: {one:.3} ct");
        let half = GemCut::Round.carats(5.0, 5.0);
        assert!((half - 0.5).abs() < 0.13, "5.0 mm round: {half:.3} ct");
        let melee = GemCut::Round.carats(1.3, 1.3);
        assert!((0.005..0.02).contains(&melee), "1.3 mm melee: {melee:.4} ct");

        // A princess of the same width outweighs a round: corners carry mass.
        assert!(GemCut::Princess.carats(5.0, 5.0) > GemCut::Round.carats(5.0, 5.0));
    }

    /// The proportions cross-checked against the CrossGems switches
    /// (tools/harvest/reports/CrossGems-Proportion.md): where we adopt, we match; where
    /// the trade disagrees with their UI defaults, we hold the trade's line.
    #[test]
    fn the_proportion_cross_check_holds() {
        assert_eq!(GemCut::Round.depth_frac(), 0.62, "classic 61.5% total depth");
        assert_eq!(GemCut::Princess.depth_frac(), 0.78, "a princess is deep");
        assert_eq!(GemCut::Oval.aspect(), 1.6);
        assert_eq!(GemCut::Pear.aspect(), 1.6);
        assert_eq!(GemCut::Emerald.aspect(), 1.5);
        assert!((GemCut::Hexagon.aspect() - 2.0 / 3.0f64.sqrt()).abs() < 1e-3);
        // Held against their defaults on purpose:
        assert_eq!(GemCut::Marquise.aspect(), 2.0, "the classic marquise make");
        assert_eq!(GemCut::Trillion.depth_frac(), 0.40, "trillions are shallow");
        // Every new cut carries a full row.
        for &cut in &[
            GemCut::Heart,
            GemCut::Radiant,
            GemCut::Asscher,
            GemCut::Hexagon,
            GemCut::HalfMoon,
        ] {
            assert!(cut.carats(4.0, 4.0 * cut.aspect()) > 0.0);
            assert!(!cut.label().is_empty());
        }
    }

    #[test]
    fn calibrated_stones_carry_their_aspect_and_depth() {
        for &cut in GemCut::ALL {
            let g = Gem::calibrated(cut, 4.0);
            assert!((g.l_mm - 4.0 * cut.aspect()).abs() < 1e-9, "{cut:?}");
            assert!(g.depth_mm() > 1.0 && g.depth_mm() < 4.0, "{cut:?}: {}", g.depth_mm());
            assert!(g.pavilion_mm() < g.depth_mm());
            assert!(g.carats() > 0.0);
            assert!(!cut.calibrated_mm().is_empty());
            assert!(!g.display().is_empty());
        }
    }
}
