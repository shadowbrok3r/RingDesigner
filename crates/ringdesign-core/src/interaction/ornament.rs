//! Address and edit an individual stamp without flattening its owning layers.
use super::picking;
use crate::{
    AlphaLibrary, RingDesign,
    field::{Decal, DecalLayer, Layer, LayerEntry, LayerStack, MAX_DECALS, Uv},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub path: Vec<usize>,
    pub instance: usize,
}

pub fn entry<'a>(stack: &'a LayerStack, path: &[usize]) -> Option<&'a LayerEntry> {
    let (&i, rest) = path.split_first()?;
    let e = stack.layers.get(i)?;
    if rest.is_empty() {
        return Some(e);
    }
    if let Layer::Group(g) = &e.layer {
        entry(&g.stack, rest)
    } else {
        None
    }
}
pub fn get<'a>(d: &'a RingDesign, target: &Target) -> Option<(&'a DecalLayer, &'a Decal)> {
    let Layer::Decals(layer) = &entry(&d.layers, &target.path)?.layer else {
        return None;
    };
    Some((layer, layer.decals.get(target.instance)?))
}
pub fn targets(d: &RingDesign) -> Vec<(Target, String)> {
    fn walk(stack: &LayerStack, path: &[usize], prefix: &str, out: &mut Vec<(Target, String)>) {
        for (i, e) in stack.layers.iter().enumerate() {
            if !e.enabled || e.opacity <= 0.001 {
                continue;
            }
            let mut path = path.to_vec();
            path.push(i);
            let label = if prefix.is_empty() {
                e.name.clone()
            } else {
                format!("{prefix} / {}", e.name)
            };
            match &e.layer {
                Layer::Group(g) if g.recipe.is_none() => walk(&g.stack, &path, &label, out),
                Layer::Decals(layer) => {
                    for j in 0..layer.decals.len().min(MAX_DECALS) {
                        out.push((
                            Target {
                                path: path.clone(),
                                instance: j,
                            },
                            format!("{label} · {}", j + 1),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = vec![];
    walk(&d.layers, &[], "", &mut out);
    out
}
/// Resolve visible relief, including ancestor windows and painted masks.
pub fn pick(d: &RingDesign, lib: &AlphaLibrary, hit: &picking::Hit) -> Option<Target> {
    if hit.radial_wall_mm < 0.05 {
        return None;
    }
    let ctx = d.field_context();
    let uv = Uv {
        u: ctx.u_of_theta(hit.theta_deg),
        v: hit.v_mm,
    };
    targets(d).into_iter().rev().find_map(|(target, _)| {
        let visible = (1..=target.path.len()).all(|n| {
            entry(&d.layers, &target.path[..n]).is_some_and(|e| e.mask_at(uv, &ctx, lib) > 0.01)
        });
        if !visible {
            return None;
        }
        let (layer, decal) = get(d, &target)?;
        let mut single = layer.clone();
        single.decals = vec![*decal];
        (single.height(uv, &ctx, lib) > 0.002).then_some(target)
    })
}
#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Replace,
    Duplicate,
    Mirror,
}

/// Copies stay in the same owner, retaining its masks, blending and alpha.
pub fn apply(
    d: &mut RingDesign,
    target: &Target,
    mut decal: Decal,
    op: Operation,
) -> Result<Target, String> {
    if d.graph.is_some()
        || d.cad.is_some()
        || picking::live_ancestor(&d.layers, &target.path).is_some()
    {
        return Err(
            "Bake the generated design or group before editing individual ornaments.".into(),
        );
    }
    let ctx = d.field_context();
    if [
        decal.theta_deg,
        decal.v_mm,
        decal.size_mm,
        decal.rotation_deg,
        decal.height_mm,
    ]
    .iter()
    .any(|v| !v.is_finite())
        || !(0.0..=ctx.band_v_len_mm).contains(&decal.v_mm)
        || !(0.1..=24.0).contains(&decal.size_mm)
        || !(0.001..=5.0).contains(&decal.height_mm)
    {
        return Err(
            "Keep the stamp on the band with a 0.1–24 mm size and a finite relief depth.".into(),
        );
    }
    decal.theta_deg = decal.theta_deg.rem_euclid(360.0);
    decal.rotation_deg = (decal.rotation_deg + 180.0).rem_euclid(360.0) - 180.0;
    match op {
        Operation::Duplicate => {
            decal.theta_deg = (decal.theta_deg
                + decal.size_mm * 1.1 / ctx.circumference_mm * 360.0)
                .rem_euclid(360.0);
        }
        Operation::Mirror => {
            decal.v_mm = ctx.band_v_len_mm - decal.v_mm;
            decal.rotation_deg = (180.0 - decal.rotation_deg + 180.0).rem_euclid(360.0) - 180.0;
            decal.flip = !decal.flip;
        }
        Operation::Replace => {}
    }
    let e = picking::entry_mut(&mut d.layers, &target.path)
        .ok_or("The ornament's layer no longer exists.")?;
    let Layer::Decals(layer) = &mut e.layer else {
        return Err("The selected layer is no longer a stamp layer.".into());
    };
    if target.instance >= layer.decals.len().min(MAX_DECALS) {
        return Err("The selected ornament no longer exists.".into());
    }
    let instance = if matches!(op, Operation::Replace) {
        layer.decals[target.instance] = decal;
        target.instance
    } else {
        if layer.decals.len() >= MAX_DECALS {
            return Err("This layer already has 64 stamps. Place a stamp in a new layer.".into());
        }
        let i = layer.decals.len();
        layer.decals.push(decal);
        i
    };
    Ok(Target {
        path: target.path.clone(),
        instance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (RingDesign, Target) {
        let mut d = RingDesign::default();
        let v = d.field_context().crest_v_mm;
        let mut e = LayerEntry::new(
            "repeated name",
            Layer::Decals(DecalLayer {
                alpha: "test".into(),
                decals: vec![Decal {
                    v_mm: v,
                    theta_deg: 359.0,
                    rotation_deg: 27.0,
                    ..Default::default()
                }],
                ..Default::default()
            }),
        );
        e.mask = Some("painted mask".into());
        e.window.enabled = true;
        d.layers.layers.push(LayerEntry::new(
            "group",
            Layer::Group(crate::field::GroupLayer {
                stack: LayerStack { layers: vec![e] },
                recipe: None,
            }),
        ));
        (
            d,
            Target {
                path: vec![0, 0],
                instance: 0,
            },
        )
    }
    #[test]
    fn edits_nested_instance_preserve_masks_and_undo() {
        let (mut d, t) = fixture();
        let before = serde_json::to_value(&d).unwrap();
        let mut history = crate::history::History::new(&d);
        let mut decal = *get(&d, &t).unwrap().1;
        decal.theta_deg = 361.0;
        decal.size_mm = 2.7;
        apply(&mut d, &t, decal, Operation::Replace).unwrap();
        history.commit(&d);
        assert_eq!(get(&d, &t).unwrap().1.theta_deg, 1.0);
        assert_eq!(
            entry(&d.layers, &t.path).unwrap().mask.as_deref(),
            Some("painted mask")
        );
        assert!(entry(&d.layers, &t.path).unwrap().window.enabled);
        assert_eq!(
            serde_json::to_value(history.undo().unwrap()).unwrap(),
            before
        );
    }
    #[test]
    fn copy_and_mirror_keep_parent_and_reflect_artwork() {
        let (mut d, t) = fixture();
        let decal = *get(&d, &t).unwrap().1;
        let copy = apply(&mut d, &t, decal, Operation::Duplicate).unwrap();
        assert_eq!(copy.path, t.path);
        assert_eq!(copy.instance, 1);
        assert!(get(&d, &copy).unwrap().1.theta_deg < 30.0);
        let mirror = apply(&mut d, &t, decal, Operation::Mirror).unwrap();
        let mirrored = *get(&d, &mirror).unwrap().1;
        assert_eq!(mirrored.flip, !decal.flip);
        assert_eq!(mirrored.rotation_deg, 153.0);
        assert!((mirrored.v_mm + decal.v_mm - d.field_context().band_v_len_mm).abs() < 1e-10);
        assert_eq!(targets(&d).len(), 3);
    }
    #[test]
    fn invalid_or_overfull_edits_are_atomic() {
        let (mut d, t) = fixture();
        let mut decal = *get(&d, &t).unwrap().1;
        decal.size_mm = f64::NAN;
        let before = serde_json::to_value(&d).unwrap();
        assert!(apply(&mut d, &t, decal, Operation::Replace).is_err());
        assert_eq!(serde_json::to_value(&d).unwrap(), before);
        let Layer::Decals(layer) = &mut picking::entry_mut(&mut d.layers, &t.path).unwrap().layer
        else {
            panic!()
        };
        layer.decals = vec![
            Decal {
                v_mm: 1.0,
                ..Default::default()
            };
            MAX_DECALS
        ];
        let before = serde_json::to_value(&d).unwrap();
        assert!(
            apply(
                &mut d,
                &t,
                Decal {
                    v_mm: 1.0,
                    ..Default::default()
                },
                Operation::Duplicate
            )
            .is_err()
        );
        assert_eq!(serde_json::to_value(&d).unwrap(), before);
    }
    #[test]
    fn generated_designs_cannot_be_edited_as_manual_stamps() {
        let (mut d, t) = fixture();
        // The graph guard is independent of how a live group was generated.
        d.graph = Some(serde_json::json!({}));
        let decal = *get(&d, &t).unwrap().1;
        assert!(apply(&mut d, &t, decal, Operation::Replace).is_err());
    }
}
