//! Actual alpha thumbnails and surface-conforming placement previews.
use egui::{Color32, TextureHandle, TextureId, Ui, Vec2};
use ringdesign_core::{Alpha, AlphaLibrary, RingDesign, field::Decal};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};

#[derive(Clone, Default)]
struct Textures(HashMap<u64, TextureHandle>);

pub fn texture(ui: &Ui, alpha: &Alpha, overlay: bool, invert: bool, feather: f64) -> TextureId {
    let (w, h, mut bytes) = alpha.thumbnail_rgba8(if overlay { 128 } else { 64 });
    if overlay {
        for y in 0..h {
            for x in 0..w {
                let k = (y * w + x) * 4;
                let mut a = f64::from(bytes[k]) / 255.0;
                if invert {
                    a = 1.0 - a;
                }
                let edge = ((x as f64 + 0.5) / w as f64)
                    .min(1.0 - (x as f64 + 0.5) / w as f64)
                    .min(
                        ((y as f64 + 0.5) / h as f64).min(1.0 - (y as f64 + 0.5) / h as f64)
                            * h as f64
                            / w as f64,
                    );
                let opacity = (a * (edge / feather.max(1e-6)).clamp(0.0, 1.0) * 185.0) as u8;
                bytes[k..k + 4].copy_from_slice(&[43, 226, 214, opacity]);
            }
        }
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (w, h, &bytes).hash(&mut hash);
    let key = hash.finish();
    let id = egui::Id::new("atelier-alpha-thumbnails");
    if let Some(tex) = ui.data(|d| {
        d.get_temp::<Textures>(id)
            .and_then(|c| c.0.get(&key).cloned())
    }) {
        return tex.id();
    }
    let tex = ui.ctx().load_texture(
        format!("alpha-{key}"),
        egui::ColorImage::from_rgba_unmultiplied([w, h], &bytes),
        egui::TextureOptions::LINEAR,
    );
    let tid = tex.id();
    ui.data_mut(|d| {
        let cache = d.get_temp_mut_or_default::<Textures>(id);
        if cache.0.len() >= 192 {
            cache.0.clear();
        }
        cache.0.insert(key, tex);
    });
    tid
}

fn thumbnail_size(a: &Alpha, side: f32) -> Vec2 {
    let longest = a.width.max(a.height).max(1) as f32;
    egui::vec2(
        side * a.width as f32 / longest,
        side * a.height as f32 / longest,
    )
}

pub fn picker(
    ui: &mut Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    selected: &mut String,
    lib: &AlphaLibrary,
) -> bool {
    let id = ui.make_persistent_id(salt);
    let image = lib
        .get(selected)
        .map(|a| egui::Image::new((texture(ui, a, false, false, 0.0), thumbnail_size(a, 30.0))));
    let response = ui.add_sized(
        [ui.available_width(), 36.0],
        egui::Button::opt_image_and_text(
            image,
            Some(egui::WidgetText::from(if selected.is_empty() {
                "Choose pattern"
            } else {
                selected.as_str()
            })),
        )
        .truncate(),
    );
    let held = crate::icons::help(
        &response,
        "Choose a pattern",
        "Browse the actual relief images.",
        "Tap a thumbnail; use search to narrow the library.",
        "Light pixels produce more relief. SVG artwork uses its rasterized alpha here.",
    );
    let safe = ui
        .ctx()
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("mobile-safe-content")))
        .unwrap_or_else(|| ui.ctx().content_rect())
        .shrink(6.0);
    let width = 260.0_f32.min(safe.width().max(40.0));
    let height = 300.0_f32.min((safe.height() - 16.0).max(40.0));
    let at = egui::pos2(
        response
            .rect
            .left()
            .clamp(safe.left(), (safe.right() - width).max(safe.left())),
        if safe.bottom() - response.rect.bottom() >= height {
            response.rect.bottom() + 3.0
        } else {
            (response.rect.top() - height - 8.0).max(safe.top())
        },
    );
    let mut changed = false;
    egui::Popup::menu(&response)
        .open_memory(if response.clicked() && !held {
            Some(egui::SetOpenCommand::Toggle)
        } else {
            None
        })
        .id(id)
        .at_position(at)
        .align(egui::RectAlign::BOTTOM_START)
        .align_alternatives(&[])
        .width(width)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_max_width(width);
            let mut search = ui
                .data(|d| d.get_temp::<String>(id.with("search")))
                .unwrap_or_default();
            ui.add(
                egui::TextEdit::singleline(&mut search)
                    .hint_text("Find a pattern")
                    .desired_width(ui.available_width()),
            );
            ui.data_mut(|d| d.insert_temp(id.with("search"), search.clone()));
            let filter = search.to_lowercase();
            let found: Vec<_> = lib
                .iter()
                .filter(|a| !a.is_empty() && a.name.to_lowercase().contains(&filter))
                .collect();
            if found.is_empty() {
                ui.weak("No matching patterns. Clear the search to browse.");
            }
            let cols = (ui.available_width() / 76.0).floor().max(1.0) as usize;
            let cell = (ui.available_width() / cols as f32 - ui.spacing().item_spacing.x).max(36.0);
            egui::ScrollArea::vertical()
                .max_height((height - 48.0).max(1.0))
                .min_scrolled_height(1.0)
                .show_rows(ui, 78.0, found.len().div_ceil(cols), |ui, range| {
                    for row in range {
                        ui.horizontal(|ui| {
                            for a in found.iter().skip(row * cols).take(cols) {
                                ui.push_id(&a.name, |ui| {
                                    let (rect, r) = ui.allocate_exact_size(
                                        egui::vec2(cell, 74.0),
                                        egui::Sense::click(),
                                    );
                                    let chosen = *selected == a.name;
                                    ui.painter().rect_filled(
                                        rect,
                                        4.0,
                                        Color32::from_rgb(24, 23, 31),
                                    );
                                    let side = (cell - 8.0).min(51.0);
                                    let image_rect = egui::Rect::from_center_size(
                                        rect.center_top() + egui::vec2(0.0, side / 2.0 + 4.0),
                                        thumbnail_size(a, side),
                                    );
                                    ui.painter().image(
                                        texture(ui, a, false, false, 0.0),
                                        image_rect,
                                        egui::Rect::from_min_max(
                                            egui::Pos2::ZERO,
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        Color32::WHITE,
                                    );
                                    let label = ui.painter().layout(
                                        a.name.clone(),
                                        egui::FontId::proportional(10.0),
                                        Color32::from_gray(235),
                                        cell - 4.0,
                                    );
                                    ui.painter().with_clip_rect(rect).galley(
                                        rect.left_top() + egui::vec2(3.0, side + 6.0),
                                        label,
                                        Color32::WHITE,
                                    );
                                    if chosen || r.hovered() {
                                        ui.painter().rect_stroke(
                                            rect,
                                            4.0,
                                            egui::Stroke::new(
                                                1.5,
                                                if chosen {
                                                    Color32::from_rgb(255, 61, 139)
                                                } else {
                                                    Color32::from_rgb(43, 226, 214)
                                                },
                                            ),
                                            egui::StrokeKind::Inside,
                                        );
                                    }
                                    r.widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::WidgetType::Button,
                                            true,
                                            &a.name,
                                        )
                                    });
                                    let held = crate::icons::help(
                                        &r,
                                        &a.name,
                                        "Use this image as relief.",
                                        "Tap to choose it; then preview its position on the ring.",
                                        "White is the strongest relief; black is the base surface.",
                                    );
                                    if r.clicked() && !held {
                                        *selected = a.name.clone();
                                        changed = true;
                                        ui.close();
                                    }
                                });
                            }
                        });
                    }
                });
        });
    changed
}

