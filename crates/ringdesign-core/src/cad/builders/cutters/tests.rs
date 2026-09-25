use super::*;
use crate::cad::{self, Document, EvaluatedComponent, FeatureStatus, builders};
use crate::castability::{PartVerdict, judged_field_report};
use crate::csg;
use crate::interaction::bvh::Bvh;
use crate::mesh::BuildResult;
use crate::profile::ProfileStyle;
use crate::{AlphaLibrary, BuildParams, Mesh};

/// Undercut a part may show as facet noise, mm²: the verdict's own floor for a part.
const PART_NOISE_MM2: f64 = 0.005;

fn params() -> BuildParams {
    BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() }
}
fn court() -> RingDesign {
    crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design()
}

fn shank_cutter(key: &str, params: Json, stage: Stage) -> Feature {
    Feature { id: 2, name: label(key).into(), enabled: true, operation: Operation::Builder { key: key.into(), on: None, params }, component: Component { stage, ..component(key) } }
}

fn gallery_band() -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.2;
    d.profile.thickness_mm = 1.8;
    d.shank.kind = crate::profile::ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = [(270.0, 1.0), (200.0, 1.0), (340.0, 1.0), (150.0, 1.35), (30.0, 1.35), (120.0, 1.85), (60.0, 1.85), (90.0, 2.3)].into_iter().map(|(theta_deg, thickness_scale)| crate::profile::ShankKey { theta_deg, thickness_scale, ..Default::default() }).collect();
    d
}

#[test]
fn split_cutters_close_at_real_tips_leave_rails_and_refuse_sand_casting() {
    let mut d = flat();
    crate::castability::CastProcess::LostWax.apply(&mut d.draft);
    for tip in ["Point", "Round"] {
        let design = with(&d, vec![shank_cutter(SPLIT, json!({"tip":tip}), Stage::Cast)]);
        let evaluated = cad::evaluate(&design, &AlphaLibrary::builtin(), params()).unwrap();
        assert_eq!(evaluated.status_of(2), Some(&FeatureStatus::Ok));
        let c = evaluated.components.iter().find(|c| c.id == 2).unwrap();
        let check = c.made.as_ref().unwrap().solid().check(true);
        assert_eq!((check.open_edges, check.repeated_edges, check.zero_area_faces, check.self_crossings), (0, 0, 0, Some(0)), "{tip}: {check:?}");
        assert!(check.volume > 0.0);
        assert_eq!(c.settings.blend_mm, 0.0, "the cutter rounds its own rims without a second seam operation");
        let built = build(&design);
        watertight(&built);
        assert!(built.parts.notes.is_empty(), "{tip}: {:?}", built.parts.notes);
        assert_eq!(pieces(&as_solid(&built.mesh)), 1);
        let bvh = Bvh::build(&built.mesh);
        let radius = d.inner_radius_mm() + d.profile.thickness_mm * 0.5;
        assert!(bvh.ray(&built.mesh, [0.0, radius, -0.5], [0.0, 0.0, 1.0]).is_some(), "the upper rail remains");
        assert!(bvh.ray(&built.mesh, [0.0, radius, 0.0], [0.0, 1.0, 0.0]).is_none(), "the slot opens from the bore to the crest");
    }
    let sand = with(&flat(), vec![shank_cutter(SPLIT, json!({}), Stage::Cast)]);
    assert!(matches!(status(&sand, 2), FeatureStatus::Failed(m) if m.contains("a true split is two crests")));
    let bench = with(&flat(), vec![shank_cutter(SPLIT, json!({}), Stage::Bench)]);
    assert_eq!(status(&bench, 2), FeatureStatus::Ok);
    let thin = with(&d, vec![shank_cutter(SPLIT, json!({"gap_mm":5.0}), Stage::Cast)]);
    assert!(matches!(status(&thin, 2), FeatureStatus::Failed(m) if m.contains("Split at") && m.contains("rail")));
}

#[test]
fn gallery_windows_are_closed_drafted_and_open_along_the_pull_with_both_rails() {
    let d = gallery_band();
    let design = with(&d, vec![shank_cutter(WINDOW, json!({}), Stage::Cast)]);
    let built = build(&design);
    watertight(&built);
    assert!(built.parts.notes.is_empty(), "{:?}", built.parts.notes);
    let c = part(&built, 2);
    assert_eq!(c.stage, Stage::Cast);
    let s = c.made.as_ref().unwrap().solid();
    let check = s.check(true);
    assert_eq!((check.open_edges, check.repeated_edges, check.zero_area_faces, check.self_crossings), (0, 0, 0, Some(0)), "{check:?}");
    let bvh = Bvh::build(&built.mesh);
    let bore = Bore::of(&d, &cadkernel::brep::Placement::IDENTITY);
    let rs = bore.crossings(90.0, 0.0);
    let mid = (rs[0] + rs[1]) * 0.5;
    for z in [-5.0, 5.0] {
        assert!(bvh.ray(&built.mesh, [0.0, mid, z], [0.0, 0.0, -z.signum()]).is_none(), "the gallery opens through both faces");
    }
    let inner = run(&built.mesh, &bvh, [0.0, rs[0] - 0.5, 0.0], [0.0, 1.0, 0.0]).unwrap().1;
    let outer = run(&built.mesh, &bvh, [0.0, rs[1] + 0.5, 0.0], [0.0, -1.0, 0.0]).unwrap().1;
    assert!(inner >= 0.8 && outer >= 0.8, "gallery rails {inner:.3}/{outer:.3} mm");
    assert_eq!(pieces(&as_solid(&built.mesh)), 1);
    let zero = with(&d, vec![shank_cutter(WINDOW, json!({"draft_deg":0.0,"tip_round_mm":0.0}), Stage::Cast)]);
    assert_eq!(status(&zero, 2), FeatureStatus::Ok);
    let no_room = with(&flat(), vec![shank_cutter(WINDOW, json!({"rail_in_mm":2.0,"rail_out_mm":2.0}), Stage::Cast)]);
    assert!(matches!(status(&no_room, 2), FeatureStatus::Failed(m) if m.contains("Gallery window at") && m.contains("rails")));
}

