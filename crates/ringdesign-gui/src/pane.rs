//! Split view: independent panes over one design.
//!
//! Each pane carries its own camera, shading and section angle, so the same
//! build can be watched from a 3/4 view, straight down the finger axis, and in
//! cross-section at once. The mesh is uploaded once and drawn per pane —
//! `GpuMeshRenderer::paint` scissors and clears depth inside its own rect, so
//! several paint callbacks compose in one frame.

use egui_phosphor::regular as icon;
use ringdesign_core::castability::Section;
use ringdesign_core::profile::TOP_DEG;

use crate::camera::{OrbitCamera, StandardView};
use crate::viewport::ShadeMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneKind {
    Solid,
    Unrolled,
    Section,
}

impl PaneKind {
    pub const ALL: &'static [PaneKind] = &[PaneKind::Solid, PaneKind::Unrolled, PaneKind::Section];

    pub fn label(self) -> &'static str {
        match self {
            PaneKind::Solid => "Ring",
            PaneKind::Unrolled => "Tile Layout",
            PaneKind::Section => "Cross Section",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            PaneKind::Solid => icon::CIRCLE_NOTCH,
            PaneKind::Unrolled => icon::GRID_FOUR,
            PaneKind::Section => icon::CHART_LINE,
        }
    }
}

pub struct Pane {
    pub kind: PaneKind,
    pub camera: OrbitCamera,
    pub shade: ShadeMode,
    pub section_theta_deg: f64,
    /// Slice at this pane's own angle, refreshed when the build lands.
    pub section: Option<Section>,
}

impl Default for Pane {
    fn default() -> Self {
        Self {
            kind: PaneKind::Solid,
            camera: OrbitCamera::default(),
            shade: ShadeMode::Metal,
            section_theta_deg: TOP_DEG,
            section: None,
        }
    }
}

impl Pane {
    fn view(kind: PaneKind, view: StandardView) -> Self {
        let mut p = Self { kind, ..Default::default() };
        p.camera.set_view(view);
        p
    }

    /// The four panes a quad layout opens with: a 3/4 view plus the two square
    /// orthographic views, and a cross-section at the top of the ring.
    pub fn defaults() -> Vec<Pane> {
        vec![
            Pane::view(PaneKind::Solid, StandardView::Iso),
            Pane::view(PaneKind::Solid, StandardView::Face),
            Pane::view(PaneKind::Solid, StandardView::Edge),
            Pane::view(PaneKind::Section, StandardView::Iso),
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Single,
    SplitH,
    SplitV,
    Quad,
}

impl Layout {
    pub const ALL: &'static [Layout] =
        &[Layout::Single, Layout::SplitH, Layout::SplitV, Layout::Quad];

    /// Panes this layout shows, always starting from pane 0.
    pub fn count(self) -> usize {
        match self {
            Layout::Single => 1,
            Layout::SplitH | Layout::SplitV => 2,
            Layout::Quad => 4,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Layout::Single => "Single",
            Layout::SplitH => "Two across",
            Layout::SplitV => "Two down",
            Layout::Quad => "Four",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Layout::Single => icon::SQUARE,
            Layout::SplitH => icon::COLUMNS,
            Layout::SplitV => icon::ROWS,
            Layout::Quad => icon::SQUARES_FOUR,
        }
    }

    /// Sub-rects for each visible pane, with a gutter left between them for the
    /// dividers.
    pub fn split(self, rect: egui::Rect, gutter: f32) -> Vec<egui::Rect> {
        let g = gutter * 0.5;
        let (cx, cy) = (rect.center().x, rect.center().y);
        let left = egui::Rect::from_min_max(rect.min, egui::pos2(cx - g, rect.max.y));
        let right = egui::Rect::from_min_max(egui::pos2(cx + g, rect.min.y), rect.max);
        let top = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, cy - g));
        let bottom = egui::Rect::from_min_max(egui::pos2(rect.min.x, cy + g), rect.max);
        match self {
            Layout::Single => vec![rect],
            Layout::SplitH => vec![left, right],
            Layout::SplitV => vec![top, bottom],
            Layout::Quad => vec![
                egui::Rect::from_min_max(rect.min, egui::pos2(cx - g, cy - g)),
                egui::Rect::from_min_max(egui::pos2(cx + g, rect.min.y), egui::pos2(rect.max.x, cy - g)),
                egui::Rect::from_min_max(egui::pos2(rect.min.x, cy + g), egui::pos2(cx - g, rect.max.y)),
                egui::Rect::from_min_max(egui::pos2(cx + g, cy + g), rect.max),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0))
    }

    #[test]
    fn every_layout_yields_one_rect_per_pane() {
        for &l in Layout::ALL {
            assert_eq!(l.split(rect(), 4.0).len(), l.count(), "{l:?}");
        }
    }

    #[test]
    fn panes_stay_inside_the_area_and_do_not_overlap() {
        for &l in Layout::ALL {
            let rs = l.split(rect(), 4.0);
            for r in &rs {
                assert!(rect().contains_rect(*r), "{l:?}: {r:?} escaped");
                assert!(r.width() > 0.0 && r.height() > 0.0, "{l:?}: empty pane");
            }
            for i in 0..rs.len() {
                for j in i + 1..rs.len() {
                    let hit = rs[i].intersect(rs[j]);
                    assert!(
                        hit.width() <= 0.0 || hit.height() <= 0.0,
                        "{l:?}: panes {i} and {j} overlap"
                    );
                }
            }
        }
    }

    #[test]
    fn the_default_panes_cover_a_quad_layout() {
        let p = Pane::defaults();
        assert_eq!(p.len(), Layout::Quad.count());
        assert!(p.iter().any(|x| x.kind == PaneKind::Section), "no section pane");
    }
}
