//! Editable documents and stone stock for the starter gallery.
use crate::{ProfileStyle, RingDesign};
use crate::cad::{Attach, Component, Document, Feature, Operation, Placement, Stage, builders};
use crate::castability::{CastProcess, SandProcess};
use crate::field::{Layer, LayerEntry, SeatPadLayer, SeatRunLayer, SeatStyle, Window};
use crate::gem::{Gem, GemCut};
use crate::profile::{ShankKey, ShankKind};
use crate::setting::SolidKind;
use anyhow::Result;
use serde_json::json;

fn base(style: ProfileStyle, width: f64, thickness: f64, wax: bool) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(style);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    if wax { CastProcess::LostWax.apply(&mut d.draft); }
    else { SandProcess::DelftClay.apply(&mut d.draft); }
    d
}

fn band() -> Result<Document> {
    let mut doc = Document::default();
    doc.append(feature(1, "Procedural shank", Operation::Band, Attach::Separate))?;
    Ok(doc)
}

fn feature(id: u64, name: &str, operation: Operation, attach: Attach) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component: Component { attach, ..Default::default() } }
}

fn oval(width: f64, length: f64) -> Gem {
    Gem { l_mm: length, ..Gem::calibrated(GemCut::Oval, width) }
}

fn settings(doc: &mut Document, id: u64, gem: Gem, placement: Placement, key: &str, sand: bool) -> Result<()> {
    doc.append(builders::stone_feature(id, gem, placement))?;
    let mut next = id;
    for f in builders::setting_features(key, id, gem, sand, &mut || { next += 1; next })? {
        doc.append(f)?;
    }
    Ok(())
}

pub fn cathedral_solitaire() -> RingDesign {
    let mut d = base(ProfileStyle::DShape, 2.1, 1.8, true);
    // Widens the band only under the head.
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = [(270.0,1.0),(180.0,0.89875),(125.0,0.83),(110.0,1.0),(105.0,1.9),(90.0,1.9),(75.0,1.9),(70.0,1.0),(55.0,0.83),(0.0,0.89875)]
        .map(|(theta_deg,width_scale)| {
            let lower = (1.0-(theta_deg-90.0_f64).to_radians().cos())*0.5;
            ShankKey { theta_deg,width_scale,thickness_scale:1.0-0.135*(1.0-lower),..Default::default() }
        }).to_vec();
    d.cad = Some(cathedral_doc(&d).expect("curated cathedral document"));
    d
}

pub fn cathedral_doc(_d: &RingDesign) -> Result<Document> {
    let mut doc = band()?;
    let gem = Gem::calibrated(GemCut::Round, 6.5);
    let mut at = Placement::ring(90.0, builders::stand_off_mm("claw6", gem));
    if let Placement::Ring { spin_deg, .. } = &mut at { *spin_deg = 45.0; }
    doc.append(builders::stone_feature(2, gem, at))?;
    doc.append(builders::feature_on(3, "Six-claw head", builders::CLAW, 2, json!({"prongs":6,"wire_mm":1.0})))?;
    let mut arches = builders::cutters::cathedral_feature(4, 2, 3, Stage::Cast);
    if let Operation::Builder { params, .. } = &mut arches.operation {
        params["spread_deg"] = json!(30.0);
        params["rise"] = json!(0.7);
    }
    doc.append(arches)?;
    doc.append(builders::cutters::azure_feature(5, 2, Some(3), 4, Stage::Cast))?;
    doc.append(builders::feature_on(6, "Seat bur", builders::BUR, 2, json!({"through":true})))?;
    Ok(doc)
}

pub fn bezel_solitaire() -> RingDesign {
    let mut d = base(ProfileStyle::LowDome, 2.4, 1.7, false);
    d.cad = Some(bezel_doc(&d).expect("curated bezel document"));
    d
}

pub fn bezel_doc(_d: &RingDesign) -> Result<Document> {
    let mut doc = band()?;
    let gem = oval(5.0, 7.0);
    settings(&mut doc, 2, gem, Placement::ring(90.0, builders::stand_off_mm("head.bezel", gem)), "bezel", true)?;
    Ok(doc)
}

