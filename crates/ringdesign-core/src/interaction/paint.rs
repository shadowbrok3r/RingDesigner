use super::picking::Hit;
use crate::{
    AlphaLibrary, RingDesign,
    drawn::{DrawnAlpha, Stroke},
    field::{Blend, Decal, DecalLayer, Layer, LayerEntry},
    tiling::TilingLayer,
};

pub const RELIEF: &str = "3D painted relief";
pub const ENGRAVING: &str = "3D engraved currents";

#[derive(Clone, Debug)]
pub struct Brush {
    pub diameter_mm: f64,
    pub depth_mm: f64,
    pub soft: f32,
    pub engrave: bool,
    pub stamp: String,
    pub rotation_deg: f64,
}
impl Default for Brush {
    fn default() -> Self {
        Self {
            diameter_mm: 0.65,
            depth_mm: 0.22,
            soft: 0.55,
            engrave: false,
            stamp: String::new(),
            rotation_deg: 0.0,
        }
    }
}
impl Brush {
    pub fn depth_at(&self, d: &RingDesign, hit: &Hit, pressure: f32) -> f64 {
        let mut depth = self.depth_mm.clamp(0.01, 1.6) * pressure.clamp(0.0, 1.0) as f64;
        if d.draft.process == crate::castability::CastProcess::SandTwoPart {
            depth = depth.min(crate::paint::ceiling_mm(&d.field_context(), hit.v_mm));
        }
        depth
    }
}

/// Unwrapped x coordinates keep seam-crossing segments local. A miss ends a
/// segment; a single gesture may therefore contain several independent strokes.
#[derive(Default)]
pub struct Gesture {
    pub strokes: Vec<Stroke>,
    connected: bool,
    pub world: Vec<Option<[f32; 3]>>,
    pub actual_mm: f64,
}
impl Gesture {
    pub fn miss(&mut self) {
        self.connected = false;
        if self.world.last().is_some_and(Option::is_some) {
            self.world.push(None);
        }
    }
    pub fn push(
        &mut self,
        d: &RingDesign,
        brush: &Brush,
        hit: &Hit,
        pressure: f32,
        tilt: [f32; 2],
    ) {
        if hit.radial_wall_mm < 0.05 || !pressure.is_finite() {
            self.miss();
            return;
        }
        let ctx = d.field_context();
        let mm = brush.depth_at(d, hit, pressure);
        self.actual_mm = mm;
        let radius =
            (brush.diameter_mm.clamp(0.1, 12.0) * 0.5 / ctx.circumference_mm.max(1.0)) as f32;
        let mut x = (hit.theta_deg / 360.0).rem_euclid(1.0) as f32;
        let y = (hit.v_mm / ctx.band_v_len_mm.max(1e-9)).clamp(0.0, 1.0) as f32;
        if !self.connected {
            if self.strokes.len() >= 256 {
                return;
            }
            self.strokes.push(Stroke::new(radius, brush.soft, false));
            self.connected = true;
        }
        let s = self.strokes.last_mut().unwrap();
        if let Some(last) = s.points.last() {
            x = last[0] + (x - last[0] + 0.5).rem_euclid(1.0) - 0.5;
        }
        let count = s.points.len();
        s.push_held(x, y, (mm / 1.6) as f32, tilt[0], tilt[1]);
        if s.points.len() > count && self.world.len() < 8192 {
            self.world.push(Some(hit.world));
        }
    }
    pub fn commit(&mut self, d: &mut RingDesign, brush: &Brush) -> Option<usize> {
        self.strokes.retain(|s| !s.is_empty());
        if self.strokes.is_empty() || d.graph.is_some() || super::surface::replaces_band(d) {
            *self = Self::default();
            return None;
        }
        let name = if brush.engrave { ENGRAVING } else { RELIEF };
        let ctx = d.field_context();
        let index = match d.drawn.iter().position(|a| a.name == name) {
            Some(i) => i,
            None => {
                let mut a = DrawnAlpha::new(
                    name,
                    2048,
                    (2048.0 * ctx.band_v_len_mm / ctx.circumference_mm)
                        .round()
                        .clamp(8.0, 4096.0) as u32,
                );
                a.wrap_x = true;
                d.drawn.push(a);
                d.drawn.len() - 1
            }
        };
        if d.drawn[index].strokes.len() + self.strokes.len() > crate::drawn::MAX_STROKES {
            *self = Self::default();
            return None;
        }
        if !d
            .layers
            .layers
            .iter()
            .any(|e| matches!(&e.layer,Layer::Tiling(t) if t.alpha==name))
        {
            let mut tile = TilingLayer::default_for(name, &ctx);
            tile.repeats_around = 1;
            tile.rows = 1;
            tile.height_mm = 1.6;
            tile.v_center_mm = ctx.band_v_len_mm * 0.5;
            tile.v_span_mm = ctx.band_v_len_mm;
            tile.feather_mm = 0.0;
            let mut entry = LayerEntry::new(name, Layer::Tiling(tile));
            entry.blend = if brush.engrave {
                Blend::Subtract
            } else {
                Blend::Add
            };
            d.layers.layers.push(entry);
        }
        d.drawn[index].strokes.append(&mut self.strokes);
        *self = Self::default();
        Some(index)
    }
}

/// Existing decals retain their millimetre placement and embedded source alpha.
pub fn stamp(d: &mut RingDesign, lib: &AlphaLibrary, brush: &Brush, hit: &Hit) -> Option<usize> {
    stamp_pattern(d, lib, brush, hit, super::surface::Arrangement::default())
}