#[test]
fn shank_cutters_preserve_the_existing_format_fence_and_refuse_ignored_placements() {
    for key in [SPLIT, WINDOW] {
        let mut d = with(&gallery_band(), vec![shank_cutter(key, json!({}), Stage::Bench)]);
        assert_eq!(crate::library::format_version_for(&d), 6);
        let text = crate::library::design_json(&d).unwrap();
        assert_eq!(serde_json::to_value(crate::library::load_design_str(&text).unwrap()).unwrap(), serde_json::to_value(&d).unwrap());
        let graph = json!({"nodes":[{"kind":"cluster","params":{"graph":{"nodes":[{"kind":"cad.feature","params":{"operation":d.cad.as_ref().unwrap().feature(2).unwrap().operation}}]}}}]});
        assert!(crate::library::template_features_in_json(&graph));
        d.cad.as_mut().unwrap().features[1].component.placement = Placement::ring(90.0, 0.0);
        assert!(matches!(status(&d, 2), FeatureStatus::Failed(m) if m.contains("ring angles")));
    }
}
/// A flat band with square side faces, 5 mm wide and 2.5 mm thick.
fn flat() -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = 5.0;
    d.profile.thickness_mm = 2.5;
    d
}
/// `band` with the procedural shank #1 and `parts` after it.
fn with(band: &RingDesign, parts: Vec<Feature>) -> RingDesign {
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    for f in parts {
        doc.append(f).unwrap();
    }
    RingDesign { cad: Some(doc), ..band.clone() }
}
fn build(d: &RingDesign) -> BuildResult {
    crate::mesh::try_build(d, &AlphaLibrary::builtin(), params()).unwrap()
}
fn part(b: &BuildResult, id: Id) -> EvaluatedComponent {
    b.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("#{id}: {:?}", b.parts.notes)).clone()
}
fn status(d: &RingDesign, id: Id) -> FeatureStatus {
    let e = cad::evaluate(d, &AlphaLibrary::builtin(), params()).unwrap();
    e.status_of(id).cloned().unwrap()
}
fn watertight(b: &BuildResult) {
    let v = &b.report.validation;
    assert!(v.watertight && v.boundary_edges == 0 && v.non_manifold_edges == 0, "{v:?}");
}
fn as_solid(mesh: &Mesh) -> Solid {
    Solid { v: mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: mesh.faces.clone() }
}
/// Faces joined by shared edges into separate pieces.
fn pieces(s: &Solid) -> usize {
    let mut root: Vec<usize> = (0..s.v.len()).collect();
    fn find(root: &mut [usize], mut i: usize) -> usize {
        while root[i] != i {
            root[i] = root[root[i]];
            i = root[i];
        }
        i
    }
    for f in &s.f {
        for k in 1..3 {
            let (a, b) = (find(&mut root, f[0] as usize), find(&mut root, f[k] as usize));
            root[a.max(b)] = a.min(b);
        }
    }
    let used: std::collections::HashSet<usize> = s.f.iter().flatten().map(|v| find(&mut root, *v as usize)).collect();
    used.len()
}
/// The first run of metal a line through `mesh` from `from` along unit `dir` crosses: how far to it, and its length.
fn run(mesh: &Mesh, bvh: &Bvh, from: P3, dir: P3) -> Option<(f64, f64)> {
    let (_, t_in) = bvh.ray(mesh, from, dir)?;
    let (_, t_out) = bvh.ray(mesh, add(from, scale(dir, t_in + 1e-5)), dir)?;
    Some((t_in, t_out + 1e-5))
}
fn perimeter(poly: &[[f64; 2]]) -> f64 {
    (0..poly.len()).map(|i| (poly[(i + 1) % poly.len()][0] - poly[i][0]).hypot(poly[(i + 1) % poly.len()][1] - poly[i][1])).sum()
}
/// The piercing's plan in its own frame, as its parameters draw it.
fn plan_of(shape: Shape, p: &Json) -> Vec<[f64; 2]> {
    let (l, w, turn) = (p["length_mm"].as_f64().unwrap(), p["width_mm"].as_f64().unwrap(), p["turn_deg"].as_f64().unwrap().to_radians());
    outline(shape, l, w, 0.0).into_iter().map(|q| turned(q, turn)).collect()
}
/// Points on a 0.04 mm grid inside `plan`.
fn inside_grid(plan: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let (lo, hi) = plan.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
    let h = 0.04;
    let (nx, ny) = (((hi[0] - lo[0]) / h) as usize + 1, ((hi[1] - lo[1]) / h) as usize + 1);
    (0..nx).flat_map(|i| (0..ny).map(move |j| [lo[0] + (i as f64 + 0.5) * h, lo[1] + (j as f64 + 0.5) * h])).filter(|q| inside(plan, *q)).collect()
}
/// The mean metal each line of the plan crosses down the cut's axis, through the band before the cut.
fn mean_wall(bare: &Mesh, bvh: &Bvh, c: &EvaluatedComponent, plan: &[[f64; 2]]) -> f64 {
    let down = scale(c.frame.z_axis, -1.0);
    let walls: Vec<f64> = inside_grid(plan).iter().filter_map(|q| run(bare, bvh, c.frame.point([q[0], q[1], 6.0]), down).map(|r| r.1)).collect();
    walls.iter().sum::<f64>() / walls.len().max(1) as f64
}

#[test]
fn every_outline_turns_anticlockwise_grows_without_folding_and_is_star_shaped_about_its_centre() {
    for shape in Shape::ALL {
        for (l, w) in [(1.0, 1.0), (1.6, 1.2), (3.0, 1.5), (2.4, 2.8)] {
            let (sl, sw) = shape.sized(l, w);
            let c = centre(shape, l, w);
            let base = outline(shape, l, w, 0.0);
            let mut last = area(&base);
            assert!(last > 0.0, "{shape:?} {l}x{w} runs clockwise");
            for g in [0.1, 0.25, 0.5] {
                let grown = outline(shape, l, w, g);
                assert_eq!(grown.len(), base.len(), "{shape:?}: rings of one outline loft point for point");
                let a = area(&grown);
                assert!(a > last, "{shape:?} {l}x{w} grown {g}: {a:.4} against {last:.4}");
                last = a;
                // Every fan triangle from the centre turns anticlockwise: nothing folds and the caps close.
                for i in 0..grown.len() {
                    let t = cross2(c, grown[i], grown[(i + 1) % grown.len()]);
                    assert!(t > 0.0, "{shape:?} {l}x{w} grown {g}: the fan folds at point {i} ({t:e})");
                }
            }
            // The plan's own extent is the length and width asked for, as the shape takes them.
            let (x0, x1) = base.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
            let (y0, y1) = base.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
            assert!((x1 - x0 - sl).abs() < 0.01 * sl && (y1 - y0 - sw).abs() < 0.01 * sw, "{shape:?} {l}x{w}: {:.3} x {:.3} against {sl:.3} x {sw:.3}", x1 - x0, y1 - y0);
        }
    }
    // A round is a circle: its area is π r² to the sampling.
    let r = outline(Shape::Round, 2.0, 2.0, 0.0);
    assert!((area(&r) - PI).abs() < 0.005, "{}", area(&r));
    // A heart's cleft is a notch: the lobes reach past the point where they meet on the axis.
    let h = outline(Shape::Heart, 3.0, 3.0, 0.0);
    let cleft = h.iter().filter(|p| p[1].abs() < 1e-9).map(|p| p[0]).fold(f64::MIN, f64::max);
    let lobes = h.iter().map(|p| p[0]).fold(f64::MIN, f64::max);
    assert!(lobes - cleft > 0.15, "the cleft stands {:.3} in from the lobes", lobes - cleft);
}

