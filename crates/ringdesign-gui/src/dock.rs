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
            ToolKind::Library => "Tiles",
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
