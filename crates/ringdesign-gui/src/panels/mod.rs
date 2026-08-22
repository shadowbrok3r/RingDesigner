//! Window layout and the top-level chrome.

pub mod design;
pub mod graph;
pub mod layers;
pub mod node;
pub mod library;
pub mod report;
pub mod section;
pub mod unrolled;

use egui_phosphor::regular as icon;

use crate::app::RingDesignerApp;
use crate::camera::StandardView;
use ringdesign_core::mesh::BuildParams;
use ringdesign_core::refine::RefineParams;

use crate::dock::{Dock, Side, ToolKind};
use crate::pane::{Layout, PaneKind};
use crate::viewport;
use crate::{export, theme};

/// Gap left between panes for the divider.
const GUTTER: f32 = 3.0;

pub fn render(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    shortcuts(app, ui);
    egui::Panel::top(egui::Id::new("toolbar")).show(ui, |ui| toolbar(app, ui));
    egui::Panel::bottom(egui::Id::new("status")).show(ui, |ui| status_bar(app, ui));

    for &side in Side::ALL {
        dock_side(app, ui, side);
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::VIEWPORT_BG))
        .show(ui, |ui| panes(app, ui));
}

/// One edge of the window: a tile tree of docked tools.
fn dock_side(app: &mut RingDesignerApp, ui: &mut egui::Ui, side: Side) {
    if app.dock.tree(side).is_empty() {
        return;
    }
    let id = egui::Id::new(("dock", side.label()));
    let panel = match side {
        Side::Left => egui::Panel::left(id),
        Side::Right => egui::Panel::right(id),
    };
    let width = app.dock.width_of(side);
    let resp = panel
        .default_size(width)
        .size_range(egui::Rangef::new(240.0, 680.0))
        .show(ui, |ui| {
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
            };
            tree.ui(&mut behavior, ui);
            let moved = behavior.moved;
            *app.dock.tree_mut(side) = tree;
            if let Some(tool) = moved {
                app.dock.open_on(tool, side.other());
            }
        });
    let w = resp.response.rect.width();
    if (w - width).abs() > 0.5 {
        app.dock.set_width(side, w);
    }
}

/// Draws each docked tool and carries the "send to the other side" request back
/// out, since the tree cannot move a pane between two separate trees itself.
struct ToolBehavior<'a> {
    app: &'a mut RingDesignerApp,
    side: Side,
    moved: Option<ToolKind>,
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
        ui.horizontal(|ui| {
            ui.spacing_mut().button_padding = egui::vec2(3.0, 1.0);
            ui.label(
                egui::RichText::new(format!("{} {}", tool.icon(), tool.label()))
                    .strong()
                    .color(theme::TEXT_DIM),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                let driven = self.app.graph_driven() && matches!(tool, ToolKind::Design | ToolKind::Layers | ToolKind::Library);
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
        egui_tiles::UiResponse::None
    }

    fn is_tab_closable(
        &self,
        _tiles: &egui_tiles::Tiles<ToolKind>,
        _id: egui_tiles::TileId,
    ) -> bool {
        true
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
    let area = ui.available_rect_before_wrap();
    ui.painter().rect_filled(area, 0.0, theme::GRID);
    let rects = app.layout.split(area, GUTTER);
    let single = rects.len() == 1;

    for (i, rect) in rects.into_iter().enumerate() {
        if rect.width() < 1.0 || rect.height() < 1.0 {
            continue;
        }
        // Clicking anywhere in a pane makes it the one the toolbar acts on.
        let hit = ui.interact(rect, egui::Id::new(("pane_focus", i)), egui::Sense::click());
        if hit.clicked() {
            app.active_pane = i;
        }

        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        child.set_clip_rect(rect);
        egui::Panel::top(egui::Id::new(("pane_head", i)))
            .frame(
                egui::Frame::NONE
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(6, 3)),
            )
            .show(&mut child, |ui| pane_head(app, ui, i));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme::VIEWPORT_BG))
            .show(&mut child, |ui| match app.panes[i].kind {
                PaneKind::Solid => viewport::ui(app, ui, i),
                PaneKind::Unrolled => unrolled::ui(app, ui),
                PaneKind::Section => section::ui(app, ui, i),
                PaneKind::Graph => graph::ui(app, ui, i),
            });

        // Only worth marking which pane is active when there is a choice.
        if !single && app.active_pane == i {
            ui.painter().rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(1.0, theme::ACCENT_DIM),
                egui::StrokeKind::Inside,
            );
        }
    }
}

