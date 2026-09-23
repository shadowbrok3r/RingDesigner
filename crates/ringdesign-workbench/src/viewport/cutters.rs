//! A right-click's cutters — a piercing on the band, azures and shoulders under a stone — and the funnel edits each makes.
use super::menu::{MenuAction, MenuItem};
use crate::icons::Icon;
use crate::touch::parts::{band_first, fresh_ids};
use ringdesign_core::{
    RingDesign,
    cad::{
        Evaluated, Operation,
        builders::{self, cutters::{self, Shape}},
        edit::CadEdit,
    },
    sketch::Id,
};

/// The piercings by the keys their items carry, and the shape each cuts.
pub const PIERCE_KEYS: [(&str, Shape); 5] = [("round", Shape::Round), ("oval", Shape::Oval), ("marquise", Shape::Marquise), ("heart", Shape::Heart), ("drop", Shape::Drop)];
/// The submenu the piercings fold into.
pub const CUT_HERE: &str = "Cut here";
/// Azures by the keys their items carry, and how many windows each cuts.
pub const AZURE_KEYS: [(&str, u32); 3] = [("azure4", 4), ("azure6", 6), ("azure8", 8)];
/// The submenu the azures fold into.
pub const AZURES: &str = "Azures";
/// Cathedral shoulders' key.
pub const CATHEDRAL: &str = "cathedral";

/// Cutters to place at a point of the band.
pub fn band_items(theta_deg: f64, across_mm: f64) -> Vec<MenuItem> {
    PIERCE_KEYS
        .iter()
        .map(|(key, shape)| {
            MenuItem::new(
                shape.name(),
                Icon::Cutters,
                MenuAction::CutHere { theta_deg, across_mm, key },
                "A piercing of this shape cut through the band here, sized to the room the band has: through a side face it casts, through the crown it is drilled at the bench",
            )
            .under(CUT_HERE)
        })
        .collect()
}

/// Builders to make under reference stone `stone`.
pub fn stone_items(stone: Id) -> Vec<MenuItem> {
    let mut items: Vec<MenuItem> = AZURE_KEYS
        .iter()
        .map(|(key, count)| {
            MenuItem::new(
                format!("{count} windows"),
                Icon::NodeOpenwork,
                MenuAction::UnderStone { stone, key },
                "Teardrop windows cut up through the band under the stone round its axis, clear of its seat's pilot and its claws, sized to fit",
            )
            .under(AZURES)
        })
        .collect();
    items.push(MenuItem::new(
        "Cathedral shoulders",
        Icon::NodeShank,
        MenuAction::UnderStone { stone, key: CATHEDRAL },
        "Two wire arches rising from the band's shoulders either side of the stone to its head's gallery rail; the stone needs a head",
    ));
    items
}

/// Piercing `key` at `theta_deg`, `across_mm`, or where `hit` (a band point and its normal) landed: its edits and its id.
pub fn cut_here(design: &RingDesign, theta_deg: f64, across_mm: f64, hit: Option<([f64; 3], [f64; 3])>, key: &str) -> Result<(Vec<CadEdit>, Id), String> {
    let shape = PIERCE_KEYS.iter().find(|(k, _)| *k == key).map(|(_, s)| *s).ok_or_else(|| format!("No piercing called {key}"))?;
    let at = cutters::pierce_at(design, theta_deg, across_mm, hit, shape).map_err(|e| format!("{e:#}"))?;
    let mut next = fresh_ids(design);
    let mut edits: Vec<CadEdit> = band_first(design, &mut next).into_iter().collect();
    let id = next();
    edits.push(CadEdit::Add { feature: cutters::pierce_feature(id, shape, &at), after: None });
    Ok((edits, id))
}

