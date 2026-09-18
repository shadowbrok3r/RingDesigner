//! Picking uses the same undisplaced surface coordinates as the mesh and layers.
use crate::{
    RingDesign,
    alpha::AlphaLibrary,
    field::{Layer, LayerEntry, LayerStack, Uv},
    mesh::Mesh,
};

#[derive(Clone, Debug)]
pub struct Hit {
    pub ray: ([f32; 3], [f32; 3]),
    pub world: [f32; 3],
    pub face: usize,
    pub theta_deg: f64,
    pub v_mm: f64,
    pub radial_wall_mm: f64,
    pub relief_mm: f64,
}

pub fn hit(
    d: &RingDesign,
    lib: &AlphaLibrary,
    mesh: &Mesh,
    origin: [f32; 3],
    direction: [f32; 3],
) -> Option<Hit> {
    let (face, world) = raycast(mesh, origin, direction)?;
    let theta_deg = (world[1] as f64)
        .atan2(world[0] as f64)
        .to_degrees()
        .rem_euclid(360.0);
    let radius = (world[0] as f64).hypot(world[1] as f64);
    let reference = d.reference_loop();
    let ctx = d.field_context();
    let base = d.section_at(theta_deg, 160, None, Some(&reference));
    let section = crate::castability::section_at_spaced(d, lib, theta_deg, 160, None);
    let mut best = f64::INFINITY;
    let mut v_mm = ctx.crest_v_mm;
    if base.pts.len() == section.points.len() {
        for i in 0..section.points.len().saturating_sub(1) {
            let a = &section.points[i];
            let b = &section.points[i + 1];
            if !a.surface || !b.surface {
                continue;
            }
            let delta = [b.r - a.r, b.z - a.z];
            let denom = delta[0] * delta[0] + delta[1] * delta[1];
            let t = (((radius - a.r) * delta[0] + (world[2] as f64 - a.z) * delta[1])
                / denom.max(1e-12))
            .clamp(0.0, 1.0);
            let distance = (radius - a.r - t * delta[0]).powi(2)
                + (world[2] as f64 - a.z - t * delta[1]).powi(2);
            if distance < best {
                best = distance;
                v_mm = (base.pts[i].v_mm * (1.0 - t) + base.pts[i + 1].v_mm * t)
                    / base.surface_len_mm.max(1e-9)
                    * ctx.band_v_len_mm;
            }
        }
    }
    let uv = Uv {
        u: ctx.u_of_theta(theta_deg),
        v: v_mm,
    };
    Some(Hit {
        ray: (origin, direction),
        world,
        face,
        theta_deg,
        v_mm,
        radial_wall_mm: radius - d.inner_radius_mm(),
        relief_mm: d.layers.height(uv, &ctx, lib),
    })
}

pub fn layers_at(d: &RingDesign, lib: &AlphaLibrary, hit: &Hit) -> Vec<usize> {
    if hit.radial_wall_mm < 0.035 {
        return Vec::new();
    }
    let ctx = d.field_context();
    let uv = Uv {
        u: ctx.u_of_theta(hit.theta_deg),
        v: hit.v_mm,
    };
    d.layers
        .layers
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(i, entry)| {
            (entry.enabled
                && entry.opacity > 0.001
                && entry.window.mask(uv, &ctx) > 0.01
                && entry.layer.height(uv, &ctx, lib).abs() * entry.opacity > 0.003)
                .then_some(i)
        })
        .collect()
}

