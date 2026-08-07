//! Clip a fragment out of a library alpha and turn it into a tile.
//!
//! Most imported ZBrush alphas are one motif on black, not a tile. The window
//! crops a fragment, mirrors it against itself so the tile's outer edges are the
//! same edge of the source, and reports the remaining seam error.
//!
//! The operations run in one fixed order:
//! `crop -> auto_trim -> rotate -> flip -> mirror_tile -> edge_fade -> levels ->
//! resize`.

use egui_phosphor::regular as icon;
use ringdesign_core::alpha::{Alpha, AlphaLibrary, Axis, CropRect};

use crate::theme;

/// Longest edge of the source preview texture.
const SOURCE_EDGE: usize = 320;
/// Longest edge of the tiled result preview texture.
const RESULT_EDGE: usize = 192;
/// Pointer distance in points that grabs a crop side.
const GRAB_PX: f32 = 7.0;
/// Smallest crop extent per axis, normalized.
const MIN_CROP: f64 = 0.02;
/// Brightness above which `auto_trim` counts a sample as content.
const TRIM_LEVEL: f32 = 0.06;
/// Margin `auto_trim` keeps around the content.
const TRIM_PAD: f64 = 0.01;
/// Longest output edge offered for the saved tile.
const SIZES: [usize; 3] = [128, 256, 512];
/// Width of the operation column.
const OPS_W: f32 = 248.0;
/// Seam error below which the tile reads seamless.
const SEAM_GOOD: f64 = 0.02;
/// Seam error above which the joint is obvious.
const SEAM_WARN: f64 = 0.08;

/// What the current crop drag is moving.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Grab {
    #[default]
    None,
    Move,
    /// Which sides follow the pointer: -1 low, 1 high, 0 fixed.
    Edge(i8, i8),
    Draw,
}

/// Every parameter of the derived alpha.
#[derive(Clone, Debug, PartialEq)]
struct Ops {
    crop: CropRect,
    trim: bool,
    rotate: u32,
    flip_h: bool,
    flip_v: bool,
    mirror: Option<Axis>,
    fade: f64,
    lo: f32,
    hi: f32,
    gamma: f32,
    size: usize,
}

impl Default for Ops {
    fn default() -> Self {
        Self {
            crop: CropRect::default(),
            trim: false,
            rotate: 0,
            flip_h: false,
            flip_v: false,
            mirror: Some(Axis::Horizontal),
            fade: 0.0,
            lo: 0.0,
            hi: 1.0,
            gamma: 1.0,
            size: 256,
        }
    }
}

/// The derived alpha, kept until an input changes.
struct Derived {
    key: (String, (usize, usize), Ops),
    alpha: Alpha,
    seam: (f64, f64),
    tex: egui::TextureHandle,
}

/// Window state for clipping a library alpha into a tileable fragment.
#[derive(Default)]
pub struct AlphaEditor {
    open: bool,
    source: String,
    name: String,
    ops: Ops,
    grab: Grab,
    /// Crop put back when a drawn box collapses to nothing.
    restore: CropRect,
    source_tex: Option<(String, egui::TextureHandle)>,
    derived: Option<Derived>,
}

impl AlphaEditor {
    /// Open the window on a library alpha, resetting every operation.
    pub fn open(&mut self, source: &str) {
        self.open = true;
        self.source = source.to_string();
        self.name = format!("{source}-clip");
        self.ops = Ops::default();
        self.grab = Grab::None;
        self.restore = CropRect::default();
        self.derived = None;
    }

    /// Whether the window is on screen.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Draws the window. Returns Some(alpha) when the user commits a save.
    pub fn ui(&mut self, ctx: &egui::Context, lib: &AlphaLibrary) -> Option<Alpha> {
        if !self.open {
            return None;
        }
        let Some(src) = lib.get(&self.source) else {
            self.open = false;
            return None;
        };
        refresh(self, ctx, src);

        let mut shown = true;
        let mut saved = None;
        egui::Window::new(format!("{} Alpha editor", icon::SCISSORS))
            .open(&mut shown)
            .resizable(true)
            .collapsible(false)
            .default_size([940.0, 560.0])
            .min_width(660.0)
            .show(ctx, |ui| saved = body(self, ui, src, lib));

        if !shown || saved.is_some() {
            self.open = false;
        }
        saved
    }
}

// --- The op stack ----------------------------------------------------------