pub fn halo() -> RingDesign {
    let mut d = base(ProfileStyle::DShape, 2.2, 1.8, true);
    d.shank.kind = ShankKind::ReverseTaper;
    d.shank.amount = 0.2;
    d.cad = Some(halo_doc(&d).expect("curated halo document"));
    let ctx = d.field_context();
    let gem = Gem::calibrated(GemCut::Round, 1.2);
    let mut seat = SeatPadLayer { v_mm: ctx.crest_v_mm, style: SeatStyle::Boss, height_mm: 0.2, crown: 1.0, blend_mm: 0.3, solid: SolidKind::Bead, ..Default::default() };
    seat.fit_stone(gem);
    let mut run = SeatRunLayer { seat, gem, bridge_mm: 0.35, ..Default::default() };
    run.bridge_mm = 0.8;
    run.solve_spacing(&ctx);
    for (name, theta) in [("Shoulder pavé, right", 40.0), ("Shoulder pavé, left", 140.0)] {
        let mut e = LayerEntry::new(name, Layer::SeatRun(run));
        e.window = Window { fade_deg: 1.0, ..Window::around(theta, 32.0) };
        d.layers.layers.push(e);
    }
    d
}

pub fn halo_doc(_d: &RingDesign) -> Result<Document> {
    let mut doc = band()?;
    let gem = Gem::calibrated(GemCut::Cushion, 6.0);
    let mut at = Placement::ring(90.0, builders::stand_off_mm("claw4", gem));
    if let Placement::Ring { spin_deg, .. } = &mut at { *spin_deg = 45.0; }
    settings(&mut doc, 2, gem, at, "halo", false)?;
    if let Operation::Builder { params, .. } = &mut doc.features[2].operation { params["wire_mm"] = json!(1.0); }
    if let Operation::Builder { params, .. } = &mut doc.features[3].operation {
        *params = json!({"melee_mm":1.2,"gap_mm":0.95,"bridge_mm":0.8,"style":"Bezel"});
    }
    Ok(doc)
}

pub fn trilogy() -> RingDesign {
    let mut d = base(ProfileStyle::DShape, 2.6, 2.2, true);
    d.shank.kind = ShankKind::ReverseTaper;
    d.shank.amount = 0.3;
    d.cad = Some(trilogy_doc(&d).expect("curated trilogy document"));
    d
}

pub fn trilogy_doc(_d: &RingDesign) -> Result<Document> {
    let mut doc = band()?;
    let gem = Gem::calibrated(GemCut::Round, 5.5);
    let mut at = Placement::ring(90.0, builders::stand_off_mm("claw4", gem));
    if let Placement::Ring { spin_deg, .. } = &mut at { *spin_deg = 45.0; }
    settings(&mut doc, 2, gem, at, "claw4", false)?;
    if let Operation::Builder { params, .. } = &mut doc.features[2].operation { params["wire_mm"] = json!(1.0); }
    for (id, theta, spin) in [(5, 62.0, 90.0), (8, 118.0, -90.0)] {
        let gem = oval(3.5, 5.0);
        let mut at = Placement::ring(theta, builders::stand_off_mm("claw4", gem));
        if let Placement::Ring { spin_deg, .. } = &mut at { *spin_deg = spin; }
        settings(&mut doc, id, gem, at, "claw4", false)?;
        let head = &mut doc.features[id as usize];
        head.name = "Three-claw head".into();
        if let Operation::Builder { params, .. } = &mut head.operation { params["prongs"] = json!(3); params["wire_mm"] = json!(1.0); }
    }
    Ok(doc)
}

pub fn toi_et_moi() -> RingDesign {
    let mut d = base(ProfileStyle::LowDome, 3.0, 2.2, true);
    d.shank.kind = ShankKind::Bypass;
    d.shank.amount = 1.0;
    d.cad = Some(toi_et_moi_doc(&d).expect("curated bypass document"));
    d
}

