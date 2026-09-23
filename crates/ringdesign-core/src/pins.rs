//! Pins: named points on the ring that the snaps and Measure read from, carried in the design.
use serde::{Deserialize, Serialize};

/// A named point on the ring: round it, along the finger, and in the world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pin {
    pub name: String,
    pub theta_deg: f64,
    pub across_mm: f64,
    pub world: [f64; 3],
}

impl Pin {
    /// A pin where the band was clicked, named by its number.
    pub fn at(world: [f64; 3], number: usize) -> Self {
        let theta = world[1].atan2(world[0]).to_degrees().rem_euclid(360.0);
        Self { name: format!("Pin {number}"), theta_deg: if theta >= 360.0 { 0.0 } else { theta + 0.0 }, across_mm: world[2], world }
    }

    /// The pin as a reader's own point type takes it, such as a snap target.
    pub fn target<T: for<'a> From<&'a Pin>>(&self) -> T {
        T::from(self)
    }
}

/// The number the next pin takes: one past the highest a pin's name carries.
pub fn next_number(pins: &[Pin]) -> usize {
    pins.iter().filter_map(|p| p.name.strip_prefix("Pin ")?.parse::<usize>().ok()).max().unwrap_or(0) + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pin_reads_its_place_round_the_ring_and_travels_in_the_design_only_when_there_is_one() {
        let pin = Pin::at([0.0, -9.5, 0.75], 3);
        assert_eq!((pin.name.as_str(), pin.theta_deg, pin.across_mm), ("Pin 3", 270.0, 0.75));
        assert_eq!(Pin::at([9.5, -1e-18, 0.0], 1).theta_deg, 0.0, "a hair under the x axis reads 0°, never 360°");
        assert_eq!(next_number(&[]), 1);
        assert_eq!(next_number(&[pin.clone(), Pin { name: "Pin 1".into(), ..pin.clone() }, Pin { name: "Mark".into(), ..pin.clone() }]), 4);
        // A design without pins writes no key, so a file stays what an older build wrote; one with pins reads back exact.
        let mut d = crate::RingDesign::default();
        let bare = serde_json::to_value(&d).unwrap();
        assert!(bare.get("pins").is_none());
        d.pins.push(pin.clone());
        let back: crate::RingDesign = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back.pins, [pin]);
        let older: crate::RingDesign = serde_json::from_value(bare).unwrap();
        assert!(older.pins.is_empty());
    }
}