/// Runs the fixed pipeline. See the module docs for the order.
fn derive(src: &Alpha, ops: &Ops) -> Alpha {
    let mut a = src.crop(ops.crop);
    if ops.trim {
        a = a.auto_trim(TRIM_LEVEL, TRIM_PAD);
    }
    a = a.rotated(ops.rotate);
    if let Some(ax) = flip_axis(ops.flip_h, ops.flip_v) {
        a = a.flipped(ax);
    }
    if let Some(ax) = ops.mirror {
        a = a.mirror_tile(ax);
    }
    if ops.fade > 0.0 {
        a = a.edge_fade(ops.fade, Axis::Both);
    }
    if ops.lo > 0.0 || ops.hi < 1.0 || (ops.gamma - 1.0).abs() > 1e-6 {
        a = a.levels(ops.lo, ops.hi, ops.gamma);
    }
    let (w, h) = fit_size(&a, ops.size);
    a.resized(w, h)
}

fn flip_axis(h: bool, v: bool) -> Option<Axis> {
    match (h, v) {
        (true, true) => Some(Axis::Both),
        (true, false) => Some(Axis::Horizontal),
        (false, true) => Some(Axis::Vertical),
        (false, false) => None,
    }
}

/// Extent whose longest edge is `size`, keeping the aspect ratio.
fn fit_size(a: &Alpha, size: usize) -> (usize, usize) {
    let long = a.width.max(a.height).max(1);
    let s = size as f64 / long as f64;
    (
        ((a.width as f64 * s).round() as usize).max(1),
        ((a.height as f64 * s).round() as usize).max(1),
    )
}

/// Rebuilds the derived alpha and both preview textures when an input changed.
fn refresh(ed: &mut AlphaEditor, ctx: &egui::Context, src: &Alpha) {
    if ed.source_tex.as_ref().is_none_or(|(n, _)| n != &ed.source) {
        let prev = ed.source_tex.take().map(|(_, t)| t);
        ed.source_tex =
            texture(ctx, "alpha_editor:source", src, SOURCE_EDGE, prev).map(|t| (ed.source.clone(), t));
    }

    let key = (ed.source.clone(), (src.width, src.height), ed.ops.clone());
    if ed.derived.as_ref().is_some_and(|d| d.key == key) {
        return;
    }
    let alpha = derive(src, &ed.ops);
    let seam = alpha.seam_error();
    let prev = ed.derived.take().map(|d| d.tex);
    if let Some(tex) = texture(ctx, "alpha_editor:result", &alpha, RESULT_EDGE, prev) {
        ed.derived = Some(Derived { key, alpha, seam, tex });
    }
}

/// Uploads a downscaled preview, reusing `prev` rather than allocating a texture.
fn texture(
    ctx: &egui::Context,
    name: &str,
    a: &Alpha,
    edge: usize,
    prev: Option<egui::TextureHandle>,
) -> Option<egui::TextureHandle> {
    let (w, h, bytes) = a.thumbnail_rgba8(edge);
    if w == 0 || h == 0 {
        return None;
    }
    let image = egui::ColorImage::from_rgba_unmultiplied([w, h], &bytes);
    Some(match prev {
        Some(mut t) => {
            t.set(image, egui::TextureOptions::LINEAR);
            t
        }
        None => ctx.load_texture(name, image, egui::TextureOptions::LINEAR),
    })
}

// --- Window body -----------------------------------------------------------

fn body(ed: &mut AlphaEditor, ui: &mut egui::Ui, src: &Alpha, lib: &AlphaLibrary) -> Option<Alpha> {
    let side = ((ui.available_width() - OPS_W - 32.0) * 0.5).clamp(150.0, 420.0);

    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(side);
            ui.set_max_width(side);
            source_column(ed, ui, src, side);
        });
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.set_min_width(OPS_W);
            ui.set_max_width(OPS_W);
            ops_column(ed, ui);
        });
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.set_min_width(side);
            ui.set_max_width(side);
            result_column(ed, ui, side);
        });
    });

    ui.add_space(6.0);
    ui.separator();
    footer(ed, ui, lib)
}

