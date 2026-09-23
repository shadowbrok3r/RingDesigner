//! Measure by touch: what a tap reads, a pin before the pick under the finger, and how taps chain.
use crate::viewport::pins::Pin;
use crate::visual::measure::{Measure, Picked, picked};
use egui::Pos2;
use ringdesign_core::{
    BuildResult, RingDesign,
    interaction::pick::{Entity, Pick},
};

/// A tap's pick added: two make a pair, a third chains on from the second and reads the corner there, a fourth starts again.
pub fn tap(m: &mut Measure, p: Picked) {
    m.add(p, true);
}

/// The pin nearest a finger at `p` within `reach` points on screen, as what a tap reads.
pub fn pin_at(pins: &[Pin], project: impl Fn([f64; 3]) -> Pos2, p: Pos2, reach: f32) -> Option<Picked> {
    pins.iter()
        .map(|pin| (pin, project(pin.world).distance(p)))
        .filter(|(_, d)| *d <= reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(pin, _)| Picked::Point { at: pin.world, label: pin.name.clone() })
}

/// What a tap reads from the picks under the finger, best first: the first one, a band point moved to where `snap` puts it when it does.
pub fn reading(picks: &[Pick], design: &RingDesign, built: &BuildResult, snap: impl FnOnce([f64; 3]) -> Option<([f64; 3], String)>) -> Option<Picked> {
    let first = picks.first()?;
    if first.entity == Entity::Band
        && let Some((at, label)) = snap(first.world)
    {
        return Some(Picked::Point { at, label });
    }
    picked(first, design, built)
}

/// What to tap next, or what the taps read: "Distance 3.142 mm", one line a reading.
pub fn said(m: &Measure) -> Vec<String> {
    match m.picks.as_slice() {
        [] => vec!["Tap a vertex, an edge, a face, a stone or the band".into()],
        [only] => vec![format!("From {}: tap the second", only.label())],
        _ => m.readings().iter().map(|r| r.line()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;
    use ringdesign_core::{
        AlphaLibrary, BuildParams,
        cad::{Attach, Component, Document, Feature, Operation, Placement},
        interaction::pick::{Filter, PickScene, Ray, ViewScale},
        mesh, templates,
    };

    fn point(at: [f64; 3], label: &str) -> Picked {
        Picked::Point { at, label: label.into() }
    }

    #[test]
    fn taps_make_a_pair_a_third_chains_to_the_corner_and_a_fourth_starts_again() {
        let mut m = Measure::default();
        assert_eq!(said(&m), ["Tap a vertex, an edge, a face, a stone or the band"]);
        tap(&mut m, point([3.0, 0.0, 0.0], "A"));
        assert_eq!(said(&m), ["From A: tap the second"]);
        tap(&mut m, point([0.0, 0.0, 0.0], "B"));
        assert_eq!(said(&m), ["Distance 3.000 mm"]);
        tap(&mut m, point([0.0, 4.0, 0.0], "C"));
        assert_eq!(said(&m), ["Distance 3.000 mm", "Distance 4.000 mm", "Corner 90.0°"]);
        tap(&mut m, point([1.0, 1.0, 1.0], "D"));
        assert_eq!(m.picks.len(), 1);
        assert_eq!(said(&m), ["From D: tap the second"]);
    }

    #[test]
    fn a_pin_under_the_finger_is_read_before_anything_else_and_the_nearest_wins() {
        let pins = [Pin::at([0.0, 10.0, 0.0], 1), Pin::at([0.0, 10.0, 1.0], 2)];
        // 10 points a millimetre, z up the screen from (200, 300).
        let project = |w: [f64; 3]| pos2(200.0 + w[0] as f32 * 10.0, 300.0 - w[2] as f32 * 10.0);
        assert_eq!(pin_at(&pins, project, pos2(203.0, 292.0), 22.0), Some(point([0.0, 10.0, 1.0], "Pin 2")));
        assert_eq!(pin_at(&pins, project, pos2(201.0, 299.0), 22.0), Some(point([0.0, 10.0, 0.0], "Pin 1")));
        assert_eq!(pin_at(&pins, project, pos2(240.0, 300.0), 22.0), None, "40 points off is past a fingertip");
        assert_eq!(pin_at(&[], project, pos2(200.0, 300.0), 22.0), None);
    }

    #[test]
    fn a_tap_reads_a_face_by_its_plane_and_a_band_point_where_the_snap_moves_it() {
        let mut d = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        let component = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.0), ..Component::default() };
        doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.0 }, component }).unwrap();
        d.cad = Some(doc);
        let built = mesh::build(&d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 256, profile_steps: 96, refine: None, ..BuildParams::default() });
        let scene = PickScene::build(&built, &d);
        let view = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 20.0 };
        let down = |x: f64, z: f64| Ray { origin: [x, 40.0, z], direction: [0.0, -1.0, 0.0] };
        // Straight down onto the post's end: its flat face, read by its plane.
        let picks = scene.pick(down(0.2, 0.3), &view, 12.0, Filter::default());
        let Some(Picked::Face { normal, flat: true, .. }) = reading(&picks, &d, &built, |_| panic!("a face is never snapped")) else { panic!("{:?}", picks.first()) };
        assert!(normal[1] > 0.999, "the end faces out along y: {normal:?}");
        // Down onto the band beside it: moved to the snap's point and named by it, else read where the finger was.
        let band = scene.pick(down(3.0, 0.5), &view, 12.0, Filter::default());
        assert_eq!(band.first().map(|p| p.entity.clone()), Some(Entity::Band));
        let snapped = reading(&band, &d, &built, |w| Some(([w[0], w[1], 0.0], "parting line".into()))).unwrap();
        assert_eq!(snapped.label(), "parting line");
        assert!(snapped.at()[2] == 0.0);
        let free = reading(&band, &d, &built, |_| None).unwrap();
        assert_eq!(free.label(), "the band");
        assert!((free.at()[2] - 0.5).abs() < 1e-6 && (free.at()[0] - 3.0).abs() < 1e-6, "{:?}", free.at());
        assert!(reading(&[], &d, &built, |_| None).is_none());
    }
}
