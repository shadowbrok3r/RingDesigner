//! Parts shown alone: their own solids as built, with only their evaluations kept, so a view draws, edges and picks them and nothing else.
use ringdesign_core::{
    BuildResult, Mesh, Vec3,
    cad::{Document, Operation},
    sketch::Id,
};

/// Parts shown alone: the build of their own solids, which of the parts asked for stand in it, and why each other one does not.
pub struct Alone {
    pub build: BuildResult,
    pub shown: Vec<Id>,
    pub left_out: Vec<(Id, String)>,
}

/// Part `id`'s own closed solid in `built` and its index among the build's parts; why not when it cannot stand alone.
fn part_of(built: &BuildResult, id: Id) -> Result<(&Mesh, u32), String> {
    let e = built.parts.evaluated.as_ref().ok_or("The ring has no parts to show alone")?;
    let c = e.components.iter().find(|c| c.id == id).ok_or_else(|| format!("#{id} did not build; mend it on the strip first"))?;
    if c.settings.reference {
        return Err(format!("{} is a stone, never metal: show its setting alone instead", c.name));
    }
    let index = built.parts.features.iter().position(|f| *f == id).ok_or_else(|| format!("{} stands on no face of the ring as built", c.name))? as u32;
    if c.mesh.faces.is_empty() {
        return Err(format!("{} has no faces to show", c.name));
    }
    Ok((&c.mesh, index))
}

/// `own` added to `mesh`, every vertex it brings naming `origin`; a part without smooth normals keeps its facets'.
fn append(mesh: &mut Mesh, own: &Mesh, origin: u32) {
    let (base, first) = (mesh.vertices.len() as u32, mesh.faces.len() as u32);
    mesh.vertices.extend_from_slice(&own.vertices);
    mesh.faces.extend(own.faces.iter().map(|f| f.map(|i| i + base)));
    mesh.origin.extend(std::iter::repeat_n(origin, own.vertices.len()));
    if own.normals.len() == own.vertices.len() {
        mesh.normals.extend_from_slice(&own.normals);
        mesh.corner_normals.extend(own.corner_normals.iter().map(|(f, n)| (f + first, *n)));
    } else {
        mesh.normals.extend(std::iter::repeat_n(Vec3(f32::NAN, f32::NAN, f32::NAN), own.vertices.len()));
        for (k, f) in own.faces.iter().enumerate() {
            let n = own.face_normal(f).map_or(Vec3(0.0, 0.0, 1.0), |n| Vec3(n[0] as f32, n[1] as f32, n[2] as f32));
            mesh.corner_normals.push((first + k as u32, [n; 3]));
        }
    }
}

/// `built` showing parts `ids` alone, each its own closed solid, every vertex naming its part so picks name its faces;
/// a part that cannot stand alone is left out and said, and when none can, why the first could not.
pub fn alone(built: &BuildResult, ids: &[Id]) -> Result<Alone, String> {
    let mut mesh = Mesh::default();
    let (mut shown, mut left_out): (Vec<Id>, Vec<(Id, String)>) = (Vec::new(), Vec::new());
    for &id in ids {
        if shown.contains(&id) || left_out.iter().any(|(l, _)| *l == id) {
            continue;
        }
        match part_of(built, id) {
            Ok((own, index)) => {
                append(&mut mesh, own, built.parts.origin_of(index));
                shown.push(id);
            }
            Err(why) => left_out.push((id, why)),
        }
    }
    if shown.is_empty() {
        return Err(left_out.into_iter().next().map_or_else(|| "No part was asked to stand alone".to_string(), |(_, why)| why));
    }
    let mut parts = built.parts.clone();
    if let Some(e) = parts.evaluated.as_mut() {
        e.components.retain(|c| shown.contains(&c.id));
    }
    let build = BuildResult { mesh, report: built.report.clone(), reference: built.reference.clone(), spacing: built.spacing.clone(), solids: built.solids.clone(), parts, band: built.band.clone() };
    Ok(Alone { build, shown, left_out })
}