/// Inverse of DecalLayer::height's rotation/flip/aspect mapping.
pub fn chart(circumference_mm: f64, a: &Alpha, decal: &Decal, uv: [f64; 2]) -> [f64; 2] {
    let x = (uv[0] - 0.5) * decal.size_mm * if decal.flip { -1.0 } else { 1.0 };
    let y = (0.5 - uv[1]) * decal.size_mm * a.height as f64 / a.width.max(1) as f64;
    let (s, c) = decal.rotation_deg.to_radians().sin_cos();
    [
        decal.theta_deg / 360.0 + (x * c - y * s) / circumference_mm.max(1e-9),
        decal.v_mm + x * s + y * c,
    ]
}

pub fn preview(
    ui: &Ui,
    rect: egui::Rect,
    d: &RingDesign,
    lib: &AlphaLibrary,
    name: &str,
    decal: &Decal,
    feather: f64,
    invert: bool,
    project: impl Fn([f64; 3]) -> egui::Pos2,
) {
    let Some(a) = lib.get(name).filter(|a| !a.is_empty()) else {
        return;
    };
    let ctx = d.field_context();
    const N: usize = 12;
    let chart: Vec<_> = (0..=N)
        .flat_map(|y| (0..=N).map(move |x| [x as f64 / N as f64, y as f64 / N as f64]))
        .map(|p| chart(ctx.circumference_mm, a, decal, p))
        .collect();
    let texture = texture(ui, a, true, invert, feather / decal.size_mm.max(1e-6));
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (crate::key(d), name, texture).hash(&mut hash);
    for p in &chart {
        p.map(f64::to_bits).hash(&mut hash);
    }
    let key = hash.finish();
    let cache = egui::Id::new("alpha-surface-preview");
    let world = ui
        .data(|data| data.get_temp::<(u64, Vec<[f64; 3]>)>(cache))
        .filter(|(k, _)| *k == key)
        .map(|(_, p)| p)
        .unwrap_or_else(|| {
            let p = ringdesign_core::interaction::surface::points(d, lib, &chart);
            ui.data_mut(|data| data.insert_temp(cache, (key, p.clone())));
            p
        });
    let band = ctx.band_v_len_mm;
    let mut mesh = egui::Mesh::with_texture(texture);
    for (i, p) in world.iter().enumerate() {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: project(*p),
            uv: egui::pos2(
                (i % (N + 1)) as f32 / N as f32,
                (i / (N + 1)) as f32 / N as f32,
            ),
            color: Color32::WHITE,
        });
    }
    for y in 0..N {
        for x in 0..N {
            let k = y * (N + 1) + x;
            let ids = [k, k + 1, k + N + 1, k + N + 2];
            if ids
                .iter()
                .all(|&i| (0.0..=band).contains(&chart[i][1]) && mesh.vertices[i].pos.is_finite())
            {
                mesh.add_triangle(k as u32, (k + 1) as u32, (k + N + 1) as u32);
                mesh.add_triangle((k + 1) as u32, (k + N + 2) as u32, (k + N + 1) as u32);
            }
        }
    }
    ui.painter_at(rect).add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_mapping_matches_asymmetric_rotated_flipped_relief() {
        use ringdesign_core::field::{DecalLayer, Uv};
        let d = RingDesign::default();
        let ctx = d.field_context();
        let a = Alpha::new(
            "asymmetric",
            4,
            2,
            vec![0.0, 0.2, 0.6, 1.0, 0.0, 0.1, 0.3, 0.7],
        );
        let mut lib = AlphaLibrary::default();
        lib.insert(a.clone());
        for flip in [false, true] {
            for rotation_deg in [-137.0, 0.0, 63.0] {
                let decal = Decal {
                    theta_deg: 359.0,
                    v_mm: ctx.crest_v_mm,
                    size_mm: 1.0,
                    rotation_deg,
                    height_mm: 0.25,
                    flip,
                };
                let layer = DecalLayer {
                    alpha: a.name.clone(),
                    decals: vec![decal],
                    feather_mm: 0.001,
                    invert: false,
                };
                for uv in [[0.2, 0.3], [0.7, 0.7], [0.8, 0.3]] {
                    let p = chart(ctx.circumference_mm, &a, &decal, uv);
                    let h = layer.height(
                        Uv {
                            u: p[0] * ctx.circumference_mm,
                            v: p[1],
                        },
                        &ctx,
                        &lib,
                    );
                    assert!(
                        (h - a.sample(uv[0], uv[1]) as f64 * 0.25).abs() < 1e-6,
                        "{flip} {rotation_deg}"
                    );
                }
            }
        }
    }
}