/// Stable index paths, aligned with core's stone census, including nested groups
/// and repeated seats. Names are not identities: duplicated names remain editable.
pub fn stone_paths(d: &RingDesign) -> Vec<Vec<usize>> {
    fn walk(
        stack: &LayerStack,
        ctx: &crate::field::FieldContext,
        prefix: &[usize],
        out: &mut Vec<Vec<usize>>,
    ) {
        for (i, entry) in stack.layers.iter().enumerate() {
            if !entry.enabled {
                continue;
            }
            let mut path = prefix.to_vec();
            path.push(i);
            match &entry.layer {
                Layer::SeatPad(seat)
                    if seat.gem.is_some()
                        && crate::setstone::kept(entry, ctx, seat.theta_deg, seat.v_mm) =>
                {
                    out.push(path)
                }
                Layer::SeatRun(run) => {
                    let mut seat = run.seat;
                    seat.fit_stone(run.gem);
                    let seat = run.turned(seat);
                    for station in 0..run.count.clamp(1, 200) {
                        if crate::setstone::kept(
                            entry,
                            ctx,
                            run.theta_of_station(station as f64, ctx),
                            seat.v_mm,
                        ) {
                            out.push(path.clone());
                        }
                    }
                }
                Layer::Group(group) => walk(&group.stack, ctx, &path, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&d.layers, &d.field_context(), &[], &mut out);
    out
}

pub fn entry_mut<'a>(stack: &'a mut LayerStack, path: &[usize]) -> Option<&'a mut LayerEntry> {
    let (&index, rest) = path.split_first()?;
    let entry = stack.layers.get_mut(index)?;
    if rest.is_empty() {
        return Some(entry);
    }
    match &mut entry.layer {
        Layer::Group(group) => entry_mut(&mut group.stack, rest),
        _ => None,
    }
}

/// A generated ancestor must be made manual before editing its stored output.
pub fn live_ancestor(stack: &LayerStack, path: &[usize]) -> Option<Vec<usize>> {
    let mut stack = stack;
    for (depth, &index) in path.iter().enumerate() {
        let entry = stack.layers.get(index)?;
        let Layer::Group(group) = &entry.layer else {
            return None;
        };
        if group.recipe.is_some() {
            return Some(path[..=depth].to_vec());
        }
        stack = &group.stack;
    }
    None
}

pub fn raycast(mesh: &Mesh, origin: [f32; 3], direction: [f32; 3]) -> Option<(usize, [f32; 3])> {
    let o = origin.map(f64::from);
    let d = direction.map(f64::from);
    let mut best = f64::INFINITY;
    let mut index = None;
    for (i, face) in mesh.faces.iter().enumerate() {
        let Some((a, b, c)) = mesh.triangle(face) else {
            continue;
        };
        let e1 = sub(b, a);
        let e2 = sub(c, a);
        let p = cross(d, e2);
        let det = dot(e1, p);
        if det.abs() < 1e-12 {
            continue;
        }
        let t = sub(o, a);
        let u = dot(t, p) / det;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = cross(t, e1);
        let v = dot(d, q) / det;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let distance = dot(e2, q) / det;
        if distance > 1e-6 && distance < best {
            best = distance;
            index = Some(i);
        }
    }
    index.map(|i| {
        (
            i,
            std::array::from_fn(|axis| (o[axis] + d[axis] * best) as f32),
        )
    })
}

pub fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
pub fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stone_paths_match_census_and_edit_only_the_picked_duplicate() {
        use crate::field::{GroupLayer, SeatPadLayer, SeatRunLayer};
        let mut d = RingDesign::default();
        let mut seat = SeatPadLayer::default();
        seat.fit_stone(crate::gem::Gem::calibrated(crate::gem::GemCut::Round, 2.0));
        let mut group = GroupLayer::default();
        group
            .stack
            .layers
            .push(LayerEntry::new("same", Layer::SeatPad(seat)));
        group
            .stack
            .layers
            .push(LayerEntry::new("same", Layer::SeatPad(seat)));
        let mut run = SeatRunLayer::default();
        run.count = 3;
        group
            .stack
            .layers
            .push(LayerEntry::new("same", Layer::SeatRun(run)));
        let mut disabled = LayerEntry::new("same", Layer::SeatPad(seat));
        disabled.enabled = false;
        group.stack.layers.push(disabled);
        d.layers
            .layers
            .push(LayerEntry::new("group", Layer::Group(group)));
        let paths = stone_paths(&d);
        assert_eq!(paths.len(), crate::setstone::set_stones(&d).len());
        assert_eq!(
            paths,
            vec![vec![0, 0], vec![0, 1], vec![0, 2], vec![0, 2], vec![0, 2]]
        );
        entry_mut(&mut d.layers, &paths[1]).unwrap().name = "picked duplicate".into();
        assert_eq!(entry_mut(&mut d.layers, &paths[0]).unwrap().name, "same");
        assert_eq!(
            entry_mut(&mut d.layers, &paths[1]).unwrap().name,
            "picked duplicate"
        );
    }
    #[test]
    fn disabled_layers_do_not_intercept_a_surface_tap() {
        let mut d = RingDesign::default();
        let lib = AlphaLibrary::builtin();
        let ctx = d.field_context();
        let mut border = crate::field::BorderLayer::default();
        border.v_mm = ctx.crest_v_mm;
        d.layers
            .layers
            .push(LayerEntry::new("border", Layer::Border(border)));
        let hit = Hit {
            ray: ([0.0; 3], [0.0; 3]),
            world: [0.0; 3],
            face: 0,
            theta_deg: 90.0,
            v_mm: ctx.crest_v_mm,
            radial_wall_mm: 2.0,
            relief_mm: 0.3,
        };
        assert_eq!(layers_at(&d, &lib, &hit), vec![0]);
        d.layers.layers[0].enabled = false;
        assert!(layers_at(&d, &lib, &hit).is_empty());
    }
    #[test]
    fn ray_hits_the_front_wall_and_misses_the_finger_hole() {
        let d = RingDesign::default();
        let lib = AlphaLibrary::builtin();
        let mesh = crate::mesh::build(
            &d,
            &lib,
            crate::mesh::BuildParams {
                theta_steps: 96,
                profile_steps: 48,
                ..Default::default()
            },
        )
        .mesh;
        assert!(raycast(&mesh, [0.0, 0.0, 40.0], [0.0, 0.0, -1.0]).is_none());
        assert!(hit(&d, &lib, &mesh, [0.0, -40.0, 0.0], [0.0, 1.0, 0.0]).is_some());
    }
}
