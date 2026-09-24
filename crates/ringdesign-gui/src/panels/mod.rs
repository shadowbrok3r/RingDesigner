//! Window layout and the top-level chrome.

pub mod builder;
pub mod cad;
pub mod casting;
pub mod design;
pub mod graph;
pub mod layers;
pub mod library;
pub mod node;
pub mod report;
pub mod section;
pub mod timeline;
pub mod unrolled;

use egui_phosphor::regular as icon;
use ringdesign_workbench::icons::Icon;

use crate::app::RingDesignerApp;
use crate::camera::StandardView;
use ringdesign_core::mesh::BuildParams;
use ringdesign_core::refine::RefineParams;

use crate::dock::{Side, ToolKind};
use crate::pane::{Layout, PaneKind};
use crate::viewport;
use crate::{export, theme};

/// Gap left between panes for the divider.
const GUTTER: f32 = 7.0;

pub fn render(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    ringdesign_graph_ui::alpha_picker::set_library(ui.ctx(), app.lib.clone());
    shortcuts(app, ui);
    workflow_window(app, ui.ctx());
    egui::Panel::top(egui::Id::new("toolbar")).show(ui, |ui| toolbar(app, ui));
    egui::Panel::bottom(egui::Id::new("status")).show(ui, |ui| status_bar(app, ui));

    if app.construction.open {
        egui::Panel::left(egui::Id::new("construction-guide"))
            .exact_size(350.)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Construction guide");
                    if ui.small_button("Close").clicked() {
                        app.construction.open = false;
                    }
                });
                let before = app.design.clone();
                let event = app.construction.ui(ui, &mut app.design);
                if event.changed {
                    app.history.commit(&before);
                    app.history.commit(&app.design);
                    let d = app.design.clone();
                    let lib = app.library_mut();
                    d.unpack_embedded(lib);
                    d.bake_all(lib);
                    app.selected_layer = None;
                    app.fit_pending = true;
                    app.show_grid = false;
                    app.finish = 0;
                    app.mark_dirty();
                }
                if let Some(view) = event.view {
                    use ringdesign_workbench::construction::View;
                    for pane in &mut app.panes {
                        pane.camera.yaw = app.design.shank.head.theta_deg.to_radians() as f32;
                        pane.camera.pitch = match view {
                            View::Seal => 0.,
                            View::ThreeQuarter => -0.72,
                            View::Cheek => -1.30,
                            View::Bore => -std::f32::consts::FRAC_PI_2 + 0.001,
                        };
                        if matches!(view, View::ThreeQuarter) {
                            pane.camera.yaw -= 0.48;
                        }
                        pane.turn = None;
                        pane.camera.centre_home();
                        pane.camera.zoom = 1.23;
                        pane.shade = viewport::ShadeMode::Metal;
                    }
                }
            });
    } else {
        for &side in Side::ALL {
            dock_side(app, ui, side);
        }
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::VIEWPORT_BG))
        .show(ui, |ui| panes(app, ui));
    command_palette(app, ui);
    ringdesign_graph_ui::alpha_picker::show(ui.ctx());
    if let Some(url) = ringdesign_workbench::feedback::show(ui.ctx(), env!("CARGO_PKG_VERSION"), app.desktop.label()) { ui.ctx().open_url(egui::OpenUrl::new_tab(url)); }
}

/// One edge of the window: a tile tree of docked tools.
fn dock_side(app: &mut RingDesignerApp, ui: &mut egui::Ui, side: Side) {
    if app.dock.tree(side).is_empty() {
        return;
    }
    let id = egui::Id::new(("dock", app.desktop.label(), side.label()));
    let panel = match side {
        Side::Left => egui::Panel::left(id),
        Side::Right => egui::Panel::right(id),
    };
    let width = app.dock.width_of(side);
    let desktop = app.desktop;
    let resp = panel
        .frame(egui::Frame::NONE.fill(theme::PANEL).stroke(egui::Stroke::new(1.5, theme::HAIRLINE)).inner_margin(6))
        .default_size(width)
        .size_range(egui::Rangef::new(240.0, 680.0))
        .show(ui, |ui| {
            let desktop_before = app.desktop;
            // The behaviour needs the app to draw a tool, and the tree lives in
            // the app, so it comes out for the duration of the call.
            let mut tree = std::mem::replace(
                app.dock.tree_mut(side),
                egui_tiles::Tree::empty(egui::Id::new(("dock_tmp", side.label()))),
            );
            let mut behavior = ToolBehavior {
                app,
                side,
                moved: None,
                closed: None,
            };
            tree.ui(&mut behavior, ui);
            let (moved, closed) = (behavior.moved, behavior.closed);
            if app.desktop == desktop_before {
                *app.dock.tree_mut(side) = tree;
            } else if let Some(saved) = app.desktops.get_mut(&desktop_before) {
                *saved.dock.tree_mut(side) = tree;
            }
            if app.desktop == desktop_before {
                if let Some(tool) = moved {
                    app.dock.open_on(tool, side.other());
                }
                if let Some(tool) = closed {
                    app.dock.close(tool);
                }
            }
        });
    let w = resp.response.rect.width();
    if app.desktop == desktop && (w - width).abs() > 0.5 {
        app.dock.set_width(side, w);
    }
}

/// Draws each docked tool and carries the "send to the other side" request back
/// out, since the tree cannot move a pane between two separate trees itself.
struct ToolBehavior<'a> {
    app: &'a mut RingDesignerApp,
    side: Side,
    moved: Option<ToolKind>,
    closed: Option<ToolKind>,
}

impl egui_tiles::Behavior<ToolKind> for ToolBehavior<'_> {
    fn tab_title_for_pane(&mut self, pane: &ToolKind) -> egui::WidgetText {
        format!("{} {}", pane.icon(), pane.label()).into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile: egui_tiles::TileId,
        pane: &mut ToolKind,
    ) -> egui_tiles::UiResponse {
        let tool = *pane;
        let bounds = ui.max_rect();
        let active = tool == ToolKind::Node && self.app.selected_node.is_some();
        let hovered = ui.rect_contains_pointer(bounds);
        let stroke = if active { theme::ACCENT } else if hovered { theme::ACCENT_DIM } else { theme::HAIRLINE };
        ui.painter().rect_stroke(bounds.shrink(1.0), 4.0, egui::Stroke::new(if active || hovered { 1.8 } else { 1.0 }, stroke), egui::StrokeKind::Inside);
        let mut drag = false;
        egui::Frame::NONE.inner_margin(8).show(ui, |ui| {
        ui.horizontal(|ui| {
            // The header is the handle: a press on it moves the tool within
            // this side's tree, which is the only tree it can be dropped in.
            let grip = ui
                .add(egui::Label::new(egui::RichText::new(icon::DOTS_SIX_VERTICAL).color(theme::TEXT_DIM)).selectable(false).sense(egui::Sense::drag()))
                .on_hover_text("Drag to rearrange this panel")
                .on_hover_cursor(egui::CursorIcon::Grab);
            drag |= grip.drag_started();
            ui.add(tool.atelier().image(ui, 18.0));
            ui.spacing_mut().button_padding = egui::vec2(3.0, 1.0);
            let title = ui.add(
                egui::Label::new(
                    egui::RichText::new(tool.label())
                        .strong()
                        .color(if active { theme::ACCENT } else { theme::TEXT }),
                )
                .selectable(false)
                .sense(egui::Sense::drag()),
            );
            drag |= title.drag_started();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button(icon::X).on_hover_text("Close this panel").clicked() {
                    self.closed = Some(tool);
                }
                let other = self.side.other();
                if ui
                    .small_button(match other {
                        Side::Left => icon::ARROW_LINE_LEFT,
                        Side::Right => icon::ARROW_LINE_RIGHT,
                    })
                    .on_hover_text(format!("Dock to the {} side", other.label().to_lowercase()))
                    .clicked()
                {
                    self.moved = Some(tool);
                }
            });
        });
        egui::ScrollArea::vertical()
            .id_salt(("tool", tool.label()))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // While a graph drives the design these panels only show;
                // the graph is where edits go.
                let driven = self.app.graph_driven()
                    && matches!(
                        tool,
                        ToolKind::Design | ToolKind::Layers | ToolKind::Library
                    );
                if driven {
                    driven_banner(self.app, ui, tool);
                }
                ui.add_enabled_ui(!driven, |ui| match tool {
                    ToolKind::Design => design::ui(self.app, ui),
                    ToolKind::Layers => layers::ui(self.app, ui),
                    ToolKind::Report => report::ui(self.app, ui),
                    ToolKind::Library => library::ui(self.app, ui),
                    ToolKind::Node => node::ui(self.app, ui),
                });
            });
        });
        if drag { egui_tiles::UiResponse::DragStarted } else { egui_tiles::UiResponse::None }
    }

    fn is_tab_closable(
        &self,
        _tiles: &egui_tiles::Tiles<ToolKind>,
        _id: egui_tiles::TileId,
    ) -> bool {
        true
    }

    fn gap_width(&self, _: &egui::Style) -> f32 { 7.0 }
    fn min_size(&self) -> f32 { 96.0 }
    fn resize_stroke(&self, _: &egui::Style, state: egui_tiles::ResizeState) -> egui::Stroke {
        let color = match state { egui_tiles::ResizeState::Idle => theme::HAIRLINE, egui_tiles::ResizeState::Hovering => theme::ACCENT_DIM, egui_tiles::ResizeState::Dragging => theme::ACCENT };
        egui::Stroke::new(2.0, color)
    }

    fn on_tab_close(
        &mut self,
        _tiles: &mut egui_tiles::Tiles<ToolKind>,
        _id: egui_tiles::TileId,
    ) -> bool {
        true
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            // Keep a lone tool in its container so its title bar survives.
            all_panes_must_have_tabs: false,
            ..Default::default()
        }
    }
}