/// What stays shown alone once features `added` land in `doc`: each metal part among them joins the view, in place of a shown part it replaces; nothing is shown alone after it when nothing was before.
pub fn after_edit(shown: &[Id], doc: &Document, added: &[Id]) -> Vec<Id> {
    let mut out = shown.to_vec();
    if out.is_empty() {
        return out;
    }
    for id in added {
        let Some(f) = doc.feature(*id) else { continue };
        if !f.operation.has_body() || matches!(f.operation, Operation::Band) || f.component.reference {
            continue;
        }
        let replaced = f.operation.consumes();
        out.retain(|s| !replaced.contains(s));
        if !out.contains(id) {
            out.push(*id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::{
        AlphaLibrary, BuildParams, RingDesign,
        cad::{Attach, Component, Document, EdgeRef, Feature, Operation, Placement},
        interaction::pick::{Entity, Filter, PickScene, Ray, ViewScale, part_owners},
        mesh, templates,
    };

    /// The Court band with a post joined at its top and a box joined at its side.
    fn parts() -> RingDesign {
        let mut d = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        let at = |theta: f64| Component { attach: Attach::Join, placement: Placement::ring(theta, 0.0), ..Component::default() };
        doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, component: at(90.0) }).unwrap();
        doc.append(Feature { id: 3, name: "Block".into(), enabled: true, operation: Operation::Box { size: [2.0, 2.0, 1.0] }, component: at(0.0) }).unwrap();
        let gem = ringdesign_core::cad::builders::stone_preset("round-5").unwrap().gem();
        doc.append(ringdesign_core::cad::builders::stone_feature(4, gem, Placement::ring(180.0, 2.0))).unwrap();
        d.cad = Some(doc);
        d
    }

    fn built(d: &RingDesign) -> BuildResult {
        mesh::build(d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 192, profile_steps: 96, refine: None, ..BuildParams::default() })
    }

    #[test]
    fn a_part_alone_is_its_own_closed_solid_picking_its_faces_and_nothing_of_the_band() {
        let d = parts();
        let built = built(&d);
        let post = built.parts.features.iter().position(|f| *f == 2).unwrap() as u32;
        let own = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap().mesh.clone();
        let alone = alone(&built, &[2]).unwrap();
        assert_eq!((alone.shown.as_slice(), alone.left_out.len()), (&[2][..], 0));
        let shown = alone.build;
        // The whole post as built on its own, the foot the join sinks into the band included, and closed.
        assert_eq!((&shown.mesh.vertices, &shown.mesh.faces), (&own.vertices, &own.faces));
        let check = shown.mesh.validate();
        assert!(check.watertight && check.boundary_edges == 0, "{check:?}");
        assert!(part_owners(&shown).iter().all(|o| *o == Some(post)), "every face is the post's");
        assert_eq!((shown.mesh.origin.len(), shown.mesh.normals.len()), (shown.mesh.vertices.len(), shown.mesh.vertices.len()), "with its provenance and normals");
        let (lo, hi) = shown.mesh.bounds().unwrap();
        let fused_low = built.mesh.faces.iter().zip(part_owners(&built)).filter(|(_, o)| *o == Some(post)).flat_map(|(f, _)| f.map(|v| built.mesh.vertices[v as usize].1)).fold(f32::INFINITY, f32::min);
        assert!(lo.1 < fused_low - 0.01, "the foot below what the fused ring shows of it: {} under {fused_low}", lo.1);
        assert!((hi.0 - lo.0 - 2.0).abs() < 0.01, "the post's 2 mm across");
        assert_eq!(shown.parts.evaluated.as_ref().unwrap().components.iter().map(|c| c.id).collect::<Vec<_>>(), [2], "only the post's edges are drawn");
        // Down onto the post: its end face; beside it, where the band was, nothing.
        let scene = PickScene::build(&shown, &d);
        let view = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 20.0 };
        let down = |x: f64, z: f64| Ray { origin: [x, 40.0, z], direction: [0.0, -1.0, 0.0] };
        let on_post = scene.pick(down(0.2, 0.3), &view, 12.0, Filter::default());
        assert!(matches!(on_post.first().map(|p| &p.entity), Some(Entity::Face { feature: 2, .. })), "{:?}", on_post.first());
        assert!(scene.pick(down(4.0, 0.5), &view, 12.0, Filter::default()).iter().all(|p| p.entity != Entity::Band), "the band is not there to pick");
        let full = PickScene::build(&built, &d);
        assert!(full.pick(down(4.0, 0.5), &view, 12.0, Filter::default()).iter().any(|p| p.entity == Entity::Band));
        // A stone is never metal, and a part the ring does not hold is refused by name.
        assert!(super::alone(&built, &[4]).err().unwrap().contains("never metal"));
        assert!(super::alone(&built, &[9]).err().unwrap().contains("#9"));
    }

    #[test]
    fn parts_alone_together_are_each_their_own_solid_and_what_cannot_stand_alone_is_left_out_and_said() {
        let d = parts();
        let built = built(&d);
        let own = |id| built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == id).unwrap().mesh.clone();
        let index = |id| built.parts.features.iter().position(|f| *f == id).unwrap() as u32;
        let (post, block) = (own(2), own(3));
        // The post, the stone, a part the ring does not hold, the block and the post again.
        let alone = alone(&built, &[2, 4, 9, 3, 2]).unwrap();
        assert_eq!(alone.shown, [2, 3]);
        assert_eq!(alone.left_out.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [4, 9]);
        assert!(alone.left_out[0].1.contains("never metal") && alone.left_out[1].1.contains("#9"), "{:?}", alone.left_out);
        let shown = &alone.build;
        assert_eq!(shown.mesh.vertices.len(), post.vertices.len() + block.vertices.len());
        assert_eq!(shown.mesh.faces.len(), post.faces.len() + block.faces.len());
        let check = shown.mesh.validate();
        assert!(check.watertight && check.boundary_edges == 0, "two closed solids: {check:?}");
        // Each face is its own part's, the post's first.
        let owners = part_owners(shown);
        assert!(owners[..post.faces.len()].iter().all(|o| *o == Some(index(2))) && owners[post.faces.len()..].iter().all(|o| *o == Some(index(3))));
        let volume = shown.mesh.volume_mm3();
        assert!((volume - post.volume_mm3() - block.volume_mm3()).abs() < 1e-6, "{volume}");
        assert_eq!(shown.parts.evaluated.as_ref().unwrap().components.iter().map(|c| c.id).collect::<Vec<_>>(), [2, 3], "both parts' edges are drawn");
        // Picked from above the ring's top: the post; from beside the ring at 0°: the block.
        let scene = PickScene::build(shown, &d);
        let view = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 20.0 };
        let top = scene.pick(Ray { origin: [0.2, 40.0, 0.3], direction: [0.0, -1.0, 0.0] }, &view, 12.0, Filter::default());
        assert!(matches!(top.first().map(|p| &p.entity), Some(Entity::Face { feature: 2, .. })), "{:?}", top.first());
        let side = scene.pick(Ray { origin: [40.0, 0.2, 0.3], direction: [-1.0, 0.0, 0.0] }, &view, 12.0, Filter::default());
        assert!(matches!(side.first().map(|p| &p.entity), Some(Entity::Face { feature: 3, .. })), "{:?}", side.first());
        // Nothing that can stand alone: the first reason is the answer.
        let none = super::alone(&built, &[4, 9]).err().unwrap();
        assert!(none.contains("never metal"), "{none}");
    }

    #[test]
    fn a_part_made_while_parts_stand_alone_joins_them_in_place_of_what_it_replaces() {
        let mut doc = parts().cad.unwrap();
        let fillet = Feature { id: 5, name: "Fillet".into(), enabled: true, operation: Operation::Fillet { source: 2, edges: vec![EdgeRef::bare(0)], radius_mm: 0.2 }, component: Component::default() };
        doc.append(fillet).unwrap();
        let sketch = Feature { id: 6, name: "Sketch".into(), enabled: true, operation: Operation::Sketch { sketch: Default::default() }, component: Component::default() };
        doc.append(sketch).unwrap();
        // A new body joins, a fillet takes the place of the post it rounds, a sketch and a stone stay out of it.
        assert_eq!(after_edit(&[2], &doc, &[5, 6, 4]), [5]);
        assert_eq!(after_edit(&[3], &doc, &[5]), [3, 5]);
        assert_eq!(after_edit(&[3], &doc, &[1, 9]), [3], "the band and a feature that is not there change nothing");
        assert!(after_edit(&[], &doc, &[5]).is_empty(), "with nothing alone the whole ring stays");
    }
}