fn source_column(ed: &mut AlphaEditor, ui: &mut egui::Ui, src: &Alpha, view: f32) {
    ui.label(egui::RichText::new(format!("{} Source", icon::IMAGE)).strong());
    ui.add(
        egui::Label::new(
            egui::RichText::new(format!("{} • {} x {} px", src.name, src.width, src.height))
                .small()
                .color(theme::TEXT_DIM),
        )
        .truncate(),
    );
    ui.add_space(3.0);

    crop_canvas(ed, ui, src, view);

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.toggle_value(&mut ed.ops.trim, format!("{} Trim to content", icon::CROP))
            .on_hover_text("Drop the dead black border inside the crop before anything else runs");
        if ui
            .button(format!("{} Reset crop", icon::ARROW_COUNTER_CLOCKWISE))
            .clicked()
        {
            ed.ops.crop = CropRect::default();
        }
    });

    let r = ed.ops.crop.normalized();
    ui.label(
        egui::RichText::new(format!(
            "crop {:.0} x {:.0} px",
            r.width() * src.width as f64,
            r.height() * src.height as f64
        ))
        .small()
        .color(theme::TEXT_DIM),
    );
    ui.label(
        egui::RichText::new("Drag inside to move, a corner or side to resize, outside to draw a new box.")
            .small()
            .color(theme::TEXT_DIM),
    );
}

fn crop_canvas(ed: &mut AlphaEditor, ui: &mut egui::Ui, src: &Alpha, view: f32) {
    let (outer, _) = ui.allocate_exact_size(egui::vec2(view, view), egui::Sense::hover());
    let painter = ui.painter_at(outer);
    painter.rect_filled(outer, 3.0, theme::BG);

    let sw = src.width.max(1) as f32;
    let sh = src.height.max(1) as f32;
    let s = (outer.width() / sw).min(outer.height() / sh);
    let img = egui::Align2::CENTER_CENTER.anchor_size(outer.center(), egui::vec2(sw * s, sh * s));
    if let Some((_, tex)) = ed.source_tex.as_ref() {
        egui::Image::new(egui::load::SizedTexture::new(tex.id(), img.size())).paint_at(ui, img);
    }

    let to_screen = |r: &CropRect| {
        egui::Rect::from_min_max(
            egui::pos2(
                img.left() + r.x0 as f32 * img.width(),
                img.top() + r.y0 as f32 * img.height(),
            ),
            egui::pos2(
                img.left() + r.x1 as f32 * img.width(),
                img.top() + r.y1 as f32 * img.height(),
            ),
        )
    };
    let to_norm = |p: egui::Pos2| {
        (
            ((p.x - img.left()) / img.width().max(1.0)) as f64,
            ((p.y - img.top()) / img.height().max(1.0)) as f64,
        )
    };

    let resp = ui.interact(img, egui::Id::new("alpha_editor_crop"), egui::Sense::click_and_drag());
    let cr = to_screen(&ed.ops.crop.normalized());

    if resp.drag_started() {
        let p = resp.interact_pointer_pos().unwrap_or(cr.center());
        ed.grab = pick_grab(p, cr);
        ed.restore = ed.ops.crop;
        if ed.grab == Grab::Draw {
            let (x, y) = to_norm(p);
            ed.ops.crop = CropRect { x0: x, y0: y, x1: x, y1: y };
        }
    }
    if resp.dragged() {
        let d = resp.drag_delta();
        let dx = (d.x / img.width().max(1.0)) as f64;
        let dy = (d.y / img.height().max(1.0)) as f64;
        match ed.grab {
            Grab::None => {}
            Grab::Move => {
                let r = ed.ops.crop.normalized();
                let (w, h) = (r.width(), r.height());
                let x0 = (r.x0 + dx).clamp(0.0, (1.0 - w).max(0.0));
                let y0 = (r.y0 + dy).clamp(0.0, (1.0 - h).max(0.0));
                ed.ops.crop = CropRect { x0, y0, x1: x0 + w, y1: y0 + h };
            }
            Grab::Edge(gx, gy) => {
                let mut r = ed.ops.crop.normalized();
                if gx < 0 {
                    r.x0 = (r.x0 + dx).clamp(0.0, (r.x1 - MIN_CROP).max(0.0));
                } else if gx > 0 {
                    r.x1 = (r.x1 + dx).clamp((r.x0 + MIN_CROP).min(1.0), 1.0);
                }
                if gy < 0 {
                    r.y0 = (r.y0 + dy).clamp(0.0, (r.y1 - MIN_CROP).max(0.0));
                } else if gy > 0 {
                    r.y1 = (r.y1 + dy).clamp((r.y0 + MIN_CROP).min(1.0), 1.0);
                }
                ed.ops.crop = r;
            }
            Grab::Draw => {
                let p = resp.interact_pointer_pos().unwrap_or(cr.max);
                let (x, y) = to_norm(p);
                ed.ops.crop.x1 = x.clamp(0.0, 1.0);
                ed.ops.crop.y1 = y.clamp(0.0, 1.0);
            }
        }
    }
    if resp.drag_stopped() {
        let r = ed.ops.crop.normalized();
        ed.ops.crop = if r.width() < MIN_CROP || r.height() < MIN_CROP { ed.restore } else { r };
        ed.grab = Grab::None;
    }

    if let Some(p) = resp.hover_pos() {
        let g = if resp.dragged() { ed.grab } else { pick_grab(p, cr) };
        ui.ctx().set_cursor_icon(grab_cursor(g));
    }

    let cr = to_screen(&ed.ops.crop.normalized());
    let dim = egui::Color32::from_black_alpha(155);
    for r in [
        egui::Rect::from_min_max(img.min, egui::pos2(img.right(), cr.top())),
        egui::Rect::from_min_max(egui::pos2(img.left(), cr.bottom()), img.max),
        egui::Rect::from_min_max(egui::pos2(img.left(), cr.top()), egui::pos2(cr.left(), cr.bottom())),
        egui::Rect::from_min_max(egui::pos2(cr.right(), cr.top()), egui::pos2(img.right(), cr.bottom())),
    ] {
        if r.is_positive() {
            painter.rect_filled(r, 0.0, dim);
        }
    }
    painter.rect_stroke(
        cr,
        0.0,
        egui::Stroke::new(1.4, theme::ACCENT),
        egui::StrokeKind::Middle,
    );
    for c in [cr.left_top(), cr.right_top(), cr.left_bottom(), cr.right_bottom()] {
        painter.rect_filled(egui::Rect::from_center_size(c, egui::vec2(7.0, 7.0)), 1.0, theme::ACCENT);
    }
}

