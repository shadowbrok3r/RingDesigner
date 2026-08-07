//! US ring sizing. Size = (inner circumference mm - 36.5) / 2.55.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RingSize(pub f64);

impl RingSize {
    /// Round to the nearest quarter size.
    pub fn new(size: f64) -> Self {
        RingSize((size * 4.0).round() / 4.0)
    }

    pub fn inner_circumference_mm(&self) -> f64 {
        36.5 + self.0 * 2.55
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

    pub fn display(&self) -> String {
        if self.0.fract() == 0.0 {
            format!("US {}", self.0 as i32)
        } else {
            format!("US {:.2}", self.0).trim_end_matches('0').to_string()
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
