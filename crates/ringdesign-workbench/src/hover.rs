//! Read-only, throttled hover picking shared by mouse and hovering pen users.
use egui::{Color32, Pos2, Rect, Response, Ui};
use ringdesign_core::{
    AlphaLibrary, RingDesign,
    interaction::picking::{self, Hit},
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
    let aqua = Color32::from_rgb(103, 217, 213);
    let painter = ui.painter_at(rect);
    // The triangle only locates an editable feature; it is not an editing
    // target itself. Keep the contact cue under the pointer between samples.
    let _ = project;
    painter.circle_stroke(pos, 7., egui::Stroke::new(1.5, aqua));
    let text = painter.layout_no_wrap(
        format!("Edit {label}"),
        egui::FontId::proportional(12.),
        aqua,
    );
    // A fixed viewport caption doesn't chase every polygon under the mouse.
    let origin = egui::pos2(rect.left() + 16., rect.bottom() - text.size().y - 38.);
    let plate = Rect::from_min_size(origin, text.size()).expand(5.);
    painter.rect_filled(plate, 4., Color32::from_rgb(22, 20, 29));
    painter.rect_stroke(
        plate,
        4.,
        egui::Stroke::new(1., aqua),
        egui::StrokeKind::Inside,
    );
    painter.galley(origin, text, aqua);
    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    Some(hit)
}