#[test]
fn a_piercing_through_the_crown_takes_its_area_times_the_wall_it_crosses_in_every_shape() {
    let band = court();
    let bare = build(&with(&band, vec![]));
    let bvh = Bvh::build(&bare.mesh);
    for shape in Shape::ALL {
        let at = pierce_at(&band, 90.0, 0.0, None, shape).unwrap();
        assert!(!at.side_face);
        assert_eq!(at.stage, Stage::Bench, "{shape:?}: a hole across the pull is drilled at the bench under sand");
        let built = build(&with(&band, vec![pierce_feature(2, shape, &at)]));
        watertight(&built);
        assert!(built.parts.notes.is_empty(), "{shape:?}: {:?}", built.parts.notes);
        assert_eq!((built.parts.cut, built.parts.joined), (1, 0));
        let c = part(&built, 2);
        let made = c.made.as_ref().unwrap();
        assert_eq!(csg::self_crossings(made.solid()), 0, "{shape:?}");
        assert_eq!(made.solid().open_edges(), (0, 0));
        let plan = plan_of(shape, &at.params);
        let wall = mean_wall(&bare.mesh, &bvh, &c, &plan);
        let chamfer = at.params["chamfer_mm"].as_f64().unwrap();
        let expected = area(&plan) * wall + 0.5 * perimeter(&plan) * chamfer * chamfer;
        let removed = bare.report.volume_mm3 - built.report.volume_mm3;
        assert!((removed / expected - 1.0).abs() < 0.03, "{shape:?}: removed {removed:.4} mm³ against {:.3} mm² × {wall:.3} mm + the bright cut = {expected:.4}", area(&plan));
        assert_eq!(pieces(&as_solid(&built.mesh)), 1, "{shape:?}");
        eprintln!("crown {shape:?}: {:.3} mm², wall {wall:.3} mm, removed {removed:.4} against {expected:.4} mm³ ({:+.2}%)", area(&plan), 100.0 * (removed / expected - 1.0));
    }
}

#[test]
fn a_blind_piercing_stops_at_its_depth_and_one_that_would_thin_the_wall_is_refused_by_name() {
    let band = court();
    let bare = build(&with(&band, vec![]));
    let bvh = Bvh::build(&bare.mesh);
    let mut at = pierce_at(&band, 90.0, 0.0, None, Shape::Oval).unwrap();
    at.params["through"] = json!(false);
    at.params["depth_mm"] = json!(0.8);
    let built = build(&with(&band, vec![pierce_feature(2, Shape::Oval, &at)]));
    watertight(&built);
    let c = part(&built, 2);
    let plan = plan_of(Shape::Oval, &at.params);
    let down = scale(c.frame.z_axis, -1.0);
    // The floor is flat, 0.8 mm under the surface over the plan's centre.
    let entry = |q: [f64; 2]| run(&bare.mesh, &bvh, c.frame.point([q[0], q[1], 6.0]), down).map(|r| 6.0 - r.0).unwrap();
    let floor = entry([0.0, 0.0]) - 0.8;
    let depths: Vec<f64> = inside_grid(&plan).iter().map(|q| entry(*q) - floor).collect();
    let depth = depths.iter().sum::<f64>() / depths.len() as f64;
    let chamfer = at.params["chamfer_mm"].as_f64().unwrap();
    let expected = area(&plan) * depth + 0.5 * perimeter(&plan) * chamfer * chamfer;
    let removed = bare.report.volume_mm3 - built.report.volume_mm3;
    assert!((removed / expected - 1.0).abs() < 0.03, "removed {removed:.4} against {expected:.4}");
    assert!((depth - 0.8).abs() < 0.1, "a flat floor under a domed crown: {depth:.3} mm deep on average");
    eprintln!("blind oval: {:.3} mm² at {depth:.3} mm, removed {removed:.4} against {expected:.4} mm³", area(&plan));
    // Deeper, the floor would leave less than the wall over the finger, and deeper still it runs through.
    at.params["depth_mm"] = json!(1.75);
    let FeatureStatus::Failed(why) = status(&with(&band, vec![pierce_feature(2, Shape::Oval, &at)]), 2) else { panic!() };
    assert!(why.starts_with("Piercing would leave 0.2") && why.ends_with("under the 0.5 mm a wall keeps: make it a through cut, or shallower"), "{why}");
    at.params["depth_mm"] = json!(2.5);
    let d = with(&band, vec![pierce_feature(2, Shape::Oval, &at)]);
    assert_eq!(status(&d, 2), FeatureStatus::Failed("Piercing 2.5 mm deep runs through the metal here: make it a through cut, or shallower".into()));
}

#[test]
fn a_piercing_through_a_side_face_runs_along_the_pull_through_the_band_width() {
    let band = flat();
    let bore = Bore::of(&band, &cadkernel::brep::Placement::IDENTITY);
    let (lo, hi) = bore.z_extent(90.0).unwrap();
    let rs = bore.crossings(90.0, hi - 0.05);
    let r = 0.5 * (rs[0] + rs[rs.len() - 1]);
    let bare = build(&with(&band, vec![]));
    let bvh = Bvh::build(&bare.mesh);
    for shape in Shape::ALL {
        let at = pierce_at(&band, 90.0, hi, Some(([0.0, r, hi], [0.0, 0.0, 1.0])), shape).unwrap();
        assert!(at.side_face);
        assert_eq!(at.stage, Stage::Cast, "{shape:?}: a hole along the pull casts");
        let built = build(&with(&band, vec![pierce_feature(2, shape, &at)]));
        watertight(&built);
        assert!(built.parts.notes.is_empty(), "{shape:?}: {:?}", built.parts.notes);
        let c = part(&built, 2);
        assert!(c.frame.z_axis[2] > 1.0 - 1e-9, "{shape:?}: the cut runs along the finger: {:?}", c.frame.z_axis);
        assert_eq!(csg::self_crossings(c.made.as_ref().unwrap().solid()), 0);
        let plan = plan_of(shape, &at.params);
        let wall = mean_wall(&bare.mesh, &bvh, &c, &plan);
        assert!((wall - (hi - lo)).abs() < 0.1, "{shape:?}: the cut crosses the band's width: {wall:.3}");
        let chamfer = at.params["chamfer_mm"].as_f64().unwrap();
        let expected = area(&plan) * wall + 0.5 * perimeter(&plan) * chamfer * chamfer;
        let removed = bare.report.volume_mm3 - built.report.volume_mm3;
        assert!((removed / expected - 1.0).abs() < 0.03, "{shape:?}: removed {removed:.4} against {expected:.4}");
        // A heart's and a drop's point is toward the bore.
        if matches!(shape, Shape::Heart | Shape::Drop) {
            let (l, turn) = (at.params["length_mm"].as_f64().unwrap(), at.params["turn_deg"].as_f64().unwrap().to_radians());
            let q = turned([-0.5 * l, 0.0], turn);
            let tip = c.frame.point([q[0], q[1], 0.0]);
            assert!(tip[0].hypot(tip[1]) < c.frame.origin[0].hypot(c.frame.origin[1]) - 0.2, "{shape:?}: its point is toward the bore");
        }
        eprintln!("side {shape:?}: {:.3} mm², wall {wall:.3} mm, removed {removed:.4} against {expected:.4} mm³", area(&plan));
    }
    // The low face too, its cut running the other way.
    let at = pierce_at(&band, 90.0, lo, Some(([0.0, r, lo], [0.0, 0.0, -1.0])), Shape::Heart).unwrap();
    let built = build(&with(&band, vec![pierce_feature(2, Shape::Heart, &at)]));
    watertight(&built);
    assert!(part(&built, 2).frame.z_axis[2] < -1.0 + 1e-9);
}

