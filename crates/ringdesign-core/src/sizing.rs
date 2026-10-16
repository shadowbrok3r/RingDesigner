//! Ring and bangle sizing on the US scale, size = (inner circumference mm - 36.5) / 2.55; a bangle is past US 20 and unrounded.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RingSize(pub f64);

/// Smallest and largest finger size the app will take.
///
/// Not a style limit — a sanity one. `RingSize` is a bare `pub f64` that
/// anything can construct, and it feeds `inner_radius_mm`, which feeds every
/// sweep and every clamp in the builder. A NaN or a 1e300 arriving from a
/// file, an interpolation or an MCP call used to travel all the way to the
/// vertices.
pub const MIN_SIZE: f64 = 0.5;
/// Largest size the engine takes, on the US scale: a [`MAX_BORE_MM`] bore.
pub const MAX_SIZE: f64 = (MAX_BORE_MM * std::f64::consts::PI - 36.5) / 2.55;

/// Largest finger size, a 27.9 mm bore: the quarter-size family stops here and a larger size is a bangle.
pub const MAX_RING_SIZE: f64 = 20.0;

/// Largest inside diameter the engine takes, mm.
pub const MAX_BORE_MM: f64 = 120.0;

/// The bangle family's usual inside diameters, mm: the picker's range.
pub const BANGLE_MIN_MM: f64 = 45.0;
pub const BANGLE_MAX_MM: f64 = 90.0;

/// A size family: what the number means to the person choosing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SizeFamily {
    /// A finger ring, in US quarter sizes.
    Ring,
    /// A bangle or cuff, by inside diameter in mm.
    Bangle,
}

impl SizeFamily {
    pub const ALL: [SizeFamily; 2] = [SizeFamily::Ring, SizeFamily::Bangle];

    pub fn label(self) -> &'static str {
        match self {
            SizeFamily::Ring => "Ring",
            SizeFamily::Bangle => "Bangle",
        }
    }

    /// The size a design switched into this family starts at: US 7, or a 63.5 mm bangle.
    pub fn default_size(self) -> RingSize {
        match self {
            SizeFamily::Ring => RingSize(7.0),
            SizeFamily::Bangle => RingSize::from_diameter_mm_exact(63.5),
        }
    }

    /// The family's ladder for a picker: US 3 to 15 in half sizes, or 2 to 2 7/8 inches inside in eighths.
    pub fn common(self) -> Vec<RingSize> {
        match self {
            SizeFamily::Ring => RingSize::common(),
            SizeFamily::Bangle => (0..8).map(|k| RingSize::from_diameter_mm_exact((2.0 + k as f64 / 8.0) * 25.4)).collect(),
        }
    }
}

/// The US-scale size of an inside diameter, unrounded.
fn size_of_diameter(diameter_mm: f64) -> f64 {
    (diameter_mm * std::f64::consts::PI - 36.5) / 2.55
}

impl RingSize {
    /// A finger size: rounded to the nearest quarter and held to the ring family, and a number that is not one refused.
    ///
    /// The quarter-size rounding was documented and then bypassed on every
    /// path that built `RingSize(x)` directly, which is most of them.
    pub fn new(size: f64) -> Self {
        let size = if size.is_nan() { 7.0 } else { size };
        RingSize((size.clamp(MIN_SIZE, MAX_RING_SIZE) * 4.0).round() / 4.0)
    }

    /// Any size the engine takes, unrounded: a bangle's must not quantize.
    pub fn exact(size: f64) -> Self {
        RingSize(if size.is_nan() { 7.0 } else { size.clamp(MIN_SIZE, MAX_SIZE) })
    }

    /// The size as a number the rest of the engine can divide by: finite and
    /// inside the range, whatever was put in the tuple.
    pub fn sane(self) -> f64 {
        if self.0.is_nan() { 7.0 } else { self.0.clamp(MIN_SIZE, MAX_SIZE) }
    }

    /// Which family the size belongs to: past US 20 it is a bangle.
    pub fn family(self) -> SizeFamily {
        if self.sane() > MAX_RING_SIZE { SizeFamily::Bangle } else { SizeFamily::Ring }
    }

    pub fn is_bangle(self) -> bool {
        self.family() == SizeFamily::Bangle
    }

    pub fn inner_circumference_mm(&self) -> f64 {
        36.5 + self.sane() * 2.55
    }

    pub fn inner_diameter_mm(&self) -> f64 {
        self.inner_circumference_mm() / std::f64::consts::PI
    }

    /// The size of a circumference: a finger's rounded to the nearest quarter, a bangle's kept exact.
    pub fn from_circumference_mm(circumference: f64) -> Self {
        let size = (circumference - 36.5) / 2.55;
        if size > MAX_RING_SIZE { Self::exact(size) } else { Self::new(size) }
    }

    /// The size of an inside diameter: a finger's rounded to the nearest quarter, a bangle's kept exact.
    pub fn from_diameter_mm(diameter: f64) -> Self {
        Self::from_circumference_mm(diameter * std::f64::consts::PI)
    }

    /// The size of an inside diameter in either family, unrounded.
    pub fn from_diameter_mm_exact(diameter: f64) -> Self {
        Self::exact(size_of_diameter(diameter))
    }

