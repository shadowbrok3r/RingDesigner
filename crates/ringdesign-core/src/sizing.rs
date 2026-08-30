//! US ring sizing. Size = (inner circumference mm - 36.5) / 2.55.

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
pub const MAX_SIZE: f64 = 20.0;

impl RingSize {
    /// Round to the nearest quarter size, and refuse a number that is not one.
    ///
    /// The quarter-size rounding was documented and then bypassed on every
    /// path that built `RingSize(x)` directly, which is most of them.
    pub fn new(size: f64) -> Self {
        let size = if size.is_nan() { 7.0 } else { size };
        RingSize((size.clamp(MIN_SIZE, MAX_SIZE) * 4.0).round() / 4.0)
    }

    /// The size as a number the rest of the engine can divide by: finite and
    /// inside the range, whatever was put in the tuple.
    pub fn sane(self) -> f64 {
        if self.0.is_nan() { 7.0 } else { self.0.clamp(MIN_SIZE, MAX_SIZE) }
    }

    pub fn inner_circumference_mm(&self) -> f64 {
        36.5 + self.sane() * 2.55
    }

    pub fn inner_diameter_mm(&self) -> f64 {
        self.inner_circumference_mm() / std::f64::consts::PI
    }

    pub fn from_circumference_mm(circumference: f64) -> Self {
        Self::new((circumference - 36.5) / 2.55)
    }

    pub fn from_diameter_mm(diameter: f64) -> Self {
        Self::from_circumference_mm(diameter * std::f64::consts::PI)
    }

    /// `US 7`, `US 7.25`, `US 7.5` — never `US 7.`, which is what trimming
    /// the zeros off `{:.2}` gives for a size that is a whisker off an
    /// integer. A size arriving from a file or an interpolation is rarely
    /// exactly 7.0, so `fract() == 0.0` is not the test it looks like.
    pub fn display(&self) -> String {
        let s = format!("US {:.2}", self.0);
        let s = s.trim_end_matches('0');
        s.strip_suffix('.').unwrap_or(s).to_string()
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
        assert_eq!(RingSize::new(f64::INFINITY).0, MAX_SIZE);
        assert_eq!(RingSize::new(-1e300).0, MIN_SIZE);
        assert!(RingSize(f64::NAN).inner_circumference_mm().is_finite());
        assert!(RingSize(1e300).inner_diameter_mm().is_finite());
        assert!(RingSize(f64::NEG_INFINITY).inner_diameter_mm() > 0.0);
    }

    #[test]
    fn new_rounds_to_the_quarter_it_documents() {
        assert_eq!(RingSize::new(7.1).0, 7.0);
        assert_eq!(RingSize::new(7.2).0, 7.25);
        assert_eq!(RingSize::new(7.4).0, 7.5);
        assert_eq!(RingSize::new(7.9).0, 8.0);
    }
}