/// Lay the visible panes out and draw each into its own sub-rect.
fn panes(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    use crate::pane::ViewportLayout;
    if !app.viewport_layout.valid_for(app.layout) {
        app.viewport_layout = ViewportLayout::new(app.layout);
    }
    let desktop = app.desktop;
    let preset = app.layout;
    let mut tree = std::mem::replace(&mut app.viewport_layout.tree, egui_tiles::Tree::empty("drawing-viewports"));
    let shown = tree.tiles.iter().filter(|(_, t)| matches!(t, egui_tiles::Tile::Pane(_))).count();
    let mut behavior = ViewportBehavior { app, shown, closed: None };
    tree.ui(&mut behavior, ui);
    // The last view cannot be closed: an empty tree is rebuilt from the preset
    // on the next frame, so the pane would simply come back.
    if let Some(tile) = behavior.closed.filter(|_| shown > 1) {
        tree.remove_recursively(tile);
        let left: Vec<_> = tree.tiles.iter().filter_map(|(_, t)| match t { egui_tiles::Tile::Pane(i) => Some(*i), _ => None }).collect();
        if let Some(first) = left.first() {
            if !left.contains(&app.active_pane) { app.active_pane = *first; }
        }
    }
    // A pane can open another workspace or choose a new preset while it draws.
    if app.desktop == desktop && app.layout == preset && app.viewport_layout.tree.root.is_none() {
        app.viewport_layout.tree = tree;
    } else if app.desktop != desktop {
        if let Some(saved) = app.desktops.get_mut(&desktop).and_then(|d| d.viewport_layout.as_mut()) {
            if saved.tree.root.is_none() { saved.tree = tree; }
        }
    }
}

struct ViewportBehavior<'a> {
    app: &'a mut RingDesignerApp,
    /// How many views this tree holds; the last one keeps no close button.
    shown: usize,
    closed: Option<egui_tiles::TileId>,
}
impl egui_tiles::Behavior<usize> for ViewportBehavior<'_> {
    fn tab_title_for_pane(&mut self, pane: &usize) -> egui::WidgetText { self.app.panes[*pane].kind.label().into() }
    fn pane_ui(&mut self, ui: &mut egui::Ui, tile: egui_tiles::TileId, pane: &mut usize) -> egui_tiles::UiResponse {
        let i = *pane;
        if i >= self.app.panes.len() { return egui_tiles::UiResponse::None; }
        let closable = self.shown > 1;
        let mut head = PaneHead::default();
        let app = &mut *self.app;
        let rect = ui.available_rect_before_wrap();
        if ui.input(|input| input.pointer.any_pressed() && input.pointer.interact_pos().is_some_and(|p| rect.contains(p)))
            && !app.palette_open && !ringdesign_graph_ui::alpha_picker::is_open(ui.ctx()) && !ringdesign_workbench::feedback::is_open(ui.ctx()) && !egui::Popup::is_any_open(ui.ctx()) {
            app.active_pane = i;
        }
        egui::Panel::top(egui::Id::new(("pane_head", i)))
            .frame(egui::Frame::NONE.fill(theme::PANEL).inner_margin(egui::Margin::symmetric(6, 3)))
            .show(ui, |ui| head = pane_head(app, ui, i, closable));
        if app.panes[i].kind == PaneKind::Solid {
            egui::Panel::bottom(egui::Id::new(("viewport-footer", i)))
                .frame(egui::Frame::NONE.fill(theme::PANEL).inner_margin(egui::Margin::symmetric(6, 5)).stroke(egui::Stroke::new(1., theme::HAIRLINE)))
                .show(ui, |ui| viewport_footer(app, ui, i));
            if timeline::shown(app) {
                egui::Panel::bottom(egui::Id::new(("viewport-timeline", i)))
                    .frame(egui::Frame::NONE.fill(theme::PANEL).inner_margin(egui::Margin::symmetric(6, 3)))
                    .show(ui, |ui| timeline::bar(app, ui, i));
            }
        }
        if app.panes[i].kind == PaneKind::Unrolled {
            egui::Panel::top(egui::Id::new(("surface-context", i)))
                .frame(egui::Frame::NONE.fill(theme::PANEL).inner_margin(6))
                .show(ui, |ui| surface_context(app, ui));
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(theme::VIEWPORT_BG)).show(ui, |ui| {
            match app.panes[i].kind {
                PaneKind::Solid => viewport::ui(app, ui, i),
                PaneKind::Unrolled => {
                    let editable = app.surface_edit_reason().is_none();
                    if !editable { app.band_paint = false; }
                    ui.add_enabled_ui(editable, |ui| unrolled::ui(app, ui));
                }
                PaneKind::Section => section::ui(app, ui, i),
                PaneKind::Graph => graph::ui(app, ui, i),
                PaneKind::Casting => casting::ui(app, ui),
                PaneKind::Cad => cad::ui(app, ui),
            }
        });
        if app.layout != Layout::Single && app.active_pane == i {
            ui.painter().rect_stroke(rect.shrink(1.), 0., egui::Stroke::new(1., theme::ACCENT_DIM), egui::StrokeKind::Inside);
        }
        if head.close { self.closed = Some(tile); }
        if head.drag { egui_tiles::UiResponse::DragStarted } else { egui_tiles::UiResponse::None }
    }
    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions { all_panes_must_have_tabs: false, ..Default::default() }
    }
    fn gap_width(&self, _: &egui::Style) -> f32 { GUTTER }
    fn min_size(&self) -> f32 { 140. }
    fn resize_stroke(&self, _: &egui::Style, state: egui_tiles::ResizeState) -> egui::Stroke {
        egui::Stroke::new(2., match state { egui_tiles::ResizeState::Idle => theme::HAIRLINE, egui_tiles::ResizeState::Hovering => theme::ACCENT_DIM, egui_tiles::ResizeState::Dragging => theme::ACCENT })
    }
}

fn surface_context(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    if let Some(reason) = app.surface_edit_reason() {
        ui.colored_label(theme::WARN, reason);
        ui.horizontal_wrapped(|ui| {
            if app.graph_driven() {
                if ui.button("Edit graph").clicked() { app.show_graph_pane(); }
                if ui.add_enabled(app.is_current(), egui::Button::new("Make editable")).on_hover_text("Keep the evaluated shape and remove its graph from this working design. Undo restores the graph.").clicked() {
                    app.bake_graph(); app.switch_desktop(crate::dock::Desktop::Surface);
                }
            } else if ui.button("Switch to Surface workspace").clicked() { app.switch_desktop(crate::dock::Desktop::Surface); }
        });
    } else { ui.weak("Direct editing · paint and arrange layers"); }
}

