//! Dockable tool panels, laid out by `egui_tiles`.
//!
//! Each side of the window holds its own tile tree, so tools can be split,
//! stacked or tabbed within a side and dragged between those slots. A stack of
//! sections sharing one scroll area hides whatever is below the fold, which is
//! how "Add layer" ended up unreachable; a tile per tool keeps each one on
//! screen and independently sized.
//!
//! The trees are kept separate per side rather than one tree over the whole
//! window because the centre belongs to the viewport panes. Moving a tool
//! across sides is an explicit command rather than a drag.

use egui_phosphor::regular as icon;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolKind {
    Design,
    Layers,
    Report,
    Library,
    Node,
}

impl ToolKind {
    pub fn atelier(self) -> ringdesign_workbench::icons::Icon {
        use ringdesign_workbench::icons::Icon;
        match self { Self::Design => Icon::Shape, Self::Layers => Icon::Layers, Self::Report => Icon::Casting, Self::Library => Icon::Pattern, Self::Node => Icon::Graph }
    }
    pub const ALL: &'static [ToolKind] = &[
        ToolKind::Design,
        ToolKind::Layers,
        ToolKind::Report,
        ToolKind::Library,
        ToolKind::Node,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ToolKind::Design => "Design",
            ToolKind::Layers => "Layers",
            ToolKind::Report => "Report",
            ToolKind::Library => "Alphas",
            ToolKind::Node => "Node",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            ToolKind::Design => icon::SLIDERS,
            ToolKind::Layers => icon::STACK,
            ToolKind::Report => icon::CLIPBOARD_TEXT,
            ToolKind::Library => icon::GRID_FOUR,
            ToolKind::Node => icon::SLIDERS_HORIZONTAL,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub const ALL: &'static [Side] = &[Side::Left, Side::Right];

    pub fn label(self) -> &'static str {
        match self {
            Side::Left => "Left",
            Side::Right => "Right",
        }
    }

    pub fn other(self) -> Self {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}

/// Both side trees plus their widths.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dock {
    pub left: egui_tiles::Tree<ToolKind>,
    pub right: egui_tiles::Tree<ToolKind>,
    pub left_width: f32,
    pub right_width: f32,
}

impl Default for Dock {
    fn default() -> Self {
        Self {
            // Layers under Design: it is the one that gets edited most, and a
            // vertical split keeps both on screen.
            left: egui_tiles::Tree::new_vertical(
                "dock_left",
                vec![ToolKind::Design, ToolKind::Layers],
            ),
            right: egui_tiles::Tree::new_vertical(
                "dock_right",
                vec![ToolKind::Report, ToolKind::Library],
            ),
            left_width: 336.0,
            right_width: 326.0,
        }
    }
}

impl Dock {
    pub fn for_desktop(desktop: Desktop) -> Self {
        if desktop == Desktop::Model { return Self::default(); }
        let (left, right) = match desktop {
            Desktop::Graph => (vec![], vec![ToolKind::Node]),
            Desktop::Surface => (vec![ToolKind::Layers], vec![ToolKind::Library]),
            // Parts are judged as they are built, so the verdict stands beside the CAD pane.
            Desktop::Cad => (vec![], vec![ToolKind::Report]),
            _ => (vec![], vec![]),
        };
        Self {
            left: if left.is_empty() { egui_tiles::Tree::empty("dock_left") } else { egui_tiles::Tree::new_vertical("dock_left", left) },
            right: if right.is_empty() { egui_tiles::Tree::empty("dock_right") } else { egui_tiles::Tree::new_vertical("dock_right", right) },
            left_width: 336.0, right_width: if desktop == Desktop::Graph { 420.0 } else { 326.0 },
        }
    }
    pub fn tree(&self, side: Side) -> &egui_tiles::Tree<ToolKind> {
        match side {
            Side::Left => &self.left,
            Side::Right => &self.right,
        }
    }

