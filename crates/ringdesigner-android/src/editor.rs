//! Shared, host-testable interaction model for the mobile visual editor.

pub mod controls;
pub mod layout;
pub mod overlay;
pub mod pen;
pub mod picking;
pub mod workspace;
pub mod visibility;

use egui_mobile::egui;
use ringdesign_core::RingDesign;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Shape,
    Surface,
    Stones,
    Casting,
}

impl Mode {
    pub const ALL: [Self; 4] = [Self::Shape, Self::Surface, Self::Stones, Self::Casting];
    pub fn label(self) -> &'static str {
        match self {
            Self::Shape => "Shape",
            Self::Surface => "Surface",
            Self::Stones => "Stones",
            Self::Casting => "Casting",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Self::Shape => "Tap the band or head. Drag a dimension handle to resize it.",
            Self::Surface => "Tap an ornament to select its layer. Drag empty space to orbit.",
            Self::Stones => "Tap a stone's marker to edit its setting. Pinch to inspect closely.",
            Self::Casting => {
                "Tap the ring to inspect its wall and draft. Colours show the last check."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Sheet {
    Edit,
    Layers,
    Findings,
    Report,
    Timeline,
    Advanced,
    Construction,
    Workflow,
    /// The recipe graph under the ring, so a node's reach shows on the metal.
    Graph,
}

impl Sheet {
    pub fn label(self) -> &'static str {
        match self {
            Self::Edit => "Inspector",
            Self::Layers => "Design layers",
            Self::Findings => "Casting findings",
            Self::Report => "Measurements",
            Self::Timeline => "Edit history",
            Self::Advanced => "All design controls",
            Self::Construction => "Construction guide",
            Self::Workflow => "Jewelry workflow",
            Self::Graph => "Recipe graph",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShapePart {
    #[default]
    Band,
    Head,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Parameter {
    #[default]
    Bore,
    Width,
    Thickness,
    Comfort,
    Taper,
    HeadLength,
    HeadRise,
    HeadRound,
    HeadDome,
}

impl Parameter {
    pub const BAND: [Self; 5] = [
        Self::Bore,
        Self::Width,
        Self::Thickness,
        Self::Comfort,
        Self::Taper,
    ];
    pub const HEAD: [Self; 5] = [
        Self::Width,
        Self::HeadLength,
        Self::HeadRise,
        Self::HeadRound,
        Self::HeadDome,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Bore => "Finger opening",
            Self::Width => "Band / face width",
            Self::Thickness => "Band thickness",
            Self::Comfort => "Inside rounding",
            Self::Taper => "Shoulder taper",
            Self::HeadLength => "Face length",
            Self::HeadRise => "Face height",
            Self::HeadRound => "Face edge rounding",
            Self::HeadDome => "Face dome",
        }
    }
    pub fn help(self) -> &'static str {
        match self {
            Self::Bore => {
                "Inside diameter: changes how the ring fits your finger. The US size updates with it."
            }
            Self::Width => {
                "Distance across the band. On a signet this is also the width of its face."
            }
            Self::Thickness => {
                "Metal between the finger opening and the outside of the plain band. Ornament adds to this."
            }
            Self::Comfort => {
                "Rounds the edges inside the finger opening for a softer fit; it does not change the ring size."
            }
            Self::Taper => {
                "How strongly the chosen shank shape changes around the ring. On a signet, higher values narrow the underside."
            }
            Self::HeadLength => {
                "Length of the signet face around the ring, perpendicular to its width."
            }
            Self::HeadRise => {
                "Lifts the signet face above the plain band's outer surface, adding metal below the face."
            }
            Self::HeadRound => {
                "Softens the edge where the face meets its side wall. Larger values make a gentler edge."
            }
            Self::HeadDome => {
                "Adds a curved crown above the signet face. Zero leaves the face flat."
            }
        }
    }
    pub fn range(self) -> std::ops::RangeInclusive<f64> {
        match self {
            Self::Bore => 13.0..=25.0,
            Self::Width => 2.0..=22.0,
            Self::Thickness => 0.8..=6.0,
            Self::Comfort => 0.0..=0.8,
            Self::Taper => 0.0..=1.0,
            Self::HeadLength => 5.0..=24.0,
            Self::HeadRise => 0.0..=4.0,
            Self::HeadRound => 0.0..=2.5,
            Self::HeadDome => 0.0..=3.0,
        }
    }
    pub fn value(self, d: &RingDesign) -> f64 {
        match self {
            Self::Bore => d.inner_radius_mm() * 2.0,
            Self::Width => d.profile.width_mm,
            Self::Thickness => d.profile.thickness_mm,
            Self::Comfort => d.profile.comfort_fit_mm,
            Self::Taper => d.shank.amount,
            Self::HeadLength => d.shank.head.length_mm,
            Self::HeadRise => d.shank.head.rise_mm,
            Self::HeadRound => d.shank.head.rim_round_mm,
            Self::HeadDome => d.shank.head.table_dome_mm,
        }
    }
    pub fn set(self, d: &mut RingDesign, value: f64) -> bool {
        let before=d.clone();
        if d.imported_base.is_some() && !matches!(self, Self::Bore|Self::Width|Self::Thickness|Self::HeadLength|Self::HeadRise) { return false; }
        if !value.is_finite() {
            return false;
        }
        let range = self.range();
        let value = value.clamp(*range.start(), *range.end());
        if (self.value(d) - value).abs() < 1e-8 {
            return false;
        }
        match self {
            Self::Bore => {
                let Ok(size) = ringdesign_core::resize::size_from_bore(value) else {
                    return false;
                };
                d.size = size;
            }
            Self::Width => d.profile.width_mm = value,
            Self::Thickness => d.profile.thickness_mm = value,
            Self::Comfort => d.profile.comfort_fit_mm = value,
            Self::Taper => d.shank.amount = value,
            Self::HeadLength => d.shank.head.length_mm = value,
            Self::HeadRise => d.shank.head.rise_mm = value,
            Self::HeadRound => d.shank.head.rim_round_mm = value,
            Self::HeadDome => d.shank.head.table_dome_mm = value,
        }
        if let Some(base)=&d.imported_base { if base.validate_design(d).is_err() { *d=before; return false; } }
        true
    }
    pub fn unit(self) -> &'static str {
        if self == Self::Taper { "" } else { " mm" }
    }
    pub fn has_handle(self) -> bool {
        matches!(
            self,
            Self::Bore | Self::Width | Self::Thickness | Self::HeadLength | Self::HeadRise
        )
    }
}

#[derive(Clone, Debug)]
pub struct HandleDrag {
    pub parameter: Parameter,
    pub initial_value: f64,
    pub screen_axis: egui::Vec2,
    pub points_per_mm: f32,
    pub pointer_start: egui::Pos2,
}

pub struct Editor {
    pub workspace: crate::prefs::Workspace,
    pub reflow: workspace::Reflow,
    pub inspector_height: Option<f32>,
    pub menu_avoidance: visibility::MenuAvoidance,
    pub palette: Option<workspace::Palette>,
    pub floating_rects: Vec<egui::Rect>,
    /// The Tools rail's layer while it is shown.
    pub rail_layer: Option<egui::LayerId>,
    pub floating_dragging: bool,
    pub reset_workspace: bool,
    pub mode: Mode,
    pub sheet: Option<Sheet>,
    pub part: ShapePart,
    pub parameter: Parameter,
    pub guides: bool,
    pub handles_active: bool,
    pub help: bool,
    pub selection: Option<picking::Hit>,
    pub overlaps: Vec<usize>,
    pub stone: Option<usize>,
    pub stone_path: Vec<usize>,
    pub drag: Option<HandleDrag>,
    pub hold_before: bool,
    pub check_pending: bool,
    pub active_field: String,
    pub isolate: bool,
    pub debug_layout: bool,
    pub last_layout: String,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            workspace: crate::prefs::Workspace::default(),
            reflow: workspace::Reflow::default(),
            inspector_height: None,
            menu_avoidance: visibility::MenuAvoidance::default(),
            palette: None,
            floating_rects: Vec::new(),
            rail_layer: None,
            floating_dragging: false,
            reset_workspace: false,
            mode: Mode::Shape,
            sheet: Some(Sheet::Edit),
            part: ShapePart::Band,
            parameter: Parameter::Bore,
            guides: true,
            handles_active: true,
            help: false,
            selection: None,
            overlaps: Vec::new(),
            stone: None,
            stone_path: Vec::new(),
            drag: None,
            hold_before: false,
            check_pending: true,
            active_field: Parameter::Bore.label().into(),
            isolate: false,
            debug_layout: false,
            last_layout: String::new(),
        }
    }
}

impl Editor {
    pub fn select_mode(&mut self, mode: Mode) {
        if self.sheet != Some(Sheet::Graph) {
            self.workspace.mode_panels[self.mode as usize] = self.sheet;
        }
        self.mode = mode;
        self.handles_active = true;
        self.sheet = self.workspace.mode_panels[mode as usize];
        self.drag = None;
        self.hold_before = false;
        if mode != Mode::Surface {
            self.isolate = false;
        }
    }
    pub fn toggle(&mut self, sheet: Sheet) {
        self.sheet = if self.sheet == Some(sheet) {
            None
        } else {
            Some(sheet)
        };
    }
    pub fn reset_selection(&mut self) {
        self.handles_active = false;
        self.selection = None;
        self.overlaps.clear();
        self.stone = None;
        self.stone_path.clear();
        self.drag = None;
        self.isolate = false;
    }
}

/// Allocate chrome from the *remaining safe rect*, including keyboard changes.
/// Primary controls keep their touch size; the inspector gives up space first.
pub fn inspector_extent(available: egui::Vec2, landscape: bool) -> f32 {
    workspace::inspector_extent(available, landscape, if landscape { 0.36 } else { 0.32 })
}

pub fn row_width(available: f32, count: usize, gap: f32) -> f32 {
    ((available - gap * count.saturating_sub(1) as f32) / count.max(1) as f32).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Behavior paths reviewed on emulator-5554 through ARTEMIS, 2026-09-19.
    #[test]
    fn workspaces_restore_collapsed_and_custom_inspectors() {
        let mut e = Editor::default();
        e.sheet = None;
        e.select_mode(Mode::Surface);
        assert_eq!(e.sheet, Some(Sheet::Layers));
        e.sheet = Some(Sheet::Report);
        e.select_mode(Mode::Casting);
        assert_eq!(e.sheet, Some(Sheet::Findings));
        e.select_mode(Mode::Shape);
        assert_eq!(e.sheet, None);
        e.select_mode(Mode::Surface);
        assert_eq!(e.sheet, Some(Sheet::Report));
    }
    #[test]
    fn clearing_selection_releases_dimension_handles_for_navigation() {
        let mut e = Editor::default();
        let d = RingDesign::default();
        let camera = crate::camera::OrbitCamera::default();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.,500.));
        let handle = camera.projector(rect).at(overlay::dimension(&d,e.parameter).0);
        assert!(overlay::blocks_orbit(&e,&d,&camera,rect,Some(handle)));
        e.reset_selection();
        assert!(!overlay::blocks_orbit(&e,&d,&camera,rect,Some(handle)));
        assert!(e.selection.is_none() && e.stone.is_none());
    }
    #[test]
    fn every_sheet_replaces_the_previous_one() {
        let mut e = Editor::default();
        for sheet in [
            Sheet::Layers,
            Sheet::Report,
            Sheet::Findings,
            Sheet::Timeline,
        ] {
            e.toggle(sheet);
            assert_eq!(e.sheet, Some(sheet));
        }
        e.toggle(Sheet::Timeline);
        assert_eq!(e.sheet, None);
    }
    #[test]
    fn narrow_and_keyboard_layouts_leave_a_viewport() {
        for width in [280.0, 320.0, 360.0, 411.0] {
            let button = row_width(width - 16.0, 5, 2.0);
            assert!(button >= 44.0);
            assert!(button * 5.0 + 8.0 <= width - 15.9);
            for height in [140.0, 280.0, 450.0, 780.0] {
                let used = inspector_extent(egui::vec2(width, height), false);
                assert!(used >= 0.0 && used <= height);
                assert!(height - used >= height.min(190.0));
            }
        }
    }
    #[test]
    fn bore_field_and_handle_use_exact_diameter_without_size_rounding() {
        let mut d = RingDesign::default();
        assert!(Parameter::Bore.set(&mut d, 18.23));
        assert!((Parameter::Bore.value(&d) - 18.23).abs() < 1e-8);
        assert!(!Parameter::Bore.set(&mut d, f64::NAN));
    }
}