/// Azures or cathedral shoulders `key` under stone part `stone` of the ring as `built`, refused by name: its edits and its id.
pub fn under_stone(design: &RingDesign, built: Option<&Evaluated>, stone: Id, key: &str) -> Result<(Vec<CadEdit>, Id), String> {
    let doc = design.cad.as_ref().ok_or_else(|| format!("No part #{stone}"))?;
    let f = doc.feature(stone).ok_or_else(|| format!("No part #{stone}"))?;
    let named = format!("#{stone} {}", f.name);
    if !matches!(&f.operation, Operation::Builder { key, .. } if key == builders::STONE) {
        return Err(format!("{named} is not a stone"));
    }
    let head = builders::head_on(doc, stone).map(|h| h.id);
    let sand = builders::sand(design);
    let frame = built.and_then(|e| e.components.iter().find(|c| c.id == stone)).map(|c| c.frame);
    let id = fresh_ids(design)();
    let feature = if let Some((_, count)) = AZURE_KEYS.iter().find(|(k, _)| *k == key) {
        // The stone's axis as built, else across the pull as on the crown.
        let axis = frame.map_or([1.0, 0.0, 0.0], |f| f.z_axis);
        cutters::azure_feature(id, stone, head, *count, cutters::cut_stage(axis, sand))
    } else if key == CATHEDRAL {
        let head = head.ok_or_else(|| format!("{named} carries no head for cathedral shoulders to meet: set it in claws, a basket or a bezel first"))?;
        cutters::cathedral_feature(id, stone, head, cutters::shoulder_stage_for(design, frame.map(|f| f.origin)))
    } else {
        return Err(format!("No builder under a stone called {key}"));
    };
    Ok((vec![CadEdit::Add { feature, after: None }], id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::cad::{Attach, Component, Document, Feature, Placement, Stage};
    use ringdesign_core::gem::{Gem, GemCut};

    fn court() -> RingDesign {
        ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design()
    }
    /// The Court band with stone #2 on its top, and head #3 round it when `head` says so.
    fn stone(head: bool) -> RingDesign {
        let gem = Gem::calibrated(GemCut::Round, 6.5);
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(builders::stone_feature(2, gem, Placement::ring(90.0, builders::stand_off_mm("claw4", gem)))).unwrap();
        if head {
            let mut next = 2;
            for f in builders::setting_features("claw4", 2, gem, true, &mut || {
                next += 1;
                next
            })
            .unwrap()
            {
                doc.append(f).unwrap();
            }
        }
        RingDesign { cad: Some(doc), ..court() }
    }
    fn added(edits: &[CadEdit]) -> Vec<Feature> {
        edits.iter().filter_map(|e| if let CadEdit::Add { feature, .. } = e { Some(feature.clone()) } else { None }).collect()
    }

    #[test]
    fn the_band_offers_every_piercing_under_cut_here_and_a_stone_its_azures_and_shoulders() {
        let items = band_items(90.0, 0.25);
        assert_eq!(items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), ["Round", "Oval", "Marquise", "Heart", "Drop"]);
        assert!(items.iter().all(|i| i.submenu == Some(CUT_HERE) && i.enabled && i.icon == Icon::Cutters));
        assert_eq!(items[3].action, MenuAction::CutHere { theta_deg: 90.0, across_mm: 0.25, key: "heart" });
        let items = stone_items(4);
        assert_eq!(items.iter().map(|i| (i.submenu, i.label.as_str())).collect::<Vec<_>>(), [(Some(AZURES), "4 windows"), (Some(AZURES), "6 windows"), (Some(AZURES), "8 windows"), (None, "Cathedral shoulders")]);
        assert_eq!(items[1].action, MenuAction::UnderStone { stone: 4, key: "azure6" });
        assert_eq!(items[3].action, MenuAction::UnderStone { stone: 4, key: CATHEDRAL });
        // Every key an item carries is one the planners answer to.
        for (key, _) in PIERCE_KEYS {
            assert!(cut_here(&court(), 90.0, 0.0, None, key).is_ok(), "{key}");
        }
        for (key, _) in AZURE_KEYS {
            assert!(under_stone(&stone(true), None, 2, key).is_ok(), "{key}");
        }
    }

    #[test]
    fn a_piercing_brings_the_shank_with_it_on_a_plain_band_and_takes_the_stage_its_walls_want() {
        // On a plain band the cut comes with the procedural shank, in one commit.
        let (edits, id) = cut_here(&court(), 90.0, 0.0, None, "heart").unwrap();
        let features = added(&edits);
        assert_eq!(features.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Procedural shank", "Heart piercing"]);
        let cut = &features[1];
        assert_eq!(cut.id, id);
        assert!(matches!(&cut.operation, Operation::Builder { key, on: None, params } if key == builders::PIERCE && params["shape"] == "Heart" && params["turn_deg"] == 90.0));
        // Down the crown's normal a hole faces both mould halves: drilled at the bench under sand, cast under lost wax.
        assert_eq!((cut.component.attach, cut.component.stage), (Attach::Cut, Stage::Bench));
        assert!(matches!(cut.component.placement, Placement::Ring { theta_deg, across_mm, height_mm, cant_deg, .. } if theta_deg == 90.0 && across_mm == 0.0 && height_mm == 0.0 && cant_deg == 0.0));
        let mut wax = court();
        wax.draft.process = ringdesign_core::castability::CastProcess::LostWax;
        assert_eq!(added(&cut_here(&wax, 90.0, 0.0, None, "round").unwrap().0)[1].component.stage, Stage::Cast);
        // Through a side face it runs along the pull and casts; the shank is already there.
        let mut flat = RingDesign::default();
        flat.profile.apply_style(ringdesign_core::profile::ProfileStyle::Flat);
        (flat.profile.width_mm, flat.profile.thickness_mm) = (5.0, 2.5);
        let with_shank = RingDesign { cad: stone(false).cad, ..flat };
        let (edits, _) = cut_here(&with_shank, 90.0, 2.5, Some(([0.0, 9.9, 2.5], [0.0, 0.0, 1.0])), "round").unwrap();
        let features = added(&edits);
        assert_eq!(features.len(), 1, "the shank is there already");
        assert_eq!(features[0].component.stage, Stage::Cast);
        assert!(matches!(features[0].component.placement, Placement::Ring { cant_deg, .. } if cant_deg == -90.0));
        // Too near the edge, refused with the band's own words; an unknown key by name.
        assert_eq!(cut_here(&court(), 90.0, 1.7, None, "round").unwrap_err(), "Too near the band's edge to pierce here: 0.30 mm from it");
        assert_eq!(cut_here(&court(), 90.0, 0.0, None, "star").unwrap_err(), "No piercing called star");
    }

    #[test]
    fn shoulders_round_a_stone_on_the_parting_line_are_cast_and_off_it_go_to_the_bench() {
        let gem = Gem::calibrated(GemCut::Round, 6.5);
        let build = |d: &RingDesign| {
            ringdesign_core::mesh::try_build(d, &ringdesign_core::AlphaLibrary::default(), ringdesign_core::mesh::BuildParams { theta_steps: 192, profile_steps: 96, ..Default::default() }).unwrap()
        };
        let d = stone(true);
        let on = build(&d);
        let (edits, _) = under_stone(&d, on.parts.evaluated.as_ref(), 2, CATHEDRAL).unwrap();
        assert_eq!(added(&edits)[0].component.stage, Stage::Cast, "the stone's girdle on the parting line: the arches pour clean");
        let mut off = d.clone();
        let placement = Placement::Ring { theta_deg: 90.0, across_mm: 1.0, height_mm: builders::stand_off_mm("claw4", gem), spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 };
        off.cad.as_mut().unwrap().features.iter_mut().find(|f| f.id == 2).unwrap().component.placement = placement;
        let (edits, _) = under_stone(&off, build(&off).parts.evaluated.as_ref(), 2, CATHEDRAL).unwrap();
        assert_eq!(added(&edits)[0].component.stage, Stage::Bench, "a millimetre along the finger the arches lock and go to the bench");
    }

    #[test]
    fn azures_and_shoulders_stand_on_the_stone_and_shoulders_want_its_head() {
        let d = stone(true);
        let (edits, id) = under_stone(&d, None, 2, "azure8").unwrap();
        let f = &added(&edits)[0];
        assert_eq!((f.id, f.name.as_str()), (id, "8 azures"));
        assert!(matches!(&f.operation, Operation::Builder { key, on: Some(2), params } if key == builders::AZURE && params["count"] == 8 && params[builders::HEAD] == 3));
        assert_eq!((f.component.attach, f.component.stage), (Attach::Cut, Stage::Bench), "a crown stone's windows run across the pull");
        let (edits, _) = under_stone(&d, None, 2, CATHEDRAL).unwrap();
        let f = &added(&edits)[0];
        assert!(matches!(&f.operation, Operation::Builder { key, on: Some(2), params } if key == builders::CATHEDRAL && params[builders::HEAD] == 3));
        assert_eq!((f.component.attach, f.component.stage), (Attach::Join, Stage::Bench), "under sand the shoulders go to the bench with their head");
        // Without a head the shoulders have nothing to meet; azures still cut, clear of the seat alone.
        let bare = stone(false);
        assert_eq!(under_stone(&bare, None, 2, CATHEDRAL).unwrap_err(), "#2 Round 6.5 mm carries no head for cathedral shoulders to meet: set it in claws, a basket or a bezel first");
        let (edits, _) = under_stone(&bare, None, 2, "azure4").unwrap();
        assert!(matches!(&added(&edits)[0].operation, Operation::Builder { params, .. } if params.get(builders::HEAD).is_none()));
        // Only a stone takes them.
        assert_eq!(under_stone(&d, None, 3, "azure4").unwrap_err(), "#3 Four-claw head is not a stone");
        assert_eq!(under_stone(&d, None, 9, "azure4").unwrap_err(), "No part #9");
    }
}
