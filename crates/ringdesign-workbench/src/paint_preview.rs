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
    follow_aim: Aim,
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
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct FollowSettings {
    pub threshold_deg: f32,
    pub smoothing_seconds: f32,
    pub surface_span_mm: f32,
}
impl Default for FollowSettings {
    fn default() -> Self {
        Self { threshold_deg: 12.0, smoothing_seconds: 0.55, surface_span_mm: 2.0 }
    }
}
fn settings(ctx: &Context) -> FollowSettings {
    ctx.data_mut(|d| d.get_persisted(id().with("settings")).unwrap_or_default())
}
pub fn control(ui: &mut Ui) {
    let mut on = following(ui.ctx());
    if ui.toggle_value(&mut on, (crate::icons::Icon::View.image(ui,18.), "Follow brush"))
        .on_hover_text("Follow broad movement around the ring while ignoring small surface changes. Locked views stay still.").changed() {
        ui.data_mut(|d| d.insert_temp(follow_id(), on));
    }
    ui.menu_button("Follow settings", |ui| {
        ui.set_width(270.0);
        let mut s = settings(ui.ctx());
        ui.label("Keep the camera steady over small details.");
        crate::controls::slider(ui, "Turn threshold", egui::Slider::new(&mut s.threshold_deg, 0.0..=35.0).suffix("°"));
        ui.weak("The brush can move this far before the camera turns.");
        crate::controls::slider(ui, "Smoothing", egui::Slider::new(&mut s.smoothing_seconds, 0.1..=1.5).suffix(" s"));
        ui.weak("Higher values make the camera respond more gently.");
        crate::controls::slider(ui, "Surface averaging", egui::Slider::new(&mut s.surface_span_mm, 0.5..=5.0).suffix(" mm"));
        ui.weak("Look across crevices instead of following each tiny face.");
        if ui.button("Reset follow settings").clicked() { s = FollowSettings::default(); }
        ui.data_mut(|d| d.insert_persisted(id().with("settings"), s));
    });
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
    // Camera orientation follows the ring's azimuth, never a tiny facet's
    // circumferential normal. Across the band, average a wider patch so a
    // groove cannot flip the view while broad side-to-crown movement still can.
    let span = f64::from(settings(ui.ctx()).surface_span_mm.clamp(0.5, 5.0));
    let lo = (chart[1] - span * 0.5).max(0.0);
    let hi = (chart[1] + span * 0.5).min(ctx.band_v_len_mm);
    let broad = surface::points(design, lib, &[[chart[0], lo], chart, [chart[0], hi]]);
    let theta = (chart[0] * std::f64::consts::TAU) as f32;
    let (sin, cos) = theta.sin_cos();
    let dr = ((broad[2][0] - broad[0][0]) * f64::from(cos)
        + (broad[2][1] - broad[0][1]) * f64::from(sin)) as f32;
    let dz = (broad[2][2] - broad[0][2]) as f32;
    let cross_normal = if dz.hypot(dr) > 1e-6 {
        let sign = if dz < 0.0 { -1.0 } else { 1.0 };
        [cos * dz * sign, sin * dz * sign, -dr * sign]
    } else { [cos, sin, 0.0] };
    let follow_aim = Aim {
        point: std::array::from_fn(|k| ((broad[0][k] + broad[1][k] + broad[2][k]) / 3.0) as f32),
        normal: cross_normal,
        ..aim
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
                follow_aim,
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
    let pose = follow_pose(from, target, radius, &p.follow_aim, settings(ui.ctx()), ui.input(|i| i.stable_dt));
    if pose != from { ui.ctx().request_repaint(); }
    Some(pose)
}

/// Frame-rate independent follow with a soft angular dead zone and speed cap.
pub fn follow_pose(from: Pose, target: [f32; 3], radius: f32, aim: &Aim, settings: FollowSettings, dt: f32) -> Pose {
    let to = focus::aim_pose(from, target, radius, aim);
    let dt = dt.clamp(0.0, 0.05);
    let t = 1.0 - (-dt / settings.smoothing_seconds.clamp(0.1, 1.5)).exp();
    let threshold = settings.threshold_deg.clamp(0.0, 35.0).to_radians();
    let turn = |a: f32, b: f32| {
        let delta = (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let outside = delta.signum() * (delta.abs() - threshold).max(0.0);
        a + (outside * t).clamp(-1.5 * dt, 1.5 * dt)
    };
    let delta = [to.pan[0] - from.pan[0], to.pan[1] - from.pan[1]];
    let distance = delta[0].hypot(delta[1]);
    let pan_zone = (radius.abs() * 0.04).clamp(0.25, 1.5);
    let pan_t = if distance > pan_zone { t * (1.0 - pan_zone / distance) } else { 0.0 };
    Pose {
        yaw: turn(from.yaw, to.yaw),
        pitch: turn(from.pitch, to.pitch),
        pan: std::array::from_fn(|k| from.pan[k] + delta[k] * pan_t),
        ..from
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    fn pose() -> Pose { Pose { yaw: 0., pitch: 0., pan: [0.;2], zoom: 2., roll: 0.3 } }
    fn aim(deg: f32) -> Aim { let a = deg.to_radians(); Aim { point: [0.;3], normal: [a.cos(), a.sin(), 0.], reach_mm: 1. } }

    #[test]
    fn tiny_crevice_turns_stay_still_but_large_movements_follow_without_jumping() {
        let settings = FollowSettings::default();
        let from = pose();
        assert_eq!(follow_pose(from, [0.;3], 12., &aim(10.), settings, 1./60.), from);
        let mut p = from;
        for _ in 0..300 {
            let next = follow_pose(p, [0.;3], 12., &aim(175.), settings, 1./60.);
            assert!((next.yaw - p.yaw).abs() <= 1.5/60. + 1e-6);
            assert_eq!((next.zoom, next.roll), (from.zoom, from.roll));
            p = next;
        }
        assert!((175. - p.yaw.to_degrees() - settings.threshold_deg).abs() < 0.5);
    }

    #[test]
    fn seam_crossing_takes_the_short_path_and_frame_rate_does_not_change_follow() {
        let from = Pose { yaw: 179_f32.to_radians(), ..pose() };
        let settings = FollowSettings { threshold_deg: 0., ..Default::default() };
        let next = follow_pose(from, [0.;3], 12., &aim(-179.), settings, 1./60.);
        assert!(next.yaw > from.yaw && next.yaw - from.yaw < 0.01);
        let simulate = |hz: usize| {
            let mut p = pose();
            for _ in 0..hz { p = follow_pose(p, [0.;3], 12., &aim(45.), settings, 1./hz as f32); }
            p
        };
        assert!((simulate(30).yaw - simulate(120).yaw).abs() < 1e-5);
    }

    #[test]
    fn pan_has_a_dead_zone_and_follows_large_displacement_with_damping() {
        let from = Pose { roll: 0., ..pose() };
        let mut target = aim(0.);
        target.point = [10., 0.1, 0.1];
        assert_eq!(follow_pose(from, [0.;3], 12., &target, FollowSettings::default(), 1./60.), from);
        target.point = [10., 5., -4.];
        let next = follow_pose(from, [0.;3], 12., &target, FollowSettings::default(), 1./60.);
        assert!(next.pan[0] > 0. && next.pan[0] < 0.2 && next.pan[1] < 0. && next.pan[1] > -0.2);
    }
}
