//! A part shown alone: its own solid as built, with only its evaluation kept, so a view draws, edges and picks it and nothing else.
use ringdesign_core::{BuildResult, sketch::Id};

/// `built` showing part `id`'s own closed solid and its own evaluation, every vertex naming the part so picks name its faces; why not when it cannot stand alone.
pub fn alone(built: &BuildResult, id: Id) -> Result<BuildResult, String> {
    let e = built.parts.evaluated.as_ref().ok_or("The ring has no parts to show alone")?;
    let c = e.components.iter().find(|c| c.id == id).ok_or_else(|| format!("#{id} did not build; mend it on the strip first"))?;
    if c.settings.reference {
        return Err(format!("{} is a stone, never metal: show its setting alone instead", c.name));
    }
    let index = built.parts.features.iter().position(|f| *f == id).ok_or_else(|| format!("{} stands on no face of the ring as built", c.name))? as u32;
    if c.mesh.faces.is_empty() {
        return Err(format!("{} has no faces to show", c.name));
    }
    let mut mesh = c.mesh.clone();
    mesh.origin = vec![built.parts.origin_of(index); mesh.vertices.len()];
    let mut parts = built.parts.clone();
    if let Some(e) = parts.evaluated.as_mut() {
        e.components.retain(|c| c.id == id);
    }
    Ok(BuildResult { mesh, report: built.report.clone(), reference: built.reference.clone(), spacing: built.spacing.clone(), solids: built.solids.clone(), parts, band: built.band.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::{
        AlphaLibrary, BuildParams, RingDesign,
        cad::{Attach, Component, Document, Feature, Operation, Placement},
        interaction::pick::{Entity, Filter, PickScene, Ray, ViewScale},
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

    #[test]
    fn a_part_alone_is_its_own_closed_solid_picking_its_faces_and_nothing_of_the_band() {
        let d = parts();
        let built = mesh::build(&d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 192, profile_steps: 96, refine: None, ..BuildParams::default() });
        let post = built.parts.features.iter().position(|f| *f == 2).unwrap() as u32;
        let own = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap().mesh.clone();
        let shown = alone(&built, 2).unwrap();
        // The whole post as built on its own, the foot the join sinks into the band included, and closed.
        assert_eq!((&shown.mesh.vertices, &shown.mesh.faces), (&own.vertices, &own.faces));
        let check = shown.mesh.validate();
        assert!(check.watertight && check.boundary_edges == 0, "{check:?}");
        assert!(ringdesign_core::interaction::pick::part_owners(&shown).iter().all(|o| *o == Some(post)), "every face is the post's");
        assert_eq!((shown.mesh.origin.len(), shown.mesh.normals.len()), (shown.mesh.vertices.len(), shown.mesh.vertices.len()), "with its provenance and normals");
        let (lo, hi) = shown.mesh.bounds().unwrap();
        let fused_low = built.mesh.faces.iter().zip(ringdesign_core::interaction::pick::part_owners(&built)).filter(|(_, o)| *o == Some(post)).flat_map(|(f, _)| f.map(|v| built.mesh.vertices[v as usize].1)).fold(f32::INFINITY, f32::min);
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
        assert!(alone(&built, 4).err().unwrap().contains("never metal"));
        assert!(alone(&built, 9).err().unwrap().contains("#9"));
    }
}