#[test]
fn a_piercing_that_would_break_the_bands_edge_is_refused_by_name() {
    let band = court();
    assert_eq!(pierce_at(&band, 90.0, 1.7, None, Shape::Round).unwrap_err().to_string(), "Too near the band's edge to pierce here: 0.30 mm from it");
    let mut at = pierce_at(&band, 90.0, 0.0, None, Shape::Round).unwrap();
    at.params["width_mm"] = json!(3.8);
    let d = with(&band, vec![pierce_feature(2, Shape::Round, &at)]);
    assert_eq!(status(&d, 2), FeatureStatus::Failed("Piercing would break through the band's edge: keep it 0.2 mm inside the band, or make it smaller".into()));
    // The Court band's side face is too narrow to take one.
    let bore = Bore::of(&band, &cadkernel::brep::Placement::IDENTITY);
    let (_, hi) = bore.z_extent(90.0).unwrap();
    let e = pierce_at(&band, 90.0, hi, Some(([0.0, 9.6, hi], [0.0, 0.0, 1.0])), Shape::Round).unwrap_err().to_string();
    assert!(e.starts_with("The side face here is") && e.ends_with("mm across, too narrow to pierce"), "{e}");
}

/// A flat cigar band 10 mm wide and 3 mm thick.
fn cigar() -> RingDesign {
    let mut d = flat();
    d.profile.width_mm = 10.0;
    d.profile.thickness_mm = 3.0;
    d
}
/// `band` with stone `gem` #2 on its top set in setting `key` — its head #3, then its seat, Bench under sand — then `more`.
fn solitaire(band: &RingDesign, gem: Gem, key: &str, more: Vec<Feature>) -> RingDesign {
    let mut parts = vec![builders::stone_feature(2, gem, Placement::ring(90.0, builders::stand_off_mm(key, gem)))];
    let mut next = 2;
    parts.extend(
        builders::setting_features(key, 2, gem, true, &mut || {
            next += 1;
            next
        })
        .unwrap(),
    );
    parts.extend(more);
    with(band, parts)
}
fn round(w: f64) -> Gem {
    Gem::calibrated(GemCut::Round, w)
}
/// Plan distance from `p` to the nearest vertex of `solid` whose height in the frame lies between `z0` and `z1`.
fn plan_distance(solid: &Solid, p: [f64; 2], z0: f64, z1: f64) -> f64 {
    solid.v.iter().filter(|v| v[2] >= z0 && v[2] <= z1).map(|v| (v[0] - p[0]).hypot(v[1] - p[1])).fold(f64::INFINITY, f64::min)
}

#[test]
fn azures_open_under_a_stone_between_its_seat_and_its_claws_and_keep_the_sands_edge_from_both() {
    let band = cigar();
    let gem = round(10.0);
    let bare = build(&solitaire(&band, gem, "claw4", vec![]));
    for count in [4u32, 6, 8] {
        let d = solitaire(&band, gem, "claw4", vec![azure_feature(10, 2, Some(3), count, Stage::Cast)]);
        let built = build(&d);
        watertight(&built);
        assert!(built.parts.notes.is_empty(), "{count}: {:?}", built.parts.notes);
        let c = part(&built, 10);
        let made = c.made.as_ref().unwrap();
        assert_eq!(made.count("Window "), count as usize);
        assert_eq!(csg::self_crossings(made.solid()), 0);
        assert_eq!(made.solid().open_edges(), (0, 0));
        // In the stone's frame: every window keeps the sand's least edge from the claws, the rails, the seat's pilot and its neighbours.
        let head = part(&built, 3);
        let local_of = |p: P3| local(&c.frame, p);
        let head_local = Solid { v: head.made.as_ref().unwrap().solid().v.iter().map(|p| local_of(*p)).collect(), f: Vec::new() };
        let seat = part(&built, 4);
        let seat_named = &seat.made.as_ref().unwrap().named;
        let pilot = seat_named.names.iter().position(|n| n == "Pilot").unwrap() as u32;
        let seat_local = Solid {
            v: seat_named.solid.f.iter().zip(&seat_named.patch).filter(|(_, p)| **p == pilot).flat_map(|(f, _)| f.map(|i| local_of(seat_named.solid.v[i as usize]))).collect(),
            f: Vec::new(),
        };
        let windows: Vec<Vec<[f64; 2]>> = (0..count as usize)
            .map(|k| {
                let name = made.named.names.iter().position(|n| *n == format!("Window {}", k + 1)).unwrap() as u32;
                let mut pts: Vec<[f64; 2]> = made.named.solid.f.iter().zip(&made.named.patch).filter(|(_, p)| **p == name).flat_map(|(f, _)| f.map(|i| made.named.solid.v[i as usize])).map(|p| local_of(p)).map(|p| [p[0], p[1]]).collect();
                pts.dedup();
                pts
            })
            .collect();
        let (lo, hi) = made.solid().v.iter().map(|p| local_of(*p)[2]).fold((f64::MAX, f64::MIN), |(a, b), z| (a.min(z), b.max(z)));
        for (k, w) in windows.iter().enumerate() {
            let to_head = w.iter().map(|p| plan_distance(&head_local, *p, lo, hi)).fold(f64::INFINITY, f64::min);
            let to_seat = w.iter().map(|p| plan_distance(&seat_local, *p, lo, hi)).fold(f64::INFINITY, f64::min);
            assert!(to_head >= MIN_EDGE_MM - 0.03 && to_seat >= MIN_EDGE_MM - 0.03, "{count}: window {} stands {to_head:.3} from the head and {to_seat:.3} from the seat", k + 1);
            if k == 0 {
                eprintln!("{count} azures under a 10 mm round: {:.3} mm from the head, {to_seat:.3} from the seat", to_head);
            }
        }
        // Each opens into the finger: a line down its centre from over the band meets metal nowhere once it is in.
        let removed = bare.report.volume_mm3 - built.report.volume_mm3;
        assert!(removed > 0.1 * count as f64, "{count}: removed {removed:.3} mm³");
        let bvh = Bvh::build(&built.mesh);
        for s in &made.stations {
            let from = c.frame.point([s[0], s[1], s[2] + 1.0]);
            let hit = bvh.ray(&built.mesh, from, scale(c.frame.z_axis, -1.0)).map(|h| h.1);
            let inner = d.inner_radius_mm();
            let reach = from[0].hypot(from[1]) - inner;
            assert!(hit.is_none_or(|t| t > reach + 0.5), "{count}: a window's line meets metal {hit:?} mm down, before the bore {reach:.2} mm down");
        }
        eprintln!("{count} azures: removed {removed:.3} mm³, the ring {} faces", built.mesh.faces.len());
    }
}