/// Per-pane strip: which view it shows, and the controls that view needs.
fn pane_head(app: &mut RingDesignerApp, ui: &mut egui::Ui, i: usize) {
    ui.horizontal(|ui| {
        let kind = app.panes[i].kind;
        egui::ComboBox::from_id_salt(("pane_kind", i))
            .selected_text(format!("{} {}", kind.icon(), kind.label()))
            .width(140.0)
            .show_ui(ui, |ui| {
                for &k in PaneKind::ALL {
                    if ui
                        .selectable_label(kind == k, format!("{} {}", k.icon(), k.label()))
                        .clicked()
                    {
                        app.panes[i].kind = k;
                        app.active_pane = i;
                        if k == PaneKind::Section {
                            app.refresh_section(i);
                        }
                        ui.close();
                    }
                }
            });

        if app.panes[i].kind != PaneKind::Solid {
            return;
        }

        ui.separator();
        for &v in StandardView::ALL {
            if ui.small_button(v.label()).clicked() {
                app.panes[i].camera.set_view(v);
                app.active_pane = i;
            }
        }
        ui.separator();
        let shade = app.panes[i].shade;
        egui::ComboBox::from_id_salt(("pane_shade", i))
            .selected_text(shade.label())
            .width(120.0)
            .show_ui(ui, |ui| {
                for &m in viewport::ShadeMode::ALL {
                    if ui.selectable_label(shade == m, m.label()).clicked() {
                        app.panes[i].shade = m;
                        app.active_pane = i;
                        ui.close();
                    }
                }
            });
    });
}

/// Ctrl+Z / Ctrl+Shift+Z, plus Ctrl+Y for the redo people expect on Windows.
fn shortcuts(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    use egui::{Key, KeyboardShortcut, Modifiers};
    const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
    const REDO: KeyboardShortcut =
        KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
    const REDO_ALT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
    const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
    const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
    const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
    const PALETTE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::K);

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
        let graph_pane = app.panes.get(app.active_pane).is_some_and(|p| p.kind == PaneKind::Graph);
        if graph_pane && app.selected_node.is_some() {
            Command::DeleteNode.run(app);
        } else {
            Command::DeleteLayer.run(app);
        }
    }

    command_palette(app, ui);
}

/// Everything the palette can do, one match away from the code that does it.
#[derive(Clone, Copy, PartialEq)]
enum Command {
    New,
    Open,
    Save,
    ExportStl,
    ExportObj,
    Export3mf,
    ExportGlb,
    RenderPng,
    TurntableGif,
    CastingSheet,
    PartingLine,
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
                ui.colored_label(theme::ACCENT, format!("{} Driven by the graph", icon::GRAPH));
                ui.weak("— edit the graph, or bake it to edit here.");
                if ui.small_button(format!("{} Edit graph", icon::GRAPH)).clicked() {
                    app.show_graph_pane();
                }
                if ui.small_button(format!("{} Bake", icon::FIRE)).on_hover_text("Drop the graph; keep the design as last evaluated").clicked() {
                    app.bake_graph();
                }
            });
            if tool == ToolKind::Layers && !app.design.layers.layers.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.weak("Edit in graph:");
                    let names: Vec<String> = app.design.layers.layers.iter().map(|e| e.name.clone()).collect();
                    for (k, name) in names.iter().enumerate() {
                        if ui.small_button(name).on_hover_text("Find the node that produced this layer").clicked() {
                            app.edit_in_graph(k);
                        }
                    }
                });
            }
        });
    ui.add_space(4.0);
}

