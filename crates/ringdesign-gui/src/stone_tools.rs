//! Stones and the settings built round them, from the Ring viewport's right-click, each one funnel commit.
use crate::app::RingDesignerApp;
use ringdesign_core::{
    RingDesign,
    cad::{self, Component, ComponentRole, FaceSeat, Feature, Operation, Placement, builders, edit::CadEdit},
    gem::Gem,
    interaction::pick::Entity,
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
            Ok((gem, held)) => {
                edits.extend(held);
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

/// The gem of stone part `id`, and the edit that stands it where setting `key` holds it, on the ring or on a part's face.
fn stone_part(design: &RingDesign, id: Id, key: &str) -> Result<(Gem, Option<CadEdit>), String> {
    let f = design.cad.as_ref().and_then(|d| d.feature(id)).ok_or_else(|| format!("No part #{id}"))?;
    let Operation::Builder { key: builder, params, .. } = &f.operation else {
        return Err(format!("#{id} {} is not a stone", f.name));
    };
    if builder != builders::STONE {
        return Err(format!("#{id} {} is not a stone", f.name));
    }
    let gem = builders::gem_of(params).map_err(|e| format!("{e:#}"))?;
    let held = builders::stand_off_mm(key, gem);
    let edit = match f.component.placement.clone() {
        Placement::Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg } => ((held - height_mm).abs() > 1e-9)
            .then_some(CadEdit::Placement { id, placement: Placement::Ring { theta_deg, across_mm, height_mm: held, spin_deg, tilt_deg, cant_deg } }),
        Placement::Free => match FaceSeat::of(params).map_err(|e| format!("{e:#}"))? {
            Some(seat) if (held - seat.height_mm).abs() > 1e-9 => {
                let mut operation = f.operation.clone();
                if let Operation::Builder { params, .. } = &mut operation {
                    FaceSeat { height_mm: held, ..seat }.write(params);
                }
                Some(CadEdit::Operation { id, operation })
            }
            _ => None,
        },
    };
    Ok((gem, edit))
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

/// Seats a reference stone named by `key` on planar face `face` of part `feature`, where the right-click landed on it.
pub fn add_stone_on_face(app: &mut RingDesignerApp, feature: u64, face: u32, key: &'static str) {
    let Some(preset) = builders::stone_preset(key) else {
        app.set_status(format!("No stone called {key}"));
        return;
    };
    let gem = preset.gem();
    let part = app.build.as_ref().and_then(|b| b.parts.evaluated.as_ref()).and_then(|e| e.components.iter().find(|c| c.id == feature)).cloned();
    let Some(part) = part else {
        app.set_status(format!("Part #{feature} is not in the ring as built yet"));
        return;
    };
    let at = app.selection.under.as_ref().filter(|p| p.entity == Entity::Face { feature, face }).map(|p| p.world);
    let seat = match FaceSeat::on(&part, face, at, builders::stand_off_mm("claw4", gem)) {
        Ok(seat) => seat,
        Err(e) => {
            app.set_status(format!("{e:#}"));
            return;
        }
    };
    let id = fresh_ids(&app.design)();
    let edits = [CadEdit::Add { feature: cad::stone_on_face(id, gem, feature, &seat), after: None }];
    if crate::cad_edit::apply(app, &edits).is_ok() {
        app.selection.click(Some(Sel::Part(id)), Mods::default());
    }
}