/// What a pane's own strip asks of the tree it sits in.
#[derive(Default)]
struct PaneHead {
    drag: bool,
    close: bool,
}

/// Per-pane strip: which view it shows, and the controls that view needs.
fn pane_head(app: &mut RingDesignerApp, ui: &mut egui::Ui, i: usize, closable: bool) -> PaneHead {
    let mut head = PaneHead::default();
    ui.horizontal(|ui| {
        // The grip is the handle: a press on it moves this view in the tree.
        let grip = ui
            .add(egui::Label::new(egui::RichText::new(icon::DOTS_SIX_VERTICAL).color(theme::TEXT_DIM)).selectable(false).sense(egui::Sense::drag()))
            .on_hover_text("Drag to rearrange this view")
            .on_hover_cursor(egui::CursorIcon::Grab);
        head.drag = grip.drag_started();
        let kind = app.panes[i].kind;
        if kind == PaneKind::Solid && app.panes[i].follow_node {
            ui.add(Icon::Locked.image(ui, 18.));
            ui.strong("Feature focus").on_hover_text("Locked orthographic preview follows the selected node and its edits.");
            head.close |= head_right(app, ui, i, closable, true);
            return;
        }
        let compact = ui.available_width() < 360.;
        let kind_width = (ui.available_width() * 0.45).clamp(80., 140.);
        egui::ComboBox::from_id_salt(("pane_kind", i))
            .selected_text(format!("{} {}", kind.icon(), kind.label()))
            .width(kind_width)
            .show_ui(ui, |ui| {
                for &k in PaneKind::ALL {
                    if ui
                        .selectable_label(kind == k, format!("{} {}", k.icon(), k.label()))
                        .clicked()
                    {
                        app.set_pane_kind(i, k);
                        ui.close();
                    }
                }
            });

        if app.panes[i].kind != PaneKind::Solid {
            head.close |= head_right(app, ui, i, closable, false);
            return;
        }

        ui.menu_button((ringdesign_workbench::icons::Icon::View.image(ui, 18.), if compact { "Cam" } else { "Camera" }), |ui| {
            camera_menu(app, ui, i);
        });
        if ui.available_width() >= 128. {
            ui.separator();
            let shade = app.panes[i].shade;
            let chip = crate::swatch::shade(ui.ctx(), shade);
            egui::ComboBox::from_id_salt(("pane_shade", i))
                .selected_text(shade.label())
                .icon(move |ui, rect, _visuals, _open| {
                    // The chip rides the combo's own arrow slot, so the
                    // closed box shows what the mode looks like.
                    let at = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(rect.height().min(16.)));
                    egui::Image::new((chip.id(), chip.size_vec2())).paint_at(ui, at);
                })
                .width(120.0)
                .show_ui(ui, |ui| {
                    for &m in viewport::ShadeMode::ALL {
                        let t = crate::swatch::shade(ui.ctx(), m);
                        if crate::swatch::row(ui, &t, m.label(), shade == m).clicked() {
                            app.panes[i].shade = m;
                            app.active_pane = i;
                            ui.close();
                        }
                    }
                });
        }
        head.close |= head_right(app, ui, i, closable, true);
    });
    head
}

/// The strip's right end: close, and on a 3D view the build quality left of
/// it. Quality belongs on the view because it is what that view costs to
/// draw — it was a menu away, three clicks from the picture it changes.
fn head_right(app: &mut RingDesignerApp, ui: &mut egui::Ui, i: usize, closable: bool, solid: bool) -> bool {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let closed = closable && ui.small_button(icon::X).on_hover_text("Close this view").clicked();
        // How the views are split is a property of the window, not of the
        // ring, so every pane offers it — a section or a graph is as likely
        // to be the one you are looking at when you want a second view.
        ui.menu_button(app.layout.icon(), |ui| layout_controls(app, ui, true))
            .response
            .on_hover_text("Split, arrange or restore the views");
        // Whatever is left after the close button, spent on the label only
        // while it fits: a split pane's strip is narrow.
        if solid && ui.available_width() >= 104. {
            let prefix = if ui.available_width() >= 172. { "Quality: " } else { "" };
            if quality_picker(ui, &format!("pane_quality{i}"), &mut app.preview_params, prefix) {
                app.mark_dirty();
            }
        }
        closed
    })
    .inner
}

/// The Camera menu: standard views and the shading modes, both with chips.
fn camera_menu(app: &mut RingDesignerApp, ui: &mut egui::Ui, i: usize) {
    if app.design.shank.kind == ringdesign_core::ShankKind::Signet {
        for (label, angled) in [("Seal", false), ("Signet 3/4", true)] {
            let t = crate::swatch::view_chip(ui.ctx(), if angled { StandardView::Iso } else { StandardView::Face });
            if crate::swatch::row(ui, &t, label, false).clicked() {
                let cam = &mut app.panes[i].camera;
                cam.yaw = app.design.shank.head.theta_deg.to_radians() as f32
                    - if angled { std::f32::consts::FRAC_PI_8 } else { 0. };
                cam.pitch = if angled { -0.55 } else { 0. };
                cam.centre_home();
                app.panes[i].turn = None;
                app.active_pane = i;
            }
        }
        ui.separator();
    }
    for &v in StandardView::ALL {
        let t = crate::swatch::view_chip(ui.ctx(), v);
        if crate::swatch::row(ui, &t, v.label(), false).clicked() {
            app.panes[i].camera.set_view(v);
            app.panes[i].turn = None;
            app.active_pane = i;
        }
    }
    ui.separator();
    for &mode in viewport::ShadeMode::ALL {
        let t = crate::swatch::shade(ui.ctx(), mode);
        if crate::swatch::row(ui, &t, mode.label(), app.panes[i].shade == mode).clicked() {
            app.panes[i].shade = mode;
            app.active_pane = i;
            ui.close();
        }
    }
}

/// The Preview menu. Every material choice carries the ball it shades, so
/// the metal, the polish and the light rig are read rather than clicked
/// through one at a time.
fn preview_menu(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    if app.showing_a_ring() {
        // Quality lives on each 3D view's own strip.
        ui.checkbox(&mut app.auto_rebuild, "Automatically rebuild changes");
        ui.separator();
    }
    ui.label("Surface polish");
    for (i, (name, _)) in ringdesign_core::render::POLISHES.iter().enumerate() {
        let t = crate::swatch::metal(ui.ctx(), app.finish, i, app.light);
        if crate::swatch::row(ui, &t, name, app.polish == i).clicked() { app.polish = i; }
    }
    ui.separator();
    ui.label("Metal");
    for (i, finish) in viewport::FINISHES.iter().enumerate() {
        let t = crate::swatch::metal(ui.ctx(), i, app.polish, app.light);
        if crate::swatch::row(ui, &t, finish.name, app.finish == i).clicked() { app.finish = i; }
    }
    ui.separator();
    ui.label("Lighting");
    for (i, light) in viewport::LIGHT_RIGS.iter().enumerate() {
        let t = crate::swatch::metal(ui.ctx(), app.finish, app.polish, i);
        if crate::swatch::row(ui, &t, light.name, app.light == i).clicked() { app.light = i; }
    }
}