/// Which side or sides the pointer is over.
fn pick_grab(p: egui::Pos2, cr: egui::Rect) -> Grab {
    let axis = |v: f32, lo: f32, hi: f32| -> i8 {
        if (v - lo).abs() <= GRAB_PX {
            -1
        } else if (v - hi).abs() <= GRAB_PX {
            1
        } else {
            0
        }
    };
    let gx = axis(p.x, cr.left(), cr.right());
    let gy = axis(p.y, cr.top(), cr.bottom());
    if cr.expand(GRAB_PX).contains(p) && (gx != 0 || gy != 0) {
        Grab::Edge(gx, gy)
    } else if cr.contains(p) {
        Grab::Move
    } else {
        Grab::Draw
    }
}

fn grab_cursor(g: Grab) -> egui::CursorIcon {
    match g {
        Grab::Move => egui::CursorIcon::Move,
        Grab::Edge(0, _) => egui::CursorIcon::ResizeVertical,
        Grab::Edge(_, 0) => egui::CursorIcon::ResizeHorizontal,
        Grab::Edge(x, y) if x == y => egui::CursorIcon::ResizeNwSe,
        Grab::Edge(_, _) => egui::CursorIcon::ResizeNeSw,
        _ => egui::CursorIcon::Crosshair,
    }
}