pub fn stamp_pattern(d: &mut RingDesign, lib: &AlphaLibrary, brush: &Brush, hit: &Hit, arrangement: super::surface::Arrangement) -> Option<usize> {
    if d.graph.is_some()
        || super::surface::replaces_band(d)
        || !hit.theta_deg.is_finite() || !hit.v_mm.is_finite()
        || !brush.diameter_mm.is_finite() || !brush.depth_mm.is_finite() || !brush.rotation_deg.is_finite()
        || hit.radial_wall_mm < 0.05
        || lib.get(&brush.stamp).is_none()
    {
        return None;
    }
    let placements = arrangement.placements(hit.theta_deg, hit.v_mm, d.field_context().band_v_len_mm);
    if placements.is_empty() { return None; }
    let decals = placements.into_iter().map(|(theta_deg,v_mm,flip)| {
        let mut local = hit.clone(); local.theta_deg=theta_deg; local.v_mm=v_mm;
        Decal { theta_deg, v_mm, size_mm: brush.diameter_mm.clamp(0.1,12.0),
            // Decal::flip reflects its local X axis. Reflecting across the
            // band's V axis also needs a half turn, otherwise letters and
            // asymmetric leaves face the wrong way on the opposite cheek.
            rotation_deg: if flip { 180.0 - brush.rotation_deg } else { brush.rotation_deg },
            height_mm: brush.depth_at(d,&local,1.0), flip }
    }).collect();
    let mut entry = LayerEntry::new(
        format!("Stamped {}", brush.stamp),
        Layer::Decals(DecalLayer {
            alpha: brush.stamp.clone(),
            decals,
            feather_mm: (brush.diameter_mm * 0.06).clamp(0.05, 0.4),
            invert: false,
        }),
    );
    entry.blend = if brush.engrave {
        Blend::Subtract
    } else {
        Blend::Add
    };
    let index = d.layers.layers.len();
    d.layers.layers.push(entry);
    // Repack referenced artwork only; built-in and drawn alphas stay procedural.
    d.embed_alphas(lib);
    Some(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mirrored_stamp_reflects_asymmetric_artwork_and_respects_the_field_budget() {
        let mut d=RingDesign::default();d.draft.process=crate::castability::CastProcess::LostWax;
        let mut lib=AlphaLibrary::default();lib.insert(crate::Alpha::new("asymmetric",2,2,vec![0.0,0.3,0.7,1.0]));
        let ctx=d.field_context();let v=ctx.band_v_len_mm*0.25;
        let brush=Brush {stamp:"asymmetric".into(),diameter_mm:1.0,rotation_deg:35.0,..Default::default()};
        let index=stamp_pattern(&mut d,&lib,&brush,&hit(90.0,v),super::super::surface::Arrangement {mirror:true,..Default::default()}).unwrap();
        let Layer::Decals(layer)=&d.layers.layers[index].layer else {panic!()};
        for du in [-0.2,0.0,0.2] {for dv in [-0.2,0.0,0.2] {
            let a=layer.height(crate::field::Uv {u:ctx.u_of_theta(90.0)+du,v:v+dv},&ctx,&lib);
            let b=layer.height(crate::field::Uv {u:ctx.u_of_theta(90.0)+du,v:ctx.band_v_len_mm-v-dv},&ctx,&lib);
            assert!((a-b).abs()<1e-6,"opposite cheek is not reflected: {a} vs {b}");
        }}
        let index=stamp_pattern(&mut d,&lib,&brush,&hit(0.0,v),super::super::surface::Arrangement {count:u32::MAX,mirror:true,..Default::default()}).unwrap();
        let Layer::Decals(layer)=&d.layers.layers[index].layer else {panic!()};
        assert_eq!(layer.decals.len(),crate::field::MAX_DECALS);
    }
    fn hit(theta: f64, v: f64) -> Hit {
        Hit {
            ray: ([0.0; 3], [0.0; 3]),
            world: [0.0; 3],
            face: 0,
            theta_deg: theta,
            v_mm: v,
            radial_wall_mm: 2.0,
            relief_mm: 0.0,
        }
    }
    #[test]
    fn seam_stroke_paints_only_the_seam_and_commits_once() {
        let mut d = RingDesign::default();
        d.draft.process = crate::castability::CastProcess::LostWax;
        let v = d.field_context().crest_v_mm;
        let b = Brush::default();
        let mut g = Gesture::default();
        g.push(&d, &b, &hit(359.0, v), 1.0, [0.0; 2]);
        g.push(&d, &b, &hit(1.0, v), 1.0, [0.0; 2]);
        assert!((g.strokes[0].points[1][0] - g.strokes[0].points[0][0]).abs() < 0.01);
        let i = g.commit(&mut d, &b).unwrap();
        let a = d.drawn[i].rasterize();
        let y = v / d.field_context().band_v_len_mm;
        assert!(a.sample_wrapped(0.0, y) > 0.01);
        assert_eq!(a.sample(0.5, y), 0.0);
        assert!(g.commit(&mut d, &b).is_none());
    }
    #[test]
    fn empty_space_separates_strokes_and_bore_is_not_painted() {
        let d = RingDesign::default();
        let b = Brush::default();
        let v = d.field_context().crest_v_mm;
        let mut g = Gesture::default();
        g.push(&d, &b, &hit(10.0, v), 1.0, [0.0; 2]);
        g.miss();
        g.push(&d, &b, &hit(30.0, v), 1.0, [0.0; 2]);
        assert_eq!(g.strokes.len(), 2);
        let mut bore = hit(90.0, v);
        bore.radial_wall_mm = 0.0;
        g.push(&d, &b, &bore, 1.0, [0.0; 2]);
        assert_eq!(g.strokes.len(), 2);
    }
}