    pub fn tree_mut(&mut self, side: Side) -> &mut egui_tiles::Tree<ToolKind> {
        match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
        }
    }

    pub fn width_of(&self, side: Side) -> f32 {
        match side {
            Side::Left => self.left_width,
            Side::Right => self.right_width,
        }
    }

    pub fn set_width(&mut self, side: Side, w: f32) {
        let w = w.clamp(240.0, 680.0);
        match side {
            Side::Left => self.left_width = w,
            Side::Right => self.right_width = w,
        }
    }

    pub fn is_open(&self, tool: ToolKind) -> bool {
        Side::ALL.iter().any(|&s| {
            self.tree(s)
                .tiles
                .iter()
                .any(|(_, t)| matches!(t, egui_tiles::Tile::Pane(p) if *p == tool))
        })
    }

    /// Drop every tile holding this tool, from both sides.
    pub fn close(&mut self, tool: ToolKind) {
        for side in Side::ALL {
            let tree = self.tree_mut(*side);
            let ids: Vec<_> = tree
                .tiles
                .iter()
                .filter(|(_, t)| matches!(t, egui_tiles::Tile::Pane(p) if *p == tool))
                .map(|(id, _)| *id)
                .collect();
            for id in ids {
                tree.remove_recursively(id);
            }
        }
    }

    /// Show a tool on a side, removing it from wherever it was.
    pub fn open_on(&mut self, tool: ToolKind, side: Side) {
        self.close(tool);
        let tree = self.tree_mut(side);
        let pane = tree.tiles.insert_pane(tool);
        match tree.root() {
            Some(root) => {
                // Push into the root container so it lands beside its siblings.
                if let Some(egui_tiles::Tile::Container(c)) = tree.tiles.get_mut(root) {
                    c.add_child(pane);
                } else {
                    let split = tree.tiles.insert_vertical_tile(vec![root, pane]);
                    tree.root = Some(split);
                }
            }
            None => tree.root = Some(pane),
        }
    }

    pub fn toggle(&mut self, tool: ToolKind, open: bool) {
        if open {
            self.open_on(tool, Side::Left);
        } else {
            self.close(tool);
        }
    }
}