/// The azures feature #10 under stone #2 held clear of head #3: `count` windows, `size` across at `radius`, turned `turn` degrees.
fn azures_at(count: u32, size: f64, radius: f64, turn: f64) -> Feature {
    let mut f = azure_feature(10, 2, Some(3), count, Stage::Cast);
    let p = params_of(&mut f);
    p["size_mm"] = json!(size);
    p["radius_mm"] = json!(radius);
    p["turn_deg"] = json!(turn);
    f
}
fn refusal(d: &RingDesign) -> String {
    match status(d, 10) {
        FeatureStatus::Failed(why) => why,
        other => panic!("{other:?}"),
    }
}

#[test]
fn azures_that_would_cut_a_claw_or_leave_the_sand_too_little_metal_are_refused_by_name() {
    let (band, gem) = (court(), round(6.5));
    // The Court band's 6.5 mm solitaire has room for six between its pilot and its claws, sized to fit.
    let fits = build(&solitaire(&band, gem, "claw4", vec![azure_feature(10, 2, Some(3), 6, Stage::Cast)]));
    watertight(&fits);
    assert!(fits.parts.notes.is_empty(), "{:?}", fits.parts.notes);
    assert_eq!(part(&fits, 10).made.as_ref().unwrap().count("Window "), 6);
    // Set on a claw's line at its foot, a window cuts into it.
    let why = refusal(&solitaire(&band, gem, "claw4", vec![azures_at(4, 0.8, 3.0, 45.0)]));
    assert_eq!(why, "Azures: 4 windows under Round 6.5 mm: window 1 would cut into Claw 1 of Four-claw head");
    // Far too big for their ring they run into each other: twelve under the cigar band's 10 mm stone, held clear of its pilot alone.
    let mut crowded = azure_feature(10, 2, None, 12, Stage::Cast);
    let p = params_of(&mut crowded);
    (p["shape"], p["size_mm"], p["radius_mm"]) = (json!("Round"), json!(1.4), json!(2.6));
    let why = refusal(&solitaire(&cigar(), round(10.0), "claw4", vec![crowded]));
    assert_eq!(why, "Azures: 12 windows under Round 10 mm: neighbouring windows would leave 0.00 mm of metal between them, under the 0.2 mm the sand fills");
    // Hard by the seat's pilot, and past the band's own edge.
    let rounds = |count, size, radius| {
        let mut f = azures_at(count, size, radius, 0.0);
        params_of(&mut f)["shape"] = json!("Round");
        f
    };
    let why = refusal(&solitaire(&band, gem, "claw4", vec![rounds(4, 0.4, 1.35)]));
    assert_eq!(why, "Azures: 4 windows under Round 6.5 mm: window 1 would leave under 0.2 mm of metal to the seat's pilot");
    let why = refusal(&solitaire(&band, gem, "claw4", vec![rounds(4, 0.6, 1.8)]));
    assert_eq!(why, "Azures: 4 windows under Round 6.5 mm: window 2 would break through the band's edge");
    // A teardrop's point reaches in past its round end: at the same radius it comes too near the pilot.
    let why = refusal(&solitaire(&band, gem, "claw4", vec![azures_at(4, 0.6, 1.8, 0.0)]));
    assert_eq!(why, "Azures: 4 windows under Round 6.5 mm: window 1 would leave under 0.2 mm of metal to the seat's pilot");
    // A stone too small for any: named with what stands in the way.
    let why = refusal(&solitaire(&band, round(3.0), "claw4", vec![azure_feature(10, 2, Some(3), 8, Stage::Cast)]));
    assert!(why.starts_with("Azures: no room for 8 windows of 0.3 mm or more under Round 3 mm — "), "{why}");
    eprintln!("{why}");
    // A head that is gone, or is not a head, is named.
    let mut stray = azure_feature(10, 2, Some(9), 6, Stage::Cast);
    assert_eq!(refusal(&solitaire(&band, gem, "claw4", vec![stray.clone()])), "Azures: head #9 is not in the document");
    params_of(&mut stray)[HEAD] = json!(4);
    assert_eq!(refusal(&solitaire(&band, gem, "claw4", vec![stray])), "Azures: #4 Seat bur is a seat bur, not a head");
}

fn params_of(f: &mut Feature) -> &mut Json {
    let Operation::Builder { params, .. } = &mut f.operation else { unreachable!() };
    params
}

#[test]
fn cathedral_shoulders_rise_from_the_band_to_the_gallery_rail_and_join_both() {
    let band = court();
    let gem = round(6.5);
    let bare = build(&solitaire(&band, gem, "claw4", vec![]));
    let d = solitaire(&band, gem, "claw4", vec![cathedral_feature(10, 2, 3, Stage::Cast)]);
    let built = build(&d);
    watertight(&built);
    assert!(built.parts.notes.is_empty(), "{:?}", built.parts.notes);
    assert_eq!((built.parts.joined, built.parts.cut), (2, 1));
    let c = part(&built, 10);
    let made = c.made.as_ref().unwrap();
    assert_eq!(made.named.names, ["Arch 1", "Arch 2"]);
    assert_eq!(csg::self_crossings(made.solid()), 0);
    assert_eq!(made.solid().open_edges(), (0, 0));
    assert_eq!(pieces(made.solid()), 2, "two arches, one either side");
    // Each arch reaches into the band at its foot and into the head's gallery rail at its top.
    let head = part(&built, 3);
    let head_made = head.made.as_ref().unwrap();
    let rail = head_made.named.names.iter().position(|n| n == "Gallery rail").unwrap() as u32;
    let rail_solid = Solid {
        v: head_made.named.solid.v.clone(),
        f: head_made.named.solid.f.iter().zip(&head_made.named.patch).filter(|(_, p)| **p == rail).map(|(f, _)| *f).collect(),
    };
    let band_solid = as_solid(built.band.as_deref().unwrap());
    let joined_band = csg::combine(made.solid(), &band_solid, csg::Op::Intersect).unwrap().volume();
    assert!(joined_band > 0.05, "the feet sink into the band: {joined_band:.4} mm³");
    let head_solid = head_made.solid();
    let joined_head = csg::combine(made.solid(), head_solid, csg::Op::Intersect).unwrap().volume();
    assert!(joined_head > 0.02, "the tops sink into the head: {joined_head:.4} mm³");
    // The tops are at the gallery rail: each arch's highest point is within a wire of the rail's lowest.
    let up = c.frame.z_axis;
    let height = |p: &P3| dot(sub(*p, c.frame.origin), up);
    let rail_low = rail_solid.f.iter().flatten().map(|i| height(&rail_solid.v[*i as usize])).fold(f64::INFINITY, f64::min);
    let top = made.solid().v.iter().map(height).fold(f64::NEG_INFINITY, f64::max);
    let wire = builders::cutters::arch_wire_mm(gem);
    assert!(top > rail_low && top < rail_low + 2.0 * wire, "the arches' tops at {top:.3} against the rail's underside at {rail_low:.3}");
    // One piece of metal: band, head and shoulders.
    assert_eq!(pieces(&as_solid(&built.mesh)), 1);
    let added = built.report.volume_mm3 - bare.report.volume_mm3;
    eprintln!("cathedral shoulders on the Court band's 6.5 mm solitaire: {:.3} mm³ of wire, {added:.3} mm³ added, into the band {joined_band:.4}, into the head {joined_head:.4}", made.solid().volume());
    assert!(added > 0.25 * made.solid().volume() && added < made.solid().volume(), "{added:.3}");
    // The feet spread round the ring as asked: each lands its spread from the stone.
    let feet: Vec<f64> = (0..2u32)
        .map(|k| {
            let pts: Vec<P3> = made.named.solid.f.iter().zip(&made.named.patch).filter(|(_, p)| **p == k).flat_map(|(f, _)| f.map(|i| made.named.solid.v[i as usize])).collect();
            let low = pts.iter().min_by(|a, b| a[0].hypot(a[1]).total_cmp(&b[0].hypot(b[1]))).unwrap();
            low[1].atan2(low[0]).to_degrees()
        })
        .collect();
    assert!(feet.iter().all(|t| ((t - 90.0).abs() - SPREAD_DEG).abs() < 3.0), "feet at {feet:?}");
    assert!(feet[0] > 90.0 && feet[1] < 90.0, "one either side: {feet:?}");
}

