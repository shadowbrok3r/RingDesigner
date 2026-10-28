//! App-drawn window top strip: mark, caller content, title, caption buttons, drag area and resize edges.
//!
//! Show [`TitleBar`] before any other panel and call [`window_edges`] last in the frame. Choose between
//! this strip and the system one with [`crate::chrome`], and call [`windows::client_area::install`]
//! once at startup. `docs/porting-title-bar.md` walks through moving an existing app over.

pub mod windows;

use eframe::egui::emath::GuiRounding as _;
use eframe::egui::{
    Align, AsIdSalt, Color32, Context, CursorIcon, Event, Frame, Id, LayerId, Layout, Modifiers,
    Order, Panel, Plugin, PointerButton, Pos2, RawInput, Rect, ResizeDirection, Role, Sense,
    Stroke, StrokeKind, TextWrapMode, Ui, UiBuilder, Vec2, ViewportCommand, WidgetInfo, pos2, vec2,
};

use crate::controls::{
    Icon, Metrics, State, Tokens, icons, kit, medium_font_sized, state_of, text_galley,
};
use crate::metrics::type_scale;

/// Width of one caption button.
pub const CAPTION_W: f32 = 46.0;
/// Depth of the left, right and bottom resize zones; over empty space a press within egui's interact radius beyond it also resizes.
pub const EDGE: f32 = 8.0;
/// Depth of the top resize zone, kept clear of the strip's controls.
pub const TOP_EDGE: f32 = 5.0;
/// Side of a corner resize zone.
pub const CORNER: f32 = 12.0;
/// Resize margin kept above and beside the caption buttons.
const CAPTION_MARGIN: f32 = 3.0;
/// Distance the injected pointer move travels so the injected release is never a click.
const RELEASE_TRAVEL: f32 = 12.0;
/// Space kept between the title and the content beside it.
const TITLE_GAP: f32 = 16.0;
/// Mark size.
const MARK: f32 = 20.0;
/// Opacity of the mark's duotone back layer.
const MARK_DUO: f32 = 0.35;
/// Caption glyph size.
const CAPTION_GLYPH: f32 = 14.0;
/// Close-button fill while hovered.
const CLOSE_HOVER: Color32 = Color32::from_rgb(196, 43, 28);
/// Close-button fill while pressed.
const CLOSE_PRESS: Color32 = Color32::from_rgb(160, 36, 24);

/// One window caption button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Caption {
    Minimize,
    Maximize,
    Close,
}

impl Caption {
    /// Left to right.
    pub const ALL: [Self; 3] = [Self::Minimize, Self::Maximize, Self::Close];

    /// Stable widget id.
    pub fn id(self) -> Id {
        Id::unique(("egui_theme_kit.title_bar.caption", self as u8))
    }

    fn icon(self, maximized: bool) -> Icon {
        match self {
            Self::Minimize => icons::MINUS,
            Self::Maximize if maximized => icons::COPY,
            Self::Maximize => icons::SQUARE,
            Self::Close => icons::X,
        }
    }

    fn tip(self, maximized: bool) -> &'static str {
        match self {
            Self::Minimize => "Minimize",
            Self::Maximize if maximized => "Restore down",
            Self::Maximize => "Maximize",
            Self::Close => "Close",
        }
    }
}

/// Which end of the strip [`TitleBar::show`] is filling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// Laid out left to right after the mark.
    Left,
    /// Laid out right to left before the caption buttons; filled first.
    Right,
}

/// What a [`TitleBar`] laid out this frame.
#[derive(Clone, Copy, Debug)]
pub struct TitleBarOutput<R> {
    pub left: R,
    pub right: R,
    /// The whole strip.
    pub rect: Rect,
    /// Free space between the two ends; drags the window when frameless.
    pub gap: Rect,
}

/// The window's top strip, drawn as a top panel.
#[must_use = "call `show` to draw the strip"]
pub struct TitleBar<'a> {
    id: Id,
    title: &'a str,
    mark: Option<Icon>,
    frameless: bool,
}

impl<'a> TitleBar<'a> {
    pub fn new(id_salt: impl AsIdSalt, title: &'a str) -> Self {
        Self {
            id: Id::unique(id_salt),
            title,
            mark: None,
            frameless: false,
        }
    }

    /// Icon at the left end.
    pub fn mark(mut self, icon: Icon) -> Self {
        self.mark = Some(icon);
        self
    }

    /// Adds caption buttons, and lets empty space drag and maximize the window.
    pub fn frameless(mut self, frameless: bool) -> Self {
        self.frameless = frameless;
        self
    }

