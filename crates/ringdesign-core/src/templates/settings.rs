//! Editable documents and stone stock for the starter gallery.
use crate::{ProfileStyle, RingDesign};
use crate::cad::{Attach, Component, Document, FaceRef, Feature, MirrorPlane, Operation, PatternKind, Placement, PlaneBase, Profile, Stage, builders};
use crate::castability::{CastProcess, SandProcess};
use crate::field::{Layer, LayerEntry, SeatPadLayer, SeatRunLayer, SeatStyle, Window};
use crate::gem::{Gem, GemCut};
use crate::profile::{ShankKey, ShankKind};
use crate::setting::SolidKind;
use crate::sketch::{FaceAnchor, Geometry, Sketch, Workplane};
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
    d.shank.kind = ShankKind::ReverseTaper;
    d.shank.amount = 0.45;
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
        if let Operation::Builder { params, .. } = &mut doc.features[(id) as usize].operation { params["prongs"] = json!(3); params["wire_mm"] = json!(1.0); }
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
    let sections = [(42.0_f64,0.025),(58.0,0.55),(74.0,1.05),(90.0,1.3),(106.0,1.05),(122.0,0.55),(138.0,0.025)].into_iter().map(|(theta, gap)| {
        let mut s = Sketch::rectangle(3.6, gap * 2.0);
        for p in &mut s.points { p.xy[0] += 9.65; }
        let t = theta.to_radians();
        s.plane = Workplane { x: [t.cos(),t.sin(),0.0], y: [0.0,0.0,1.0], ..Default::default() };
        Profile::Inline(s)
    }).collect();
    let mut cut = feature(2, "Open split", Operation::Loft { sections }, Attach::Cut);
    cut.component.blend_mm = 0.25;
    doc.append(cut)?;
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
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = [(270.0,1.0),(200.0,1.0),(340.0,1.0),(150.0,1.35),(30.0,1.35),(120.0,1.85),(60.0,1.85),(90.0,2.3)].map(|(theta_deg,thickness_scale)| ShankKey { theta_deg, thickness_scale, ..Default::default() }).to_vec();
    d.cad = Some(gallery_doc(&d).expect("curated gallery document"));
    d
}

pub fn gallery_doc(_d: &RingDesign) -> Result<Document> {
    let mut doc = band()?;
    doc.append(feature(2, "Window parting plane", Operation::Plane { base: PlaneBase::Parting, offset_mm: -0.05 }, Attach::Separate))?;
    let mut s = Sketch { name: "Rounded gallery window".into(), ..Default::default() };
    s.plane.on_face = Some(FaceAnchor { feature: 2, face: FaceRef::bare(0) });
    let inner: f64 = 9.65;
    let tip_y = inner * 38_f64.to_radians().sin();
    let high: f64 = 11.75;
    let cy = (high * high - inner * inner) / (2.0 * (high - tip_y));
    let outer = high - cy;
    let r: f64 = 0.35;
    let fillet_y = ((inner + r).powi(2) - (outer - r).powi(2) + cy * cy) / (2.0 * cy);
    let fillet_x = ((inner + r).powi(2) - fillet_y * fillet_y).sqrt();
    let ci = s.point([0.0,0.0]);
    let co = s.point([0.0,cy]);
    let mut ends = Vec::new();
    for sign in [1.0,-1.0] {
        let centre = [sign * fillet_x,fillet_y];
        let pi = [centre[0] * inner / (inner+r),centre[1] * inner / (inner+r)];
        let po = [centre[0] * outer / (outer-r),cy+(centre[1]-cy)*outer/(outer-r)];
        let c = s.point(centre);
        let i = s.point(pi);
        let o = s.point(po);
        let ai = (pi[1]-centre[1]).atan2(pi[0]-centre[0]);
        let ao = (po[1]-centre[1]).atan2(po[0]-centre[0]);
        let (start,end) = if (ao-ai).rem_euclid(std::f64::consts::TAU) < std::f64::consts::PI { (i,o) } else { (o,i) };
        s.entity(Geometry::Arc { center: c, start, end });
        ends.push((i,o));
    }
    s.entity(Geometry::Arc { center: ci, start: ends[0].0, end: ends[1].0 });
    s.entity(Geometry::Arc { center: co, start: ends[0].1, end: ends[1].1 });
    doc.append(feature(3, "Window", Operation::Sketch { sketch: s }, Attach::Separate))?;
    doc.append(feature(4, "Cope window", Operation::Extrude { sketch: Profile::Feature { feature: 3 }, height_mm: 2.65, draft_deg: -2.0 }, Attach::Cut))?;
    doc.append(feature(5, "Drag window", Operation::Pattern { sources: 4.into(), kind: PatternKind::Mirror { plane: MirrorPlane::Band } }, Attach::Cut))?;
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