/// Ctrl+Z / Ctrl+Shift+Z, plus Ctrl+Y for the redo people expect on Windows.
fn shortcuts(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    if ringdesign_graph_ui::alpha_picker::is_open(ui.ctx()) { return; }
    use egui::{Key, KeyboardShortcut, Modifiers};
    const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
    const REDO: KeyboardShortcut =
        KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
    const REDO_ALT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
    const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
    const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
    const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
    const PALETTE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::K);

    // A live viewport command or an armed box answers Escape itself.
    let viewport_holds = app.command.session.is_live() || app.command.box_armed;
    if !viewport_holds && !egui::Popup::is_any_open(ui.ctx()) && !app.palette_open && !ringdesign_graph_ui::alpha_picker::is_open(ui.ctx()) && !ringdesign_workbench::feedback::is_open(ui.ctx()) && !ui.ctx().egui_wants_keyboard_input() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        let cad_active = app.panes.get(app.active_pane).is_some_and(|p| p.kind == PaneKind::Cad);
        if !cad_active || !app.cad.cancel_shortcut() { app.clear_selection(); }
    }
    // Redo is checked first: its shortcut also matches undo's once the shift is
    // ignored, and consuming undo would swallow it.
    let (redo, redo_alt, undo, save, open, new, palette, delete) = ui.input_mut(|i| {
        (
            i.consume_shortcut(&REDO),
            i.consume_shortcut(&REDO_ALT),
            i.consume_shortcut(&UNDO),
            i.consume_shortcut(&SAVE),
            i.consume_shortcut(&OPEN),
            i.consume_shortcut(&NEW),
            i.consume_shortcut(&PALETTE),
            i.consume_key(Modifiers::NONE, Key::Delete),
        )
    });
    if redo || redo_alt {
        app.redo();
    } else if undo {
        app.undo();
    }
    if save {
        Command::Save.run(app);
    }
    if open {
        Command::Open.run(app);
    }
    if new {
        Command::New.run(app);
    }
    if palette {
        app.palette_open = !app.palette_open;
        app.palette_query.clear();
    }
    // Delete only acts when a layer is selected and no text field has focus —
    // egui routes Delete to text editing itself, but a consumed key here would
    // otherwise still fire while typing in a field that ignored it.
    if delete && !ui.ctx().memory(|m| m.focused().is_some()) {
        // The Delete key acts on what the active pane shows.
        let kind = app.panes.get(app.active_pane).map(|p| p.kind);
        let graph_pane = kind == Some(PaneKind::Graph);
        if kind == Some(PaneKind::Cad) && app.cad.delete_shortcut() {
        } else if graph_pane && app.selected_node.is_some() {
            Command::DeleteNode.run(app);
        } else if let Some(k) = app.selection.items.iter().rev().find_map(|s| match s {
            ringdesign_workbench::viewport::Sel::Stamp(k) => Some(*k),
            _ => None,
        }) {
            crate::viewport::stamp_edit(app, k, &ringdesign_workbench::viewport::StampEdit::Delete);
        } else {
            Command::DeleteLayer.run(app);
        }
    }

}

/// Everything the palette can do, one match away from the code that does it.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Command {
    New,
    Open,
    Save,
    ExportStl,
    ExportObj,
    Export3mf,
    ExportGlb,
    ExportStep,
    ImportPart,
    RenderPng,
    TurntableGif,
    CastingSheet,
    PartingLine,
    StoneMap,
    ToggleGhost,
    ToggleStones,
    ToggleAsCast,
    DeleteLayer,
    Undo,
    Redo,
    ConvertToGraph,
    BakeGraph,
    ShowGraphPane,
    ArrangeGraph,
    DeleteNode,
    DefaultLayout,
    CadWorkspace,
    CadAddBox,
    CadAddCylinder,
    CadAddSphere,
    CadAddExtrude,
    CadPreview,
    CadApply,
    CadDiscard,
    ToolMove,
    ToolRotate,
    ToolScale,
    ToolPlace,
    ToolAddBox,
    ToolAddCylinder,
    ToolAddSphere,
    ToolAttach,
    ToolArray,
    ToolPressPull,
}

/// The strip over a panel whose design is driven by a graph.
fn driven_banner(app: &mut RingDesignerApp, ui: &mut egui::Ui, tool: ToolKind) {
    egui::Frame::NONE
        .fill(theme::PANEL)
        .stroke(egui::Stroke::new(1.0, theme::ACCENT_DIM))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    theme::ACCENT,
                    format!("{} Driven by the graph", icon::GRAPH),
                );
                ui.weak("— edit the graph, or bake it to edit here.");
                if ui
                    .small_button(format!("{} Edit graph", icon::GRAPH))
                    .clicked()
                {
                    app.show_graph_pane();
                }
                if ui
                    .small_button(format!("{} Bake", icon::FIRE))
                    .on_hover_text("Drop the graph; keep the design as last evaluated")
                    .clicked()
                {
                    app.bake_graph();
                }
            });
            if tool == ToolKind::Layers && !app.design.layers.layers.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.weak("Edit in graph:");
                    let names: Vec<String> = app
                        .design
                        .layers
                        .layers
                        .iter()
                        .map(|e| e.name.clone())
                        .collect();
                    for (k, name) in names.iter().enumerate() {
                        if ui
                            .small_button(name)
                            .on_hover_text("Find the node that produced this layer")
                            .clicked()
                        {
                            app.edit_in_graph(k);
                        }
                    }
                });
            }
        });
    ui.add_space(4.0);
}