    /// Height of the strip at the current density.
    pub fn height(m: &Metrics) -> f32 {
        m.bar_h
    }

    /// Draws the strip; `add` fills [`Side::Right`] and then [`Side::Left`].
    ///
    /// The title and the drag area live in the free space between the two ends, so the content must
    /// not claim the whole width: `egui::MenuBar::ui` does, so put its menu buttons in the slot directly.
    pub fn show<R>(self, ui: &mut Ui, add: impl FnMut(&mut Ui, Side) -> R) -> TitleBarOutput<R> {
        let (t, m) = kit(ui);
        Panel::top(self.id)
            .exact_size(Self::height(&m))
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::NONE.fill(t.base))
            .show(ui, |ui| self.contents(ui, &t, &m, add))
            .inner
    }

    fn contents<R>(
        &self,
        ui: &mut Ui,
        t: &Tokens,
        m: &Metrics,
        mut add: impl FnMut(&mut Ui, Side) -> R,
    ) -> TitleBarOutput<R> {
        let full = ui.max_rect();
        let hairline = 1.0 / ui.pixels_per_point();
        let line_y = ui
            .painter()
            .round_to_pixel_center(full.bottom() - hairline / 2.0);
        ui.painter()
            .hline(full.x_range(), line_y, Stroke::new(hairline, t.separator));
        let ends = if self.frameless {
            full.right() - Caption::ALL.len() as f32 * CAPTION_W
        } else {
            full.right() - m.pad.x
        };

        let mut start = full.left() + m.pad.x;
        if let Some(mark) = self.mark {
            let rect = Rect::from_center_size(
                pos2(start + MARK / 2.0, full.center().y),
                Vec2::splat(MARK),
            );
            mark.paint_duo(
                ui,
                rect,
                MARK,
                t.primary,
                t.primary.gamma_multiply(MARK_DUO),
            );
            start = rect.right() + m.gap.x;
        }

        let right_rect = Rect::from_x_y_ranges(start..=(ends - m.gap.x).max(start), full.y_range());
        let (right, right_used) = side_ui(ui, right_rect, Side::Right, m, &mut add);
        let right_start = right_used.left().min(right_rect.right());

        let left_rect =
            Rect::from_x_y_ranges(start..=(right_start - TITLE_GAP).max(start), full.y_range());
        let (left, left_used) = side_ui(ui, left_rect, Side::Left, m, &mut add);
        let left_end = left_used.right().max(start);

        let gap = Rect::from_x_y_ranges(left_end..=right_start.max(left_end), full.y_range());
        self.paint_title(ui, t, full, gap);
        if self.frameless {
            drag_area(ui, self.id.with("drag"), gap, self.title);
            let lead = Rect::from_x_y_ranges(full.left()..=start, full.y_range());
            drag_area(ui, self.id.with("lead"), lead, self.title);
            caption_buttons(ui, full, t);
        }
        TitleBarOutput {
            left,
            right,
            rect: full,
            gap,
        }
    }

    /// The title centred on the strip, slid to stay inside `gap`; omitted when it does not fit.
    fn paint_title(&self, ui: &Ui, t: &Tokens, full: Rect, gap: Rect) {
        if self.title.is_empty() {
            return;
        }
        let galley = text_galley(ui, self.title, medium_font_sized(ui, type_scale::BODY));
        let width = galley.size().x;
        let Some(x) = title_x(full.center().x, gap.left(), gap.right(), width) else {
            return;
        };
        let y = full.center().y - galley.size().y / 2.0;
        let pos = pos2(x, y).round_to_pixels(ui.pixels_per_point());
        ui.painter().galley(pos, galley, t.text_dim);
    }
}

/// One end of the strip in its own child Ui; returns `add`'s result and the space used.
fn side_ui<R>(
    ui: &mut Ui,
    rect: Rect,
    side: Side,
    m: &Metrics,
    add: &mut impl FnMut(&mut Ui, Side) -> R,
) -> (R, Rect) {
    let layout = match side {
        Side::Left => Layout::left_to_right(Align::Center),
        Side::Right => Layout::right_to_left(Align::Center),
    };
    let salt = match side {
        Side::Left => "left",
        Side::Right => "right",
    };
    let mut child = ui.new_child(UiBuilder::new().id_salt(salt).max_rect(rect).layout(layout));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child.spacing_mut().item_spacing.x = m.gap.x;
    child.style_mut().wrap_mode = Some(TextWrapMode::Truncate);
    let out = add(&mut child, side);
    (out, child.min_rect())
}