pub fn toi_et_moi_doc(_d: &RingDesign) -> Result<Document> {
    let mut doc = band()?;
    for (id, theta, across, gem) in [(2, 107.0, 0.7, oval(5.0, 7.0)), (5, 73.0, -0.7, oval(4.0, 6.0))] {
        let mut at = Placement::ring(theta, builders::stand_off_mm("claw4", gem));
        if let Placement::Ring { across_mm, spin_deg, .. } = &mut at { *across_mm = across; *spin_deg = 90.0; }
        settings(&mut doc, id, gem, at, "claw4", false)?;
    }
    Ok(doc)
}

fn split_base() -> RingDesign {
    let mut d = base(ProfileStyle::LowDome, 2.4, 1.8, true);
    d.profile.comfort_fit_mm = 0.1;
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = [(270.0,1.0,1.0),(200.0,1.0,1.0),(340.0,1.0,1.0),(150.0,1.35,1.0),(30.0,1.35,1.0),(125.0,2.0,1.05),(55.0,2.0,1.05),(90.0,2.75,1.1)]
        .map(|(theta_deg,width_scale,thickness_scale)| ShankKey { theta_deg, width_scale, thickness_scale, ..Default::default() }).to_vec();
    d
}

pub fn split_shank() -> RingDesign {
    let mut d = split_base();
    d.cad = Some(split_doc(&d).expect("curated split document"));
    d
}

pub fn split_doc(_d: &RingDesign) -> Result<Document> {
    let mut doc = band()?;
    doc.append(feature(2, "Open split", Operation::Builder {
        key: builders::SPLIT.into(), on: None,
        params: json!({"theta_deg":90.0,"spread_deg":56.0,"gap_mm":3.2,"rail_round_mm":0.3,"tip":"Point"}),
    }, Attach::Cut))?;
    Ok(doc)
}

pub fn split_shank_basket() -> RingDesign {
    let mut d = split_base();
    d.cad = Some(split_basket_doc(&d).expect("curated split basket document"));
    d
}

pub fn split_basket_doc(d: &RingDesign) -> Result<Document> {
    let mut doc = split_doc(d)?;
    let gem = oval(6.0, 8.0);
    doc.append(builders::stone_feature(3, gem, Placement::ring(90.0, builders::stand_off_mm("basket", gem))))?;
    doc.append(builders::feature_on(4, "Basket", builders::BASKET, 3, json!({"prongs":4,"rails":2})))?;
    doc.append(builders::feature_on(5, "Seat bur", builders::BUR, 3, json!({"through":true})))?;
    Ok(doc)
}

pub fn split_gallery() -> RingDesign {
    let mut d = base(ProfileStyle::LowDome, 4.2, 1.8, false);
    d.profile.crown_mm = 0.2;
    d.profile.edge_round_mm = 0.15;
    d.profile.comfort_fit_mm = 0.1;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = [(270.0,1.0),(200.0,1.0),(340.0,1.0),(150.0,1.6),(30.0,1.6),(120.0,2.24),(60.0,2.24),(90.0,2.67)].map(|(theta_deg,thickness_scale)| ShankKey { theta_deg, thickness_scale, ..Default::default() }).to_vec();
    d.cad = Some(gallery_doc(&d).expect("curated gallery document"));
    d
}

pub fn gallery_doc(_d: &RingDesign) -> Result<Document> {
    let mut doc = band()?;
    doc.append(feature(2, "Gallery window", Operation::Builder {
        key: builders::WINDOW.into(), on: None,
        params: json!({"from_deg":38.0,"to_deg":142.0,"rail_in_mm":1.2,"rail_out_mm":1.5,"draft_deg":4.0,"tip_round_mm":0.35}),
    }, Attach::Cut))?;
    Ok(doc)
}