#[test]
fn cathedral_shoulders_fail_by_name_without_a_head_or_where_they_would_dive_into_the_band() {
    let gem = round(6.5);
    let band = court();
    let mut lone = cathedral_feature(10, 2, 3, Stage::Cast);
    params_of(&mut lone).as_object_mut().unwrap().remove(HEAD);
    let d = with(&band, vec![builders::stone_feature(2, gem, Placement::ring(90.0, builders::stand_off_mm("claw4", gem))), lone]);
    assert_eq!(status(&d, 10), FeatureStatus::Failed("Cathedral shoulders rise to a head's gallery rail and name no head: set Round 6.5 mm in claws, a basket or a bezel first".into()));
    // Shallow and far, the arches run back into the band on their way up.
    let mut low = cathedral_feature(10, 2, 3, Stage::Cast);
    params_of(&mut low)["spread_deg"] = json!(75.0);
    params_of(&mut low)["rise"] = json!(0.15);
    let FeatureStatus::Failed(why) = status(&solitaire(&band, gem, "claw4", vec![low]), 10) else { panic!() };
    assert!(why.starts_with("Cathedral shoulders: Arch 1 would run back into the band"), "{why}");
    // A bezel is met at its wall, and a basket at its highest rail.
    for key in ["bezel", "basket"] {
        let d = solitaire(&band, gem, key, vec![cathedral_feature(10, 2, 3, Stage::Cast)]);
        let built = build(&d);
        watertight(&built);
        assert!(built.parts.notes.is_empty(), "{key}: {:?}", built.parts.notes);
        assert_eq!(pieces(&as_solid(&built.mesh)), 1, "{key}");
    }
}

/// A part's verdict read with the ring as built, under sand.
fn verdict(d: &RingDesign, id: Id) -> PartVerdict {
    let built = build(d);
    assert!(built.parts.notes.is_empty(), "{:?}", built.parts.notes);
    let f = judged_field_report(d, &AlphaLibrary::builtin(), &d.draft, 192, 128, Some(&built));
    f.parts.into_iter().find(|p| p.feature == id).unwrap_or_else(|| panic!("#{id} judged"))
}

#[test]
fn under_sand_a_hole_along_the_pull_casts_one_across_it_locks_and_shoulders_on_the_parting_line_cast() {
    let (court, flat) = (court(), flat());
    let locking = |p: &PartVerdict| p.undercut_area_mm2 - p.silhouette_mm2;
    let row = |what: &str, p: &PartVerdict| eprintln!("{what:<40} {:>8.3} mm² undercut ({:.3} chord), {:>8.3} vertical, {:>8.3} total, worst {:>6.1}°", p.undercut_area_mm2, p.silhouette_mm2, p.vertical_area_mm2, p.total_area_mm2, p.worst_draft_deg);
    // A round piercing staged Cast through a side face, then through the crown.
    let bore = Bore::of(&flat, &cadkernel::brep::Placement::IDENTITY);
    let (_, hi) = bore.z_extent(90.0).unwrap();
    let rs = bore.crossings(90.0, hi - 0.05);
    let r = 0.5 * (rs[0] + rs[rs.len() - 1]);
    let mut side = pierce_at(&flat, 90.0, hi, Some(([0.0, r, hi], [0.0, 0.0, 1.0])), Shape::Round).unwrap();
    side.stage = Stage::Cast;
    let side = verdict(&with(&flat, vec![pierce_feature(2, Shape::Round, &side)]), 2);
    row("round piercing through a side face", &side);
    assert!(side.judged && locking(&side) < PART_NOISE_MM2, "{side:?}");
    let mut crown = pierce_at(&court, 90.0, 0.0, None, Shape::Round).unwrap();
    crown.stage = Stage::Cast;
    let crown = verdict(&with(&court, vec![pierce_feature(2, Shape::Round, &crown)]), 2);
    row("round piercing through the crown", &crown);
    assert!(locking(&crown) > 2.0 && crown.worst_draft_deg < -30.0, "{crown:?}");
    assert!(crown.note.contains("drill it at the bench"), "{}", crown.note);
    // Azures under a crown stone run across the pull too.
    let big = round(10.0);
    let azures = verdict(&solitaire(&cigar(), big, "claw4", vec![azure_feature(10, 2, Some(3), 6, Stage::Cast)]), 10);
    row("six azures under a 10 mm round", &azures);
    assert!(locking(&azures) > 1.0, "{azures:?}");
    // Cathedral shoulders on the parting line: every section of the wire a drop both ways from its crest.
    let gem = round(6.5);
    let arches = verdict(&solitaire(&court, gem, "claw4", vec![cathedral_feature(10, 2, 3, Stage::Cast)]), 10);
    row("cathedral shoulders on the parting line", &arches);
    assert!(arches.judged && locking(&arches) < PART_NOISE_MM2, "{arches:?}");
    // Off it, on a court wide enough to seat the stone there, the wire's underside faces back across the plane.
    let mut wide = court.clone();
    (wide.profile.width_mm, wide.profile.thickness_mm) = (6.0, 2.5);
    let on = verdict(&solitaire(&wide, gem, "claw4", vec![cathedral_feature(10, 2, 3, Stage::Cast)]), 10);
    row("the same on a 6 x 2.5 mm court", &on);
    assert!(locking(&on) < PART_NOISE_MM2, "{on:?}");
    let mut off = solitaire(&wide, gem, "claw4", vec![cathedral_feature(10, 2, 3, Stage::Cast)]);
    if let Some(Placement::Ring { across_mm, .. }) = off.cad.as_mut().unwrap().features.iter_mut().find(|f| f.id == 2).map(|f| &mut f.component.placement) {
        *across_mm = 0.8;
    }
    let off = verdict(&off, 10);
    row("cathedral shoulders 0.8 mm off it", &off);
    assert!(locking(&off) > 0.05, "{off:?}");
    // The stage each gesture takes follows: a cut along the pull and shoulders on the line cast, the rest go to the bench.
    assert_eq!(cut_stage([0.0, 0.0, 1.0], true), Stage::Cast);
    assert_eq!(cut_stage([0.0, 1.0, 0.0], true), Stage::Bench);
    assert_eq!(cut_stage([0.0, 1.0, 0.0], false), Stage::Cast);
    assert_eq!((shoulder_stage(true), shoulder_stage(false)), (Stage::Bench, Stage::Cast));
}

