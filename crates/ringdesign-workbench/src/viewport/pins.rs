//! Pins: named points on the ring the snaps and Measure read from, and the band's right-click items for them.
use super::menu::{MenuAction, MenuItem};
use crate::command::{RingPoint, SnapKind, Target};
use crate::icons::Icon;

pub use ringdesign_core::pins::{Pin, next_number};

impl From<&Pin> for Target {
    fn from(pin: &Pin) -> Self {
        let ring = RingPoint { theta_deg: pin.theta_deg, across_mm: pin.across_mm, height_mm: 0.0 };
        Target { kind: SnapKind::Pin, world: pin.world, ring, label: pin.name.clone() }
    }
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
        let t: Target = pin.target();
        assert_eq!((t.kind, t.world, t.ring, t.label.as_str()), (SnapKind::Pin, [0.0, -9.5, 0.75], RingPoint { theta_deg: 270.0, across_mm: 0.75, height_mm: 0.0 }, "Pin 3"));
        assert_eq!(RingPoint::of_world(pin.world, 0.0), t.ring, "the design's pin reads round the ring as the snaps do");
        assert_eq!(next_number(&[]), 1);
        assert_eq!(next_number(&[pin.clone(), Pin { name: "Pin 1".into(), ..pin }]), 4);
        let items = band_items([0.0, 9.5, 0.0]);
        assert_eq!(items.iter().map(|i| (i.label.as_str(), i.action.clone())).collect::<Vec<_>>(), [("Pin here", MenuAction::PinHere { world: [0.0, 9.5, 0.0] }), ("Clear pins", MenuAction::ClearPins)]);
        assert!(items.iter().all(|i| i.enabled && i.submenu.is_none()));
    }
}