/// Left edge of a title `width` wide centred on `center` within `left..right`; `None` if too wide.
fn title_x(center: f32, left: f32, right: f32, width: f32) -> Option<f32> {
    let (lo, hi) = (left + TITLE_GAP, right - TITLE_GAP - width);
    if !(lo.is_finite() && hi.is_finite()) || hi < lo {
        return None;
    }
    Some((center - width / 2.0).clamp(lo, hi))
}

fn is_maximized(ctx: &Context) -> bool {
    ctx.input(|i| i.viewport().maximized.unwrap_or(false))
}

fn is_fullscreen(ctx: &Context) -> bool {
    ctx.input(|i| i.viewport().fullscreen.unwrap_or(false))
}

/// Empty strip space: a drag moves the window and a double-click maximizes or restores it.
fn drag_area(ui: &Ui, id: Id, rect: Rect, title: &str) {
    if rect.width() <= 0.0 {
        return;
    }
    let response = ui.interact(rect, id, Sense::click_and_drag());
    response.widget_info(|| WidgetInfo::labeled(Role::TitleBar, true, title));
    if response.drag_started_by(PointerButton::Primary) {
        hand_to_window_manager(ui.ctx(), ViewportCommand::StartDrag);
    }
    if response.double_clicked() {
        let maximized = is_maximized(ui.ctx());
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
}

/// Sends a window-manager move or resize `command` and ends egui's pointer press, whose release the window manager takes.
fn hand_to_window_manager(ctx: &Context, command: ViewportCommand) {
    let pending = ctx
        .input(|i| i.pointer.interact_pos())
        .map(|pos| (ctx.viewport_id(), pos));
    ctx.add_plugin(PointerRelease::default());
    ctx.with_plugin::<PointerRelease, _>(|plugin| plugin.pending = pending);
    ctx.send_viewport_cmd(command);
}

/// Ends the pointer press a window-manager drag left open.
///
/// While the window manager moves or resizes the window it owns the pointer and egui never sees the
/// button release, so egui keeps the press and the next click lands on the wrong widget.
#[derive(Default)]
struct PointerRelease {
    pending: Option<(eframe::egui::ViewportId, Pos2)>,
}

impl Plugin for PointerRelease {
    fn debug_name(&self) -> &'static str {
        "egui_theme_kit::title_bar::PointerRelease"
    }

    fn input_hook(&mut self, _ctx: &Context, input: &mut RawInput) {
        let Some((viewport, pos)) = self.pending else {
            return;
        };
        if input.viewport_id != viewport {
            return;
        }
        self.pending = None;
        let moved = pos + vec2(RELEASE_TRAVEL, 0.0);
        let release = Event::PointerButton {
            pos: moved,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        };
        input
            .events
            .splice(0..0, [Event::PointerMoved(moved), release]);
    }
}

/// Minimize, maximize or restore, and close, flush with the strip's right end.
fn caption_buttons(ui: &Ui, full: Rect, t: &Tokens) {
    let maximized = is_maximized(ui.ctx());
    let left = full.right() - Caption::ALL.len() as f32 * CAPTION_W;
    let mut clicked = None;
    for (i, caption) in Caption::ALL.into_iter().enumerate() {
        let rect = Rect::from_min_size(
            pos2(left + i as f32 * CAPTION_W, full.top()),
            vec2(CAPTION_W, full.height()),
        );
        let response = ui.interact(rect, caption.id(), Sense::click());
        let tip = caption.tip(maximized);
        response.widget_info(|| WidgetInfo::labeled(Role::Button, true, tip));
        let (state, _) = state_of(ui, &response);
        let close = caption == Caption::Close;
        let (fill, ink) = match state {
            State::Rest => (Color32::TRANSPARENT, t.icon),
            State::Hovered if close => (CLOSE_HOVER, Color32::WHITE),
            State::Pressed if close => (CLOSE_PRESS, Color32::WHITE),
            State::Hovered => (t.hover, t.text),
            State::Pressed => (t.pressed, t.text),
        };
        ui.painter().rect_filled(rect, 0, fill);
        caption.icon(maximized).paint(ui, rect, CAPTION_GLYPH, ink);
        if response.on_hover_text(tip).clicked() {
            clicked = Some(caption);
        }
    }
    let command = match clicked {
        Some(Caption::Minimize) => ViewportCommand::Minimized(true),
        Some(Caption::Maximize) => ViewportCommand::Maximized(!maximized),
        Some(Caption::Close) => ViewportCommand::Close,
        None => return,
    };
    ui.ctx().send_viewport_cmd(command);
}

/// One resize zone: where it sits, the resize it starts and the cursor over it.
type Zone = (Rect, ResizeDirection, CursorIcon);

