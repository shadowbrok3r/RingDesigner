//! A primitive dragged out on the ring by one finger: seated where the menu was opened, its size and then its height dragged on the view plane through that point in twentieths of a millimetre, or typed.
use crate::command::{AddPrimitiveCmd, BandSurface, Outcome, Primitive, RingPoint, Session, SnapHit, StepInput, ViewCommand, plane_basis, ring::on_view_plane, ring_point};
use ringdesign_core::{RingDesign, cad::Placement, interaction::pick::Ray};

/// A dragged size moves in steps of this, mm.
pub const SIZE_STEP_MM: f64 = 0.05;

/// The primitive a menu row's label names: a box, a cylinder or a sphere is dragged out, anything else is placed as it is.
pub fn kind(label: &str) -> Option<Primitive> {
    match label {
        "Box" => Some(Primitive::Box),
        "Cylinder" => Some(Primitive::Cylinder),
        "Sphere" => Some(Primitive::Sphere),
        _ => None,
    }
}

/// The add of `kind` seated at `at` on `design`'s ring, `normal` the surface's there and `band` the surface parts are seated on, waiting for its size; a plain ring's first part takes id 2, leaving 1 to its shank.
/// The seat lands on the ring point `snap` gives for where it would sit, as a click on the desktop does, and keeps the pressed point where it gives none.
pub fn start(design: &RingDesign, kind: Primitive, at: [f64; 3], normal: [f64; 3], band: Option<&BandSurface>, snap: &dyn Fn(RingPoint) -> Option<SnapHit>) -> Result<AddPrimitiveCmd, String> {
    let empty = design.cad.as_ref().is_none_or(|d| d.features.is_empty());
    let id = if empty { 2 } else { crate::touch::parts::fresh_ids(design)() };
    let mut cmd = AddPrimitiveCmd::new(kind, id);
    let ring = ring_point(at, band, design.inner_radius_mm() + design.profile.thickness_mm);
    cmd.feed(&StepInput::Pointer { world: at, normal, theta_deg: ring.theta_deg, across_mm: ring.across_mm, height_mm: 0.0, snapped: None, dragging: false });
    if let Some(Placement::Ring { theta_deg, across_mm, height_mm, .. }) = cmd.preview().placement
        && let Some(hit) = snap(RingPoint { theta_deg, across_mm, height_mm })
    {
        let (theta_deg, across_mm) = (hit.ring.theta_deg, hit.ring.across_mm);
        cmd.feed(&StepInput::Pointer { world: hit.world, normal, theta_deg, across_mm, height_mm: hit.ring.height_mm - height_mm, snapped: Some(hit), dragging: false });
    }
    match cmd.feed(&StepInput::Click) {
        Outcome::NextStep => Ok(cmd),
        Outcome::Refused(why) => Err(why),
        other => Err(format!("The part would not seat there: {other:?}")),
    }
}

/// Where a live add is seated: the first point its preview's ghost marks.
pub fn centre(cmd: &dyn ViewCommand) -> Option<[f64; 3]> {
    cmd.preview().ghost.first().copied()
}

/// The pointer a finger on `ray` gives a primitive sized about `centre`: where the ray crosses the view plane through the centre, its distance from the centre on the nearest step.
pub fn size_token(ray: Ray, centre: [f64; 3]) -> Option<StepInput> {
    let p = on_view_plane(ray, centre)?;
    let d: [f64; 3] = std::array::from_fn(|k| p[k] - centre[k]);
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let dir = if len > 1e-9 { d.map(|x| x / len) } else { plane_basis(ray.direction).0 };
    let stepped = ((len / SIZE_STEP_MM).round() * SIZE_STEP_MM).max(SIZE_STEP_MM);
    let back = {
        let l = (ray.direction[0].powi(2) + ray.direction[1].powi(2) + ray.direction[2].powi(2)).sqrt().max(1e-12);
        ray.direction.map(|v| -v / l)
    };
    Some(StepInput::Pointer { world: std::array::from_fn(|k| centre[k] + dir[k] * stepped), normal: back, theta_deg: 0.0, across_mm: 0.0, height_mm: 0.0, snapped: None, dragging: true })
}