impl Command {
    const ALL: &'static [Command] = &[
        Command::New,
        Command::Open,
        Command::Save,
        Command::ExportStl,
        Command::ExportObj,
        Command::Export3mf,
        Command::ExportGlb,
        Command::RenderPng,
        Command::TurntableGif,
        Command::CastingSheet,
        Command::PartingLine,
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
    ];

    fn label(self) -> &'static str {
        match self {
            Command::New => "New design",
            Command::Open => "Open design…  (Ctrl+O)",
            Command::Save => "Save design as…  (Ctrl+S)",
            Command::ExportStl => "Export STL…",
            Command::ExportObj => "Export OBJ…",
            Command::Export3mf => "Export 3MF…",
            Command::ExportGlb => "Export GLB…",
            Command::RenderPng => "Render PNG…",
            Command::TurntableGif => "Turntable GIF…",
            Command::CastingSheet => "Casting sheet…",
            Command::PartingLine => "Parting line SVG…",
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
        }
    }

    fn run(self, app: &mut RingDesignerApp) {
        match self {
            Command::New => {
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
            Command::RenderPng => export::export_render(app),
            Command::TurntableGif => export::export_turntable(app),
            Command::CastingSheet => export::export_spec(app),
            Command::PartingLine => export::export_parting(app),
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
    egui::Window::new("Command palette")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
        .fixed_size(egui::vec2(360.0, 0.0))
        .show(&ctx, |ui| {
            let edit = ui.add(
                egui::TextEdit::singleline(&mut app.palette_query)
                    .hint_text("Type a command…")
                    .desired_width(f32::INFINITY),
            );
            edit.request_focus();
            let query = app.palette_query.to_lowercase();
            let hits: Vec<Command> = Command::ALL
                .iter()
                .copied()
                .filter(|c| c.label().to_lowercase().contains(&query))
                .collect();
            let go = ui.input(|i| i.key_pressed(egui::Key::Enter));
            for (k, c) in hits.iter().enumerate() {
                let first = k == 0;
                let label = if first && !hits.is_empty() {
                    egui::RichText::new(c.label()).strong()
                } else {
                    egui::RichText::new(c.label())
                };
                if ui.add(egui::Button::new(label).frame(false)).clicked() || (go && first) {
                    run = Some(*c);
                }
            }
            if hits.is_empty() {
                ui.label(egui::RichText::new("No matching command").weak());
            }
        });
    if let Some(c) = run {
        app.palette_open = false;
        c.run(app);
    }
}

/// Undo, redo, and the timeline they walk.
fn history_controls(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    let undo = egui::Button::new(icon::ARROW_ARC_LEFT);
    let hint = app
        .history
        .undo_label()
        .unwrap_or("nothing to undo")
        .to_string();
    if ui
        .add_enabled(app.history.can_undo(), undo)
        .on_hover_text(format!("Undo {hint}  (Ctrl+Z)"))
        .clicked()
    {
        app.undo();
    }

    let redo = egui::Button::new(icon::ARROW_ARC_RIGHT);
    let hint = app
        .history
        .redo_label()
        .unwrap_or("nothing to redo")
        .to_string();
    if ui
        .add_enabled(app.history.can_redo(), redo)
        .on_hover_text(format!("Redo {hint}  (Ctrl+Shift+Z)"))
        .clicked()
    {
        app.redo();
    }

    ui.menu_button(format!("{} History", icon::CLOCK_COUNTER_CLOCKWISE), |ui| {
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

fn toolbar(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        ui.menu_button(format!("{} File", icon::FOLDER_OPEN), |ui| {
            if ui.button(format!("{} New", icon::FILE_PLUS)).clicked() {
                app.design = ringdesign_core::RingDesign::default();
                app.history.reset(&app.design.clone());
                app.selected_layer = None;
                app.fit_pending = true;
                app.mark_dirty();
                ui.close();
            }
            ui.menu_button(format!("{} New from template", icon::SPARKLE), |ui| {
                for t in ringdesign_core::templates::all() {
                    if ui.button(t.name).on_hover_text(t.blurb).clicked() {
                        export::load_template(app, t);
                        ui.close();
                    }
                }
            });
            if ui.button(format!("{} Open…", icon::FOLDER_OPEN)).clicked() {
                export::open_design(app);
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
            if ui.button(format!("{} Save As…", icon::FLOPPY_DISK)).clicked() {
                export::save_design(app);
                ui.close();
            }
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
            if ui.button(format!("{} Export STL…", icon::EXPORT)).clicked() {
                export::export_stl(app);
                ui.close();
            }
            if ui.button(format!("{} Export OBJ…", icon::EXPORT)).clicked() {
                export::export_obj(app);
                ui.close();
            }
            if ui
                .button(format!("{} Casting sheet…", icon::FILE_TEXT))
                .on_hover_text(
                    "A printable HTML tech sheet: dimensions, weights, the field verdict and \
                     its notes, stones, and DFM findings — everything the pour needs.",
                )
                .clicked()
            {
                export::export_spec(app);
                ui.close();
            }
            if ui
                .button(format!("{} Cost JSON…", icon::COINS))
                .on_hover_text("Volume and per-alloy weights for the cost calculator.")
                .clicked()
            {
                export::export_cost_json(app);
                ui.close();
            }
            if ui
                .button(format!("{} Parting line…", icon::WAVE_SINE))
                .on_hover_text("The mould split as a printable SVG: plan view plus the line's height unrolled.")
                .clicked()
            {
                export::export_parting(app);
                ui.close();
            }
            if ui
                .button(format!("{} Render PNG…", icon::CAMERA))
                .on_hover_text("A polished still at export resolution, tinted to the chosen finish.")
                .clicked()
            {
                export::export_render(app);
                ui.close();
            }
            if ui
                .button(format!("{} Turntable GIF…", icon::FILM_STRIP))
                .on_hover_text("A looping 36-frame spin — takes a few seconds to build and draw.")
                .clicked()
            {
                export::export_turntable(app);
                ui.close();
            }
            if ui
                .button(format!("{} Export GLB…", icon::EXPORT))
                .on_hover_text("glTF binary with the alloy's PBR tint — for renders and web viewers.")
                .clicked()
            {
                export::export_glb(app);
                ui.close();
            }
            if ui
                .button(format!("{} Export 3MF…", icon::EXPORT))
                .on_hover_text("Zip-packaged model that states its units — no mm/inch guessing downstream.")
                .clicked()
            {
                export::export_3mf(app);
                ui.close();
            }
        });

        ui.menu_button(format!("{} Panels", icon::SIDEBAR), |ui| {
            for &t in ToolKind::ALL {
                let mut open = app.dock.is_open(t);
                if ui.checkbox(&mut open, format!("{} {}", t.icon(), t.label())).changed() {
                    app.dock.toggle(t, open);
                }
            }
            ui.separator();
            if ui.button(format!("{} Reset panel layout", icon::ARROW_COUNTER_CLOCKWISE)).clicked() {
                app.dock = Dock::default();
                ui.close();
            }
        });

        ui.separator();
        history_controls(app, ui);
        ui.separator();

        for &l in Layout::ALL {
            if ui
                .selectable_label(app.layout == l, format!("{} {}", l.icon(), l.label()))
                .on_hover_text("Split the view; each pane picks what it shows")
                .clicked()
            {
                app.layout = l;
                app.active_pane = app.active_pane.min(l.count() - 1);
                app.refresh_sections();
            }
        }

        ui.separator();

        if ui
            .small_button(format!("{} Reset views", icon::ARROW_COUNTER_CLOCKWISE))
            .clicked()
        {
            let bounds = app.build.as_ref().and_then(|b| b.mesh.bounds());
            for pane in &mut app.panes {
                pane.camera.reset();
                pane.camera.fit(bounds);
            }
        }
        ui.checkbox(&mut app.show_wireframe, "Wire");
        ui.checkbox(&mut app.show_grid, "Grid");
        {
            let mut pinned = app.pinned.is_some();
            if ui
                .checkbox(&mut pinned, "Ghost")
                .on_hover_text(
                    "Pin the current shape as a translucent ghost, then edit against it.                      The section view overlays its outline dashed.",
                )
                .changed()
            {
                app.toggle_pin();
            }
        }
        ui.checkbox(&mut app.show_gems, "Stones")
            .on_hover_text(
                "Preview the stones in their seats. Render only — never in the mesh, never exported.",
            );
        if ui
            .checkbox(&mut app.as_cast, "As-cast")
            .on_hover_text(
                "Soften the 3D preview at the sand's detail radius, so beads merge and fine \
                 cells mush the way the pour will. Exports and the section view stay exact.",
            )
            .changed()
        {
            app.mark_dirty();
        }
        egui::ComboBox::from_id_salt("metal_finish")
            .selected_text(crate::viewport::FINISHES[app.finish.min(crate::viewport::FINISHES.len() - 1)].name)
            .width(104.0)
            .show_ui(ui, |ui| {
                for (i, f) in crate::viewport::FINISHES.iter().enumerate() {
                    ui.selectable_value(&mut app.finish, i, f.name);
                }
            })
            .response
            .on_hover_text("Metal colour in the viewport. Weight per alloy is in the report.");
        egui::ComboBox::from_id_salt("light_rig")
            .selected_text(crate::viewport::LIGHT_RIGS[app.light.min(crate::viewport::LIGHT_RIGS.len() - 1)].name)
            .width(88.0)
            .show_ui(ui, |ui| {
                for (i, l) in crate::viewport::LIGHT_RIGS.iter().enumerate() {
                    ui.selectable_value(&mut app.light, i, l.name);
                }
            })
            .response
            .on_hover_text("Key light for the polished-metal view");

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if app.is_building() {
                ui.add(egui::Spinner::new().size(14.0));
            }
            if ui
                .button(format!("{} Rebuild", icon::ARROWS_CLOCKWISE))
                .on_hover_text("Rebuild the mesh now")
                .clicked()
            {
                app.rebuild_now();
            }
            ui.checkbox(&mut app.auto_rebuild, "Auto");

            if quality_picker(ui, "quality", &mut app.preview_params) {
                app.mark_dirty();
            }
            ui.label(egui::RichText::new("Preview").color(theme::TEXT_DIM));

            ui.separator();
            mcp_control(app, ui);
        });
    });
    ui.add_space(3.0);
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

    if ui
        .button(format!("{} MCP", icon::PLUGS))
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
pub fn quality_picker(ui: &mut egui::Ui, salt: &str, params: &mut BuildParams) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(salt)
        .selected_text(quality_label(params))
        .width(158.0)
        .show_ui(ui, |ui| {
            ui.label(
                egui::RichText::new("Swept grid — fixed step count")
                    .small()
                    .color(theme::TEXT_DIM),
            );
            for &(name, t, p) in BuildParams::PRESETS {
                let at =
                    params.refine.is_none() && params.theta_steps == t && params.profile_steps == p;
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
                if ui
                    .selectable_label(at, format!("{name} • {tol} mm"))
                    .clicked()
                {
                    params.refine = Some(RefineParams {
                        tolerance_mm: tol,
                        normal_tolerance_deg: tilt,
                        ..RefineParams::default()
                    });
                    changed = true;
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text(
            "A swept grid is fastest to build; refining puts the triangles only where the \
             surface bends, which is far fewer of them below about 0.05 mm.",
        );
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
        let (glyph, color) = match app.cast.as_ref().map(|c| c.verdict) {
            Some(v) => (
                match v {
                    ringdesign_core::castability::Verdict::Castable => icon::CHECK_CIRCLE,
                    _ => icon::WARNING,
                },
                theme::verdict_color(v),
            ),
            None => (icon::CIRCLE_DASHED, theme::TEXT_DIM),
        };
        if let Some(c) = app.cast.as_ref() {
            ui.label(egui::RichText::new(format!("{glyph} {}", c.verdict.label())).color(color));
        } else {
            ui.label(egui::RichText::new(format!("{glyph} —")).color(color));
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