#[test]
fn at_the_stages_they_default_to_the_ring_pours_clean_and_the_pattern_marks_where_the_bench_cuts() {
    let (band, gem) = (court(), round(6.5));
    let pierce = pierce_feature(11, Shape::Heart, &pierce_at(&band, 30.0, 0.0, None, Shape::Heart).unwrap());
    // The stone stands on the top of the ring across the band's mid-plane, where the parting line runs: its shoulders pour.
    let shoulders = shoulder_stage_at(0.0, 0.0, true);
    assert_eq!(shoulders, Stage::Cast);
    let d = solitaire(&band, gem, "claw4", vec![azure_feature(10, 2, Some(3), 6, cut_stage([1.0, 0.0, 0.0], true)), cathedral_feature(12, 2, 3, shoulders), pierce]);
    let built = build(&d);
    watertight(&built);
    assert!(built.parts.notes.is_empty(), "{:?}", built.parts.notes);
    let f = judged_field_report(&d, &AlphaLibrary::builtin(), &d.draft, 192, 128, Some(&built));
    assert_eq!(f.verdict, crate::castability::Verdict::Castable, "{:?}", f.notes);
    for (id, what) in [(10, "Azures \"6 azures\" (#10) is drilled at the bench"), (11, "Piercing \"Heart piercing\" (#11) is drilled at the bench")] {
        assert!(!f.parts.iter().find(|p| p.feature == id).unwrap().judged, "#{id} is left to the bench");
        assert!(f.notes.iter().any(|n| n.starts_with(what)), "{what}: {:?}", f.notes);
    }
    let arches = f.parts.iter().find(|p| p.feature == 12).unwrap();
    assert!(arches.judged && arches.undercut_area_mm2 - arches.silhouette_mm2 < PART_NOISE_MM2, "{arches:?}");
    // The sand pattern pours the band and the shoulders: the head, its seat, the windows and the piercing come after.
    let pattern = crate::mesh::try_build_pattern(&d, &AlphaLibrary::builtin(), params()).unwrap();
    assert!(pattern.parts.notes.is_empty(), "{:?}", pattern.parts.notes);
    assert_eq!((pattern.parts.joined, pattern.parts.cut, pattern.parts.features.clone()), (1, 0, vec![12]));
    watertight(&pattern);
    eprintln!("{}", f.notes.join("\n"));
}

#[test]
fn cathedral_shoulders_read_the_head_they_meet_and_follow_it_when_it_changes() {
    let (band, gem) = (court(), round(6.5));
    let d = solitaire(&band, gem, "claw4", vec![cathedral_feature(10, 2, 3, Stage::Cast)]);
    let surface = build(&with(&band, vec![])).mesh;
    let never = std::sync::atomic::AtomicBool::new(false);
    let top = |d: &RingDesign| {
        let e = cad::evaluate_with(d, &AlphaLibrary::builtin(), params(), &cad::BuildCtx::new(&never).with_surface(&surface)).unwrap();
        let c = e.components.iter().find(|c| c.id == 10).unwrap_or_else(|| panic!("{:?}", e.status_of(10)));
        c.made.as_ref().unwrap().solid().v.iter().map(|p| dot(sub(*p, c.frame.origin), c.frame.z_axis)).fold(f64::NEG_INFINITY, f64::max)
    };
    let claws = top(&d);
    // The same stone in a basket: its highest gallery rail stands at 0.35 of the pavilion under the girdle, not 0.45 of the head's depth.
    let mut basket = d.clone();
    basket.cad.as_mut().unwrap().apply(&cad::edit::CadEdit::Operation { id: 3, operation: Operation::Builder { key: builders::BASKET.into(), on: Some(2), params: json!({}) } }).unwrap();
    let (p, g) = (gem.pavilion_mm(), setting::girdle_half_mm(gem));
    let rise = (-g - 0.35 * p) - (-0.45 * (p + 0.2));
    let moved = top(&basket) - claws;
    assert!((moved - rise).abs() < 0.03, "the arches' tops rose {moved:.4} mm with the rail's {rise:.4}");
    // The head is a source: suppressed, the arches are skipped with it and say which.
    let mut off = d.clone();
    off.cad.as_mut().unwrap().apply(&cad::edit::CadEdit::Enable { id: 3, enabled: false }).unwrap();
    assert_eq!(status(&off, 10), FeatureStatus::Skipped("source #3 Four-claw head was suppressed".into()));
    assert!(d.cad.as_ref().unwrap().features.iter().find(|f| f.id == 10).is_some_and(|f| f.operation.sources().contains(&3)));
}

/// Where the sand pattern seats a stone against the finished ring: `cargo test -p ringdesign-core pattern_stone_probe -- --ignored --nocapture`.
#[test]
#[ignore = "probe"]
fn pattern_stone_probe() {
    let (band, gem) = (court(), round(6.5));
    for (label, more) in [("solitaire", vec![]), ("with shoulders", vec![cathedral_feature(10, 2, 3, Stage::Cast)]), ("with azures and shoulders", vec![azure_feature(11, 2, Some(3), 6, Stage::Bench), cathedral_feature(10, 2, 3, Stage::Cast)])] {
        let d = solitaire(&band, gem, "claw4", more);
        let finished = build(&d);
        let pattern = crate::mesh::try_build_pattern(&d, &AlphaLibrary::builtin(), params()).unwrap();
        let frame = |b: &BuildResult| b.parts.evaluated.as_ref().and_then(|e| e.components.iter().find(|c| c.id == 2)).map(|c| c.frame);
        let (a, b) = (frame(&finished).unwrap(), frame(&pattern).unwrap());
        let lift = dot(sub(b.origin, a.origin), a.z_axis);
        let tilt = dot(a.z_axis, b.z_axis).clamp(-1.0, 1.0).acos().to_degrees();
        eprintln!("{label:<28} the stone sits {lift:+.3} mm and {tilt:.2}° off in the pattern; pattern notes {:?}", pattern.parts.notes);
    }
}