impl Command {
    pub(crate) const ALL: &'static [Command] = &[
        Command::New,
        Command::Open,
        Command::Save,
        Command::ExportStl,
        Command::ExportObj,
        Command::Export3mf,
        Command::ExportGlb,
        Command::ExportStep,
        Command::ImportPart,
        Command::RenderPng,
        Command::TurntableGif,
        Command::CastingSheet,
        Command::PartingLine,
        Command::StoneMap,
        Command::ToggleGhost,
        Command::ToggleStones,
        Command::ToggleAsCast,
        Command::DeleteLayer,
        Command::Undo,
        Command::Redo,
        Command::ConvertToGraph,
        Command::BakeGraph,
        Command::ShowGraphPane,
        Command::ArrangeGraph,
        Command::DeleteNode,
        Command::DefaultLayout,
        Command::CadWorkspace,
        Command::CadAddBox,
        Command::CadAddCylinder,
        Command::CadAddSphere,
        Command::CadAddExtrude,
        Command::CadPreview,
        Command::CadApply,
        Command::CadDiscard,
        Command::ToolMove,
        Command::ToolRotate,
        Command::ToolScale,
        Command::ToolPlace,
        Command::ToolAddBox,
        Command::ToolAddCylinder,
        Command::ToolAddSphere,
        Command::ToolAttach,
        Command::ToolArray,
        Command::ToolPressPull,
    ];

    /// The viewport command catalog key an entry starts, as its hotkey and its rail slot start it.
    pub(crate) fn tool(self) -> Option<&'static str> {
        Some(match self {
            Command::ToolMove => "move",
            Command::ToolRotate => "rotate",
            Command::ToolScale => "scale",
            Command::ToolPlace => "place",
            Command::ToolAddBox => "add-box",
            Command::ToolAddCylinder => "add-cylinder",
            Command::ToolAddSphere => "add-sphere",
            Command::ToolAttach => "attach",
            Command::ToolArray => "array",
            Command::ToolPressPull => "press-pull",
            _ => return None,
        })
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Command::New => "New design",
            Command::Open => "Open design…  (Ctrl+O)",
            Command::Save => "Save design as…  (Ctrl+S)",
            Command::ExportStl => "Export STL…",
            Command::ExportObj => "Export OBJ…",
            Command::Export3mf => "Export 3MF…",
            Command::ExportGlb => "Export GLB…",
            Command::ExportStep => "Export STEP…",
            Command::ImportPart => "Import part (STL, OBJ, STEP)…",
            Command::RenderPng => "Render PNG…",
            Command::TurntableGif => "Turntable GIF…",
            Command::CastingSheet => "Casting sheet…",
            Command::PartingLine => "Parting line SVG…",
            Command::StoneMap => "Stone map SVG…",
            Command::ToggleGhost => "Toggle comparison ghost",
            Command::ToggleStones => "Toggle stone previews",
            Command::ToggleAsCast => "Toggle as-cast softening",
            Command::DeleteLayer => "Delete selected layer  (Del)",
            Command::Undo => "Undo  (Ctrl+Z)",
            Command::Redo => "Redo  (Ctrl+Shift+Z)",
            Command::ConvertToGraph => "Convert design to graph",
            Command::BakeGraph => "Bake graph into the design",
            Command::ShowGraphPane => "Show graph pane",
            Command::ArrangeGraph => "Arrange graph nodes",
            Command::DeleteNode => "Delete selected node  (Del)",
            Command::DefaultLayout => "Default layout for this workspace",
            Command::CadWorkspace => "CAD: open the workspace",
            Command::CadAddBox => "CAD: add a box",
            Command::CadAddCylinder => "CAD: add a cylinder",
            Command::CadAddSphere => "CAD: add a sphere",
            Command::CadAddExtrude => "CAD: add an extrusion",
            Command::CadPreview => "CAD: preview the candidate  (Enter)",
            Command::CadApply => "CAD: apply the candidate  (Ctrl+Enter)",
            Command::CadDiscard => "CAD: set the candidate aside",
            Command::ToolMove => "Move selected part  (G)",
            Command::ToolRotate => "Rotate selected part  (R)",
            Command::ToolScale => "Scale selected part  (S)",
            Command::ToolPlace => "Place selected part on the ring  (P)",
            Command::ToolAddBox => "Add a box on the ring  (Shift+A)",
            Command::ToolAddCylinder => "Add a cylinder on the ring  (Shift+A)",
            Command::ToolAddSphere => "Add a sphere on the ring  (Shift+A)",
            Command::ToolAttach => "Cycle Join / Cut / Separate  (J)",
            Command::ToolArray => "Array the selected part round the ring  (A)",
            Command::ToolPressPull => "Press-pull the selected face  (Q)",
        }
    }

    pub(crate) fn run(self, app: &mut RingDesignerApp) {
        match self {
            Command::New => {
                app.document_path = None;
                app.design = ringdesign_core::RingDesign::default();
                app.history.reset(&app.design.clone());
                app.selected_layer = None;
                app.fit_pending = true;
                app.mark_dirty();
            }
            Command::Open => export::open_design(app),
            Command::Save => export::save_design(app),
            Command::ExportStl => export::export_stl(app),
            Command::ExportObj => export::export_obj(app),
            Command::Export3mf => export::export_3mf(app),
            Command::ExportGlb => export::export_glb(app),
            Command::ExportStep => export::export_step(app),
            Command::ImportPart => export::import_part(app),
            Command::RenderPng => export::export_render(app),
            Command::TurntableGif => export::export_turntable(app),
            Command::CastingSheet => export::export_spec(app),
            Command::PartingLine => export::export_parting(app),
            Command::StoneMap => export::export_stone_map(app),
            Command::ToggleGhost => app.toggle_pin(),
            Command::ToggleStones => {
                app.show_gems = !app.show_gems;
                app.mark_dirty();
            }
            Command::ToggleAsCast => {
                app.as_cast = !app.as_cast;
                app.mark_dirty();
            }
            Command::DeleteLayer => {
                if let Some(i) = app.selected_layer
                    && i < app.design.layers.layers.len()
                {
                    let name = app.design.layers.layers.remove(i).name;
                    app.selected_layer = None;
                    app.mark_dirty();
                    app.set_status(format!("Deleted layer {name}"));
                }
            }
            Command::Undo => app.undo(),
            Command::Redo => app.redo(),
            Command::ConvertToGraph => app.convert_to_graph(),
            Command::BakeGraph => app.bake_graph(),
            Command::ShowGraphPane => app.show_graph_pane(),
            Command::ArrangeGraph => app.arrange_graph(),
            Command::DeleteNode => app.delete_selected_node(),
            Command::DefaultLayout => app.restore_default_layout(),
            Command::CadWorkspace => app.focus(PaneKind::Cad),
            Command::CadAddBox => cad::add_starter(app, "Box", None),
            Command::CadAddCylinder => cad::add_starter(app, "Cylinder", None),
            Command::CadAddSphere => cad::add_starter(app, "Sphere", None),
            Command::CadAddExtrude => cad::add_starter(app, "Extrude", None),
            Command::CadPreview => cad::ask(app, cad::CadRequest::Preview),
            Command::CadApply => cad::ask(app, cad::CadRequest::Apply),
            Command::CadDiscard => cad::ask(app, cad::CadRequest::Discard),
            Command::ToolMove
            | Command::ToolRotate
            | Command::ToolScale
            | Command::ToolPlace
            | Command::ToolAddBox
            | Command::ToolAddCylinder
            | Command::ToolAddSphere
            | Command::ToolAttach
            | Command::ToolArray
            | Command::ToolPressPull => {
                if let Some(key) = self.tool() {
                    crate::command::start(app, key);
                }
            }
        }
    }
}

/// Ctrl+K: a centred filter-and-run list over every command.
fn command_palette(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    if !app.palette_open {
        return;
    }
    let ctx = ui.ctx().clone();
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.palette_open = false;
        return;
    }
    let mut run: Option<Command> = None;
    let id = egui::Id::new("command-palette-modal");
    let modal = egui::Modal::new(id)
        .area(egui::Modal::default_area(id).anchor(egui::Align2::CENTER_TOP, egui::vec2(0., 80.)))
        .show(&ctx, |ui| {
            ui.set_width(360.0_f32.min(ctx.content_rect().width() - 32.));
            ui.strong("Command search");
            let (up, down, go) = ui.input_mut(|i| (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            ));
            let edit = ui.add(
                egui::TextEdit::singleline(&mut app.palette_query)
                    .hint_text("Type a command…")
                    .desired_width(f32::INFINITY),
            );
            edit.request_focus();
            let query = app.palette_query.to_lowercase();
            let hits: Vec<Command> = Command::ALL.iter().copied()
                .filter(|c| c.label().to_lowercase().contains(&query)).collect();
            if edit.changed() { app.palette_selection = 0; }
            let count = hits.len();
            app.palette_selection = app.palette_selection.min(count.saturating_sub(1));
            if count > 0 {
                if down { app.palette_selection = (app.palette_selection + 1) % count; }
                if up { app.palette_selection = (app.palette_selection + count - 1) % count; }
                if go { run = Some(hits[app.palette_selection]); }
            }
            let height = (ctx.content_rect().height() - 210.).clamp(100., 520.);
            let list_height = ((ui.spacing().interact_size.y + ui.spacing().item_spacing.y) * count as f32).clamp(40., height);
            egui::ScrollArea::vertical().auto_shrink([false, false])
                .min_scrolled_height(list_height).max_height(list_height).show(ui, |ui| {
                for (k, c) in hits.iter().enumerate() {
                    let selected = k == app.palette_selection;
                    let response = ui.add_sized(
                        [ui.available_width(), ui.spacing().interact_size.y],
                        egui::Button::new(c.label()).selected(selected).right_text(egui::Atom::grow()),
                    );
                    if selected && (up || down || edit.changed()) {
                        response.scroll_to_me(None);
                    }
                    if response.clicked() { run = Some(*c); }
                }
                if hits.is_empty() { ui.weak("No matching command"); }
            });
            ui.small("Up/Down Choose    Enter Run    Esc Close");
        });
    if modal.should_close() { app.palette_open = false; }
    if let Some(c) = run {
        app.palette_open = false;
        c.run(app);
    }
}

/// Undo, redo, and the timeline they walk.
fn history_controls(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    use ringdesign_workbench::icons::{self, Icon};
    // Keep the targets available while a just-finished gesture is entering
    // history. egui hit-tests against the preceding frame.
    let button = |ui: &mut egui::Ui, icon, available: bool| {
        ui.add_enabled_ui(available, |ui| icons::compact(ui, icon, false)).inner
    };
    if button(ui, Icon::Undo, app.history.can_undo() || app.history.is_pending()).clicked() {
        app.undo();
    }
    if button(ui, Icon::Redo, app.history.can_redo()).clicked() {
        app.redo();
    }

    ui.menu_button((Icon::History.image(ui, 18.), "History"), |ui| {
        let timeline = app.history.timeline();
        let present = app.history.present();
        ui.set_min_width(240.0);
        egui::ScrollArea::vertical()
            .max_height(320.0)
            .show(ui, |ui| {
                // Newest at the top, which is where the eye goes first.
                for (i, (label, now)) in timeline.iter().enumerate().rev() {
                    let text = if i == 0 && present == 0 {
                        egui::RichText::new(label.clone())
                    } else {
                        egui::RichText::new(label.clone())
                    };
                    let text = if *now {
                        text.color(theme::ACCENT)
                    } else if i > present {
                        // Ahead of the present: still reachable, but undone.
                        text.color(theme::TEXT_DIM)
                    } else {
                        text
                    };
                    if ui.selectable_label(*now, text).clicked() {
                        app.jump_history(i);
                        ui.close();
                    }
                }
            });
    })
    .response
    .on_hover_text("Step back to any point in the session");
}