fn ops_column(ed: &mut AlphaEditor, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new(format!("{} Operations", icon::SLIDERS)).strong());
    ui.label(
        egui::RichText::new("crop -> trim -> rotate -> flip -> mirror -> fade -> levels -> resize")
            .small()
            .color(theme::TEXT_DIM),
    );
    ui.add_space(5.0);

    mirror_block(ed, ui);
    ui.add_space(7.0);

    ui.horizontal(|ui| {
        ui.label("Rotate");
        for q in 0..4u32 {
            if ui
                .selectable_label(ed.ops.rotate == q, format!("{}°", q * 90))
                .clicked()
            {
                ed.ops.rotate = q;
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Flip");
        ui.toggle_value(&mut ed.ops.flip_h, format!("{} H", icon::FLIP_HORIZONTAL));
        ui.toggle_value(&mut ed.ops.flip_v, format!("{} V", icon::FLIP_VERTICAL));
    });

    ui.add_space(5.0);
    ui.add(egui::Slider::new(&mut ed.ops.fade, 0.0..=0.45).text("Edge fade"))
        .on_hover_text("Sinks the border to zero so the clip settles into the band instead of ending on a wall");

    ui.add_space(5.0);
    ui.horizontal(|ui| {
        ui.label("Levels");
        ui.add(egui::DragValue::new(&mut ed.ops.lo).speed(0.004).range(0.0..=0.95).prefix("lo "));
        ui.add(egui::DragValue::new(&mut ed.ops.hi).speed(0.004).range(0.05..=1.0).prefix("hi "));
    });
    if ed.ops.hi <= ed.ops.lo {
        ed.ops.lo = (ed.ops.hi - 0.01).max(0.0);
    }
    ui.add(egui::Slider::new(&mut ed.ops.gamma, 0.25..=4.0).logarithmic(true).text("Gamma"));

    ui.add_space(5.0);
    ui.horizontal(|ui| {
        ui.label("Output");
        for s in SIZES {
            if ui.selectable_label(ed.ops.size == s, format!("{s}")).clicked() {
                ed.ops.size = s;
            }
        }
    });
    ui.label(
        egui::RichText::new("Longest edge; the clip keeps its shape.")
            .small()
            .color(theme::TEXT_DIM),
    );
}

fn mirror_block(ed: &mut AlphaEditor, ui: &mut egui::Ui) {
    egui::Frame::group(ui.style())
        .stroke(egui::Stroke::new(1.0, theme::ACCENT_DIM))
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("{} Mirror tile", icon::FLIP_HORIZONTAL))
                    .strong()
                    .color(theme::ACCENT),
            );
            ui.horizontal(|ui| {
                for (label, v) in [
                    ("None", None),
                    ("Across", Some(Axis::Horizontal)),
                    ("Down", Some(Axis::Vertical)),
                    ("Both", Some(Axis::Both)),
                ] {
                    if ui.selectable_label(ed.ops.mirror == v, label).clicked() {
                        ed.ops.mirror = v;
                    }
                }
            });
            ui.label(
                egui::RichText::new(
                    "Doubles the clip against its own flip, so the tile's two outer edges are the same edge and it repeats with no seam.",
                )
                .small()
                .color(theme::TEXT_DIM),
            );
        });
}

fn result_column(ed: &AlphaEditor, ui: &mut egui::Ui, view: f32) {
    ui.label(egui::RichText::new(format!("{} Result • tiled 3 x 2", icon::GRID_FOUR)).strong());
    let Some((dims, seam, tex)) = ed
        .derived
        .as_ref()
        .map(|d| ((d.alpha.width, d.alpha.height), d.seam, d.tex.id()))
    else {
        ui.label(egui::RichText::new("Nothing to preview").color(theme::TEXT_DIM));
        return;
    };

    ui.label(
        egui::RichText::new(format!("{} x {} px per tile", dims.0, dims.1))
            .small()
            .color(theme::TEXT_DIM),
    );
    ui.add_space(3.0);
    tiled_preview(ui, tex, dims, seam, view);

    ui.add_space(5.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(egui::RichText::new("seams:").color(theme::TEXT_DIM));
        ui.label(
            egui::RichText::new(format!("{:.1}% across,", seam.0 * 100.0))
                .strong()
                .color(seam_color(seam.0)),
        );
        ui.label(
            egui::RichText::new(format!("{:.1}% down", seam.1 * 100.0))
                .strong()
                .color(seam_color(seam.1)),
        );
    });
    ui.label(
        egui::RichText::new("Mean mismatch between the edges that butt. Under 2% reads seamless.")
            .small()
            .color(theme::TEXT_DIM),
    );
}