pub fn half_eternity() -> RingDesign {
    let mut d = base(ProfileStyle::LowDome, 3.0, 2.1, false);
    let ctx = d.field_context();
    let gem = Gem::calibrated(GemCut::Round, 1.8);
    let mut seat = SeatPadLayer { v_mm: ctx.crest_v_mm, style: SeatStyle::Boss, height_mm: 0.2, crown: 1.0, blend_mm: 0.4, solid: SolidKind::Bead, mark_mm: 0.7, ..Default::default() };
    seat.fit_stone(gem);
    let mut run = SeatRunLayer { seat, gem, count: 13, bridge_mm: 0.5, ..Default::default() };
    run.solve_spacing(&ctx);
    run.seat.diameter_mm = 2.7;
    let mut e = LayerEntry::new("Bead-set half row", Layer::SeatRun(run));
    e.window = Window { fade_deg: 1.0, ..Window::around(90.0,160.0) };
    d.layers.layers.push(e);
    d
}

pub fn gypsy_trio() -> RingDesign {
    let mut d = base(ProfileStyle::LowDome, 6.5, 2.4, false);
    let ctx = d.field_context();
    for (name,theta,width,height,blend,mark) in [("Centre",90.0,4.0,0.55,0.6,0.8),("Right",62.0,2.5,0.4,0.5,0.6),("Left",118.0,2.5,0.4,0.5,0.6)] {
        let mut seat = SeatPadLayer { theta_deg: theta, v_mm: ctx.crest_v_mm, style: SeatStyle::GypsyMound, crown: 1.0, solid: SolidKind::Flush, through: true, height_mm: height, blend_mm: blend, mark_mm: mark, ..Default::default() };
        seat.fit_stone(Gem::calibrated(GemCut::Round,width));
        d.layers.layers.push(LayerEntry::new(name,Layer::SeatPad(seat)));
    }
    d
}

/// Radial stock inside and outside the gallery window at one offset across the band, sampled from a mesh section.
#[derive(Debug, serde::Serialize)]
pub struct GallerySection {
    pub across_mm: f64,
    pub complete_sections: usize,
    pub missing_sections: usize,
    pub inner_min_mm: Option<f64>,
    pub outer_min_mm: Option<f64>,
}

pub fn gallery_rail_sections(mesh: &crate::Mesh, width_mm: f64) -> Vec<GallerySection> {
    [-0.96,-0.9,-0.75,-0.5,-0.25,0.0,0.25,0.5,0.75,0.9,0.96].into_iter().map(|fraction| {
        let across_mm = if fraction == 0.0 { 0.001 } else { width_mm * 0.5 * fraction };
        let lines = crate::cad::measure::section(mesh,2,across_mm);
        let mut row = GallerySection { across_mm,complete_sections:0,missing_sections:0,inner_min_mm:None,outer_min_mm:None };
        // Ray angles sit 0.137° off the tessellation's vertex columns.
        for theta in (39..=141).step_by(3).map(|t|t as f64+0.137).chain([38.137,141.863]) {
            let (sin,cos) = theta.to_radians().sin_cos();
            let mut hits: Vec<f64> = lines.iter().filter_map(|[a,b]| {
                let delta = [b[0]-a[0],b[1]-a[1]];
                let den = cos*delta[1]-sin*delta[0];
                if den.abs()<1e-10 { return None; }
                let radius = (a[0]*delta[1]-a[1]*delta[0])/den;
                let at = (a[0]*sin-a[1]*cos)/den;
                (radius>0.0 && (0.0..=1.0).contains(&at)).then_some(radius)
            }).collect();
            hits.sort_by(f64::total_cmp);
            hits.dedup_by(|a,b|(*a-*b).abs()<1e-4);
            if hits.len()==4 {
                row.complete_sections+=1;
                let inner=hits[1]-hits[0];
                let outer=hits[3]-hits[2];
                row.inner_min_mm=Some(row.inner_min_mm.map_or(inner,|v|v.min(inner)));
                row.outer_min_mm=Some(row.outer_min_mm.map_or(outer,|v|v.min(outer)));
            } else { row.missing_sections+=1; }
        }
        row
    }).collect()
}