fn atelier_button(ui: &mut egui::Ui, icon: Icon, label: &str) -> egui::Response {
    ui.add(egui::Button::image_and_text(icon.image(ui, 18.), label).image_tint_follows_text_color(false))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn toolbar(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    use ringdesign_workbench::icons::{self, Icon};
    let row = ui.available_rect_before_wrap();
    let compact = row.width() < 1300.;
    ui.spacing_mut().item_spacing.x = if compact { 3. } else { 6. };
    ui.horizontal(|ui| {
        ui.menu_button((Icon::Files.image(ui, 18.0), "File"), |ui| file_menu(app, ui));
        ui.menu_button((Icon::View.image(ui, 18.0), "View"), |ui| {
            ui.menu_button("Panels", |ui| {
                for &tool in ToolKind::ALL {
                    let open = app.dock.is_open(tool);
                    if icons::button(ui, tool.atelier(), tool.label(), open, egui::vec2(160., 28.)).clicked() {
                        app.dock.toggle(tool, !open);
                    }
                }
            });
            ui.menu_button("Viewport layout", |ui| layout_controls(app, ui, true));
            ui.separator();
            if ui.button("Default layout")
                .on_hover_text("Put this workspace's panels and views back where they start")
                .clicked()
            {
                app.restore_default_layout();
                ui.close();
            }
            if ui.button("Reset cameras").clicked() {
                let bounds = app.build.as_ref().and_then(|b| b.mesh.bounds());
                for pane in &mut app.panes {
                    if !pane.follow_node { pane.camera.reset(); pane.camera.fit(bounds); }
                }
                ui.close();
            }
        });
        ui.separator();
        let workspace_label = format!("{} workspace", app.desktop.label());
        let workspace = ui.menu_button((app.desktop.icon().image(ui, 18.0), egui::RichText::new(if compact { app.desktop.label() } else { &workspace_label }).strong().color(theme::ACCENT)), |ui| {
            ui.weak("Workspaces remember their own panels");
            for desktop in crate::dock::Desktop::ALL {
                if icons::button(ui, desktop.icon(), desktop.label(), desktop == app.desktop, egui::vec2(180., 30.)).clicked() {
                    app.switch_desktop(desktop);
                    ui.close();
                }
            }
        });
        workspace.response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &workspace_label));
        ui.menu_button((Icon::Workshop.image(ui, 18.0), "Tools"), |ui| {
            if icons::button(ui, Icon::Guide, "Workflow guide", false, egui::vec2(0., 28.)).clicked() {
                ui.data_mut(|d| d.insert_temp(egui::Id::new("workflow-open"), true)); ui.close();
            }
            if icons::button(ui, Icon::Shape, "Construction guide", app.construction.open, egui::vec2(0., 28.)).clicked() {
                app.construction.open = !app.construction.open; ui.close();
            }
            ui.separator();
            ui.menu_button("MCP server", |ui| mcp_control(app, ui));
            if ui.button("Feature request / bug report…").clicked() { ringdesign_workbench::feedback::open(ui.ctx()); ui.close(); }
            if ui.button("Command search…  Ctrl+K").clicked() { app.palette_open = true; ui.close(); }
        });
        history_controls(app, ui);
        let left_end = ui.min_rect().right();
        let right_start = ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let can_restart = !app.is_building() && app.exporting.is_none();
            app.install_update |= app.updater.menu(ui, can_restart);
            let exporting = app.exporting.is_some();
            ui.add_enabled_ui(!exporting, |ui| {
                ui.menu_button((Icon::Export.image(ui, 18.0), "Export"), |ui| export_menu(app, ui));
            }).response.on_disabled_hover_text("An export is running");
            let shown = app.viewport_layout.shown();
            let cad_visible = if shown.is_empty() {
                (0..app.layout.count()).any(|i| app.panes.get(i).is_some_and(|p| p.kind == PaneKind::Cad))
            } else {
                shown.into_iter().any(|i| app.panes.get(i).is_some_and(|p| p.kind == PaneKind::Cad))
            };
            if app.showing_a_ring() || cad_visible {
                ui.menu_button((Icon::View.image(ui, 18.0), "Preview"), |ui| preview_menu(app, ui));
            }
            let building = app.is_building();
            let candidate_only = cad_visible && !app.showing_a_ring();
            ui.add_enabled_ui(!building && !candidate_only, |ui| {
                if icons::compact(ui, Icon::Rebuild, false).clicked() { app.rebuild_now(); }
            }).response.on_disabled_hover_text(if candidate_only { "Use Preview in CAD to evaluate the candidate." } else { "Mesh rebuild in progress" });
            if building || exporting { ui.add(egui::Spinner::new().size(16.)); }
            ui.min_rect().left()
        }).inner;
        let center = row.center().x;
        let half = (center - left_end - 8.).min(right_start - center - 8.).max(0.);
        if half > 8. {
            let title = app.document_path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| app.design.name.clone());
            let rect = egui::Rect::from_center_size(egui::pos2(center, ui.min_rect().center().y), egui::vec2(half * 2., ui.spacing().interact_size.y));
            ui.put(rect, egui::Label::new(egui::RichText::new(&title).strong()).truncate().selectable(false))
                .on_hover_text(app.document_path.as_ref().map(|p| p.display().to_string()).unwrap_or(title));
        }
    });
}

fn file_menu(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
            if atelier_button(ui, Icon::Add, "New").clicked() {
                Command::New.run(app);
                ui.close();
            }
            ui.menu_button(format!("{} New from template", icon::SPARKLE), |ui| {
                if let Some(template) = ringdesign_workbench::templates::menu(ui) {
                    export::load_catalog_template(app, template);
                    ui.close();
                }
            });
            if atelier_button(ui, Icon::Files, "Open…").clicked() {
                export::open_design(app);
                ui.close();
            }
            if atelier_button(ui, Icon::Files, "Import part…")
                .on_hover_text("An STL, OBJ or STEP solid, kept in the design and joined at the top of the ring")
                .clicked()
            {
                export::import_part(app);
                ui.close();
            }
            ui.menu_button(format!("{} Recent", icon::CLOCK), |ui| {
                if app.recent.is_empty() {
                    ui.weak("Nothing opened or saved yet");
                }
                let mut pick = None;
                for r in &app.recent {
                    let path = std::path::PathBuf::from(r);
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| r.clone());
                    let missing = !path.exists();
                    let btn = ui.add_enabled(!missing, egui::Button::new(&name)).on_hover_text(r);
                    if missing {
                        btn.on_disabled_hover_text("File no longer exists");
                    } else if btn.clicked() {
                        pick = Some(path);
                        ui.close();
                    }
                }
                if let Some(path) = pick {
                    export::open_design_path(app, &path);
                }
                if !app.recent.is_empty() {
                    ui.separator();
                    if ui.button("Clear list").clicked() {
                        app.recent.clear();
                        ui.close();
                    }
                }
            });
            if atelier_button(ui, Icon::Save, "Save As…").clicked() {
                export::save_design(app);
                ui.close();
            }
            if atelier_button(ui, Icon::Export, "Export STEP…")
                .on_hover_text("The whole ring for CAD programs: parts exact, the band and meshes faceted")
                .clicked()
            {
                export::export_step(app);
                ui.close();
            }

}