/// Cast shoulders under a bench head, their stone `off_mm` off the field's parting plane: what they lock poured and on the finished ring, the pattern's verdict, and the parts it joins.
fn shoulders_locking(off_mm: f64) -> (f64, f64, crate::castability::Verdict, usize) {
    let gem = round(6.5);
    let mut wide = court();
    (wide.profile.width_mm, wide.profile.thickness_mm) = (6.0, 2.5);
    let lib = AlphaLibrary::builtin();
    let parting = crate::castability::attributed_field_report(&wide, &lib, &wide.draft, 192, 128).parting_z_mm;
    let mut d = solitaire(&wide, gem, "claw4", vec![cathedral_feature(10, 2, 3, Stage::Cast)]);
    if let Some(Placement::Ring { across_mm, .. }) = d.cad.as_mut().unwrap().features.iter_mut().find(|f| f.id == 2).map(|f| &mut f.component.placement) {
        *across_mm = parting + off_mm;
    }
    let pattern = crate::mesh::try_build_pattern(&d, &lib, params()).unwrap();
    assert!(pattern.parts.notes.is_empty(), "{off_mm}: {:?}", pattern.parts.notes);
    let read = |b: &BuildResult| {
        let f = judged_field_report(&d, &lib, &d.draft, 192, 128, Some(b));
        let p = f.parts.iter().find(|p| p.feature == 10).unwrap_or_else(|| panic!("{off_mm}: the shoulders are judged"));
        assert!(p.judged, "{off_mm}: {p:?}");
        (p.undercut_area_mm2 - p.silhouette_mm2, f.verdict)
    };
    let ((poured, verdict), (finished, _)) = (read(&pattern), read(&build(&d)));
    (poured, finished, verdict, pattern.parts.joined)
}

#[test]
fn cast_shoulders_pour_clean_only_on_the_parting_line_and_default_to_the_bench_off_it() {
    use crate::castability::Verdict;
    // Poured against finished, mm² locking: 0.000 mm off 0.0001 / 0.0001, 0.004 mm 0.0011 / 0.0014, 0.1 mm 1.40 / 1.03, 0.8 mm 9.84 / 8.04.
    for (off, clean) in [(0.0, true), (0.004, true), (0.1, false), (0.8, false)] {
        let (poured, finished, verdict, joined) = shoulders_locking(off);
        eprintln!("shoulders {off:.3} mm off the parting plane: {poured:.4} mm² poured, {finished:.4} on the finished ring, {verdict:?}");
        assert_eq!(joined, 1, "{off}: the shoulders pour with the band, their head soldered on after");
        let stage = shoulder_stage_at(off, 0.0, true);
        if clean {
            assert!(poured < PART_NOISE_MM2 && finished < PART_NOISE_MM2 && verdict == Verdict::Castable, "{off}: {poured} {finished} {verdict:?}");
            assert_eq!(stage, Stage::Cast, "{off}");
        } else {
            assert!(poured > 1.0 && finished > 1.0 && verdict != Verdict::Castable, "{off}: {poured} {finished} {verdict:?}");
            assert_eq!(stage, Stage::Bench, "{off}");
        }
    }
    // Just past the tolerance it is off the line; lost wax casts them anywhere; an unknown seat is the bench's.
    assert_eq!((shoulder_stage_at(SHOULDER_PARTING_MM + 1e-4, 0.0, true), shoulder_stage_at(-0.3, -0.3 + 1e-4, true)), (Stage::Bench, Stage::Cast));
    assert_eq!((shoulder_stage_at(0.8, 0.0, false), shoulder_stage_at(f64::NAN, 0.0, true)), (Stage::Cast, Stage::Bench));
    assert_eq!((shoulder_stage(true), shoulder_stage(false)), (Stage::Bench, Stage::Cast));
    // Read off a design as built: the Court band's stone on its crest casts them, one not yet built leaves them to the bench.
    let mut d = solitaire(&court(), round(6.5), "claw4", vec![]);
    let stone = part(&build(&d), 2).frame.origin;
    assert!(stone[2].abs() < 1e-3, "{stone:?}");
    assert_eq!((shoulder_stage_for(&d, Some(stone)), shoulder_stage_for(&d, None)), (Stage::Cast, Stage::Bench));
    // A parting plane the draft names 0.4 mm up the finger leaves the same stone off it; lost wax casts them wherever it stands.
    (d.draft.auto_parting, d.draft.parting_z_mm) = (false, 0.4);
    assert_eq!(shoulder_stage_for(&d, Some(stone)), Stage::Bench);
    d.draft.process = crate::castability::CastProcess::LostWax;
    assert_eq!(shoulder_stage_for(&d, Some(stone)), Stage::Cast);
}

/// Build costs for the report: `cargo test -p ringdesign-core measured_cutters -- --ignored --nocapture`.
#[test]
#[ignore = "timings only"]
fn measured_cutters() {
    let (band, gem) = (court(), round(6.5));
    let designs = [
        ("round piercing, crown", with(&band, vec![pierce_feature(2, Shape::Round, &pierce_at(&band, 90.0, 0.0, None, Shape::Round).unwrap())])),
        ("heart piercing, crown", with(&band, vec![pierce_feature(2, Shape::Heart, &pierce_at(&band, 90.0, 0.0, None, Shape::Heart).unwrap())])),
        ("solitaire", solitaire(&band, gem, "claw4", vec![])),
        ("solitaire, 6 azures", solitaire(&band, gem, "claw4", vec![azure_feature(10, 2, Some(3), 6, Stage::Bench)])),
        ("solitaire, cathedral", solitaire(&band, gem, "claw4", vec![cathedral_feature(10, 2, 3, Stage::Cast)])),
    ];
    for (label, p) in [("preview 256x128", params()), ("export 1024x384", BuildParams { theta_steps: 1024, profile_steps: 384, ..BuildParams::default() })] {
        for (name, d) in &designs {
            let best = (0..3)
                .map(|_| {
                    let started = std::time::Instant::now();
                    let b = crate::mesh::try_build(d, &AlphaLibrary::builtin(), p).unwrap();
                    (started.elapsed().as_secs_f64() * 1e3, b)
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .unwrap();
            let v = &best.1.report.validation;
            eprintln!("{label:<16} {name:<24} {:>7.1} ms, parts {:>4} ms, {} faces, open {} non-manifold {}", best.0, best.1.parts.ms, best.1.mesh.faces.len(), v.boundary_edges, v.non_manifold_edges);
        }
    }
}