/// Each working environment remembers its own panes and inspector arrangement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Desktop { #[default] Model, Graph, Surface, Casting, Cad }
impl Desktop {
    pub const ALL: [Self; 5] = [Self::Model, Self::Graph, Self::Surface, Self::Casting, Self::Cad];
    pub fn label(self) -> &'static str { match self { Self::Model => "Model", Self::Graph => "Graph", Self::Surface => "Surface", Self::Casting => "Casting", Self::Cad => "CAD" } }
    pub fn pane(self) -> crate::pane::PaneKind { use crate::pane::PaneKind::*; match self { Self::Model => Solid, Self::Graph => Graph, Self::Surface => Unrolled, Self::Casting => Casting, Self::Cad => Cad } }
    pub fn icon(self) -> ringdesign_workbench::icons::Icon { use ringdesign_workbench::icons::Icon::*; match self { Self::Model => Shape, Self::Graph => Graph, Self::Surface => Surface, Self::Casting => Casting, Self::Cad => Workshop } }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DesktopLayout {
    pub dock: Dock,
    pub panes: Vec<crate::pane::Pane>,
    pub layout: crate::pane::Layout,
    pub active: usize,
    #[serde(default)]
    pub viewport_layout: Option<crate::pane::ViewportLayout>,
}
impl DesktopLayout {
    pub fn new(desktop: Desktop) -> Self {
        use crate::pane::{Layout, PaneKind};
        let mut panes = crate::pane::Pane::defaults();
        panes[0].kind = desktop.pane();
        // Each workspace opens on the pair of views its work needs: the graph
        // beside two previews, the tile layout under the ring it wraps.
        let (layout, active) = match desktop {
            Desktop::Graph => {
                panes = crate::pane::graph_panes();
                (Layout::GraphReview, 2)
            }
            Desktop::Surface => {
                panes[0].kind = PaneKind::Solid;
                panes[1].kind = PaneKind::Unrolled;
                (Layout::SplitV, 1)
            }
            _ => (Layout::Single, 0),
        };
        Self { dock: Dock::for_desktop(desktop), panes, layout, active,
            viewport_layout: Some(crate::pane::ViewportLayout::new(layout)) }

    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Which tools are docked to a side. A set, not a list: `Tiles::iter`
    /// walks its own storage, which is not the order they were inserted in.
    fn tools_on(dock: &Dock, side: Side) -> std::collections::BTreeSet<&'static str> {
        dock.tree(side)
            .tiles
            .iter()
            .filter_map(|(_, t)| match t {
                egui_tiles::Tile::Pane(p) => Some(p.label()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_workspace_opens_on_the_views_its_work_needs() {
        use crate::pane::{Layout, PaneKind};
        // The surface is painted on the ring it wraps, so both are on screen.
        let surface = DesktopLayout::new(Desktop::Surface);
        assert_eq!(surface.layout, Layout::SplitV, "stacked, not side by side");
        assert_eq!(surface.panes[0].kind, PaneKind::Solid, "the ring on top");
        assert_eq!(surface.panes[1].kind, PaneKind::Unrolled, "its layout under it");
        assert_eq!(surface.active, 1, "the editor is what the work happens in");

        let graph = DesktopLayout::new(Desktop::Graph);
        assert_eq!(graph.layout, Layout::GraphReview);
        assert_eq!(graph.panes[2].kind, PaneKind::Graph);

        // Everything else opens on one view of its own thing.
        for desktop in [Desktop::Model, Desktop::Casting, Desktop::Cad] {
            let d = DesktopLayout::new(desktop);
            assert_eq!(d.layout, Layout::Single, "{desktop:?}");
            assert_eq!(d.panes[0].kind, desktop.pane(), "{desktop:?}");
        }
        // The CAD pane's parts are judged as they are built: the Report stands on its right, and nothing else is docked.
        let cad = DesktopLayout::new(Desktop::Cad);
        assert_eq!((tools_on(&cad.dock, Side::Left), tools_on(&cad.dock, Side::Right)), (Default::default(), ["Report"].into()));
        assert!(cad.dock.is_open(ToolKind::Report));
    }

    #[test]
    fn a_default_layout_is_the_one_every_workspace_opens_with() {
        // What `RingDesignerApp::restore_default_layout` puts back: the same
        // arrangement a fresh workspace builds, whatever was moved since.
        for desktop in Desktop::ALL {
            let fresh = DesktopLayout::new(desktop);

            let mut moved = DesktopLayout::new(desktop);
            for tool in ToolKind::ALL {
                moved.dock.close(*tool);
                moved.dock.open_on(*tool, Side::Right);
            }
            assert_ne!(
                tools_on(&moved.dock, Side::Right),
                tools_on(&fresh.dock, Side::Right),
                "{desktop:?}: the test has to actually move something"
            );

            let back = DesktopLayout::new(desktop);
            assert_eq!(back.layout, fresh.layout, "{desktop:?}");
            assert_eq!(back.active, fresh.active, "{desktop:?}");
            assert_eq!(back.panes.len(), fresh.panes.len(), "{desktop:?}");
            for side in Side::ALL {
                assert_eq!(tools_on(&back.dock, *side), tools_on(&fresh.dock, *side), "{desktop:?} {side:?}");
            }
            // Every view the preset names is back in the tree it hands over.
            let views = back.viewport_layout.as_ref().expect("a default layout carries its views");
            assert!(views.valid_for(fresh.layout), "{desktop:?}");
            assert_eq!(views.shown().len(), fresh.layout.count(), "{desktop:?}");
        }
    }
}