fn export_menu(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Shrink for");
                let current = app
                    .shrink_metal
                    .and_then(|i| ringdesign_core::metal::METALS.get(i))
                    .map(|m| format!("{} +{:.1}%", m.name, m.shrink_pct))
                    .unwrap_or_else(|| "Nominal".into());
                egui::ComboBox::from_id_salt("shrink_metal")
                    .selected_text(current)
                    .width(140.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.shrink_metal, None, "Nominal");
                        for (i, m) in ringdesign_core::metal::METALS.iter().enumerate() {
                            ui.selectable_value(
                                &mut app.shrink_metal,
                                Some(i),
                                format!("{} +{:.1}%", m.name, m.shrink_pct),
                            );
                        }
                    });
            })
            .response
            .on_hover_text(
                "Cut exported patterns oversize so the cast cools to nominal size. \
                 The file is named as a pattern so it cannot be mistaken for nominal.",
            );
            if atelier_button(ui, Icon::Export, "Export STL…").clicked() {
                export::export_stl(app);
                ui.close();
            }
            if atelier_button(ui, Icon::Export, "Export OBJ…").clicked() {
                export::export_obj(app);
                ui.close();
            }
            if atelier_button(ui, Icon::Export, "Export STEP…")
                .on_hover_text("The whole ring for CAD programs: every part the kernel built exact, the band and the builders' meshes as faceted solids.")
                .clicked()
            {
                export::export_step(app);
                ui.close();
            }
            if atelier_button(ui, Icon::Casting, "Casting sheet…")
                .on_hover_text(
                    "A printable HTML tech sheet: dimensions, weights, the field verdict and \
                     its notes, stones, and DFM findings — everything the pour needs.",
                )
                .clicked()
            {
                export::export_spec(app);
                ui.close();
            }
            if atelier_button(ui, Icon::Workshop, "Cost JSON…")
                .on_hover_text("Volume and per-alloy weights for the cost calculator.")
                .clicked()
            {
                export::export_cost_json(app);
                ui.close();
            }
            if atelier_button(ui, Icon::Mould, "Parting line…")
                .on_hover_text("The mould split as a printable SVG: plan view plus the line's height unrolled.")
                .clicked()
            {
                export::export_parting(app);
                ui.close();
            }
            if atelier_button(ui, Icon::Stones, "Stone map…")
                .on_hover_text("Every stone to scale, plan and unrolled, with the tight gaps drawn: the setter's map.")
                .clicked()
            {
                export::export_stone_map(app);
                ui.close();
            }
            if atelier_button(ui, Icon::View, "Render PNG…")
                .on_hover_text("A polished still at export resolution, tinted to the chosen finish.")
                .clicked()
            {
                export::export_render(app);
                ui.close();
            }
            if atelier_button(ui, Icon::View, "Turntable GIF…")
                .on_hover_text("A looping 36-frame spin — takes a few seconds to build and draw.")
                .clicked()
            {
                export::export_turntable(app);
                ui.close();
            }
            if atelier_button(ui, Icon::Export, "Export GLB…")
                .on_hover_text("glTF binary with the alloy's PBR tint — for renders and web viewers.")
                .clicked()
            {
                export::export_glb(app);
                ui.close();
            }
            if atelier_button(ui, Icon::Export, "Export 3MF…")
                .on_hover_text("Zip-packaged model that states its units — no mm/inch guessing downstream.")
                .clicked()
            {
                export::export_3mf(app);
                ui.close();
            }
}

fn layout_controls(app: &mut RingDesignerApp, ui: &mut egui::Ui, labels: bool) {
    use ringdesign_workbench::icons::{self, Icon};
    for (&layout, symbol) in Layout::ALL.iter().zip([Icon::Single, Icon::SplitVertical, Icon::SplitHorizontal, Icon::Four, Icon::Graph]) {
        if icons::button(ui, symbol, if labels { layout.label() } else { "" }, app.layout == layout, egui::vec2(if labels { 180. } else { 28. }, 28.)).clicked() {
            app.set_layout(layout);
        }
    }
    // A preset restores the views; this restores the panels with them, which
    // is what "I have moved a bunch of stuff around" asks for.
    let reset = icons::button(ui, Icon::Reset, if labels { "Default layout" } else { "" }, false, egui::vec2(if labels { 180. } else { 28. }, 28.));
    // Named even without its label, so the icon-only cluster still says what
    // it does to a reader and to anything driving the app.
    reset.response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Default layout"));
    if reset.clicked() {
        app.restore_default_layout();
    }
}

fn viewport_footer(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize) {
    use ringdesign_workbench::{icons::{self, Icon}, visual::Tool};
    ui.horizontal(|ui| {
        if app.panes[pane].follow_node {
            ui.weak("Orthographic · follows selected node");
            return;
        }
        let tool = app.visual.tool;
        let narrow = ui.available_width() < 390.;
        ui.menu_button((tool.icon().image(ui, 18.), if narrow { "Tool" } else { tool.label() }), |ui| {
            for choice in Tool::ALL {
                let enabled = app.surface_edit_reason().is_none() || !matches!(choice, Tool::Paint | Tool::Stamp | Tool::Path | Tool::Transform);
                ui.add_enabled_ui(enabled, |ui| {
                    if icons::button(ui, choice.icon(), choice.label(), tool == choice, egui::vec2(160., 28.)).clicked() {
                        app.active_pane = pane; app.visual.select(choice); ui.close();
                    }
                }).response.on_disabled_hover_text(app.surface_edit_reason().unwrap_or("This preview follows the selected node"));
            }
        });
        let selected = app.selected_layer.is_some() || app.selected_node.is_some() || app.probe.is_some() || tool != Tool::Select;
        ui.add_enabled_ui(selected, |ui| {
            if icons::compact(ui, Icon::Close, false).response.on_hover_text("Clear selection / exit tool • Esc • click empty space").clicked() { app.clear_selection(); }
        }).response.on_disabled_hover_text("Nothing selected — drag to orbit");
        ui.separator();
        if !narrow {
        if icons::compact(ui, Icon::Wire, app.show_wireframe).clicked() { app.show_wireframe = !app.show_wireframe; }
        if icons::compact(ui, Icon::Grid, app.show_grid).clicked() { app.show_grid = !app.show_grid; }
        ui.add_enabled_ui(app.build.is_some(), |ui| {
            if icons::compact(ui, Icon::Ghost, app.pinned.is_some()).clicked() { app.toggle_pin(); }
        }).response.on_disabled_hover_text("Build the ring before pinning a ghost");
        }
        ui.menu_button((Icon::Layers.image(ui, 18.), if narrow { "View" } else { "Display" }), |ui| {
            if narrow {
                if icons::button(ui, Icon::Wire, "Wire", app.show_wireframe, egui::vec2(160.,28.)).clicked() { app.show_wireframe = !app.show_wireframe; }
                if icons::button(ui, Icon::Grid, "Grid", app.show_grid, egui::vec2(160.,28.)).clicked() { app.show_grid = !app.show_grid; }
                ui.add_enabled_ui(app.build.is_some(), |ui| {
                    if icons::button(ui, Icon::Ghost, "Ghost", app.pinned.is_some(), egui::vec2(160.,28.)).clicked() { app.toggle_pin(); }
                });
                ui.separator();
            }
            ui.add_enabled_ui(app.stones.as_ref().is_some_and(|report| report.stone_count > 0), |ui| {
                if icons::button(ui, Icon::Stones, "Stones", app.show_gems, egui::vec2(160.,28.)).clicked() { app.show_gems = !app.show_gems; }
            }).response.on_disabled_hover_text("This design has no stones");
            let part_edges = icons::button(ui, Icon::CadBox, "Part edges", app.show_part_edges, egui::vec2(160.,28.));
            if part_edges.clicked() {
                app.show_part_edges = !app.show_part_edges;
            }
            part_edges.response.on_hover_text("Every CAD part's edges and every builder's creases over the metal; a chosen or hovered edge shows either way");
            let mut changed = false;
            if icons::button(ui, Icon::Cutters, "Live cuts", app.live_cuts, egui::vec2(160.,28.)).clicked() { app.live_cuts = !app.live_cuts; changed = true; }
            if icons::button(ui, Icon::Cutters, "Cutters", app.show_cutters, egui::vec2(160.,28.)).clicked() { app.show_cutters = !app.show_cutters; changed = true; }
            if icons::button(ui, Icon::Casting, "As-cast surface", app.as_cast, egui::vec2(160.,28.)).clicked() { app.as_cast = !app.as_cast; changed = true; }
            if changed { app.mark_dirty(); }
        });
    });
}

/// Start/stop toggle for the embedded MCP server.
fn mcp_control(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    if let Some(addr) = app.mcp_addr() {
        if ui
            .button(
                egui::RichText::new(format!("{} MCP", icon::PLUGS_CONNECTED)).color(theme::GOOD),
            )
            .on_hover_text(format!("Serving http://{addr}/ — click to stop"))
            .clicked()
        {
            app.stop_mcp();
        }
        ui.label(egui::RichText::new(addr.to_string()).color(theme::TEXT_DIM));
        return;
    }

    if atelier_button(ui, Icon::Settings, "MCP")
        .on_hover_text("Serve this design to an agent over MCP on 127.0.0.1")
        .clicked()
    {
        app.start_mcp();
    }
    ui.add(
        egui::DragValue::new(&mut app.mcp_port)
            .range(1..=65535)
            .speed(1.0),
    )
    .on_hover_text("Listen port");

    if let Some(err) = app.mcp_error.clone() {
        let short: String = err.chars().take(34).collect();
        ui.label(egui::RichText::new(format!("{} {short}", icon::WARNING)).color(theme::BAD))
            .on_hover_text(err);
    }
}

