//! Stones and the settings built round them, from the Ring viewport's right-click, each one funnel commit.
use crate::app::RingDesignerApp;
use ringdesign_core::{
    RingDesign,
    cad::{Component, ComponentRole, Feature, Operation, Placement, builders, edit::CadEdit},
    gem::Gem,
    sketch::Id,
};
use ringdesign_workbench::viewport::{Sel, selection::Mods};

/// Ids no feature and no graph node holds yet, in order.
fn fresh_ids(design: &RingDesign) -> impl FnMut() -> Id + use<> {
    let doc = design.cad.as_ref().map_or(1, |d| d.fresh_id());
    let graph = design
        .graph
        .as_ref()
        .and_then(|g| serde_json::from_value::<ringdesign_graph::graph::Graph>(g.clone()).ok())
        .map_or(0, |g| g.nodes.iter().map(|n| n.id.0 + 1).max().unwrap_or(0).max(g.next_id));
    let mut next = doc.max(graph).max(1);
    move || {
        next += 1;
        next - 1
    }
}

/// The procedural shank a plain ring's first part brings with it, so the part stands beside the band.
fn band_first(design: &RingDesign, next: &mut impl FnMut() -> Id) -> Option<CadEdit> {
    design.cad.as_ref().is_none_or(|d| d.features.is_empty()).then(|| {
        let component = Component { role: ComponentRole::Shank, ..Component::default() };
        CadEdit::Add { feature: Feature { id: next(), name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component }, after: None }
    })
}

/// Seats a reference stone named by `key` on the band at `theta_deg`, its culet clear of the metal.
pub fn add_stone(app: &mut RingDesignerApp, theta_deg: f64, _height_mm: f64, key: &'static str) {
    let Some(preset) = builders::stone_preset(key) else {
        app.set_status(format!("No stone called {key}"));
        return;
    };
    let gem = preset.gem();
    let mut next = fresh_ids(&app.design);
    let mut edits: Vec<CadEdit> = band_first(&app.design, &mut next).into_iter().collect();
    let id = next();
    edits.push(CadEdit::Add { feature: builders::stone_feature(id, gem, Placement::ring(theta_deg, builders::stand_off_mm("claw4", gem))), after: None });
    if crate::cad_edit::apply(app, &edits).is_ok() {
        app.selection.click(Some(Sel::Part(id)), Mods::default());
    }
}

/// Builds the setting named by `key` round a stone part, or round a height-field stone first added as a part.
pub fn setting(app: &mut RingDesignerApp, part: Option<u64>, stone: Option<Vec<usize>>, key: &'static str) {
    if builders::setting_preset(key).is_none() {
        app.set_status(format!("No setting called {key}"));
        return;
    }
    let mut next = fresh_ids(&app.design);
    let mut edits = Vec::new();
    let (stone_id, gem) = match (part, stone) {
        (Some(id), _) => match stone_part(&app.design, id, key) {
            Ok((gem, placement)) => {
                edits.extend(placement.map(|placement| CadEdit::Placement { id, placement }));
                (id, gem)
            }
            Err(why) => {
                app.set_status(why);
                return;
            }
        },
        (None, Some(path)) => {
            let Some((placement, gem)) = seat_stone(app, &path) else {
                app.set_status("That stone is no longer in the design");
                return;
            };
            edits.extend(band_first(&app.design, &mut next));
            let id = next();
            edits.push(CadEdit::Add { feature: builders::stone_feature(id, gem, placement), after: None });
            (id, gem)
        }
        (None, None) => return,
    };
    let features = match builders::setting_features(key, stone_id, gem, builders::sand(&app.design), &mut next) {
        Ok(f) => f,
        Err(e) => {
            app.set_status(format!("{e:#}"));
            return;
        }
    };
    let head = features.first().map(|f| f.id);
    edits.extend(features.into_iter().map(|feature| CadEdit::Add { feature, after: None }));
    if crate::cad_edit::apply(app, &edits).is_ok() {
        if let Some(head) = head {
            app.selection.click(Some(Sel::Part(head)), Mods::default());
        }
    }
}

/// The gem of stone part `id`, and the placement that stands it where setting `key` holds it when it stands on the ring.
fn stone_part(design: &RingDesign, id: Id, key: &str) -> Result<(Gem, Option<Placement>), String> {
    let f = design.cad.as_ref().and_then(|d| d.feature(id)).ok_or_else(|| format!("No part #{id}"))?;
    let Operation::Builder { key: builder, params, .. } = &f.operation else {
        return Err(format!("#{id} {} is not a stone", f.name));
    };
    if builder != builders::STONE {
        return Err(format!("#{id} {} is not a stone", f.name));
    }
    let gem = builders::gem_of(params).map_err(|e| format!("{e:#}"))?;
    let placement = match f.component.placement.clone() {
        Placement::Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg } => {
            let held = builders::stand_off_mm(key, gem);
            ((held - height_mm).abs() > 1e-9).then_some(Placement::Ring { theta_deg, across_mm, height_mm: held, spin_deg, tilt_deg, cant_deg })
        }
        Placement::Free => None,
    };
    Ok((gem, placement))
}

/// A height-field stone as a ring placement at its own girdle frame, read against the ring as last built, and its gem.
fn seat_stone(app: &RingDesignerApp, path: &[usize]) -> Option<(Placement, Gem)> {
    let (st, frame) = ringdesign_core::stones::stone_frames(&app.design).into_iter().find(|(s, _)| s.path == path)?;
    let stand_off = st.stand_off_mm();
    let across_mm = frame.girdle[2] - frame.normal[2] * stand_off;
    let height_mm = app
        .build
        .as_ref()
        .and_then(|b| ringdesign_core::cad::surface_hit(&b.mesh, st.theta_deg, across_mm))
        .map_or(stand_off, |(hit, n)| (0..3).map(|k| (frame.girdle[k] - hit[k]) * n[k]).sum());
    Some((Placement::Ring { theta_deg: st.theta_deg, across_mm, height_mm, spin_deg: st.rot_deg(), tilt_deg: 0.0, cant_deg: 0.0 }, st.gem))
}

/// Seats a reference stone named by `key` on a planar face of a part.
pub fn add_stone_on_face(app: &mut RingDesignerApp, _feature: u64, _face: u32, _key: &'static str) {
    app.set_status("Stones on a part's face arrive with M13");
}
