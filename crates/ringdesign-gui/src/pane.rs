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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PaneKind {
    Solid,
    Unrolled,
    Section,
    Graph,
    Casting,
    Cad,
}

impl PaneKind {
    pub const ALL: &'static [PaneKind] = &[PaneKind::Solid, PaneKind::Unrolled, PaneKind::Section, PaneKind::Graph, PaneKind::Casting, PaneKind::Cad];

    pub fn label(self) -> &'static str {
        match self {
            PaneKind::Solid => "Ring",
            PaneKind::Unrolled => "Tile Layout",
            PaneKind::Section => "Cross Section",
            PaneKind::Graph => "Graph",
            PaneKind::Casting => "Casting",
            PaneKind::Cad => "CAD & components",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            PaneKind::Solid => icon::CIRCLE_NOTCH,
            PaneKind::Unrolled => icon::GRID_FOUR,
            PaneKind::Section => icon::CHART_LINE,
            PaneKind::Graph => icon::GRAPH,
            PaneKind::Casting => icon::SHIELD_CHECK,
            PaneKind::Cad => icon::RULER,
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Pane {
    pub kind: PaneKind,
    pub camera: OrbitCamera,
    #[serde(default)]
    pub navigation: ringdesign_workbench::navigation::Settings,
    /// This preview stays orthographic and follows the selected graph feature.
    #[serde(default)]
    pub follow_node: bool,
    pub shade: ShadeMode,
    pub section_theta_deg: f64,
    /// Slice at this pane's own angle, refreshed when the build lands.
    #[serde(skip)]
    pub section: Option<Section>,
    /// The camera easing to a pose: a chosen node's patch, or a view from the cube.
    #[serde(skip)]
    pub turn: Option<ringdesign_workbench::focus::Turn>,
}

impl Default for Pane {
    fn default() -> Self {
        Self {
            kind: PaneKind::Solid,
            camera: OrbitCamera::default(),
            navigation: Default::default(),
            follow_node: false,
            shade: ShadeMode::Metal,
            section_theta_deg: TOP_DEG,
            section: None,
            turn: None,
        }
    }
}

impl Pane {
    fn view(kind: PaneKind, view: StandardView) -> Self {
        let mut p = Self {
            kind,
            ..Default::default()
        };
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Layout {
    Single,
    SplitH,
    SplitV,
    Quad,
    GraphReview,
}

impl Layout {
    pub const ALL: &'static [Layout] =
        &[Layout::Single, Layout::SplitH, Layout::SplitV, Layout::Quad, Layout::GraphReview];

    /// Panes this layout shows, always starting from pane 0.
    pub fn count(self) -> usize {
        match self {
            Layout::Single => 1,
            Layout::SplitH | Layout::SplitV => 2,
            Layout::Quad => 4,
            Layout::GraphReview => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Layout::Single => "Single",
            Layout::SplitH => "Split Vertical",
            Layout::SplitV => "Split Horizontal",
            Layout::Quad => "Four",
            Layout::GraphReview => "Graph & previews",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Layout::Single => icon::SQUARE,
            Layout::SplitH => icon::COLUMNS,
            Layout::SplitV => icon::ROWS,
            Layout::Quad => icon::SQUARES_FOUR,
            Layout::GraphReview => icon::GRAPH,
        }
    }

    pub fn tree(self) -> egui_tiles::Tree<usize> {
        use egui_tiles::{Container, Linear, LinearDir, Tiles, Tree};
        let mut tiles = Tiles::default();
        let panes: Vec<_> = (0..self.count()).map(|i| tiles.insert_pane(i)).collect();
        let root = match self {
            Self::Single => panes[0],
            Self::SplitH => tiles.insert_horizontal_tile(panes),
            Self::SplitV => tiles.insert_vertical_tile(panes),
            Self::Quad => {
                let left = tiles.insert_vertical_tile(vec![panes[0], panes[2]]);
                let right = tiles.insert_vertical_tile(vec![panes[1], panes[3]]);
                tiles.insert_horizontal_tile(vec![left, right])
            }
            Self::GraphReview => {
                let previews = tiles.insert_vertical_tile(vec![panes[0], panes[1]]);
                tiles.insert_container(Container::Linear(Linear::new_binary(
                    LinearDir::Horizontal, [previews, panes[2]], 0.35,
                )))
            }
        };
        Tree::new("viewports", root, tiles)
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ViewportLayout {
    pub preset: Layout,
    pub tree: egui_tiles::Tree<usize>,
}
impl ViewportLayout {
    pub fn new(preset: Layout) -> Self { Self { preset, tree: preset.tree() } }
    /// A layout still serves its preset while it holds some of the preset's
    /// views and nothing else. A subset, not the whole set, because closing a
    /// view leaves one — rebuilt from the preset, it would simply come back.
    pub fn valid_for(&self, preset: Layout) -> bool {
        if self.preset != preset || self.tree.root.is_none() {
            return false;
        }
        let shown: std::collections::BTreeSet<_> = self.tree.tiles.iter().filter_map(|(_, tile)| match tile {
            egui_tiles::Tile::Pane(i) => Some(*i), _ => None,
        }).collect();
        !shown.is_empty() && shown.iter().all(|i| *i < preset.count())
    }

    /// The views this layout still shows, in pane order.
    pub fn shown(&self) -> Vec<usize> {
        let mut v: Vec<_> = self.tree.tiles.iter().filter_map(|(_, tile)| match tile {
            egui_tiles::Tile::Pane(i) => Some(*i), _ => None,
        }).collect();
        v.sort_unstable();
        v
    }
}

pub fn graph_panes() -> Vec<Pane> {
    let mut panes = Pane::defaults();
    panes[1].follow_node = true;
    panes[1].navigation.locked = true;
    panes[2].kind = PaneKind::Graph;
    panes
}