/// Pick how the mesh is built: a swept grid at a step count, or refinement to
/// a tolerance. Returns whether the choice changed.
///
/// One control rather than two, because the question is the same either way —
/// how close to the design should the mesh sit — and the two families answer it
/// differently. A swept grid spends its resolution everywhere, so below about
/// 0.05 mm refining is both smaller and faster; above it the sweep wins because
/// it is a trivial loop.
pub fn quality_picker(ui: &mut egui::Ui, salt: &str, params: &mut BuildParams, prefix: &str) -> bool {
    // Inside a menu this is a submenu, not a combo box. A menu closes on any
    // click it does not recognise and only a submenu registers itself as the
    // open item, so a combo opened here shut the menu around it.
    if egui::containers::menu::is_in_menu(ui) {
        let mut changed = false;
        ui.menu_button(format!("{} {prefix}{}", icon::GAUGE, quality_label(params)), |ui| {
            changed = quality_options(ui, params);
        })
        .response
        .on_hover_text(QUALITY_HINT);
        return changed;
    }
    let mut changed = false;
    egui::ComboBox::from_id_salt(salt)
        .selected_text(format!("{prefix}{}", quality_label(params)))
        .width(if prefix.is_empty() { 118.0 } else { 168.0 })
        .show_ui(ui, |ui| changed = quality_options(ui, params))
        .response
        .on_hover_text(QUALITY_HINT);
    changed
}

const QUALITY_HINT: &str = "A swept grid is fastest to build; refining puts the triangles only \
     where the surface bends, which is far fewer of them below about 0.05 mm.";

/// The two families of build setting, as one list of choices.
fn quality_options(ui: &mut egui::Ui, params: &mut BuildParams) -> bool {
    let mut changed = false;
    ui.label(
        egui::RichText::new("Swept grid — fixed step count")
            .small()
            .color(theme::TEXT_DIM),
    );
    for &(name, t, p) in BuildParams::PRESETS {
        let at = params.refine.is_none() && params.theta_steps == t && params.profile_steps == p;
        if ui
            .selectable_label(at, format!("{name} • {}k tris", t * p * 2 / 1000))
            .clicked()
        {
            params.theta_steps = t;
            params.profile_steps = p;
            params.refine = None;
            changed = true;
            ui.close();
        }
    }

    ui.separator();
    ui.label(
        egui::RichText::new("Refined — to a tolerance")
            .small()
            .color(theme::TEXT_DIM),
    );
    for &(name, tol, tilt) in RefineParams::PRESETS {
        let at = params.refine.is_some_and(|r| r.tolerance_mm == tol);
        if ui.selectable_label(at, format!("{name} • {tol} mm")).clicked() {
            params.refine = Some(RefineParams {
                tolerance_mm: tol,
                normal_tolerance_deg: tilt,
                ..RefineParams::default()
            });
            changed = true;
            ui.close();
        }
    }
    changed
}

fn quality_label(params: &BuildParams) -> String {
    if let Some(r) = params.refine {
        return match RefineParams::PRESETS
            .iter()
            .find(|(_, t, _)| *t == r.tolerance_mm)
        {
            Some((name, _, _)) => format!("{name} • {} mm", r.tolerance_mm),
            None => format!("{} mm", r.tolerance_mm),
        };
    }
    match BuildParams::PRESETS
        .iter()
        .find(|(_, t, p)| *t == params.theta_steps && *p == params.profile_steps)
    {
        Some((name, _, _)) => name.to_string(),
        None => format!("{}x{}", params.theta_steps, params.profile_steps),
    }
}

fn status_bar(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        // The field-sampled verdict, not the mesh analyzer's. A refined build
        // reports a phantom on the crest line that does not fall with the
        // tolerance, so the chip used to disagree with the banner six inches
        // above it on exactly the designs that most need a clear answer.
        if casting::active(app) {
            casting::status_chip(app, ui);
        } else if app
            .panes
            .get(app.active_pane)
            .is_some_and(|p| p.kind == PaneKind::Cad)
        {
            ui.weak("CAD candidate; see feature inspection");
        } else if !app.is_current() {
            ui.weak("Geometry report pending or unavailable");
        } else if !app.design.band_is_procedural() {
            ui.weak("CAD components — use Casting for release inspection");
        } else {
            let verdict = app
                .field
                .as_ref()
                .map(|f| f.verdict)
                .or_else(|| app.cast.as_ref().map(|c| c.verdict));
            let (glyph, color) = match verdict {
                Some(v) => (
                    match v {
                        ringdesign_core::castability::Verdict::Castable => icon::CHECK_CIRCLE,
                        _ => icon::WARNING,
                    },
                    theme::verdict_color(v),
                ),
                None => (icon::CIRCLE_DASHED, theme::TEXT_DIM),
            };
            match verdict {
                Some(v) => {
                    ui.label(egui::RichText::new(format!("{glyph} {}", v.label())).color(color))
                }
                None => ui.label(egui::RichText::new(format!("{glyph} —")).color(color)),
            };
        }

        ui.separator();
        ui.label(egui::RichText::new(&app.status).color(theme::TEXT_DIM));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(b) = &app.build {
                let r = &b.report;
                ui.label(
                    egui::RichText::new(format!(
                        "{} • {:.1} x {:.1} mm • {:.2} g in 14k",
                        app.design.size.display(),
                        r.outer_diameter_mm,
                        r.band_width_mm,
                        r.metals
                            .iter()
                            .find(|m| m.metal == "Gold 14k")
                            .map(|m| m.grams)
                            .unwrap_or(0.0)
                    ))
                    .color(theme::TEXT_DIM),
                );
            }
        });
    });
    ui.add_space(2.0);
}

fn workflow_window(app: &mut RingDesignerApp, ctx: &egui::Context) {
    let id = egui::Id::new("workflow-open");
    let mut open = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let mut action = None;
    egui::Window::new("Jewelry workflow")
        .frame(egui::Frame::window(&ctx.global_style()).fill(theme::FLOAT))
        .default_height(600.0)
        .default_pos(egui::pos2(350.0, 140.0))
        .open(&mut open)
        .default_width(325.0)
        .resizable(true)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(580.0)
                .show(ui, |ui| action = ringdesign_workbench::workflow::show(ui));
        });
    if let Some(action) = action {
        use ringdesign_workbench::{visual::Tool, workflow::Action};
        match action {
            Action::Fit | Action::Shape => {
                app.dock.open_on(ToolKind::Design, Side::Left);
                app.construction.open = false;
            }
            Action::Tool(tool) => {
                app.panes[app.active_pane].kind = PaneKind::Solid;
                app.visual.select(tool);
            }
            Action::Stones => {
                app.dock.open_on(ToolKind::Layers, Side::Right);
                app.visual.select(Tool::Clearance);
            }
            Action::Checks => app.panes[app.active_pane].kind = PaneKind::Casting,
            Action::Export => {
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("guided-export"), true));
            }
        }
        open = false;
    }
    ctx.data_mut(|d| d.insert_temp(id, open));
    let id = egui::Id::new("guided-export");
    let mut open = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    egui::Window::new("Save & export")
        .frame(egui::Frame::window(&ctx.global_style()).fill(theme::FLOAT))
        .open(&mut open)
        .resizable(false)
        .show(ctx, |ui| {
            use ringdesign_workbench::icons::{self, Icon};
            ui.label("Save the editable design, then export a mesh for your next step.");
            for (icon, label, action) in [
                (
                    Icon::Save,
                    "Save design",
                    export::save_design as fn(&mut RingDesignerApp),
                ),
                (Icon::Export, "Export STL", export::export_stl),
                (Icon::Export, "Export 3MF", export::export_3mf),
            ] {
                if icons::button(ui, icon, label, false, egui::vec2(0.0, 28.0)).clicked() {
                    action(app);
                }
            }
        });
    ctx.data_mut(|d| d.insert_temp(id, open));
}