/// Edge then corner zones of `window`, each shrunk to a third of the window so a small window keeps a middle.
fn zones(window: Rect) -> [Zone; 8] {
    use ResizeDirection::*;
    let w = window;
    let third = w.size() / 3.0;
    let (side, bottom, top) = (EDGE.min(third.x), EDGE.min(third.y), TOP_EDGE.min(third.y));
    let corner = Vec2::new(CORNER.min(third.x), CORNER.min(third.y));
    [
        (
            Rect::from_min_max(w.left_top(), pos2(w.right(), w.top() + top)),
            North,
            CursorIcon::ResizeNorth,
        ),
        (
            Rect::from_min_max(pos2(w.left(), w.bottom() - bottom), w.right_bottom()),
            South,
            CursorIcon::ResizeSouth,
        ),
        (
            Rect::from_min_max(w.left_top(), pos2(w.left() + side, w.bottom())),
            West,
            CursorIcon::ResizeWest,
        ),
        (
            Rect::from_min_max(pos2(w.right() - side, w.top()), w.right_bottom()),
            East,
            CursorIcon::ResizeEast,
        ),
        (
            Rect::from_min_size(w.left_top(), corner),
            NorthWest,
            CursorIcon::ResizeNwSe,
        ),
        (
            Rect::from_min_size(w.right_top() - vec2(corner.x, 0.0), corner),
            NorthEast,
            CursorIcon::ResizeNeSw,
        ),
        (
            Rect::from_min_size(w.left_bottom() - vec2(0.0, corner.y), corner),
            SouthWest,
            CursorIcon::ResizeNeSw,
        ),
        (
            Rect::from_min_size(w.right_bottom() - corner, corner),
            SouthEast,
            CursorIcon::ResizeNwSe,
        ),
    ]
}

/// The caption buttons' area inside `window`, less the margin that stays a resize border.
fn caption_hole(window: Rect, bar_h: f32) -> Rect {
    let width = Caption::ALL.len() as f32 * CAPTION_W;
    Rect::from_min_max(
        pos2(window.right() - width, window.top() + CAPTION_MARGIN),
        pos2(window.right() - CAPTION_MARGIN, window.top() + bar_h),
    )
}

/// `zone` without `hole`: up to four disjoint rectangles.
fn carve(zone: Rect, hole: Rect) -> Vec<Rect> {
    if !zone.intersects(hole) {
        return vec![zone];
    }
    let h = hole.intersect(zone);
    [
        Rect::from_min_max(zone.min, pos2(h.left(), zone.bottom())),
        Rect::from_min_max(pos2(h.right(), zone.top()), zone.max),
        Rect::from_min_max(pos2(h.left(), zone.top()), pos2(h.right(), h.top())),
        Rect::from_min_max(pos2(h.left(), h.bottom()), pos2(h.right(), zone.bottom())),
    ]
    .into_iter()
    .filter(|r| r.width() > 0.0 && r.height() > 0.0)
    .collect()
}

/// The pointer-sensitive pieces of every resize zone of `window`, clear of the caption buttons.
fn resize_pieces(window: Rect, bar_h: f32) -> Vec<Zone> {
    let hole = caption_hole(window, bar_h);
    zones(window)
        .into_iter()
        .flat_map(|(zone, direction, cursor)| {
            carve(zone, hole)
                .into_iter()
                .map(move |piece| (piece, direction, cursor))
        })
        .collect()
}

