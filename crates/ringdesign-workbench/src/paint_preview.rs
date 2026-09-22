//! A brush footprint shared between the unrolled painting surface and 3D views.
use crate::focus::{self, Aim, Pose};
use egui::{Context, Rect, Ui};
use ringdesign_core::{AlphaLibrary, RingDesign, interaction::surface};

#[derive(Clone)]
struct Preview {
    chart: [f64; 2],
    radii: [f64; 2],
    at: f64,
    built: f64,
    dragging: bool,
    points: Vec<[f32; 3]>,
    aim: Aim,
}
fn id() -> egui::Id {
    egui::Id::new("paint-surface-preview")
}
fn follow_id() -> egui::Id {
    id().with("follow")
}
pub fn following(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(follow_id()).unwrap_or(true))
}
pub fn control(ui: &mut Ui) {
    let mut on = following(ui.ctx());
    if ui.toggle_value(&mut on,(crate::icons::Icon::View.image(ui,18.),"Follow brush"))
        .on_hover_text("Smoothly turn and pan the 3D preview while painting. Turn off to keep your current view.").changed() {
        ui.data_mut(|d|d.insert_temp(follow_id(),on));
    }
}
pub fn publish(
    ui: &Ui,
    design: &RingDesign,
    lib: &AlphaLibrary,
    chart: [f64; 2],
    radii: [f64; 2],
    dragging: bool,
) {
    let now = ui.input(|i| i.time);
    let old = ui.data(|d| d.get_temp::<Preview>(id()));
    if let Some(mut old) =
        old.filter(|p| p.chart == chart && p.radii == radii && now - p.built < 0.15)
    {
        old.at = now;
        old.dragging = dragging;
        ui.data_mut(|d| d.insert_temp(id(), old));
        return;
    }
    let ctx = design.field_context();
    let mut samples = vec![
        chart,
        [chart[0] + 0.0001, chart[1]],
        [chart[0], (chart[1] + 0.01).min(ctx.band_v_len_mm)],
        [chart[0], (chart[1] - 0.01).max(0.)],
    ];
    for i in 0..=32 {
        let angle = i as f64 / 32. * std::f64::consts::TAU;
        samples.push([
            chart[0] + radii[0] * angle.cos() / ctx.circumference_mm,
            (chart[1] + radii[1] * angle.sin()).clamp(0., ctx.band_v_len_mm),
        ]);
    }
    let points: Vec<_> = surface::points(design, lib, &samples)
        .into_iter()
        .map(|p| p.map(|v| v as f32))
        .collect();
    if points.len() < 4 {
        return;
    }
    let a: [f32; 3] = std::array::from_fn(|k| points[1][k] - points[0][k]);
    let b: [f32; 3] = std::array::from_fn(|k| points[2][k] - points[3][k]);
    let mut normal = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    if normal[0] * points[0][0] + normal[1] * points[0][1] < 0. {
        normal = normal.map(|v| -v);
    }
    let aim = Aim {
        point: points[0],
        normal,
        reach_mm: radii[0].max(radii[1]) as f32,
    };
    ui.data_mut(|d| {
        d.insert_temp(
            id(),
            Preview {
                chart,
                radii,
                at: now,
                built: now,
                dragging,
                points: points[4..].to_vec(),
                aim,
            },
        )
    });
    ui.ctx().request_repaint();
}
pub fn follow(ui: &Ui, from: Pose, target: [f32; 3], radius: f32) -> Option<Pose> {
    let p = ui.data(|d| d.get_temp::<Preview>(id()))?;
    if !following(ui.ctx()) || !p.dragging || ui.input(|i| i.time) - p.at > 0.12 {
        return None;
    }
    let mut to = focus::aim_pose(from, target, radius, &p.aim);
    to.zoom = from.zoom;
    // Exponential damping is independent of frame rate and takes the short
    // route through the seam at 0/360 degrees.
    let t = 1. - (-ui.input(|i| i.stable_dt).clamp(0.001, 0.05) / 0.18).exp();
    let short = |a: f32, b: f32| {
        (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
    };
    let pose = Pose {
        yaw: from.yaw + short(from.yaw, to.yaw) * t,
        pitch: from.pitch + short(from.pitch, to.pitch) * t,
        pan: std::array::from_fn(|k| from.pan[k] + (to.pan[k] - from.pan[k]) * t),
        ..from
    };
    ui.ctx().request_repaint();
    Some(pose)
}
pub fn draw(ui: &Ui, rect: Rect, project: impl Fn([f32; 3]) -> egui::Pos2) {
    let Some(p) = ui.data(|d| d.get_temp::<Preview>(id())) else {
        return;
    };
    if ui.input(|i| i.time) - p.at > 0.12 {
        return;
    }
    let painter = ui.painter_at(rect);
    let color = egui::Color32::from_rgb(230, 108, 153);
    painter.add(egui::Shape::line(
        p.points.into_iter().map(&project).collect(),
        egui::Stroke::new(2., color),
    ));
    painter.circle_filled(project(p.aim.point), 2.5, color);
}