/// Finishes a live add as it stands: each step it still waits on is confirmed with what it holds.
pub fn finish(session: &mut Session) -> Outcome {
    let mut o = session.enter();
    for _ in 0..2 {
        if !matches!(o, Outcome::NextStep) {
            break;
        }
        o = session.enter();
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Effect;
    use ringdesign_core::{
        AlphaLibrary, BuildParams,
        cad::{Attach, Operation, Placement},
        mesh, templates,
    };

    fn court() -> RingDesign {
        templates::all().iter().find(|t| t.name == "Court band").unwrap().design()
    }

    /// Straight down the finger's axis's normal onto the ring's top, `x` mm right of the middle and `z` along the finger.
    fn down(x: f64, z: f64) -> Ray {
        Ray { origin: [x, 40.0, z], direction: [0.0, -1.0, 0.0] }
    }

    #[test]
    fn a_box_is_seated_where_the_menu_opened_then_dragged_to_size_and_height_and_added_as_one_part_on_its_shank() {
        let d = court();
        let params = BuildParams { theta_steps: 192, profile_steps: 96, refine: None, ..BuildParams::default() };
        let built = mesh::build(&d, &AlphaLibrary::builtin(), params);
        let (at, n) = ringdesign_core::cad::surface_hit(&built.mesh, 90.0, 0.0).unwrap();
        let mut s = Session::default();
        s.start(Box::new(start(&d, Primitive::Box, at, n, None, &|_| None).unwrap()));
        assert_eq!(s.command().map(|c| (c.key(), c.step())), Some(("add-box", 1)), "seated, it waits for its size");
        let c = centre(s.command().unwrap()).unwrap();
        assert_eq!(c, at);
        // A finger 1.23 mm right of the seat on the view plane: a half-size of 1.25, the nearest twentieth.
        s.feed(size_token(down(1.23, 0.0), c).unwrap());
        let dims = s.dimensions();
        assert!(dims[0].key == "radius" && (dims[0].value - 1.25).abs() < 1e-9, "{dims:?}");
        assert_eq!(s.preview().unwrap().caption, "Add box 2.50 × 2.50 × h 1.00 mm at 90.0°");
        // Lifted, the height is next; 0.77 mm up the screen stands it 0.75 tall, and the lift adds it.
        assert!(matches!(s.feed(StepInput::Click), Outcome::NextStep));
        s.feed(size_token(down(0.0, 0.77), c).unwrap());
        let Outcome::Commit(effects) = s.feed(StepInput::Click) else { panic!("the second lift adds it") };
        let [Effect::Add { feature }] = effects.as_slice() else { panic!("{effects:?}") };
        let Operation::Box { size } = feature.operation else { panic!("{:?}", feature.operation) };
        assert!((0..3).all(|k| (size[k] - [2.5, 2.5, 0.75][k]).abs() < 1e-9), "{size:?}");
        let Placement::Ring { theta_deg, across_mm, height_mm, .. } = feature.component.placement else { panic!() };
        assert!((theta_deg - 90.0).abs() < 1e-6 && across_mm.abs() < 1e-3 && height_mm == 0.0, "{theta_deg} {across_mm} {height_mm}");
        assert_eq!((feature.id, feature.component.attach), (2, Attach::Join));
        // Through the funnel it brings the plain ring's shank and stands on it, joined.
        let (edits, added) = crate::touch::parts::effect_edits(&d, effects);
        let p = crate::touch::prepare(&d, &edits, None).unwrap().unwrap();
        assert!(added && p.label == "Add Procedural shank · Add Box", "{}", p.label);
        let after = mesh::build(&p.design, &AlphaLibrary::builtin(), params);
        let part = after.parts.evaluated.as_ref().unwrap().components.iter().find(|x| x.id == 2).unwrap();
        let (lo, hi) = part.mesh.bounds().unwrap();
        assert!(((hi.0 - lo.0) - 2.5).abs() < 1e-3 && ((hi.2 - lo.2) - 2.5).abs() < 1e-3, "{lo:?} {hi:?}");
        let (was, now) = (built.mesh.volume_mm3(), after.mesh.volume_mm3());
        assert!(now > was + 3.0, "the box joins the band: {was} then {now}");
    }

    #[test]
    fn a_typed_size_holds_against_the_finger_and_done_adds_what_stands() {
        let d = court();
        let at = [0.0, 10.4, 0.0];
        let mut s = Session::default();
        s.start(Box::new(start(&d, Primitive::Cylinder, at, [0.0, 1.0, 0.0], None, &|_| None).unwrap()));
        s.feed(StepInput::Typed { key: "radius", value: 1.5 });
        s.feed(size_token(down(3.0, 0.0), at).unwrap());
        assert_eq!(s.dimensions()[0].value, 1.5, "a typed radius holds");
        // Done at the size step adds it with the height it has, 1 mm until one is dragged or typed.
        let Outcome::Commit(effects) = finish(&mut s) else { panic!("Done adds it") };
        let [Effect::Add { feature }] = effects.as_slice() else { panic!("{effects:?}") };
        assert!(matches!(feature.operation, Operation::Cylinder { radius_mm, height_mm } if radius_mm == 1.5 && height_mm == 1.0), "{:?}", feature.operation);
        // A sphere has no height: Done straight after the size.
        let mut s = Session::default();
        s.start(Box::new(start(&d, Primitive::Sphere, at, [0.0, 1.0, 0.0], None, &|_| None).unwrap()));
        s.feed(size_token(down(0.0, 0.0), at).unwrap());
        assert!((s.dimensions()[0].value - SIZE_STEP_MM).abs() < 1e-12, "a finger on the seat still gives a size");
        let Outcome::Commit(effects) = finish(&mut s) else { panic!() };
        assert!(matches!(&effects[..], [Effect::Add { feature }] if matches!(feature.operation, Operation::Sphere { radius_mm } if (radius_mm - SIZE_STEP_MM).abs() < 1e-12)));
        assert_eq!([kind("Box"), kind("Cylinder"), kind("Sphere"), kind("Loft")], [Some(Primitive::Box), Some(Primitive::Cylinder), Some(Primitive::Sphere), None]);
    }

    #[test]
    fn a_seat_pressed_by_a_ring_feature_lands_on_it_as_the_desktops_click_does() {
        use crate::command::{Dofs, Grid, RingFeatures, Scene, SnapGeometry, Snapper};
        use ringdesign_core::interaction::pick::ViewScale;
        let d = court();
        let features = RingFeatures::of(&d, 0.0);
        let nominal = d.inner_radius_mm() + d.profile.thickness_mm;
        let world_of = |p: RingPoint| {
            let (s, c) = p.theta_deg.to_radians().sin_cos();
            let r = nominal + p.height_mm;
            Some([r * c, r * s, p.across_mm])
        };
        // Looking down on the top at 10 px a millimetre, the desktop's snapper: its 5° grid, the crest.
        let view = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 10.0 };
        let scene = Scene { view, aperture_px: crate::touch::APERTURE_PT, geometry: SnapGeometry::default(), features: &features, design: Some(&d), world_of: &world_of };
        let snapper = Snapper { grid: Some(Grid { theta_deg: 5.0, across_mm: 0.5, height_mm: 0.5 }), crest: true, ..Snapper::default() };
        let snap = |p: RingPoint| snapper.snap_ring(world_of(p)?, p, Dofs::ALL, &scene);
        // Pressed 1.4° short of the top and 0.3 mm off the parting line: seated on both.
        let pressed = world_of(RingPoint { theta_deg: 88.6, across_mm: 0.3, height_mm: 0.0 }).unwrap();
        let cmd = start(&d, Primitive::Cylinder, pressed, [0.0, 1.0, 0.0], None, &snap).unwrap();
        let Some(Placement::Ring { theta_deg, across_mm, .. }) = cmd.preview().placement else { panic!() };
        assert!((theta_deg - 90.0).abs() < 1e-9 && across_mm.abs() < 1e-9, "{theta_deg} {across_mm}");
        let at = centre(&cmd).unwrap();
        assert!(at[0].abs() < 1e-9 && (at[1] - nominal).abs() < 1e-9 && at[2].abs() < 1e-9, "the size is dragged from the snapped seat: {at:?}");
        assert!(cmd.preview().caption.ends_with("at 90.0° · top 90.0° · parting line"), "{}", cmd.preview().caption);
        // Without a snap the press stands as it was.
        let cmd = start(&d, Primitive::Cylinder, pressed, [0.0, 1.0, 0.0], None, &|_| None).unwrap();
        let Some(Placement::Ring { theta_deg, across_mm, .. }) = cmd.preview().placement else { panic!() };
        assert!((theta_deg - 88.6).abs() < 1e-9 && (across_mm - 0.3).abs() < 1e-9, "{theta_deg} {across_mm}");
    }

    #[test]
    fn a_second_part_takes_a_fresh_id() {
        let d = court();
        let (edits, _) = crate::touch::parts::part_here(&d, "Cylinder", 90.0, 0.0).unwrap();
        let d = crate::touch::prepare(&d, &edits, None).unwrap().unwrap().design;
        let mut s = Session::default();
        s.start(Box::new(start(&d, Primitive::Box, [10.4, 0.0, 0.0], [1.0, 0.0, 0.0], None, &|_| None).unwrap()));
        let Outcome::Commit(effects) = finish(&mut s) else { panic!() };
        let [Effect::Add { feature }] = effects.as_slice() else { panic!() };
        assert_eq!(feature.id, 3);
        let Placement::Ring { theta_deg, .. } = feature.component.placement else { panic!() };
        assert!(theta_deg.abs() < 1e-9, "seated at 0°: {theta_deg}");
    }
}
