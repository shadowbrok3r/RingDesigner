//! Read-only, throttled hover picking shared by mouse and hovering pen users.
use egui::{Color32, Pos2, Rect, Response, Ui};
use ringdesign_core::{
    AlphaLibrary, RingDesign,
    interaction::{
        pick::{Filter, Pick, PickScene, Ray, ViewScale},
        picking::{self, Hit},
    },
    mesh::Mesh,
};

#[derive(Clone, Default)]
struct Cached {
    ray: Option<([f32; 3], [f32; 3])>,
    mesh: usize,
    at: f64,
    hit: Option<Hit>,
}

/// The resolver must return a name only for a feature the click handler can edit.
pub fn show(
    ui: &Ui,
    rect: Rect,
    response: &Response,
    design: &RingDesign,
    lib: &AlphaLibrary,
    mesh: &Mesh,
    ray: impl Fn(Pos2) -> ([f32; 3], [f32; 3]),
    project: impl Fn([f32; 3]) -> Pos2,
    name: impl Fn(&Hit) -> Option<String>,
) -> Option<Hit> {
    let id = response.id.with("select-hover");
    let pos = ui.input(|i| {
        (!i.pointer.any_down())
            .then(|| i.pointer.hover_pos())
            .flatten()
    });
    let Some(pos) = pos.filter(|p| response.hovered() && rect.contains(*p)) else {
        ui.data_mut(|d| d.remove::<Cached>(id));
        return None;
    };
    let now = ui.input(|i| i.time);
    let ray = ray(pos);
    let mesh_id = mesh.vertices.as_ptr() as usize;
    let mut cache = ui.data(|d| d.get_temp::<Cached>(id)).unwrap_or_default();
    if cache.ray != Some(ray) || cache.mesh != mesh_id {
        if now - cache.at < 0.035 && cache.ray.is_some() && cache.mesh == mesh_id {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(35));
            // Retain the feature between samples. Clearing it every throttled
            // frame made both the label and the graph highlight blink.
        } else {
            cache = Cached {
                ray: Some(ray),
                mesh: mesh_id,
                at: now,
                hit: picking::hit(design, lib, mesh, ray.0, ray.1),
            };
            ui.data_mut(|d| d.insert_temp(id, cache.clone()));
        }
    }
    let hit = cache.hit?;
    let label = name(&hit)?;
    let painter = ui.painter_at(rect);
    // The triangle only locates an editable feature; it is not an editing
    // target itself. Keep the contact cue under the pointer between samples.
    let _ = project;
    painter.circle_stroke(pos, 7., egui::Stroke::new(1.5, AQUA));
    caption(&painter, rect, &format!("Edit {label}"), AQUA);
    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    Some(hit)
}

/// The hover cue's colour.
pub const AQUA: Color32 = Color32::from_rgb(103, 217, 213);

/// A fixed viewport caption that doesn't chase every polygon under the mouse.
pub fn caption(painter: &egui::Painter, rect: Rect, text: &str, color: Color32) {
    let text = painter.layout_no_wrap(text.to_owned(), egui::FontId::proportional(12.), color);
    let origin = egui::pos2(rect.left() + 16., rect.bottom() - text.size().y - 38.);
    let plate = Rect::from_min_size(origin, text.size()).expand(5.);
    painter.rect_filled(plate, 4., Color32::from_rgb(22, 20, 29));
    painter.rect_stroke(plate, 4., egui::Stroke::new(1., color), egui::StrokeKind::Inside);
    painter.galley(origin, text, color);
}

#[derive(Clone, Default)]
struct CachedPicks {
    ray: Option<([f64; 3], [f64; 3])>,
    scene: usize,
    at: f64,
    picks: Vec<Pick>,
}

/// The pick stack under the pointer, sampled at most every 35 ms and held between samples; `None`
/// while the pointer is off the rect or a button is down.
pub fn picks(
    ui: &Ui,
    rect: Rect,
    response: &Response,
    scene: &PickScene,
    ray: impl Fn(Pos2) -> ([f32; 3], [f32; 3]),
    aperture_px: f32,
    filter: Filter,
) -> Option<Vec<Pick>> {
    let id = response.id.with("scene-hover");
    let pos = ui.input(|i| (!i.pointer.any_down()).then(|| i.pointer.hover_pos()).flatten());
    let Some(pos) = pos.filter(|p| response.hovered() && rect.contains(*p)) else {
        ui.data_mut(|d| d.remove::<CachedPicks>(id));
        return None;
    };
    let now = ui.input(|i| i.time);
    let (o, d) = ray(pos);
    let key = (o.map(f64::from), d.map(f64::from));
    let scene_id = scene as *const PickScene as usize;
    let mut cache = ui.data(|d| d.get_temp::<CachedPicks>(id)).unwrap_or_default();
    if cache.ray != Some(key) || cache.scene != scene_id {
        if now - cache.at < 0.035 && cache.ray.is_some() && cache.scene == scene_id {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(35));
        } else {
            cache = CachedPicks { ray: Some(key), scene: scene_id, at: now, picks: pick_at(scene, pos, &ray, aperture_px, filter) };
            ui.data_mut(|d| d.insert_temp(id, cache.clone()));
        }
    }
    Some(cache.picks)
}

/// One unthrottled pick through the scene at a screen position.
pub fn pick_at(scene: &PickScene, pos: Pos2, ray: &impl Fn(Pos2) -> ([f32; 3], [f32; 3]), aperture_px: f32, filter: Filter) -> Vec<Pick> {
    let (view, r) = view_scale(pos, ray);
    scene.pick(r, &view, aperture_px, filter)
}

/// The ray under `pos` and the screen's axes and scale there, read off the rays one pixel to
/// the right and one down, so an orthographic camera needs no second interface.
pub fn view_scale(pos: Pos2, ray: &impl Fn(Pos2) -> ([f32; 3], [f32; 3])) -> (ViewScale, Ray) {
    let (o, d) = ray(pos);
    let (ox, _) = ray(pos + egui::vec2(1.0, 0.0));
    let (oy, _) = ray(pos + egui::vec2(0.0, 1.0));
    let right: [f64; 3] = std::array::from_fn(|k| (ox[k] - o[k]) as f64);
    let down: [f64; 3] = std::array::from_fn(|k| (oy[k] - o[k]) as f64);
    let len = |v: [f64; 3]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
    let mm_per_px = (len(right) + len(down)) * 0.5;
    let unit = |v: [f64; 3]| {
        let l = len(v);
        if l > 0.0 { v.map(|x| x / l) } else { v }
    };
    let view = ViewScale { right: unit(right), up: unit(down.map(|v| -v)), px_per_mm: if mm_per_px > 0.0 { 1.0 / mm_per_px } else { 0.0 } };
    (view, Ray { origin: o.map(f64::from), direction: d.map(f64::from) })
}