/// Resize zones and a one-pixel outline on a frameless window's edges unless maximized.
///
/// Call it last in the frame so the zones sit above the content. The zones keep out of the caption
/// buttons except for a thin margin at the window's border.
pub fn window_edges(ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    if is_maximized(&ctx) || is_fullscreen(&ctx) {
        return;
    }
    let window = ctx.content_rect();
    let (t, m) = kit(ui);
    let layer = LayerId::new(
        Order::Foreground,
        Id::unique("egui_theme_kit.title_bar.edges"),
    );
    let edges = ui.new_child(UiBuilder::new().layer_id(layer).max_rect(window));
    for (i, (rect, direction, cursor)) in resize_pieces(window, m.bar_h).into_iter().enumerate() {
        let response = edges
            .interact(
                rect,
                Id::unique(("egui_theme_kit.title_bar.resize", i)),
                Sense::drag(),
            )
            .on_hover_cursor(cursor);
        if response.drag_started_by(PointerButton::Primary) {
            hand_to_window_manager(&ctx, ViewportCommand::BeginResize(direction));
        }
    }
    let hairline = 1.0 / ctx.pixels_per_point();
    edges.painter().rect_stroke(
        window,
        0,
        Stroke::new(hairline, t.card_border),
        StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{self, Density};
    use eframe::egui::{
        Event, FontDefinitions, Modifiers, Pos2, RawInput, Shape, Style, ViewportId, ViewportInfo,
    };

    /// A headless window `width` points wide showing a title strip and the edge zones.
    struct Win {
        ctx: Context,
        width: f32,
        time: f64,
        maximized: bool,
        fullscreen: bool,
        frameless: bool,
        /// Names of the left-slot buttons clicked so far.
        clicked: Vec<&'static str>,
        /// Shapes of the last frame.
        shapes: Vec<eframe::egui::epaint::ClippedShape>,
    }

    impl Win {
        fn new(width: f32) -> Self {
            let ctx = Context::default();
            let mut fonts = FontDefinitions::default();
            crate::fonts::install(&mut fonts);
            ctx.set_fonts(fonts);
            metrics::set_density(&ctx, Density::Regular);
            metrics::install_overlaid_style(&ctx, Style::default());
            Self {
                ctx,
                width,
                time: 0.0,
                maximized: false,
                fullscreen: false,
                frameless: true,
                clicked: Vec::new(),
                shapes: Vec::new(),
            }
        }

        fn frame(&mut self, events: Vec<Event>) -> Vec<ViewportCommand> {
            self.time += 0.05;
            let mut input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(self.width, 600.0))),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            input.viewports.insert(
                ViewportId::ROOT,
                ViewportInfo {
                    maximized: Some(self.maximized),
                    fullscreen: Some(self.fullscreen),
                    ..Default::default()
                },
            );
            let frameless = self.frameless;
            let mut clicked = Vec::new();
            let mut out = self.ctx.run_ui(input, |ui| {
                let _ = TitleBar::new("test.title_bar", "Example App 1.0")
                    .mark(icons::GRID)
                    .frameless(frameless)
                    .show(ui, |ui, side| match side {
                        Side::Left => {
                            for name in ["View", "Mode"] {
                                if ui.button(name).clicked() {
                                    clicked.push(name);
                                }
                            }
                        }
                        Side::Right => {
                            let _ = ui.button("Account");
                        }
                    });
                if frameless {
                    window_edges(ui);
                }
            });
            self.clicked.extend(clicked);
            self.shapes = std::mem::take(&mut out.shapes);
            out.textures_delta.clear();
            out.viewport_output
                .remove(&ViewportId::ROOT)
                .map(|v| v.commands)
                .unwrap_or_default()
        }

        /// Move to `path[0]`, press, move along `path`, release; the commands sent.
        fn gesture(&mut self, path: &[Pos2]) -> Vec<ViewportCommand> {
            let mut steps = vec![
                vec![Event::PointerMoved(path[0])],
                vec![button(path[0], true)],
            ];
            steps.extend(path[1..].iter().map(|p| vec![Event::PointerMoved(*p)]));
            steps.push(vec![button(*path.last().unwrap_or(&path[0]), false)]);
            steps.into_iter().flat_map(|e| self.frame(e)).collect()
        }

        fn click(&mut self, p: Pos2) -> Vec<ViewportCommand> {
            self.gesture(&[p])
        }

        fn rect_of(&self, id: Id) -> Rect {
            self.ctx.read_response(id).map_or(Rect::NOTHING, |r| r.rect)
        }
    }

    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::default(),
        }
    }

    #[test]
    fn caption_buttons_sit_flush_in_the_top_right_corner() {
        for width in [760.0, 1000.0, 1440.0] {
            let mut w = Win::new(width);
            w.frame(vec![]);
            w.frame(vec![]);
            let [min, max, close] = Caption::ALL.map(|c| w.rect_of(c.id()));
            let bar_h = Metrics::for_density(Density::Regular).bar_h;
            assert_eq!((close.right(), close.top()), (width, 0.0), "at {width}");
            for r in [min, max, close] {
                assert_eq!((r.width(), r.height()), (CAPTION_W, bar_h), "at {width}");
            }
            assert_eq!((min.right(), max.right()), (max.left(), close.left()));
        }
    }

    #[test]
    fn caption_buttons_minimize_maximize_restore_and_close() {
        let mut w = Win::new(1000.0);
        w.frame(vec![]);
        let [min, max, close] = Caption::ALL.map(|c| w.rect_of(c.id()).center());
        assert!(w.click(min).contains(&ViewportCommand::Minimized(true)));
        assert!(w.click(max).contains(&ViewportCommand::Maximized(true)));
        w.maximized = true;
        assert!(w.click(max).contains(&ViewportCommand::Maximized(false)));
        assert!(w.click(close).contains(&ViewportCommand::Close));
    }

    #[test]
    fn empty_strip_drags_and_double_click_maximizes_but_widgets_keep_their_clicks() {
        let mut w = Win::new(1440.0);
        w.frame(vec![]);
        let empty = pos2(1000.0, 18.0);
        let cmds = w.gesture(&[empty, empty + vec2(12.0, 4.0), empty + vec2(30.0, 8.0)]);
        let drags = cmds
            .iter()
            .filter(|c| matches!(c, ViewportCommand::StartDrag))
            .count();
        assert_eq!(drags, 1, "{cmds:?}");
        let mut cmds = w.click(empty);
        cmds.extend(w.click(empty));
        assert!(cmds.contains(&ViewportCommand::Maximized(true)), "{cmds:?}");

        w.time += 1.0;
        let view = pos2(60.0, 18.0);
        let cmds = w.gesture(&[view, view + vec2(12.0, 4.0), view + vec2(30.0, 8.0)]);
        assert!(
            !cmds.iter().any(|c| matches!(c, ViewportCommand::StartDrag)),
            "a press on a widget moved the window: {cmds:?}"
        );
    }

    #[test]
    fn edges_resize_unless_maximized() {
        use ResizeDirection::*;
        let mut w = Win::new(1000.0);
        w.frame(vec![]);
        for (p, direction) in [
            (pos2(1.0, 300.0), West),
            (pos2(998.0, 300.0), East),
            (pos2(500.0, 1.0), North),
            (pos2(500.0, 598.0), South),
            (pos2(2.0, 2.0), NorthWest),
            (pos2(997.0, 3.0), NorthEast),
            (pos2(3.0, 597.0), SouthWest),
            (pos2(998.0, 598.0), SouthEast),
        ] {
            let cmds = w.gesture(&[p, p + vec2(6.0, 6.0)]);
            assert_eq!(
                cmds,
                vec![ViewportCommand::BeginResize(direction)],
                "at {p:?}"
            );
        }
        w.maximized = true;
        let cmds = w.gesture(&[pos2(1.0, 300.0), pos2(8.0, 300.0)]);
        assert!(
            !cmds
                .iter()
                .any(|c| matches!(c, ViewportCommand::BeginResize(_))),
            "{cmds:?}"
        );
    }

    #[test]
    fn a_framed_window_gets_no_captions_drag_or_edges() {
        let mut w = Win::new(1000.0);
        w.frameless = false;
        w.frame(vec![]);
        assert!(w.ctx.read_response(Caption::Close.id()).is_none());
        let empty = pos2(600.0, 18.0);
        let cmds = w.gesture(&[empty, empty + vec2(30.0, 8.0)]);
        assert!(cmds.is_empty(), "{cmds:?}");
    }

    #[test]
    fn title_stays_inside_the_free_gap() {
        assert_eq!(title_x(800.0, 420.0, 1300.0, 60.0), Some(770.0));
        assert_eq!(title_x(400.0, 420.0, 1300.0, 60.0), Some(436.0));
        assert_eq!(title_x(1280.0, 420.0, 1300.0, 60.0), Some(1224.0));
        assert_eq!(title_x(400.0, 420.0, 480.0, 60.0), None);
    }

    fn has_resize(cmds: &[ViewportCommand]) -> bool {
        cmds.iter()
            .any(|c| matches!(c, ViewportCommand::BeginResize(_)))
    }

    /// Every painted rectangle and line stroke of the last frame as (shape, stroke width, colour).
    fn strokes(w: &Win) -> Vec<(Shape, f32, Color32)> {
        fn walk(shape: &Shape, out: &mut Vec<(Shape, f32, Color32)>) {
            match shape {
                Shape::Vec(inner) => inner.iter().for_each(|s| walk(s, out)),
                Shape::Rect(r) if r.stroke.width > 0.0 => {
                    out.push((shape.clone(), r.stroke.width, r.stroke.color));
                }
                Shape::LineSegment { stroke, .. } if stroke.width > 0.0 => {
                    out.push((shape.clone(), stroke.width, stroke.color));
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        w.shapes.iter().for_each(|c| walk(&c.shape, &mut out));
        out
    }

    #[test]
    fn caption_buttons_stay_flush_in_narrow_windows() {
        for width in [360.0, 420.0, 600.0] {
            let mut w = Win::new(width);
            w.frame(vec![]);
            w.frame(vec![]);
            let [min, max, close] = Caption::ALL.map(|c| w.rect_of(c.id()));
            assert_eq!((close.right(), close.top()), (width, 0.0), "at {width}");
            assert_eq!((min.right(), max.right()), (max.left(), close.left()));
        }
    }

    #[test]
    fn caption_labels_follow_the_maximized_state() {
        assert_eq!(Caption::Maximize.tip(false), "Maximize");
        assert_eq!(Caption::Maximize.tip(true), "Restore down");
        assert_eq!(Caption::Minimize.tip(true), "Minimize");
        assert_eq!(Caption::Close.tip(false), "Close");
    }

    #[test]
    fn double_click_restores_a_maximized_window() {
        let mut w = Win::new(1440.0);
        w.maximized = true;
        w.frame(vec![]);
        let empty = pos2(1000.0, 18.0);
        let mut cmds = w.click(empty);
        cmds.extend(w.click(empty));
        assert!(
            cmds.contains(&ViewportCommand::Maximized(false)),
            "{cmds:?}"
        );
    }

    #[test]
    fn widgets_below_the_top_edge_click_and_the_edge_above_them_resizes() {
        use ResizeDirection::*;
        let mut w = Win::new(1000.0);
        w.frame(vec![]);
        let cmds = w.click(pos2(60.0, TOP_EDGE + 2.0));
        assert!(!has_resize(&cmds), "{cmds:?}");
        assert_eq!(w.clicked, ["View"]);
        let cmds = w.gesture(&[pos2(60.0, 2.0), pos2(60.0, 9.0)]);
        assert_eq!(cmds, vec![ViewportCommand::BeginResize(North)]);
        assert_eq!(w.clicked, ["View"], "the edge press clicked a widget");
    }

    #[test]
    fn the_close_button_wins_its_top_rows_and_the_window_corner_still_resizes() {
        use ResizeDirection::*;
        let mut w = Win::new(1000.0);
        w.frame(vec![]);
        let cmds = w.click(pos2(977.0, 4.0));
        assert!(cmds.contains(&ViewportCommand::Close), "{cmds:?}");
        assert!(!has_resize(&cmds), "{cmds:?}");
        for (p, direction) in [
            (pos2(977.0, 1.0), North),
            (pos2(999.0, 1.0), NorthEast),
            (pos2(999.0, 20.0), East),
        ] {
            let cmds = w.gesture(&[p, p + vec2(-6.0, 6.0)]);
            assert_eq!(
                cmds,
                vec![ViewportCommand::BeginResize(direction)],
                "at {p:?}"
            );
        }
    }

    #[test]
    fn resize_zones_leave_the_middle_of_a_tiny_window_free() {
        for side in [14.0, 30.0, 60.0] {
            let window = Rect::from_min_size(Pos2::ZERO, Vec2::splat(side));
            for (zone, _, _) in zones(window) {
                assert!(!zone.contains(window.center()), "{zone:?} in {side}");
                assert!(window.contains_rect(zone), "{zone:?} outside {side}");
            }
        }
    }

    #[test]
    fn carving_a_hole_out_of_a_zone_keeps_the_rest() {
        let zone = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 10.0));
        let far = Rect::from_min_max(pos2(200.0, 0.0), pos2(300.0, 10.0));
        assert_eq!(carve(zone, far), vec![zone]);
        assert!(carve(zone, zone.expand(1.0)).is_empty());
        let pieces = carve(zone, Rect::from_min_max(pos2(40.0, 3.0), pos2(60.0, 20.0)));
        assert_eq!(pieces.len(), 3);
        let area: f32 = pieces.iter().map(|r| r.width() * r.height()).sum();
        assert_eq!(area, 100.0 * 10.0 - 20.0 * 7.0);
    }

    #[test]
    fn resize_pieces_never_cover_the_caption_buttons() {
        let window = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 600.0));
        let bar_h = Metrics::for_density(Density::Regular).bar_h;
        let hole = caption_hole(window, bar_h);
        for (piece, _, _) in resize_pieces(window, bar_h) {
            assert!(
                !piece.intersects(hole.shrink(0.5)),
                "{piece:?} over {hole:?}"
            );
        }
    }

    #[test]
    fn edges_are_skipped_in_fullscreen_and_when_maximized() {
        for (maximized, fullscreen) in [(true, false), (false, true)] {
            let mut w = Win::new(1000.0);
            w.maximized = maximized;
            w.fullscreen = fullscreen;
            w.frame(vec![]);
            let cmds = w.gesture(&[pos2(1.0, 300.0), pos2(8.0, 300.0)]);
            assert!(!has_resize(&cmds), "{maximized} {fullscreen}: {cmds:?}");
            let outline = Tokens::get(&w.ctx).card_border;
            let window = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 600.0));
            let drawn = strokes(&w).into_iter().any(|(shape, _, color)| {
                color == outline && matches!(shape, Shape::Rect(r) if r.rect == window)
            });
            assert!(!drawn, "outline drawn at {maximized} {fullscreen}");
        }
    }

    #[test]
    fn the_outline_and_the_separator_are_one_physical_pixel() {
        for ppp in [1.0_f32, 1.25, 1.5, 2.0] {
            let mut w = Win::new(800.0);
            w.ctx.set_pixels_per_point(ppp);
            w.frame(vec![]);
            w.frame(vec![]);
            let hairline = 1.0 / ppp;
            let outline = Tokens::get(&w.ctx).card_border;
            let separator = Tokens::get(&w.ctx).separator;
            let found = |color: Color32, rect: bool| {
                strokes(&w).into_iter().any(|(shape, width, c)| {
                    c == color
                        && (width - hairline).abs() < 1e-4
                        && matches!(shape, Shape::Rect(_)) == rect
                })
            };
            assert!(found(outline, true), "outline at {ppp}");
            assert!(found(separator, false), "separator at {ppp}");
        }
    }

    #[test]
    fn a_window_manager_drag_does_not_leave_the_next_click_swallowed() {
        let mut w = Win::new(1440.0);
        w.frame(vec![]);
        let empty = pos2(1000.0, 18.0);
        w.frame(vec![Event::PointerMoved(empty)]);
        w.frame(vec![button(empty, true)]);
        let cmds = w.frame(vec![Event::PointerMoved(empty + vec2(14.0, 6.0))]);
        assert!(cmds.contains(&ViewportCommand::StartDrag), "{cmds:?}");
        // The window manager now owns the pointer: egui sees it leave and never sees the release.
        w.frame(vec![Event::PointerGone]);
        w.frame(vec![]);
        assert!(
            !w.ctx.input(|i| i.pointer.any_down()),
            "egui kept the press"
        );
        w.click(pos2(60.0, 18.0));
        assert_eq!(w.clicked, ["View"]);
    }

    #[test]
    fn the_press_that_started_a_resize_is_ended_without_a_click() {
        let mut w = Win::new(1000.0);
        w.frame(vec![]);
        let edge = pos2(998.0, 300.0);
        w.frame(vec![Event::PointerMoved(edge)]);
        let mut cmds = w.frame(vec![button(edge, true)]);
        cmds.extend(w.frame(vec![]));
        assert!(
            cmds.contains(&ViewportCommand::BeginResize(ResizeDirection::East)),
            "{cmds:?}"
        );
        let mut clicks = w.ctx.input(|i| i.pointer.any_click());
        for _ in 0..3 {
            w.frame(vec![]);
            clicks |= w.ctx.input(|i| i.pointer.any_click());
        }
        assert!(!clicks, "the injected release counted as a click");
        assert!(!w.ctx.input(|i| i.pointer.any_down()));
        assert!(w.clicked.is_empty());
    }

    /// Every filled rectangle of the last frame.
    fn fills(w: &Win) -> Vec<Color32> {
        fn walk(shape: &Shape, out: &mut Vec<Color32>) {
            match shape {
                Shape::Vec(inner) => inner.iter().for_each(|s| walk(s, out)),
                Shape::Rect(r) => out.push(r.fill),
                _ => {}
            }
        }
        let mut out = Vec::new();
        w.shapes.iter().for_each(|c| walk(&c.shape, &mut out));
        out
    }

    #[test]
    fn caption_buttons_fill_on_hover_and_press_and_close_goes_red() {
        let mut w = Win::new(1000.0);
        w.frame(vec![]);
        w.frame(vec![]);
        let t = Tokens::get(&w.ctx);
        let [min, _, close] = Caption::ALL.map(|c| w.rect_of(c.id()).center());
        assert!(!fills(&w).contains(&CLOSE_HOVER));
        w.frame(vec![Event::PointerMoved(close)]);
        w.frame(vec![]);
        assert!(fills(&w).contains(&CLOSE_HOVER), "no red hover fill");
        w.frame(vec![button(close, true)]);
        assert!(fills(&w).contains(&CLOSE_PRESS), "no pressed fill");
        w.frame(vec![button(close, false)]);
        w.frame(vec![Event::PointerMoved(min)]);
        w.frame(vec![]);
        let fills = fills(&w);
        assert!(fills.contains(&t.hover), "no neutral hover fill");
        assert!(
            !fills.contains(&CLOSE_HOVER),
            "the red fill stayed after leaving"
        );
    }
}
