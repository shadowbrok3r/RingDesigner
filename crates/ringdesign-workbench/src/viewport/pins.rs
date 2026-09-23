//! Pins: named points on the ring the snaps and Measure read from, and the band's right-click items for them.
use super::menu::{MenuAction, MenuItem};
use crate::command::{RingPoint, SnapKind, Target};
use crate::icons::Icon;
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
        let p = RingPoint::of_world(world, 0.0);
        Self { name: format!("Pin {number}"), theta_deg: p.theta_deg, across_mm: p.across_mm, world }
    }

    /// The pin as a snap reads it.
    pub fn target(&self) -> Target {
        let ring = RingPoint { theta_deg: self.theta_deg, across_mm: self.across_mm, height_mm: 0.0 };
        Target { kind: SnapKind::Pin, world: self.world, ring, label: self.name.clone() }
    }
}

/// The number the next pin takes: one past the highest a pin's name carries.
pub fn next_number(pins: &[Pin]) -> usize {
    pins.iter().filter_map(|p| p.name.strip_prefix("Pin ")?.parse::<usize>().ok()).max().unwrap_or(0) + 1
}

/// Pin items for a point of the band.
pub fn band_items(world: [f64; 3]) -> Vec<MenuItem> {
    vec![
        MenuItem::new("Pin here", Icon::Guide, MenuAction::PinHere { world }, "A reference point on the ring that the snaps and Measure read from"),
        MenuItem::new("Clear pins", Icon::Delete, MenuAction::ClearPins, "Take every pin off the ring"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pin_reads_its_place_on_the_ring_and_snaps_as_a_pin() {
        let pin = Pin::at([0.0, -9.5, 0.75], 3);
        assert_eq!((pin.name.as_str(), pin.theta_deg, pin.across_mm), ("Pin 3", 270.0, 0.75));
        let t = pin.target();
        assert_eq!((t.kind, t.world, t.ring, t.label.as_str()), (SnapKind::Pin, [0.0, -9.5, 0.75], RingPoint { theta_deg: 270.0, across_mm: 0.75, height_mm: 0.0 }, "Pin 3"));
        assert_eq!(next_number(&[]), 1);
        assert_eq!(next_number(&[pin.clone(), Pin { name: "Pin 1".into(), ..pin }]), 4);
        let items = band_items([0.0, 9.5, 0.0]);
        assert_eq!(items.iter().map(|i| (i.label.as_str(), i.action.clone())).collect::<Vec<_>>(), [("Pin here", MenuAction::PinHere { world: [0.0, 9.5, 0.0] }), ("Clear pins", MenuAction::ClearPins)]);
        assert!(items.iter().all(|i| i.enabled && i.submenu.is_none()));
    }
}