/// Draws the result repeated 3 across and 2 down with the joints marked.
fn tiled_preview(
    ui: &mut egui::Ui,
    tex: egui::TextureId,
    dims: (usize, usize),
    seam: (f64, f64),
    view: f32,
) {
    const COLS: usize = 3;
    const ROWS: usize = 2;
    let aw = dims.0.max(1) as f32;
    let ah = dims.1.max(1) as f32;
    let aspect = (COLS as f32 * aw) / (ROWS as f32 * ah).max(1.0);
    let h = (view / aspect.max(0.01)).clamp(52.0, view * 0.92);
    let w = (h * aspect).min(view);

    let (outer, resp) = ui.allocate_exact_size(egui::vec2(view, h), egui::Sense::hover());
    let painter = ui.painter_at(outer);
    painter.rect_filled(outer, 3.0, theme::BG);
    let area = egui::Align2::CENTER_CENTER.anchor_size(outer.center(), egui::vec2(w, h));
    let cw = area.width() / COLS as f32;
    let ch = area.height() / ROWS as f32;

    for r in 0..ROWS {
        for c in 0..COLS {
            let cell = egui::Rect::from_min_size(
                area.min + egui::vec2(c as f32 * cw, r as f32 * ch),
                egui::vec2(cw, ch),
            );
            egui::Image::new(egui::load::SizedTexture::new(tex, cell.size())).paint_at(ui, cell);
        }
    }

    let tick = 6.0;
    let across = seam_color(seam.0);
    let down = seam_color(seam.1);
    for c in 1..COLS {
        let x = area.left() + c as f32 * cw;
        painter.line_segment(
            [egui::pos2(x, area.top()), egui::pos2(x, area.top() + tick)],
            egui::Stroke::new(1.5, across),
        );
        painter.line_segment(
            [egui::pos2(x, area.bottom() - tick), egui::pos2(x, area.bottom())],
            egui::Stroke::new(1.5, across),
        );
        painter.line_segment(
            [egui::pos2(x, area.top()), egui::pos2(x, area.bottom())],
            egui::Stroke::new(1.0, across.gamma_multiply(0.22)),
        );
    }
    let y = area.top() + ch;
    painter.line_segment(
        [egui::pos2(area.left(), y), egui::pos2(area.left() + tick, y)],
        egui::Stroke::new(1.5, down),
    );
    painter.line_segment(
        [egui::pos2(area.right() - tick, y), egui::pos2(area.right(), y)],
        egui::Stroke::new(1.5, down),
    );
    painter.line_segment(
        [egui::pos2(area.left(), y), egui::pos2(area.right(), y)],
        egui::Stroke::new(1.0, down.gamma_multiply(0.22)),
    );
    painter.rect_stroke(
        area,
        0.0,
        egui::Stroke::new(1.0, theme::HAIRLINE),
        egui::StrokeKind::Inside,
    );

    resp.on_hover_text("Each cell is one repeat. The marks sit where neighbouring tiles butt.");
}

fn seam_color(e: f64) -> egui::Color32 {
    if e < SEAM_GOOD {
        theme::GOOD
    } else if e < SEAM_WARN {
        theme::WARN
    } else {
        theme::BAD
    }
}