    /// `US 7`, `US 7.25`, `US 7.5` — never `US 7.`, which is what trimming
    /// the zeros off `{:.2}` gives for a size that is a whisker off an
    /// integer. A size arriving from a file or an interpolation is rarely
    /// exactly 7.0, so `fract() == 0.0` is not the test it looks like.
    /// A bangle reads by its inside diameter: `Bangle 63.5 mm`.
    pub fn display(&self) -> String {
        let trimmed = |s: String| {
            let t = s.trim_end_matches('0');
            t.strip_suffix('.').unwrap_or(t).to_string()
        };
        match self.family() {
            SizeFamily::Ring => trimmed(format!("US {:.2}", self.0)),
            SizeFamily::Bangle => format!("Bangle {} mm", trimmed(format!("{:.2}", self.inner_diameter_mm()))),
        }
    }

    /// Sizes from 3 to 15 in half-size steps.
    pub fn common() -> Vec<RingSize> {
        let mut out = Vec::new();
        let mut s = 3.0;
        while s <= 15.0 + 1e-9 {
            out.push(RingSize(s));
            s += 0.5;
        }
        out
    }
}

impl std::fmt::Display for RingSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The only module in core with no tests, on the one number a customer
    /// gives you.
    #[test]
    fn a_size_round_trips_through_its_own_geometry() {
        for s in RingSize::common() {
            let back = RingSize::from_circumference_mm(s.inner_circumference_mm());
            assert!((back.0 - s.0).abs() < 1e-9, "{s} -> {back}");
            let d = RingSize::from_diameter_mm(s.inner_diameter_mm());
            assert!((d.0 - s.0).abs() < 1e-9, "{s} by diameter -> {d}");
        }
    }

    /// A bare `pub f64` that feeds every clamp in the builder needs a floor
    /// and a ceiling, or a NaN out of a file reaches the vertices.
    #[test]
    fn a_hostile_size_cannot_reach_the_geometry() {
        assert_eq!(RingSize::new(f64::NAN).0, 7.0);
        assert_eq!(RingSize::new(f64::INFINITY).0, MAX_RING_SIZE);
        assert_eq!(RingSize::new(-1e300).0, MIN_SIZE);
        assert_eq!(RingSize::exact(f64::NAN).0, 7.0);
        assert_eq!(RingSize::exact(f64::INFINITY).0, MAX_SIZE);
        assert_eq!(RingSize::exact(-1e300).0, MIN_SIZE);
        assert!(RingSize(f64::NAN).inner_circumference_mm().is_finite());
        assert!(RingSize(1e300).inner_diameter_mm().is_finite());
        assert!((RingSize(1e300).inner_diameter_mm() - MAX_BORE_MM).abs() < 1e-9);
        assert!(RingSize(f64::NEG_INFINITY).inner_diameter_mm() > 0.0);
    }

    #[test]
    fn new_rounds_to_the_quarter_it_documents() {
        assert_eq!(RingSize::new(7.1).0, 7.0);
        assert_eq!(RingSize::new(7.2).0, 7.25);
        assert_eq!(RingSize::new(7.4).0, 7.5);
        assert_eq!(RingSize::new(7.9).0, 8.0);
        assert_eq!(RingSize::new(63.9).0, MAX_RING_SIZE, "a finger size stays a finger size");
    }

    /// A bangle is its inside diameter: never quarter-rounded, never clamped to a finger.
    #[test]
    fn a_bangle_keeps_its_bore_exactly() {
        for mm in [45.0, 50.8, 57.15, 60.0, 63.5, 66.675, 70.0, 75.0, 90.0, 120.0] {
            let s = RingSize::from_diameter_mm_exact(mm);
            assert!(s.is_bangle(), "{mm} mm is a bangle");
            assert!((s.inner_diameter_mm() - mm).abs() < 1e-12, "{mm} mm reads back {}", s.inner_diameter_mm());
            assert_eq!(RingSize::from_diameter_mm(mm), s, "past the finger sizes a diameter is kept exact");
        }
        assert_eq!(RingSize::from_diameter_mm_exact(63.5).inner_diameter_mm(), 63.5);
        assert_eq!(SizeFamily::Bangle.default_size().inner_diameter_mm(), 63.5);
        let back: RingSize = serde_json::from_str(&serde_json::to_string(&RingSize::from_diameter_mm_exact(63.5)).unwrap()).unwrap();
        assert_eq!(back.inner_diameter_mm(), 63.5, "the file carries the exact size");
        assert_eq!(RingSize::from_diameter_mm_exact(63.5).display(), "Bangle 63.5 mm");
        assert_eq!(RingSize::from_diameter_mm_exact(60.325).display(), "Bangle 60.33 mm");
        assert_eq!(RingSize::from_diameter_mm(17.35), RingSize(7.0), "a finger's bore still rounds to its quarter");
        assert!(!RingSize(MAX_RING_SIZE).is_bangle());
        assert!(RingSize::exact(MAX_RING_SIZE + 0.01).is_bangle());
        assert_eq!(RingSize(7.0).display(), "US 7");
        assert_eq!(serde_json::to_string(&RingSize(7.25)).unwrap(), "7.25", "a ring size serializes as it always did");
    }

    #[test]
    fn each_family_ladder_stays_in_its_family() {
        for f in SizeFamily::ALL {
            let ladder = f.common();
            assert!(!ladder.is_empty());
            assert!(ladder.iter().all(|s| s.family() == f), "{f:?}");
            assert!(ladder.windows(2).all(|w| w[0].0 < w[1].0), "{f:?} rises");
            assert_eq!(f.default_size().family(), f);
        }
        let bangles: Vec<f64> = SizeFamily::Bangle.common().iter().map(|s| s.inner_diameter_mm()).collect();
        assert!((bangles[0] - 50.8).abs() < 1e-9 && bangles.iter().any(|d| (d - 63.5).abs() < 1e-9));
        assert!(bangles.iter().all(|d| (BANGLE_MIN_MM..=BANGLE_MAX_MM).contains(d)));
    }
}