fn footer(ed: &mut AlphaEditor, ui: &mut egui::Ui, lib: &AlphaLibrary) -> Option<Alpha> {
    let mut saved = None;
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.add(
            egui::TextEdit::singleline(&mut ed.name)
                .desired_width(200.0)
                .hint_text("tile name"),
        );
        let trimmed = ed.name.trim().to_string();
        if lib.get(&trimmed).is_some() {
            ui.label(
                egui::RichText::new(format!("{} replaces an existing tile", icon::WARNING))
                    .small()
                    .color(theme::WARN),
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Cancel").clicked() {
                ed.open = false;
            }
            let ready = !trimmed.is_empty()
                && ed.derived.as_ref().is_some_and(|d| !d.alpha.is_empty());
            if ui
                .add_enabled(
                    ready,
                    egui::Button::new(format!("{} Save to library", icon::FLOPPY_DISK)),
                )
                .on_disabled_hover_text("Give the clip a name first")
                .clicked()
            {
                saved = ed.derived.as_ref().map(|d| d.alpha.renamed(trimmed.as_str()));
            }
        });
    });
    saved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob() -> Alpha {
        let (w, h) = (32usize, 32usize);
        let mut data = vec![0.0f32; w * h];
        for j in 8..24 {
            for i in 6..18 {
                data[j * w + i] = 0.9;
            }
        }
        Alpha::new("blob", w, h, data)
    }

    #[test]
    fn mirroring_across_kills_the_horizontal_seam() {
        let src = blob();
        let ops = Ops { crop: CropRect { x0: 0.1, y0: 0.2, x1: 0.6, y1: 0.8 }, ..Ops::default() };
        let out = derive(&src, &ops);
        assert!(out.seam_error().0 < 1e-9, "{:?}", out.seam_error());
        assert!(out.width.max(out.height) <= ops.size);
    }

    #[test]
    fn output_keeps_its_aspect_and_the_longest_edge() {
        let a = Alpha::new("t", 40, 10, vec![0.5; 400]);
        assert_eq!(fit_size(&a, 128), (128, 32));
        assert_eq!(fit_size(&Alpha::new("t", 10, 40, vec![0.5; 400]), 256), (64, 256));
    }

    #[test]
    fn every_grab_maps_to_a_cursor_and_the_interior_moves() {
        let cr = egui::Rect::from_min_max(egui::pos2(10.0, 10.0), egui::pos2(90.0, 90.0));
        assert_eq!(pick_grab(egui::pos2(50.0, 50.0), cr), Grab::Move);
        assert_eq!(pick_grab(egui::pos2(10.0, 10.0), cr), Grab::Edge(-1, -1));
        assert_eq!(pick_grab(egui::pos2(90.0, 50.0), cr), Grab::Edge(1, 0));
        assert_eq!(pick_grab(egui::pos2(50.0, 90.0), cr), Grab::Edge(0, 1));
        assert_eq!(pick_grab(egui::pos2(300.0, 300.0), cr), Grab::Draw);
        assert_eq!(grab_cursor(Grab::Edge(-1, -1)), egui::CursorIcon::ResizeNwSe);
        assert_eq!(grab_cursor(Grab::Edge(1, -1)), egui::CursorIcon::ResizeNeSw);
    }

    #[test]
    fn the_cache_key_moves_with_every_parameter_and_nothing_else() {
        let base = Ops::default();
        // An untouched clone must compare equal, or the editor rederives per frame.
        assert_eq!(base, base.clone());
        assert_eq!(
            ("a".to_string(), (32usize, 32usize), base.clone()),
            ("a".to_string(), (32usize, 32usize), base.clone())
        );

        let variants: [(&str, Ops); 11] = [
            ("crop", Ops { crop: CropRect { x0: 0.1, y0: 0.0, x1: 1.0, y1: 1.0 }, ..base.clone() }),
            ("trim", Ops { trim: !base.trim, ..base.clone() }),
            ("rotate", Ops { rotate: base.rotate + 1, ..base.clone() }),
            ("flip_h", Ops { flip_h: !base.flip_h, ..base.clone() }),
            ("flip_v", Ops { flip_v: !base.flip_v, ..base.clone() }),
            ("mirror", Ops { mirror: Some(Axis::Vertical), ..base.clone() }),
            ("mirror=None", Ops { mirror: None, ..base.clone() }),
            ("fade", Ops { fade: 0.25, ..base.clone() }),
            ("levels", Ops { lo: 0.1, hi: 0.9, gamma: 1.4, ..base.clone() }),
            ("size", Ops { size: 512, ..base.clone() }),
            ("all", Ops { trim: true, rotate: 2, size: 128, ..base.clone() }),
        ];
        // A shaded patch off-centre in both axes, on a dead border: asymmetric
        // enough that a flip shows, bordered enough that a trim shows.
        let (w, h) = (32usize, 32usize);
        let mut data = vec![0.0f32; w * h];
        for j in 5..21 {
            for i in 6..18 {
                data[j * w + i] = 0.25 + 0.6 * (i - 6) as f32 / 12.0 + 0.1 * (j - 5) as f32 / 16.0;
            }
        }
        let src = Alpha::new("skew", w, h, data);
        for (name, ops) in &variants {
            assert_ne!(base, *ops, "{name} does not move the cache key");
            // A changed key must also change what the pipeline produces.
            let a = derive(&src, &base);
            let b = derive(&src, ops);
            assert!(
                (a.width, a.height) != (b.width, b.height) || a.data != b.data,
                "{name} changed the key but not the output"
            );
        }

        // Source identity and extent are part of the key too.
        let k = ("a".to_string(), (32usize, 32usize), base.clone());
        assert_ne!(k, ("b".to_string(), (32, 32), base.clone()));
        assert_ne!(k, ("a".to_string(), (64, 32), base.clone()));
    }

    #[test]
    fn seam_colour_tracks_the_thresholds() {
        assert_eq!(seam_color(0.0), theme::GOOD);
        assert_eq!(seam_color(0.05), theme::WARN);
        assert_eq!(seam_color(0.5), theme::BAD);
    }
}
